use std::fmt;
use thiserror::Error;

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
    GuardianNotFound(String),
    BoxNotFound(String),
    InternalError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::VersionConflict(msg) => write!(f, "Version conflict: {}", msg),
            AppError::GuardianNotFound(msg) => write!(f, "Guardian not found: {}", msg),
            AppError::BoxNotFound(msg) => write!(f, "Box not found: {}", msg),
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// Convert anyhow::Error to AppError
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        // Check if this is one of our custom errors wrapped in anyhow
        if let Some(app_err) = err.downcast_ref::<AppError>() {
            match app_err {
                AppError::GuardianNotFound(msg) => AppError::GuardianNotFound(msg.clone()),
                AppError::BoxNotFound(msg) => AppError::BoxNotFound(msg.clone()),
                AppError::VersionConflict(msg) => AppError::VersionConflict(msg.clone()),
                AppError::InternalError(msg) => AppError::InternalError(msg.clone()),
            }
        } else {
            AppError::InternalError(err.to_string())
        }
    }
}

// Convert StoreError to AppError
impl From<lockbox_shared::error::StoreError> for AppError {
    fn from(err: lockbox_shared::error::StoreError) -> Self {
        match err {
            lockbox_shared::error::StoreError::VersionConflict(msg) => Self::VersionConflict(msg),
            lockbox_shared::error::StoreError::NotFound(msg) => Self::BoxNotFound(msg),
            _ => Self::InternalError(err.to_string()),
        }
    }
}

// Implement conversion from other error types if needed
impl From<lockbox_shared::error::StoreError> for InvitationEventError {
    fn from(err: lockbox_shared::error::StoreError) -> Self {
        match err {
            lockbox_shared::error::StoreError::NotFound(msg) => Self::BoxNotFound(msg),
            lockbox_shared::error::StoreError::VersionConflict(msg) => {
                Self::UpdateError(format!("Concurrent update conflict: {}", msg))
            }
            _ => Self::StoreError(err.to_string()),
        }
    }
}
