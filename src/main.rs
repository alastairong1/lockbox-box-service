mod error;
mod handlers;
mod models;
mod routes;
mod store;
#[cfg(test)]
mod tests;

use axum::{body::Body, extract::Request, response::Response};
use lambda_http::{run, service_fn, Error, Request as LambdaRequest, Response as LambdaResponse, Body as LambdaBody};
use tower::ServiceExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// The Lambda handler function
async fn function_handler(event: LambdaRequest) -> Result<LambdaResponse<LambdaBody>, Error> {
    // Get router
    let app = routes::create_router();
    
    // Convert the Lambda event to an HTTP request for Axum
    let (parts, body) = event.into_parts();
    let body = match body {
        LambdaBody::Empty => Body::empty(),
        LambdaBody::Text(text) => Body::from(text),
        LambdaBody::Binary(data) => Body::from(data),
    };
    
    let http_request = Request::from_parts(parts, body);

    // Process the request through Axum
    let response = app.oneshot(http_request).await?;
    
    // Convert Axum's response to Lambda's response
    let lambda_response = response_to_lambda(response).await?;
    
    Ok(lambda_response)
}

// Convert the Axum response to a format suitable for Lambda
async fn response_to_lambda(response: Response) -> Result<LambdaResponse<LambdaBody>, Error> {
    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await?;
    
    let builder = LambdaResponse::builder()
        .status(parts.status);
    
    // Add response headers
    let builder_with_headers = parts.headers.iter().fold(builder, |builder, (name, value)| {
        builder.header(name.as_str(), value.as_bytes())
    });
    
    let lambda_response = if bytes.is_empty() {
        builder_with_headers.body(LambdaBody::Empty)?
    } else {
        builder_with_headers.body(LambdaBody::Binary(bytes.to_vec()))?
    };
    
    Ok(lambda_response)
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
    run(service_fn(function_handler)).await?;
    Ok(())
}
