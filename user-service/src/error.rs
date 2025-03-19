use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("User not found: {0}")]
    UserNotFound(String),
    
    #[error("Unauthorized access")]
    Unauthorized,
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Internal server error: {0}")]
    InternalServerError(String),
}

impl ServiceError {
    pub fn status_code(&self) -> u16 {
        match self {
            ServiceError::UserNotFound(_) => 404,
            ServiceError::Unauthorized => 401,
            ServiceError::DatabaseError(_) => 500,
            ServiceError::InvalidInput(_) => 400,
            ServiceError::InternalServerError(_) => 500,
        }
    }
}
