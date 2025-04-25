use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    InternalServerError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

// Add back compatibility methods
impl AppError {
    pub fn unauthorized(msg: String) -> Self {
        AppError::Unauthorized(msg)
    }

    pub fn not_found(msg: String) -> Self {
        AppError::NotFound(msg)
    }

    pub fn validation_error(msg: String) -> Self {
        AppError::BadRequest(msg)
    }

    pub fn bad_request(msg: String) -> Self {
        AppError::BadRequest(msg)
    }

    pub fn internal_server_error(msg: String) -> Self {
        AppError::InternalServerError(msg)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized(msg) => {
                tracing::warn!("Unauthorized error: {}", msg);
                (StatusCode::UNAUTHORIZED, msg.clone())
            }
            AppError::NotFound(msg) => {
                tracing::warn!("Not found error: {}", msg);
                (StatusCode::NOT_FOUND, msg.clone())
            }
            AppError::BadRequest(msg) => {
                tracing::warn!("Bad request error: {}", msg);
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            AppError::InternalServerError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
            AppError::SerializationError(err) => {
                tracing::warn!("Serialization error: {}", err);
                (StatusCode::BAD_REQUEST, err.to_string())
            }
        };

        tracing::info!(
            "Returning error response: status={}, message={}",
            status,
            message
        );
        (status, Json(json!({ "error": message }))).into_response()
    }
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
            lockbox_shared::error::StoreError::InvitationExpired => {
                AppError::BadRequest("Invitation has expired".into())
            }
            lockbox_shared::error::StoreError::AuthError(msg) => AppError::Unauthorized(msg),
            lockbox_shared::error::StoreError::VersionConflict(msg) => {
                AppError::BadRequest(format!("Concurrent modification detected: {}", msg))
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
