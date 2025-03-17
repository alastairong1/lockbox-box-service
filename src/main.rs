mod models;
mod handlers;
mod routes;
mod store;
mod error;
#[cfg(test)]
mod tests;

use lambda_runtime::{service_fn, LambdaEvent, Error};
use axum::{extract::Request, response::Response};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// This is the lambda request type
#[derive(Deserialize)]
struct LambdaRequest {
    #[serde(flatten)]
    event: serde_json::Value,
}

// This is the lambda response type
#[derive(Serialize)]
struct LambdaResponse {
    #[serde(flatten)]
    body: serde_json::Value,
}

// The Lambda handler function
async fn function_handler(event: LambdaEvent<LambdaRequest>) -> Result<LambdaResponse, Error> {
    // Get router
    let app = routes::create_router();
    
    // Convert the Lambda event to an HTTP request
    let (event, _context) = event.into_parts();
    let http_request: Request = serde_json::from_value(event.event)?;
    
    // Process the request through Axum
    let response = app.oneshot(http_request).await?;
    
    // Convert Axum's response to Lambda's response
    let body = response_to_lambda(response).await?;
    
    Ok(LambdaResponse { body })
}

// Convert the Axum response to a format suitable for Lambda
async fn response_to_lambda(response: Response) -> Result<serde_json::Value, Error> {
    let (parts, body) = response.into_parts();
    let bytes = hyper::body::to_bytes(body).await?;
    let body_str = String::from_utf8(bytes.to_vec())?;
    
    let mut response_json = serde_json::json!({
        "statusCode": parts.status.as_u16(),
        "headers": {},
        "body": body_str,
        "isBase64Encoded": false
    });
    
    // Add headers
    let headers_obj = response_json["headers"].as_object_mut().unwrap();
    for (key, value) in parts.headers.iter() {
        if let Ok(value_str) = value.to_str() {
            headers_obj.insert(key.to_string(), serde_json::Value::String(value_str.to_string()));
        }
    }
    
    Ok(response_json)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    // Run as Lambda function
    tracing::info!("Starting AWS Lambda function");
    lambda_runtime::run(service_fn(function_handler)).await?;
    Ok(())
}
