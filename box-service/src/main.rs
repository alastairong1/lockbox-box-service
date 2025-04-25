mod error;
mod handlers;
// Keep models for request/response types
mod models;
mod routes;

#[cfg(test)]
mod tests;

use axum::{body::Body, extract::Request, response::Response};
use lambda_http::{
    run, service_fn, Body as LambdaBody, Error, Request as LambdaRequest,
    Response as LambdaResponse,
};
use tower::ServiceExt;

// The Lambda handler function
async fn function_handler(event: LambdaRequest) -> Result<LambdaResponse<LambdaBody>, Error> {
    // Log request details
    tracing::info!(
        "Received Lambda request: method={:?}, path={:?}, query_params={:?}",
        event.method(),
        event.uri().path(),
        event.uri().query()
    );

    // Create application state including DynamoDB client
    let app = routes::create_router().await;

    // Convert the Lambda event to an HTTP request for Axum
    let (parts, body) = event.into_parts();
    let body = match body {
        LambdaBody::Empty => Body::empty(),
        LambdaBody::Text(text) => {
            tracing::debug!("Request body (text): {}", text);
            Body::from(text)
        }
        LambdaBody::Binary(data) => {
            tracing::debug!("Request body (binary): {} bytes", data.len());
            Body::from(data)
        }
    };

    let http_request = Request::from_parts(parts, body);
    tracing::debug!("Created HTTP request: {:?}", http_request);

    // Process the request through Axum
    tracing::info!("Passing request to Axum router");
    let response = match app.oneshot(http_request).await {
        Ok(response) => {
            tracing::info!("Received response from Axum: status={}", response.status());
            response
        }
        Err(err) => {
            tracing::error!("Error from Axum router: {:?}", err);
            return Err(err.into());
        }
    };

    // Convert Axum's response to Lambda's response
    let lambda_response = response_to_lambda(response).await?;
    tracing::info!(
        "Returning Lambda response: status={}",
        lambda_response.status()
    );

    Ok(lambda_response)
}

// Convert the Axum response to a format suitable for Lambda
async fn response_to_lambda(response: Response) -> Result<LambdaResponse<LambdaBody>, Error> {
    let (parts, body) = response.into_parts();
    tracing::debug!(
        "Converting response: status={}, headers={:?}",
        parts.status,
        parts.headers
    );

    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => {
            tracing::debug!("Response body size: {} bytes", bytes.len());
            bytes
        }
        Err(err) => {
            tracing::error!("Failed to read response body: {:?}", err);
            return Err(err.into());
        }
    };

    let builder = LambdaResponse::builder().status(parts.status);

    // Add response headers
    let builder_with_headers = parts
        .headers
        .iter()
        .fold(builder, |builder, (name, value)| {
            tracing::trace!("Adding response header: {}={:?}", name, value);
            builder.header(name.as_str(), value.as_bytes())
        });

    let lambda_response = if bytes.is_empty() {
        tracing::debug!("Creating empty response body");
        builder_with_headers.body(LambdaBody::Empty)?
    } else {
        tracing::debug!("Creating binary response body: {} bytes", bytes.len());
        builder_with_headers.body(LambdaBody::Binary(bytes.to_vec()))?
    };

    Ok(lambda_response)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing with enhanced configuration
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info,box_service=debug".into());

    // Configure and initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_ansi(false) // Disable ANSI colors in Lambda environment
        .with_target(true) // Include the target (module path) in logs
        .init();

    tracing::info!(
        "Logging initialized at level: {}",
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
    );

    // Log AWS Lambda environment information
    if let Ok(function_name) = std::env::var("AWS_LAMBDA_FUNCTION_NAME") {
        tracing::info!(
            "Starting AWS Lambda function: {} (version: {})",
            function_name,
            std::env::var("AWS_LAMBDA_FUNCTION_VERSION").unwrap_or_else(|_| "unknown".into())
        );
    } else {
        tracing::info!("Starting service in non-Lambda environment");
    }

    // Run as Lambda function
    run(service_fn(function_handler)).await?;

    tracing::info!("Lambda function completed");
    Ok(())
}
