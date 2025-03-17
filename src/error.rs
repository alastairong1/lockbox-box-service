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

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::SerializationError(err) => (StatusCode::BAD_REQUEST, err.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
