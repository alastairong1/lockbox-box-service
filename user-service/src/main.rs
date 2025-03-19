use lambda_http::{run, service_fn, Body, Error, Request, Response};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Handler for AWS Lambda requests
async fn handler(event: Request) -> Result<Response<Body>, Error> {
    // For now, just return a simple response
    let response = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(r#"{"message": "User service is running"}"#.into())
        .map_err(Box::new)?;

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize the tracing subscriber
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!("User service starting");

    // Run the Lambda service
    run(service_fn(handler)).await
}
