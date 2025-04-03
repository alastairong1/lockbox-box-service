pub mod box_handlers;
pub mod guardian_handlers;

use aws_lambda_events::apigw::ApiGatewayProxyRequestContext;
use axum::{
    async_trait, extract::FromRequestParts, extract::Request, http::request::Parts,
    middleware::Next, response::Response, Extension,
};
use serde_json::Value;

use crate::error::AppError;

// Extractor for user_id from header
#[allow(dead_code)]
pub struct UserId(String);

#[async_trait]
impl<S> FromRequestParts<S> for UserId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        if let Some(user_id) = parts.headers.get("x-user-id") {
            let user_id = user_id
                .to_str()
                .map_err(|_| AppError::Unauthorized("Invalid x-user-id header".into()))?
                .to_string();
            Ok(UserId(user_id))
        } else {
            Err(AppError::Unauthorized("Missing x-user-id header".into()))
        }
    }
}

// Middleware to extract user_id from Cognito JWT in the request
pub async fn auth_middleware(mut request: Request, next: Next) -> Result<Response, AppError> {
    // Extract the Cognito user from the request context
    // API Gateway with Cognito Authorizer (Lambda Proxy integration) adds this information
    tracing::error!("Incoming event: {:?}", request);

    let user_id = if let Some(context) = request.extensions().get::<ApiGatewayProxyRequestContext>()
    {
        // Standard Cognito authorizer puts claims here: context.authorizer["claims"]
        // authorizer is a HashMap<String, Value>, not an Option
        if let Some(claims_val) = context.authorizer.get("claims") {
            // claims_val should be a Value::Object containing the JWT claims
            if let Value::Object(claims) = claims_val {
                if let Some(sub_val) = claims.get("sub") {
                    if let Value::String(sub) = sub_val {
                        sub.clone()
                    } else {
                        tracing::error!("Cognito 'sub' claim is not a string: {:?}", sub_val);
                        return Err(AppError::Unauthorized(
                            "Invalid user ID format in claims".into(),
                        ));
                    }
                } else {
                    tracing::error!(
                        "Authorizer claims object found but no 'sub' key: {:?}",
                        claims
                    );
                    return Err(AppError::Unauthorized(
                        "Could not extract user ID from authorizer claims".into(),
                    ));
                }
            } else {
                tracing::error!(
                    "Authorizer 'claims' field is not an object: {:?}",
                    claims_val
                );
                return Err(AppError::Unauthorized(
                    "Invalid claims format in authorizer".into(),
                ));
            }
        } else {
            tracing::error!(
                "Request context found but no 'claims' key in authorizer: {:?}",
                context.authorizer
            );
            return Err(AppError::Unauthorized(
                "No claims found in authorizer context".into(),
            ));
        }
    } else if let Some(authorizer_header) = request.headers().get("x-amzn-oidc-identity") {
        // Fallback for potentially different integration types (e.g., ALB OIDC)
        // This header typically contains the Cognito user ID directly
        authorizer_header
            .to_str()
            .map_err(|_| AppError::Unauthorized("Invalid identity header".into()))?
            .to_string()
    } else {
        // For development/testing only - use header from the request directly
        // This should be removed or guarded strictly in production
        if cfg!(debug_assertions) {
            if let Some(user_id_header) = request.headers().get("x-user-id") {
                user_id_header
                    .to_str()
                    .map_err(|_| AppError::Unauthorized("Invalid x-user-id header".into()))?
                    .to_string()
            } else {
                tracing::error!(
                    "No authentication information found in request (debug mode, header missing)"
                );
                return Err(AppError::Unauthorized(
                    "No authentication information found".into(),
                ));
            }
        } else {
            tracing::error!("No authentication information found in request (production mode)");
            return Err(AppError::Unauthorized(
                "No authentication information found".into(),
            ));
        }
    };

    tracing::debug!("Authenticated user ID: {}", user_id);

    // Store the user_id in the request extensions for later retrieval
    request.extensions_mut().insert(user_id);

    // Continue to the handler
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::get, Router};
    use tower::ServiceExt;

    // Dummy handler to check if user_id extension is present
    async fn check_user_id_handler(Extension(user_id): Extension<String>) -> StatusCode {
        if !user_id.is_empty() {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR // Should not happen if middleware works
        }
    }

    #[tokio::test]
    #[cfg(debug_assertions)] // Only run this test in debug builds
    async fn test_auth_middleware_success_debug_header_fallback() {
        // Arrange: Router with middleware
        let app = Router::new()
            .route("/", get(check_user_id_handler))
            .layer(axum::middleware::from_fn(auth_middleware));

        // Arrange: Request without Cognito context but with the debug header
        let request = Request::builder()
            .uri("/")
            .header("x-user-id", "debug-user-456")
            .body(axum::body::Body::empty())
            .unwrap();

        // Act: Call the middleware
        let response = app.oneshot(request).await.unwrap();

        // Assert: Handler received user_id from header
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[cfg(debug_assertions)] // Only run this test in debug builds
    async fn test_auth_middleware_failure_debug_no_header() {
        // Arrange: Router with middleware
        let app = Router::new()
            .route("/", get(check_user_id_handler))
            .layer(axum::middleware::from_fn(auth_middleware));

        // Arrange: Request without Cognito context and without debug header
        let request = Request::builder()
            .uri("/")
            .body(axum::body::Body::empty())
            .unwrap();

        // Act: Call the middleware
        let response = app.oneshot(request).await.unwrap();

        // Assert: Middleware returned Unauthorized
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
