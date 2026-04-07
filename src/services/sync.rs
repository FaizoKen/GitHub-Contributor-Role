use std::collections::HashSet;

use futures_util::stream::{self, StreamExt};
use sqlx::PgPool;

use crate::error::AppError;
use crate::models::condition::{Condition, ConditionOperator};
use crate::services::auth_gateway;
use crate::services::condition_eval::evaluate_condition;
use crate::services::github::ContributorStats;
use crate::AppState;

#[derive(Debug, Clone)]
pub enum PlayerSyncEvent {
    PlayerUpdated { discord_id: String },
    AccountLinked { discord_id: String },
    AccountUnlinked { discord_id: String },
}

#[derive(Debug, Clone)]
pub struct ConfigSyncEvent {
    pub guild_id: String,
    pub role_id: String,
}

/// Sync roles for a single player across all guilds.
pub async fn sync_for_player(
    discord_id: &str,
    state: &AppState,
) -> Result<(), AppError> {
    let pool = &state.pool;
    let rl_client = &state.rl_client;

    // Get user's GitHub username
    let github_username = sqlx::query_scalar::<_, String>(
        "SELECT github_username FROM linked_accounts WHERE discord_id = $1",
    )
    .bind(discord_id)
    .fetch_optional(pool)
    .await?;

    let Some(github_username) = github_username else {
        return Ok(());
    };

    // Ask the Auth Gateway which guilds this user is currently a member of.
    // Replaces the old JOIN against the local `user_guilds` table — the
    // gateway is the source of truth, kept fresh by its OAuth callback and
    // guild_refresh_worker.
    let guild_ids = auth_gateway::fetch_user_guild_ids(
        &state.http,
        &state.config.auth_gateway_url,
        &state.config.internal_api_key,
        discord_id,
    )
    .await?;

    if guild_ids.is_empty() {
        return Ok(());
    }

    // Get role links for guilds this user is in
    let role_links = sqlx::query_as::<_, (String, String, String, sqlx::types::Json<Vec<Condition>>)>(
        "SELECT rl.guild_id, rl.role_id, rl.api_token, rl.conditions \
         FROM role_links rl \
         WHERE rl.guild_id = ANY($1)",
    )
    .bind(&guild_ids[..])
    .fetch_all(pool)
    .await?;

    // Batch-fetch existing assignments
    let existing: HashSet<(String, String)> = sqlx::query_as::<_, (String, String)>(
        "SELECT guild_id, role_id FROM role_assignments WHERE discord_id = $1",
    )
    .bind(discord_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    enum Action {
        Add {
            guild_id: String,
            role_id: String,
            api_token: String,
        },
        Remove {
            guild_id: String,
            role_id: String,
            api_token: String,
        },
    }

    let mut actions: Vec<Action> = Vec::new();

    for (guild_id, role_id, api_token, conditions) in &role_links {
        let condition = match conditions.first() {
            Some(c) => c,
            None => continue,
        };

        // Look up this user's stats for the condition's repo
        let stats = sqlx::query_as::<_, (i32, i32, i32, i32)>(
            "SELECT commits, pull_requests, merged_prs, issues \
             FROM repo_contributors \
             WHERE repo_full_name = $1 AND LOWER(github_username) = LOWER($2)",
        )
        .bind(&condition.repo)
        .bind(&github_username)
        .fetch_optional(pool)
        .await?;

        let qualifies = match stats {
            Some((commits, prs, merged, issues)) => {
                let contributor_stats = ContributorStats {
                    commits,
                    pull_requests: prs,
                    merged_prs: merged,
                    issues,
                };
                evaluate_condition(condition, &contributor_stats)
            }
            None => {
                // User not in repo_contributors — all metrics are 0
                crate::services::condition_eval::evaluate_condition_zero(condition)
            }
        };

        let currently_assigned = existing.contains(&(guild_id.clone(), role_id.clone()));
        match (qualifies, currently_assigned) {
            (true, false) => actions.push(Action::Add {
                guild_id: guild_id.clone(),
                role_id: role_id.clone(),
                api_token: api_token.clone(),
            }),
            (false, true) => actions.push(Action::Remove {
                guild_id: guild_id.clone(),
                role_id: role_id.clone(),
                api_token: api_token.clone(),
            }),
            _ => {}
        }
    }

    if actions.is_empty() {
        return Ok(());
    }

    let discord_id_owned = discord_id.to_string();
    stream::iter(actions)
        .for_each_concurrent(10, |action| {
            let pool = pool.clone();
            let rl_client = rl_client.clone();
            let discord_id = discord_id_owned.clone();
            async move {
                match action {
                    Action::Add {
                        guild_id,
                        role_id,
                        api_token,
                    } => {
                        match rl_client
                            .add_user(&guild_id, &role_id, &discord_id, &api_token)
                            .await
                        {
                            Err(AppError::UserLimitReached { limit }) => {
                                tracing::warn!(guild_id, role_id, discord_id, limit, "User limit reached");
                                return;
                            }
                            Err(e) => {
                                tracing::error!(guild_id, role_id, discord_id, "Failed to add user: {e}");
                                return;
                            }
                            Ok(_) => {}
                        }
                        if let Err(e) = sqlx::query(
                            "INSERT INTO role_assignments (guild_id, role_id, discord_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                        )
                        .bind(&guild_id)
                        .bind(&role_id)
                        .bind(&discord_id)
                        .execute(&pool)
                        .await
                        {
                            tracing::error!(guild_id, role_id, discord_id, "Failed to insert assignment: {e}");
                        }
                    }
                    Action::Remove {
                        guild_id,
                        role_id,
                        api_token,
                    } => {
                        if let Err(e) = rl_client
                            .remove_user(&guild_id, &role_id, &discord_id, &api_token)
                            .await
                        {
                            tracing::error!(guild_id, role_id, discord_id, "Failed to remove user: {e}");
                            return;
                        }
                        if let Err(e) = sqlx::query(
                            "DELETE FROM role_assignments WHERE guild_id = $1 AND role_id = $2 AND discord_id = $3",
                        )
                        .bind(&guild_id)
                        .bind(&role_id)
                        .bind(&discord_id)
                        .execute(&pool)
                        .await
                        {
                            tracing::error!(guild_id, role_id, discord_id, "Failed to delete assignment: {e}");
                        }
                    }
                }
            }
        })
        .await;

    Ok(())
}

/// SQL bind value types for dynamic condition queries.
enum ConditionBind {
    Int(i64),
    Text(String),
}

/// Build SQL WHERE clause from conditions for SQL-side filtering.
fn build_condition_where(conditions: &[Condition]) -> (String, Vec<ConditionBind>) {
    if conditions.is_empty() {
        return ("TRUE".to_string(), vec![]);
    }

    let mut clauses: Vec<String> = Vec::new();
    let mut binds: Vec<ConditionBind> = Vec::new();

    for condition in conditions {
        // Add repo filter
        let repo_idx = binds.len() + 1;
        clauses.push(format!("rc.repo_full_name = ${repo_idx}"));
        binds.push(ConditionBind::Text(condition.repo.clone()));

        let col = condition.field.sql_column();
        let val = condition.value.as_i64().unwrap_or(0);

        if matches!(condition.operator, ConditionOperator::Between) {
            let end = condition
                .value_end
                .as_ref()
                .and_then(|v| v.as_i64())
                .unwrap_or(val);
            let idx_start = binds.len() + 1;
            let idx_end = binds.len() + 2;
            clauses.push(format!("{col} >= ${idx_start} AND {col} <= ${idx_end}"));
            binds.push(ConditionBind::Int(val));
            binds.push(ConditionBind::Int(end));
        } else {
            let op = condition.operator.sql_operator();
            let idx = binds.len() + 1;
            clauses.push(format!("{col} {op} ${idx}"));
            binds.push(ConditionBind::Int(val));
        }
    }

    (clauses.join(" AND "), binds)
}

/// Re-evaluate all users for a specific role link (after config change).
pub async fn sync_for_role_link(
    guild_id: &str,
    role_id: &str,
    state: &AppState,
) -> Result<(), AppError> {
    let pool = &state.pool;
    let rl_client = &state.rl_client;

    let link = sqlx::query_as::<_, (String, sqlx::types::Json<Vec<Condition>>)>(
        "SELECT api_token, conditions FROM role_links WHERE guild_id = $1 AND role_id = $2",
    )
    .bind(guild_id)
    .bind(role_id)
    .fetch_optional(pool)
    .await?;

    let Some((api_token, conditions)) = link else {
        return Ok(());
    };

    let (_user_count, user_limit) = rl_client
        .get_user_info(guild_id, role_id, &api_token)
        .await
        .unwrap_or((0, 100));

    // Ask the Auth Gateway for the current member list of this guild.
    // Replaces the old JOIN against the local `user_guilds` table.
    let member_ids = auth_gateway::fetch_guild_member_ids(
        &state.http,
        &state.config.auth_gateway_url,
        &state.config.internal_api_key,
        guild_id,
    )
    .await?;

    if member_ids.is_empty() {
        // No one in this guild (per the gateway) — clear the role and stop.
        rl_client
            .replace_users(guild_id, role_id, &[], &api_token)
            .await?;
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM role_assignments WHERE guild_id = $1 AND role_id = $2")
            .bind(guild_id)
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(());
    }

    let (where_clause, binds) = build_condition_where(&conditions);

    let members_bind_idx = binds.len() + 1;
    let limit_bind_idx = binds.len() + 2;
    let query_str = format!(
        "SELECT la.discord_id \
         FROM repo_contributors rc \
         JOIN linked_accounts la ON LOWER(la.github_username) = LOWER(rc.github_username) \
         WHERE la.discord_id = ANY(${members_bind_idx}::text[]) \
           AND ({where_clause}) \
         ORDER BY la.linked_at ASC \
         LIMIT ${limit_bind_idx}",
    );

    let qualifying_ids =
        exec_condition_query(&query_str, &binds, &member_ids, user_limit, pool).await?;

    if !qualifying_ids.is_empty() && qualifying_ids.len() == user_limit {
        let count_query = format!(
            "SELECT COUNT(*) FROM repo_contributors rc \
             JOIN linked_accounts la ON LOWER(la.github_username) = LOWER(rc.github_username) \
             WHERE la.discord_id = ANY(${members_bind_idx}::text[]) \
               AND ({where_clause})",
        );
        let total: i64 = exec_condition_count(&count_query, &binds, &member_ids, pool)
            .await
            .unwrap_or(qualifying_ids.len() as i64);
        if total as usize > user_limit {
            tracing::warn!(
                guild_id, role_id, total, user_limit,
                "User limit reached: {total} qualify but limit is {user_limit}"
            );
        }
    }

    rl_client
        .replace_users(guild_id, role_id, &qualifying_ids, &api_token)
        .await?;

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM role_assignments WHERE guild_id = $1 AND role_id = $2")
        .bind(guild_id)
        .bind(role_id)
        .execute(&mut *tx)
        .await?;

    if !qualifying_ids.is_empty() {
        sqlx::query(
            "INSERT INTO role_assignments (guild_id, role_id, discord_id) \
             SELECT $1, $2, UNNEST($3::text[])",
        )
        .bind(guild_id)
        .bind(role_id)
        .bind(&qualifying_ids)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Execute a dynamic condition query that returns discord_id strings.
/// `member_ids` is bound as a `text[]` for the gateway-sourced guild
/// membership filter (`WHERE la.discord_id = ANY($N::text[])`).
async fn exec_condition_query(
    query: &str,
    binds: &[ConditionBind],
    member_ids: &[String],
    limit: usize,
    pool: &PgPool,
) -> Result<Vec<String>, AppError> {
    let mut q = sqlx::query_scalar::<_, String>(query);
    for bind in binds {
        q = match bind {
            ConditionBind::Int(v) => q.bind(*v),
            ConditionBind::Text(v) => q.bind(v),
        };
    }
    q = q.bind(member_ids);
    q = q.bind(limit as i64);
    Ok(q.fetch_all(pool).await?)
}

async fn exec_condition_count(
    query: &str,
    binds: &[ConditionBind],
    member_ids: &[String],
    pool: &PgPool,
) -> Result<i64, AppError> {
    let mut q = sqlx::query_scalar::<_, i64>(query);
    for bind in binds {
        q = match bind {
            ConditionBind::Int(v) => q.bind(*v),
            ConditionBind::Text(v) => q.bind(v),
        };
    }
    q = q.bind(member_ids);
    Ok(q.fetch_one(pool).await?)
}

/// Remove a user from all role assignments (after account unlink).
pub async fn remove_all_assignments(
    discord_id: &str,
    state: &AppState,
) -> Result<(), AppError> {
    let pool = &state.pool;
    let rl_client = &state.rl_client;
    let assignments = sqlx::query_as::<_, (String, String, String)>(
        "SELECT ra.guild_id, ra.role_id, rl.api_token \
         FROM role_assignments ra \
         JOIN role_links rl ON rl.guild_id = ra.guild_id AND rl.role_id = ra.role_id \
         WHERE ra.discord_id = $1",
    )
    .bind(discord_id)
    .fetch_all(pool)
    .await?;

    for (guild_id, role_id, api_token) in &assignments {
        if let Err(e) = rl_client
            .remove_user(guild_id, role_id, discord_id, api_token)
            .await
        {
            tracing::error!(guild_id, role_id, discord_id, "Failed to remove during unlink: {e}");
        }
    }

    sqlx::query("DELETE FROM role_assignments WHERE discord_id = $1")
        .bind(discord_id)
        .execute(pool)
        .await?;

    Ok(())
}
