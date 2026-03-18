use std::env;

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
    pub session_secret: String,
    pub base_url: String,
    pub listen_addr: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_token: String,
    pub github_max_requests_per_hour: i64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            discord_client_id: env::var("DISCORD_CLIENT_ID")
                .expect("DISCORD_CLIENT_ID must be set"),
            discord_client_secret: env::var("DISCORD_CLIENT_SECRET")
                .expect("DISCORD_CLIENT_SECRET must be set"),
            session_secret: env::var("SESSION_SECRET").expect("SESSION_SECRET must be set"),
            base_url: env::var("BASE_URL").expect("BASE_URL must be set"),
            listen_addr: env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            github_client_id: env::var("GITHUB_CLIENT_ID")
                .expect("GITHUB_CLIENT_ID must be set"),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .expect("GITHUB_CLIENT_SECRET must be set"),
            github_token: env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
            github_max_requests_per_hour: env::var("GITHUB_MAX_REQUESTS_PER_HOUR")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4500),
        }
    }

    pub fn discord_oauth_redirect_uri(&self) -> String {
        format!("{}/verify/callback", self.base_url)
    }

    pub fn github_oauth_redirect_uri(&self) -> String {
        format!("{}/verify/github/callback", self.base_url)
    }
}
