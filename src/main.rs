mod models;
mod handlers;
mod routes;
mod store;
mod error;
#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    // Get the router
    let app = routes::create_router();
    
    // Define the address to listen on
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    
    tracing::info!("Starting server on {}", addr);
    
    // Start the server
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
