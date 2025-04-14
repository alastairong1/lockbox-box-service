use axum::{
    extract::Request,
    middleware,
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::handlers::{
    auth_middleware,
    invitation_handlers::{
        handle_invitation, create_invitation, refresh_invitation,
    },
};
use lockbox_shared::store::{dynamo::DynamoInvitationStore, InvitationStore};

/// Creates a router with the default store
pub async fn create_router() -> Router {
    tracing::info!("Creating router with DynamoDB store");

    // Create the DynamoDB store
    let dynamo_store = Arc::new(DynamoInvitationStore::new().await);

    // Hardcode the prefix as "/Prod"
    let prefix = "/Prod";

    create_router_with_store(dynamo_store, prefix)
}

/// Creates a router with a given store implementation
pub fn create_router_with_store<S>(store: Arc<S>, prefix: &str) -> Router
where
    S: InvitationStore + 'static,
{
    tracing::info!("Setting up API routes with prefix: {}", prefix);

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    tracing::debug!("CORS configured for all origins, methods and headers");

    // Logging middleware to trace all requests
    async fn logging_middleware(
        req: Request,
        next: axum::middleware::Next,
    ) -> impl axum::response::IntoResponse {
        tracing::info!("Logging request: {:?}", req);

        tracing::info!(
            "Router received request: method={}, uri={}",
            req.method(),
            req.uri()
        );
        next.run(req).await
    }

    // Create the API routes
    let api_routes = Router::new()
        .route("/invitation", post(create_invitation))
        .route("/invitation/handle", put(handle_invitation))
        .route("/invitations/:inviteId/refresh", post(refresh_invitation))
        .layer(middleware::from_fn(auth_middleware))
        .with_state(store);

    // Create the main router with the prefix
    let router = Router::new()
        .nest(prefix, api_routes)
        .layer(cors)
        .layer(middleware::from_fn(logging_middleware));

    tracing::info!(
        "Router configured with all routes and middleware under prefix: {}",
        prefix
    );

    // Add a fallback handler for 404s
    router.fallback(|req: Request| async move {
        tracing::warn!("No route matched for: {} {}", req.method(), req.uri());
        (
            axum::http::StatusCode::NOT_FOUND,
            "The requested resource was not found".to_string(),
        )
    })
} 