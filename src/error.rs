use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("GitHub API error: {0}")]
    GitHub(#[from] GitHubError),

    #[error("RoleLogic API error: {0}")]
    RoleLogic(String),

    #[error("Role link user limit reached ({limit})")]
    UserLimitReached { limit: usize },

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("Repository not found")]
    NotFound,
    #[error("Rate limited")]
    RateLimited,
    #[error("Forbidden (bad token or permissions)")]
    Forbidden,
    #[error("Repository contributor list too large")]
    RepoTooLarge,
    #[error("Server error: {0}")]
    Server(u16),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => {
                tracing::error!("Database error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
            AppError::GitHub(GitHubError::NotFound) => {
                (StatusCode::NOT_FOUND, "GitHub repository or user not found")
            }
            AppError::GitHub(GitHubError::RateLimited) => {
                (StatusCode::TOO_MANY_REQUESTS, "GitHub API rate limited. Please try again later.")
            }
            AppError::GitHub(GitHubError::RepoTooLarge) => {
                (StatusCode::BAD_REQUEST, "Repository has too many contributors (>500)")
            }
            AppError::GitHub(e) => {
                tracing::error!("GitHub API error: {e}");
                (StatusCode::BAD_GATEWAY, "Failed to fetch GitHub data")
            }
            AppError::RoleLogic(e) => {
                tracing::error!("RoleLogic API error: {e}");
                (StatusCode::BAD_GATEWAY, "Failed to sync roles")
            }
            AppError::UserLimitReached { limit } => {
                tracing::warn!("Role link user limit reached: {limit}");
                (StatusCode::FORBIDDEN, "Role link user limit reached")
            }
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Invalid or missing authorization"),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            AppError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        let body = json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}
