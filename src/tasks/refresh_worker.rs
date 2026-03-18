use std::sync::Arc;

use crate::error::GitHubError;
use crate::services::sync::PlayerSyncEvent;
use crate::AppState;

const MIN_REFRESH_SECS: i64 = 1800; // 30 min floor
const MAX_REFRESH_SECS: i64 = 86400; // 24 hour cap
const CALLS_PER_REPO: i64 = 20; // average API calls per repo refresh

pub async fn run(state: Arc<AppState>) {
    let max_req = state.config.github_max_requests_per_hour;
    tracing::info!(max_req, "Refresh worker started");

    // Initial delay
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    loop {
        // Discover repos to track from role_links conditions
        if let Err(e) = discover_repos(&state).await {
            tracing::error!("Failed to discover repos: {e}");
        }

        // Pick the repo with oldest next_fetch_at
        let next = sqlx::query_as::<_, (String,)>(
            "SELECT repo_full_name FROM repo_cache \
             WHERE next_fetch_at <= now() \
             ORDER BY fetch_failures ASC, next_fetch_at ASC \
             LIMIT 1",
        )
        .fetch_optional(&state.pool)
        .await;

        let repo_full_name = match next {
            Ok(Some((name,))) => name,
            Ok(None) => {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
            Err(e) => {
                tracing::error!("Refresh worker DB error: {e}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        tracing::debug!(repo_full_name, "Refreshing repo data");

        match state.github_client.fetch_repo_data(&repo_full_name).await {
            Ok(repo_data) => {
                let interval = compute_interval(&state).await;
                let next_fetch = chrono::Utc::now() + chrono::Duration::seconds(interval);

                // Transaction: replace contributor data + update cache metadata
                let result = update_repo_data(&state, &repo_full_name, &repo_data, next_fetch).await;

                if let Err(e) = result {
                    tracing::error!(repo_full_name, "Failed to update repo data: {e}");
                    continue;
                }

                let contributor_count = repo_data.contributors.len();
                tracing::debug!(repo_full_name, contributor_count, interval, "Repo data refreshed");

                // Trigger sync for all linked Discord users who are contributors
                trigger_syncs_for_repo(&state, &repo_full_name).await;
            }
            Err(GitHubError::RateLimited) => {
                tracing::warn!("GitHub rate limited, backing off 60s");
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
            Err(GitHubError::NotFound) => {
                tracing::warn!(repo_full_name, "Repo not found, backing off 1h");
                let backoff = chrono::Utc::now() + chrono::Duration::hours(1);
                let _ = sqlx::query(
                    "UPDATE repo_cache SET fetch_failures = fetch_failures + 1, next_fetch_at = $1 \
                     WHERE repo_full_name = $2",
                )
                .bind(backoff)
                .bind(&repo_full_name)
                .execute(&state.pool)
                .await;
            }
            Err(e) => {
                tracing::warn!(repo_full_name, "Repo fetch failed: {e}");
                let _ = sqlx::query(
                    "UPDATE repo_cache SET fetch_failures = fetch_failures + 1, \
                     next_fetch_at = now() + LEAST(INTERVAL '60 seconds' * POWER(2, fetch_failures), INTERVAL '1 hour') \
                     WHERE repo_full_name = $1",
                )
                .bind(&repo_full_name)
                .execute(&state.pool)
                .await;
            }
        }
    }
}

/// Discover repos from role_links conditions and ensure they're in repo_cache.
async fn discover_repos(state: &AppState) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO repo_cache (repo_full_name) \
         SELECT DISTINCT conditions->0->>'repo' FROM role_links \
         WHERE conditions != '[]'::jsonb AND conditions->0->>'repo' IS NOT NULL \
         ON CONFLICT DO NOTHING",
    )
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Compute refresh interval based on number of tracked repos.
async fn compute_interval(state: &AppState) -> i64 {
    let repo_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM repo_cache")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(1);

    if repo_count == 0 {
        return MIN_REFRESH_SECS;
    }

    let max_req = state.config.github_max_requests_per_hour;
    ((repo_count * 3600 * CALLS_PER_REPO) / max_req).clamp(MIN_REFRESH_SECS, MAX_REFRESH_SECS)
}

/// Update repo_contributors in a transaction.
async fn update_repo_data(
    state: &AppState,
    repo_full_name: &str,
    repo_data: &crate::services::github::RepoData,
    next_fetch: chrono::DateTime<chrono::Utc>,
) -> Result<(), crate::error::AppError> {
    let mut tx = state.pool.begin().await?;

    // Delete old contributor data for this repo
    sqlx::query("DELETE FROM repo_contributors WHERE repo_full_name = $1")
        .bind(repo_full_name)
        .execute(&mut *tx)
        .await?;

    // Insert new contributor data
    for (username, stats) in &repo_data.contributors {
        sqlx::query(
            "INSERT INTO repo_contributors (repo_full_name, github_username, commits, pull_requests, merged_prs, issues) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(repo_full_name)
        .bind(username)
        .bind(stats.commits)
        .bind(stats.pull_requests)
        .bind(stats.merged_prs)
        .bind(stats.issues)
        .execute(&mut *tx)
        .await?;
    }

    // Update repo_cache metadata
    let pr_count: i32 = repo_data
        .contributors
        .values()
        .map(|s| s.pull_requests)
        .sum();
    let issue_count: i32 = repo_data.contributors.values().map(|s| s.issues).sum();

    sqlx::query(
        "UPDATE repo_cache SET \
         fetched_at = now(), next_fetch_at = $1, fetch_failures = 0, \
         contributor_count = $2, pr_count = $3, issue_count = $4 \
         WHERE repo_full_name = $5",
    )
    .bind(next_fetch)
    .bind(repo_data.contributors.len() as i32)
    .bind(pr_count)
    .bind(issue_count)
    .bind(repo_full_name)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Send PlayerSyncEvent for all linked Discord users who are contributors to this repo.
async fn trigger_syncs_for_repo(state: &AppState, repo_full_name: &str) {
    let users = sqlx::query_scalar::<_, String>(
        "SELECT la.discord_id FROM linked_accounts la \
         JOIN repo_contributors rc ON LOWER(rc.github_username) = LOWER(la.github_username) \
         WHERE rc.repo_full_name = $1",
    )
    .bind(repo_full_name)
    .fetch_all(&state.pool)
    .await;

    match users {
        Ok(discord_ids) => {
            for discord_id in discord_ids {
                let _ = state
                    .player_sync_tx
                    .send(PlayerSyncEvent::PlayerUpdated { discord_id })
                    .await;
            }
        }
        Err(e) => {
            tracing::error!(repo_full_name, "Failed to find linked users for sync: {e}");
        }
    }
}
