use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Invitation expired")]
    InvitationExpired,

    #[error("Request timeout: {0}")]
    Timeout(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => {
                tracing::warn!("Not found error: {}", msg);
                (StatusCode::NOT_FOUND, msg)
            }
            AppError::Unauthorized(msg) => {
                tracing::warn!("Unauthorized error: {}", msg);
                (StatusCode::UNAUTHORIZED, msg)
            }
            AppError::BadRequest(msg) => {
                tracing::warn!("Bad request error: {}", msg);
                (StatusCode::BAD_REQUEST, msg)
            }
            AppError::InvitationExpired => {
                tracing::warn!("Invitation expired");
                (StatusCode::GONE, "Invitation has expired".to_string())
            }
            AppError::Timeout(msg) => {
                tracing::warn!("Request timeout: {}", msg);
                (StatusCode::REQUEST_TIMEOUT, msg)
            }
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            AppError::SerializationError(err) => {
                tracing::warn!("Serialization error: {}", err);
                (StatusCode::BAD_REQUEST, err.to_string())
            }
        };

        // Build the error response
        (status, Json(json!({ "error": message }))).into_response()
    }
}

// Helper function to map DynamoDB errors to our application errors
pub fn map_dynamo_error(operation: &str, err: impl std::fmt::Display) -> AppError {
    AppError::InternalServerError(format!("DynamoDB {} error: {}", operation, err))
}

// Add conversion from shared StoreError to AppError
impl From<lockbox_shared::error::StoreError> for AppError {
    fn from(err: lockbox_shared::error::StoreError) -> Self {
        match err {
            lockbox_shared::error::StoreError::NotFound(msg) => AppError::NotFound(msg),
            lockbox_shared::error::StoreError::ValidationError(msg) => AppError::BadRequest(msg),
            lockbox_shared::error::StoreError::InternalError(msg) => {
                AppError::InternalServerError(msg)
            }
            lockbox_shared::error::StoreError::InvitationExpired => AppError::InvitationExpired,
            lockbox_shared::error::StoreError::AuthError(msg) => AppError::Unauthorized(msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
