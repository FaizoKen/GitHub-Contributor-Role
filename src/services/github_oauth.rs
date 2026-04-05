use crate::config::AppConfig;
use crate::error::AppError;

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
