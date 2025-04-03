pub mod box_handlers;
pub mod guardian_handlers;

use axum::{extract::Request, middleware::Next, response::Response};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

// Cognito JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    #[serde(rename = "email_verified")]
    pub email_verified: Option<bool>,
    pub iss: String,
    #[serde(rename = "cognito:username")]
    pub cognito_username: Option<String>,
    pub origin_jti: Option<String>,
    pub aud: String,
    pub event_id: Option<String>,
    #[serde(rename = "token_use")]
    pub token_use: Option<String>,
    pub auth_time: Option<usize>,
    pub exp: usize,
    pub iat: usize,
    pub jti: Option<String>,
    pub email: Option<String>,
}

// Simple JWT decoder without verification
pub fn decode_jwt_payload(token: &str) -> Result<Claims, AppError> {
    tracing::debug!("Decoding JWT payload");
    
    // Extract payload (second part of the JWT)
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        tracing::warn!("Invalid JWT format: expected 3 parts, got {}", parts.len());
        return Err(AppError::Unauthorized("Invalid JWT format".into()));
    }
    
    tracing::debug!("JWT structure verified, decoding payload part");

    // Decode the payload
    let payload_data = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(data) => data,
        Err(err) => {
            tracing::warn!("Failed to base64 decode JWT payload: {:?}", err);
            return Err(AppError::Unauthorized("Could not decode JWT payload".into()));
        }
    };

    // Parse the payload
    match serde_json::from_slice::<Claims>(&payload_data) {
        Ok(claims) => {
            tracing::debug!("JWT claims parsed successfully: sub={}", claims.sub);
            Ok(claims)
        }
        Err(err) => {
            tracing::warn!("Failed to parse JWT claims: {:?}", err);
            
            // Try to parse as generic JSON to see what fields are missing
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&payload_data) {
                tracing::debug!("Raw JWT payload: {:?}", value);
            }
            
            Err(AppError::Unauthorized("Could not parse JWT claims".into()))
        }
    }
}

// Original middleware to extract user_id from Cognito JWT in the request
pub async fn auth_middleware(mut request: Request, next: Next) -> Result<Response, AppError> {
    // Log request details
    tracing::info!(
        "Auth middleware: method={:?}, path={:?}, query_params={:?}",
        request.method(),
        request.uri().path(),
        request.uri().query()
    );

    // Extract the JWT from the Authorization header
    let auth_header = match request.headers().get("authorization") {
        Some(header) => header,
        None => {
            tracing::warn!("Missing authorization header in request");
            return Err(AppError::Unauthorized("Missing authorization header".into()));
        }
    };

    // Parse the auth header to get the token
    let bearer_token = match auth_header.to_str() {
        Ok(token) => token,
        Err(err) => {
            tracing::warn!("Invalid authorization header format: {:?}", err);
            return Err(AppError::Unauthorized("Invalid authorization header".into()));
        }
    };

    if !bearer_token.starts_with("Bearer ") {
        tracing::warn!("Authorization header doesn't start with 'Bearer '");
        return Err(AppError::Unauthorized(
            "Invalid authorization format. Expected 'Bearer <token>'".into(),
        ));
    }

    let token = &bearer_token[7..]; // Skip "Bearer " prefix
    tracing::debug!("JWT token length: {}", token.len());

    // Decode the JWT (without verification since API Gateway already did that)
    let claims = match decode_jwt_payload(token) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::warn!("Failed to decode JWT payload: {:?}", err);
            return Err(err);
        }
    };
    
    let user_id = claims.sub;
    tracing::info!("Authenticated user ID: {}", user_id);

    // Store the user_id in the request extensions for later retrieval
    request.extensions_mut().insert(user_id);

    // Continue to the handler
    tracing::debug!("Forwarding authenticated request to handler");
    let response = next.run(request).await;
    tracing::info!("Handler response status: {:?}", response.status());
    
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::Extension,
        http::{Request as HttpRequest, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;
    // Dummy handler to check if user_id extension is present
    async fn check_user_id_handler(Extension(user_id): Extension<String>) -> impl IntoResponse {
        if !user_id.is_empty() {
            StatusCode::OK
        } else {
            StatusCode::INTERNAL_SERVER_ERROR // Should not happen if middleware works
        }
    }

    #[tokio::test]
    async fn test_auth_middleware_jwt_token() {
        // Arrange: Router with middleware
        let app = Router::new()
            .route("/", get(check_user_id_handler))
            .layer(axum::middleware::from_fn(auth_middleware));

        // Create claims for the JWT with a future expiration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before Unix epoch")
            .as_secs() as usize;

        let exp = now + 3600; // 1 hour in the future

        let claims = Claims {
            sub: "56a20244-0061-708a-0441-62c42ace7b39".to_string(),
            email_verified: Some(true),
            iss: "https://cognito-idp.eu-west-2.amazonaws.com/eu-west-2_SnyjSmOpW".to_string(),
            cognito_username: Some("56a20244-0061-708a-0441-62c42ace7b39".to_string()),
            origin_jti: Some("2961a64b-e7ec-4885-994a-d650cc7a7c2d".to_string()),
            aud: "5pgt5gkfulqs0tkdi279c895gp".to_string(),
            event_id: Some("2096030a-d0cb-480a-9318-6f255408c66c".to_string()),
            token_use: Some("id".to_string()),
            auth_time: Some(now - 100),
            exp,
            iat: now - 100,
            jti: Some("021ba19b-7fce-4bc0-b246-852346c43d4e".to_string()),
            email: Some("alastair.ong@icloud.com".to_string()),
        };

        // Create JWT header
        let header = Header::new(Algorithm::HS256);

        // In a real scenario, Cognito would use RS256 with a proper key pair
        // For testing purposes, we use HS256 with a simple secret
        let secret = "test_secret_key_for_jwt_encoding_in_tests";
        let encoding_key = EncodingKey::from_secret(secret.as_bytes());

        // Generate the JWT
        let token = encode(&header, &claims, &encoding_key).expect("Failed to create JWT");

        // Create request with Authorization header
        let request = HttpRequest::builder()
            .uri("/")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        // Act: Call the middleware
        let response = app.oneshot(request).await.unwrap();

        // Assert: Handler received user_id from JWT
        assert_eq!(response.status(), StatusCode::OK);
    }
}
