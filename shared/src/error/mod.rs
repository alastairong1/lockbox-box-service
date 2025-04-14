use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::delete_item::DeleteItemError;
use aws_sdk_dynamodb::operation::get_item::GetItemError;
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use aws_sdk_dynamodb::operation::query::QueryError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ServiceError>;

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Internal server error: {0}")]
    InternalError(String),

    #[error("Invitation expired")]
    InvitationExpired,

    #[error("Request timeout: {0}")]
    Timeout(String),
}

// Helper function to map general DynamoDB errors
pub fn map_dynamo_error<E>(operation: &str, err: SdkError<E>) -> ServiceError {
    ServiceError::InternalError(format!("DynamoDB {} error: {}", operation, err))
}

// Helper function to map GetItem errors
pub fn map_get_dynamo_error(err: SdkError<GetItemError>, id: &str) -> ServiceError {
    match &err {
        SdkError::ServiceError(service_err) => {
            if service_err.err().is_resource_not_found_exception() {
                ServiceError::NotFound(format!("Resource not found with ID: {}", id))
            } else {
                ServiceError::InternalError(format!("DynamoDB get_item error: {}", err))
            }
        }
        _ => ServiceError::InternalError(format!("DynamoDB get_item error: {}", err)),
    }
}

// Helper function to map DeleteItem errors
pub fn map_delete_dynamo_error(err: SdkError<DeleteItemError>) -> ServiceError {
    ServiceError::InternalError(format!("DynamoDB delete_item error: {}", err))
}

// Helper function to map Query errors
pub fn map_query_dynamo_error(err: SdkError<QueryError>) -> ServiceError {
    ServiceError::InternalError(format!("DynamoDB query error: {}", err))
}

// Helper function to map PutItem errors
pub fn map_put_dynamo_error(err: SdkError<PutItemError>) -> ServiceError {
    ServiceError::InternalError(format!("DynamoDB put_item error: {}", err))
}

// Keep conversion for compatibility
impl From<serde_dynamo::Error> for ServiceError {
    fn from(err: serde_dynamo::Error) -> Self {
        ServiceError::InternalError(format!("DynamoDB serialization error: {}", err))
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(err: serde_json::Error) -> Self {
        ServiceError::InternalError(format!("JSON serialization error: {}", err))
    }
} 