use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::services::github_oauth;
use crate::services::session;
use crate::services::sync::PlayerSyncEvent;
use crate::AppState;

const SESSION_COOKIE: &str = "rl_session";

fn get_session(jar: &CookieJar, secret: &str) -> Result<(String, String), AppError> {
    let cookie = jar.get(SESSION_COOKIE).ok_or(AppError::Unauthorized)?;
    session::verify_session(cookie.value(), secret).ok_or(AppError::Unauthorized)
}

pub fn render_verify_page(base_url: &str) -> String {
    let login_url = format!("{base_url}/verify/login");

    format!(
        r##"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>GitHub Roles - Link Account</title>
    <meta name="description" content="Link your Discord account with your GitHub profile to automatically receive server roles based on your contributions.">
    <meta property="og:type" content="website">
    <meta property="og:title" content="GitHub Roles - Link Account">
    <meta property="og:description" content="Link your Discord and GitHub accounts to earn roles based on repository contributions.">
    <meta name="theme-color" content="#238636">
    <link rel="icon" href="{base_url}/favicon.ico" type="image/x-icon">
    <style>
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{ font-family: system-ui, -apple-system, sans-serif; max-width: 580px; margin: 0 auto; padding: 32px 20px; background: #0d1117; color: #c9d1d9; min-height: 100vh; }}
        h1 {{ color: #58a6ff; font-size: 24px; margin-bottom: 4px; }}
        h2 {{ color: #fff; font-size: 17px; margin-bottom: 14px; }}
        p {{ line-height: 1.6; margin: 6px 0; font-size: 14px; }}
        a {{ color: #58a6ff; }}
        .subtitle {{ color: #8b949e; font-size: 14px; margin-bottom: 20px; }}
        .card {{ background: #161b22; padding: 22px; border-radius: 10px; margin: 14px 0; border: 1px solid #30363d; }}
        .btn {{ display: inline-flex; align-items: center; gap: 8px; padding: 10px 22px; color: #fff; text-decoration: none; border-radius: 6px; font-size: 14px; font-weight: 500; border: none; cursor: pointer; font-family: inherit; transition: background .15s; }}
        .btn-discord {{ background: #5865f2; }}
        .btn-discord:hover {{ background: #4752c4; }}
        .btn-github {{ background: #238636; }}
        .btn-github:hover {{ background: #2ea043; }}
        .btn-danger {{ background: transparent; color: #f85149; border: 1px solid #da3633; font-size: 13px; padding: 8px 16px; }}
        .btn-danger:hover {{ background: #da363322; }}
        .btn:disabled {{ opacity: 0.5; cursor: not-allowed; }}
        .badge {{ display: inline-block; padding: 3px 10px; border-radius: 20px; font-size: 12px; font-weight: 500; }}
        .badge-ok {{ background: #0d3321; color: #3fb950; border: 1px solid #238636; }}
        .msg {{ padding: 10px 14px; border-radius: 6px; margin: 12px 0; font-size: 13px; line-height: 1.5; }}
        .msg-error {{ background: #1c0a0a; color: #fca5a5; border: 1px solid #7f1d1d; }}
        .msg-success {{ background: #0d3321; color: #86efac; border: 1px solid #238636; }}
        .info-row {{ display: flex; align-items: center; gap: 8px; margin: 6px 0; font-size: 14px; }}
        .info-row .label {{ color: #8b949e; min-width: 80px; }}
        .info-row .val {{ color: #58a6ff; font-weight: 600; }}
        .actions {{ display: flex; gap: 8px; margin-top: 16px; flex-wrap: wrap; }}
        .hidden {{ display: none !important; }}
        .divider {{ border: none; border-top: 1px solid #30363d; margin: 16px 0; }}
        .trust-note {{ font-size: 13px; color: #8b949e; background: #0d1117; border-left: 3px solid #58a6ff; padding: 10px 14px; border-radius: 0 6px 6px 0; margin: 10px 0; line-height: 1.6; }}
        .trust-note strong {{ color: #c9d1d9; }}
        .btn-logout {{ background: transparent; color: #8b949e; border: 1px solid #30363d; padding: 5px 12px; border-radius: 6px; font-size: 12px; cursor: pointer; font-family: inherit; transition: all .15s; }}
        .btn-logout:hover {{ color: #f85149; border-color: #da3633; background: #da363322; }}
    </style>
</head>
<body>
    <div style="display:flex; align-items:center; justify-content:space-between; margin-bottom:4px;">
        <div style="display:flex; align-items:center; gap:10px;">
            <h1 style="margin:0;">GitHub Roles</h1>
            <span style="font-size:11px; color:#8b949e; background:#21262d; padding:2px 8px; border-radius:4px;">Powered by <a href="https://rolelogic.faizo.net" target="_blank" rel="noopener" style="color:#58a6ff; text-decoration:none;">RoleLogic</a></span>
        </div>
        <button id="logout-btn" class="btn-logout hidden" onclick="doLogout()">Logout</button>
    </div>
    <p class="subtitle">Link your Discord and GitHub accounts to automatically receive server roles based on your repository contributions.</p>

    <div id="loading-section" class="card"><p style="color:#8b949e;">Loading...</p></div>

    <div id="login-section" class="card hidden">
        <h2>Step 1: Sign in with Discord</h2>
        <p>Sign in so we know which Discord account to assign roles to.</p>
        <p class="trust-note">We request the <strong>identify</strong> and <strong>guilds</strong> scopes only.</p>
        <div class="actions">
            <a href="{login_url}" class="btn btn-discord">
                <svg width="20" height="15" viewBox="0 0 71 55" fill="white"><path d="M60.1 4.9A58.5 58.5 0 0045.4.2a.2.2 0 00-.2.1 40.8 40.8 0 00-1.8 3.7 54 54 0 00-16.2 0A37.3 37.3 0 0025.4.3a.2.2 0 00-.2-.1A58.4 58.4 0 0010.6 4.9a.2.2 0 00-.1.1C1.5 18 -.9 30.6.3 43a.2.2 0 00.1.2 58.7 58.7 0 0017.7 9 .2.2 0 00.3-.1 42 42 0 003.6-5.9.2.2 0 00-.1-.3 38.6 38.6 0 01-5.5-2.6.2.2 0 01 0-.4l1.1-.9a.2.2 0 01.2 0 41.9 41.9 0 0035.6 0 .2.2 0 01.2 0l1.1.9a.2.2 0 010 .3 36.3 36.3 0 01-5.5 2.7.2.2 0 00-.1.3 47.2 47.2 0 003.6 5.9.2.2 0 00.3.1A58.5 58.5 0 0070.3 43a.2.2 0 00.1-.2c1.4-14.7-2.4-27.5-10.2-38.8a.2.2 0 00-.1 0zM23.7 35.3c-3.4 0-6.1-3.1-6.1-6.8s2.7-6.9 6.1-6.9 6.2 3.1 6.1 6.9c0 3.7-2.7 6.8-6.1 6.8zm22.6 0c-3.4 0-6.1-3.1-6.1-6.8s2.7-6.9 6.1-6.9 6.2 3.1 6.1 6.9c0 3.7-2.7 6.8-6.1 6.8z"/></svg>
                Login with Discord
            </a>
        </div>
    </div>

    <div id="github-section" class="card hidden">
        <h2>Step 2: Connect GitHub</h2>
        <p>Signed in as <span id="gh-discord" style="color:#58a6ff;"></span></p>
        <p>Connect your GitHub account so we can check your contributions.</p>
        <p class="trust-note">We request <strong>read:user</strong> scope only — we cannot modify your repositories or access private data.</p>
        <div class="actions">
            <a id="github-link" href="#" class="btn btn-github">
                <svg width="20" height="20" viewBox="0 0 16 16" fill="white"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>
                Connect GitHub
            </a>
        </div>
    </div>

    <div id="linked-section" class="card hidden">
        <div style="display:flex; align-items:center; gap:10px; margin-bottom:14px;">
            <h2 style="margin:0;">Account Linked</h2>
            <span class="badge badge-ok">Verified</span>
        </div>
        <div class="info-row"><span class="label">Discord</span> <span class="val" id="linked-discord" style="color:#8b949e;font-weight:400;font-size:13px;"></span></div>
        <div class="info-row"><span class="label">GitHub</span> <span class="val" id="linked-github"></span></div>
        <p style="color:#3fb950; margin-top:12px; font-size:13px;">Your roles are assigned automatically based on your repository contributions.</p>
        <hr class="divider">
        <div class="actions">
            <button class="btn btn-danger" onclick="doUnlink()">Unlink Account</button>
        </div>
    </div>

    <div id="msg" class="hidden"></div>
    <noscript><p style="color:#f85149; margin-top:20px;">JavaScript is required.</p></noscript>

    <script>
    const API = '{base_url}';
    const GITHUB_LOGIN_URL = API + '/verify/github/login';

    async function api(method, path, body) {{
        const opts = {{ method, headers: {{}}, credentials: 'include' }};
        if (body) {{ opts.headers['Content-Type'] = 'application/json'; opts.body = JSON.stringify(body); }}
        const res = await fetch(API + path, opts);
        const data = await res.json();
        if (!res.ok) throw new Error(data.error || 'Request failed');
        return data;
    }}

    function showSection(id) {{
        ['loading-section','login-section','github-section','linked-section'].forEach(s =>
            document.getElementById(s).classList.add('hidden')
        );
        document.getElementById(id).classList.remove('hidden');
    }}

    function showMsg(text, type) {{
        const el = document.getElementById('msg');
        el.className = 'msg msg-' + type;
        el.textContent = text;
        el.classList.remove('hidden');
        if (type === 'success') setTimeout(() => el.classList.add('hidden'), 6000);
    }}

    function clearMsg() {{ document.getElementById('msg').classList.add('hidden'); }}

    async function init() {{
        try {{
            const s = await api('GET', '/verify/status');
            document.getElementById('logout-btn').classList.remove('hidden');
            if (s.github_username) {{
                document.getElementById('linked-discord').textContent = s.display_name;
                document.getElementById('linked-github').textContent = s.github_username;
                showSection('linked-section');
            }} else {{
                document.getElementById('gh-discord').textContent = s.display_name;
                document.getElementById('github-link').href = GITHUB_LOGIN_URL;
                showSection('github-section');
            }}
        }} catch (e) {{
            showSection('login-section');
        }}
    }}

    async function doLogout() {{
        clearMsg();
        try {{
            await api('POST', '/verify/logout');
            document.getElementById('logout-btn').classList.add('hidden');
            showSection('login-section');
            showMsg('Logged out.', 'success');
        }} catch (e) {{ showMsg(e.message, 'error'); }}
    }}

    async function doUnlink() {{
        clearMsg();
        if (!confirm('Unlink your account? You will lose all assigned roles.')) return;
        try {{
            await api('POST', '/verify/unlink');
            document.getElementById('gh-discord').textContent = document.getElementById('linked-discord').textContent;
            document.getElementById('github-link').href = GITHUB_LOGIN_URL;
            showSection('github-section');
            showMsg('Account unlinked.', 'success');
        }} catch (e) {{ showMsg(e.message, 'error'); }}
    }}

    init();
    </script>
</body>
</html>"##
    )
}

pub async fn verify_page(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        state.verify_html.clone(),
    )
}

pub async fn login(State(state): State<Arc<AppState>>) -> Response {
    let return_to = "/github-contributor-role/verify";
    let url = format!(
        "/auth/login?return_to={}",
        urlencoding::encode(return_to),
    );
    Redirect::temporary(&url).into_response()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: String,
    pub error: Option<String>,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    let (discord_id, display_name) = get_session(&jar, &state.config.session_secret)?;

    let account = sqlx::query_as::<_, (String,)>(
        "SELECT github_username FROM linked_accounts WHERE discord_id = $1",
    )
    .bind(&discord_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(json!({
        "discord_id": discord_id,
        "display_name": display_name,
        "github_username": account.as_ref().map(|a| &a.0),
    })))
}

/// GitHub OAuth redirect — requires active Discord session
pub async fn github_login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Response, AppError> {
    let _ = get_session(&jar, &state.config.session_secret)?;

    let state_param: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let expires = chrono::Utc::now() + chrono::Duration::minutes(10);

    sqlx::query(
        "INSERT INTO oauth_states (state, redirect_data, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(&state_param)
    .bind(serde_json::json!({"purpose": "github_link"}))
    .bind(expires)
    .execute(&state.pool)
    .await?;

    let url = github_oauth::github_authorize_url(&state.config, &state_param);
    Ok(Redirect::temporary(&url).into_response())
}

/// GitHub OAuth callback — link GitHub account to Discord
pub async fn github_callback(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> Result<(CookieJar, Redirect), AppError> {
    let (discord_id, _) = get_session(&jar, &state.config.session_secret)?;

    if query.error.is_some() || query.code.is_none() {
        return Ok((jar, Redirect::to(&format!("{}/verify", state.config.base_url))));
    }
    let code = query.code.unwrap();

    let valid = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM oauth_states WHERE state = $1 AND expires_at > now())",
    )
    .bind(&query.state)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !valid {
        return Err(AppError::BadRequest("Invalid or expired OAuth state".into()));
    }

    sqlx::query("DELETE FROM oauth_states WHERE state = $1")
        .bind(&query.state)
        .execute(&state.pool)
        .await?;

    let (github_username, github_id) =
        github_oauth::github_exchange_code(&state.http, &state.config, &code).await?;

    // Check if GitHub account is already linked to another Discord user
    let existing = sqlx::query_scalar::<_, String>(
        "SELECT discord_id FROM linked_accounts WHERE github_id = $1",
    )
    .bind(github_id)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(other_discord) = existing {
        if other_discord != discord_id {
            return Err(AppError::BadRequest(
                "This GitHub account is already linked to another Discord user".into(),
            ));
        }
    }

    // Check if this Discord user already has a different GitHub linked
    let existing_discord = sqlx::query_scalar::<_, String>(
        "SELECT github_username FROM linked_accounts WHERE discord_id = $1",
    )
    .bind(&discord_id)
    .fetch_optional(&state.pool)
    .await?;

    if existing_discord.is_some() {
        return Err(AppError::BadRequest(
            "You already have a linked GitHub account. Unlink it first.".into(),
        ));
    }

    // Link accounts
    sqlx::query(
        "INSERT INTO linked_accounts (discord_id, github_username, github_id) VALUES ($1, $2, $3) \
         ON CONFLICT (discord_id) DO UPDATE SET github_username = $2, github_id = $3, linked_at = now()",
    )
    .bind(&discord_id)
    .bind(&github_username)
    .bind(github_id)
    .execute(&state.pool)
    .await?;

    // Trigger role sync
    let _ = state
        .player_sync_tx
        .send(PlayerSyncEvent::AccountLinked {
            discord_id: discord_id.clone(),
        })
        .await;

    tracing::info!(discord_id, github_username, "Account linked");

    Ok((jar, Redirect::to(&format!("{}/verify", state.config.base_url))))
}

pub async fn logout(jar: CookieJar) -> (CookieJar, Json<Value>) {
    let cookie = Cookie::build(SESSION_COOKIE).path("/");
    let jar = jar.remove(cookie);
    (jar, Json(json!({"success": true})))
}

pub async fn unlink(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    let (discord_id, _) = get_session(&jar, &state.config.session_secret)?;

    let account = sqlx::query_as::<_, (String,)>(
        "SELECT github_username FROM linked_accounts WHERE discord_id = $1",
    )
    .bind(&discord_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound("No linked account found".into()))?;

    sqlx::query("DELETE FROM linked_accounts WHERE discord_id = $1")
        .bind(&discord_id)
        .execute(&state.pool)
        .await?;

    let _ = state
        .player_sync_tx
        .send(PlayerSyncEvent::AccountUnlinked {
            discord_id: discord_id.clone(),
        })
        .await;

    tracing::info!(discord_id, github_username = account.0, "Account unlinked");

    Ok(Json(json!({"success": true})))
}
