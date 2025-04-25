use thiserror::Error;
use std::fmt;

#[derive(Debug, Error)]
pub enum InvitationEventError {
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Box not found: {0}")]
    BoxNotFound(String),
    
    #[error("Failed to update box: {0}")]
    UpdateError(String),
    
    #[error("Store error: {0}")]
    StoreError(String),
}

#[derive(Debug)]
pub enum AppError {
    VersionConflict(String),
    InternalError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::VersionConflict(msg) => write!(f, "Version conflict: {}", msg),
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// Convert anyhow::Error to AppError
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::InternalError(err.to_string())
    }
}

// Convert StoreError to AppError
impl From<lockbox_shared::error::StoreError> for AppError {
    fn from(err: lockbox_shared::error::StoreError) -> Self {
        match err {
            lockbox_shared::error::StoreError::VersionConflict(msg) => Self::VersionConflict(msg),
            _ => Self::InternalError(err.to_string()),
        }
    }
}

// Implement conversion from other error types if needed
impl From<lockbox_shared::error::StoreError> for InvitationEventError {
    fn from(err: lockbox_shared::error::StoreError) -> Self {
        match err {
            lockbox_shared::error::StoreError::NotFound(msg) => Self::BoxNotFound(msg),
            lockbox_shared::error::StoreError::VersionConflict(msg) => Self::UpdateError(format!("Concurrent update conflict: {}", msg)),
            _ => Self::StoreError(err.to_string()),
        }
    }
} 