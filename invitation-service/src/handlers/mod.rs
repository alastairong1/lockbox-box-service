use axum::{
    extract::Request,
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::env;

pub mod invitation_handlers;

// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    iat: usize,
    #[serde(rename = "cognito:username")]
    cognito_username: String,
}

// Authentication middleware
pub async fn auth_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Allow health checks and other public endpoints without authentication
    let path = request.uri().path();
    if path.ends_with("/health") || path.contains("/invitations/") && request.method() == http::Method::GET {
        return Ok(next.run(request).await);
    }

    // Get token from Authorization header
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Extract token
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Get the JWK from environment or use a fixed value for development
    let jwk = env::var("COGNITO_JWK").unwrap_or_else(|_| {
        // This is a placeholder for development, in production you should always provide the actual JWK
        "dev_key".to_string()
    });

    // Decode and validate the token
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwk.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add user information to request extensions
    let user_id = token_data.claims.sub;
    request.extensions_mut().insert(user_id);

    // Continue with the request
    Ok(next.run(request).await)
} 