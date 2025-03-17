pub mod box_handlers;
pub mod guardian_handlers;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    middleware::Next,
    response::Response,
    async_trait,
    extract::Request
};

use crate::error::AppError;

// Extractor for user_id from header
pub struct UserId(String);

#[async_trait]
impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(user_id) = parts.headers.get("x-user-id") {
            let user_id = user_id.to_str()
                .map_err(|_| AppError::Unauthorized("Invalid x-user-id header".into()))?
                .to_string();
            Ok(UserId(user_id))
        } else {
            Err(AppError::Unauthorized("Missing x-user-id header".into()))
        }
    }
}

// Middleware to extract user_id from header and make it available for handlers
pub async fn auth_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // Get the user_id from header
    let user_id = request.headers()
        .get("x-user-id")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .ok_or_else(|| AppError::Unauthorized("Unauthorized: Missing x-user-id header".into()))?;
    
    // Store the user_id in the request extensions for later retrieval
    request.extensions_mut().insert(user_id);
    
    // Continue to the handler
    Ok(next.run(request).await)
}
