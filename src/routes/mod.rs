use std::sync::Arc;
use axum::{
    middleware,
    routing::{get, patch},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::handlers::{
    auth_middleware,
    box_handlers::{create_box, delete_box, get_box, get_boxes, update_box},
    guardian_handlers::{get_guardian_box, get_guardian_boxes, request_unlock, respond_to_unlock_request},
};
use crate::store::{BoxStore, dynamo::DynamoBoxStore};

/// Creates a router with the default store
pub async fn create_router() -> Router {
    // Initialize the DynamoDB store
    let dynamo_store = Arc::new(DynamoBoxStore::new().await);
    
    create_router_with_store(dynamo_store)
}

/// Creates a router with a given store implementation
pub fn create_router_with_store<S>(store: Arc<S>) -> Router 
where
    S: BoxStore + 'static,
{
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create router with the store and routes
    Router::new()
        .route("/boxes", get(get_boxes).post(create_box))
        .route("/boxes/:id", get(get_box).patch(update_box).delete(delete_box))
        .route("/guardianBoxes", get(get_guardian_boxes))
        .route("/guardianBoxes/:id", get(get_guardian_box))
        .route("/boxes/guardian/:id/request", patch(request_unlock))
        .route("/boxes/guardian/:id/respond", patch(respond_to_unlock_request))
        .layer(cors)
        .layer(middleware::from_fn(auth_middleware))
        .with_state(store)
}
