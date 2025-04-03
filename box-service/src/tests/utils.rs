use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::handlers::Claims;

/// Creates a JWT token for testing purposes
///
/// This function creates a Cognito-like JWT token for a specific user ID
/// which can be used in test authorization headers
pub fn create_jwt_token(user_id: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time is before Unix epoch")
        .as_secs() as usize;

    let exp = now + 3600; // 1 hour in the future

    let claims = Claims {
        sub: user_id.to_string(),
        email_verified: Some(true),
        iss: "https://cognito-idp.eu-west-2.amazonaws.com/eu-west-2_SnyjSmOpW".to_string(),
        cognito_username: Some(user_id.to_string()),
        origin_jti: Some("2961a64b-e7ec-4885-994a-d650cc7a7c2d".to_string()),
        aud: "5pgt5gkfulqs0tkdi279c895gp".to_string(),
        event_id: Some("2096030a-d0cb-480a-9318-6f255408c66c".to_string()),
        token_use: Some("id".to_string()),
        auth_time: Some(now - 100),
        exp,
        iat: now - 100,
        jti: Some("021ba19b-7fce-4bc0-b246-852346c43d4e".to_string()),
        email: Some("test@example.com".to_string()),
    };

    // Create JWT header
    let header = Header::new(Algorithm::HS256);

    // In a real scenario, Cognito would use RS256 with a proper key pair
    // For testing purposes, we use HS256 with a simple secret
    let secret = "test_secret_key_for_jwt_encoding_in_tests";
    let encoding_key = EncodingKey::from_secret(secret.as_bytes());

    // Generate the JWT
    encode(&header, &claims, &encoding_key).expect("Failed to create JWT")
}

/// Helper function to create an authorization header with a bearer token
pub fn create_auth_header(user_id: &str) -> (String, String) {
    let token = create_jwt_token(user_id);
    ("authorization".to_string(), format!("Bearer {}", token))
}
