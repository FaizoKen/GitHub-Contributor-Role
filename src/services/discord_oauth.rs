use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::config::AppConfig;
use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    global_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct DiscordGuild {
    id: String,
    name: String,
}

pub struct DiscordOAuth {
    http: reqwest::Client,
}

impl DiscordOAuth {
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    pub fn authorize_url(config: &AppConfig, state: &str) -> String {
        let redirect_uri = config.discord_oauth_redirect_uri();
        format!(
            "https://discord.com/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify%20guilds&state={}",
            config.discord_client_id,
            urlencoding::encode(&redirect_uri),
            state
        )
    }

    pub async fn exchange_code(
        &self,
        config: &AppConfig,
        code: &str,
    ) -> Result<(String, Option<String>), AppError> {
        let resp: TokenResponse = self
            .http
            .post("https://discord.com/api/v10/oauth2/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", &config.discord_oauth_redirect_uri()),
                ("client_id", &config.discord_client_id),
                ("client_secret", &config.discord_client_secret),
            ])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Discord token exchange failed: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Discord token parse failed: {e}")))?;

        Ok((resp.access_token, resp.refresh_token))
    }

    pub async fn refresh_access_token(
        &self,
        config: &AppConfig,
        refresh_token: &str,
    ) -> Result<(String, String), AppError> {
        let resp: TokenResponse = self
            .http
            .post("https://discord.com/api/v10/oauth2/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token),
                ("client_id", &config.discord_client_id),
                ("client_secret", &config.discord_client_secret),
            ])
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Discord token refresh failed: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Discord token refresh parse failed: {e}")))?;

        let new_refresh = resp
            .refresh_token
            .ok_or_else(|| {
                AppError::Internal("Discord token refresh returned no refresh_token".into())
            })?;

        Ok((resp.access_token, new_refresh))
    }

    pub async fn get_user(&self, access_token: &str) -> Result<(String, String), AppError> {
        let user: DiscordUser = self
            .http
            .get("https://discord.com/api/v10/users/@me")
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Discord user fetch failed: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Discord user parse failed: {e}")))?;

        let display_name = user.global_name.unwrap_or(user.username);
        Ok((user.id, display_name))
    }

    pub async fn get_user_guilds(
        &self,
        access_token: &str,
    ) -> Result<Vec<(String, String)>, AppError> {
        let guilds: Vec<DiscordGuild> = self
            .http
            .get("https://discord.com/api/v10/users/@me/guilds")
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Discord guilds fetch failed: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Discord guilds parse failed: {e}")))?;

        Ok(guilds.into_iter().map(|g| (g.id, g.name)).collect())
    }
}

// --- GitHub OAuth helpers ---

pub fn github_authorize_url(config: &AppConfig, state: &str) -> String {
    let redirect_uri = config.github_oauth_redirect_uri();
    format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user&state={}",
        config.github_client_id,
        urlencoding::encode(&redirect_uri),
        state
    )
}

#[derive(serde::Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
}

#[derive(serde::Deserialize)]
struct GitHubUser {
    login: String,
    id: i64,
}

/// Exchange GitHub OAuth code for access token, then fetch user info.
/// Returns (username, github_id).
pub async fn github_exchange_code(
    http: &reqwest::Client,
    config: &AppConfig,
    code: &str,
) -> Result<(String, i64), AppError> {
    let token_resp: GitHubTokenResponse = http
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", config.github_client_id.as_str()),
            ("client_secret", config.github_client_secret.as_str()),
            ("code", code),
            ("redirect_uri", &config.github_oauth_redirect_uri()),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub token exchange failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub token parse failed: {e}")))?;

    let user: GitHubUser = http
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token_resp.access_token))
        .header("User-Agent", "GitHubContributorRoles/1.0")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub user fetch failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub user parse failed: {e}")))?;

    Ok((user.login, user.id))
}

// --- Session signing (identical to Genshin reference) ---

pub fn sign_session(discord_id: &str, display_name: &str, secret: &str) -> String {
    let expires = chrono::Utc::now().timestamp() + 3600;
    let encoded_name = urlencoding::encode(display_name);
    let payload = format!("{discord_id}:{encoded_name}:{expires}");

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());

    format!("{payload}:{sig}")
}

pub fn verify_session(cookie_value: &str, secret: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = cookie_value.splitn(4, ':').collect();
    if parts.len() != 4 {
        return None;
    }

    let discord_id = parts[0];
    let encoded_name = parts[1];
    let expires_str = parts[2];
    let sig = parts[3];

    let expires: i64 = expires_str.parse().ok()?;
    if chrono::Utc::now().timestamp() > expires {
        return None;
    }

    let payload = format!("{discord_id}:{encoded_name}:{expires_str}");
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());

    let expected_sig = hex::encode(mac.finalize().into_bytes());
    if sig != expected_sig {
        return None;
    }

    let display_name = urlencoding::decode(encoded_name).ok()?.into_owned();
    Some((discord_id.to_string(), display_name))
}
