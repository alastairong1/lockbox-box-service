use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use lockbox_shared::{
    error::ServiceError as SharedServiceError,
    models::ErrorResponse,
};

// Error handler for our API
impl IntoResponse for SharedServiceError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            SharedServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            SharedServiceError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            SharedServiceError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
            SharedServiceError::InvitationExpired => (
                StatusCode::GONE,
                "Invitation has expired".to_string(),
            ),
            SharedServiceError::Timeout(msg) => (StatusCode::REQUEST_TIMEOUT, msg),
            SharedServiceError::InternalError(msg) => {
                tracing::error!("Internal server error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        // Build the error response
        (status, Json(ErrorResponse { error: message })).into_response()
    }
} 