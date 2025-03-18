use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::{Arc, Mutex};

use crate::{
    handlers::{
        auth_middleware,
        box_handlers::{create_box, delete_box, get_box, get_boxes, update_box},
        guardian_handlers::{
            get_guardian_box, get_guardian_boxes, request_unlock, respond_to_unlock_request,
        },
    },
    store::{BoxStore, memory::MemoryBoxStore, dynamo::DynamoBoxStore, BOXES},
};

/// Creates a router with the default store
pub async fn create_router() -> Router {
    // Create store with initial data
    let legacy_store = Arc::new(Mutex::new(BOXES.lock().unwrap().clone()));
    
    // Create DynamoDB store with BoxStore trait
    let box_store = Arc::new(DynamoBoxStore::new().await);
    
    // Create router with stores
    create_router_with_stores(legacy_store, box_store)
}

/// Creates a router with a custom store (for testing)
pub fn create_router_with_store<T: BoxStore + 'static>(store: Arc<T>) -> Router {
    // For testing, create a mock DynamoDB store
    let dynamo_store = Arc::new(DynamoBoxStore::default());
    create_router_with_stores(store, dynamo_store)
}

/// Creates a router with both store types
pub fn create_router_with_stores<T: BoxStore + 'static>(store: Arc<T>, dynamo_store: Arc<DynamoBoxStore>) -> Router {
    // Routes that use the in-memory store
    let memory_routes = Router::new()
        .route("/boxes", get(get_boxes))
        .route("/boxes/:id", get(get_box))
        .route("/boxes/:id", patch(update_box))
        .route("/boxes/:id", delete(delete_box))
        .route("/guardianBoxes", get(get_guardian_boxes))
        .route("/guardianBoxes/:id", get(get_guardian_box))
        .route("/boxes/guardian/:id/request", patch(request_unlock))
        .route("/boxes/guardian/:id/respond", patch(respond_to_unlock_request))
        .with_state(store);
    
    // Routes that use DynamoDB
    let dynamo_routes = Router::new()
        .route("/boxes", post(create_box))
        .with_state(dynamo_store);
    
    // Combine routes and add middleware
    memory_routes
        .merge(dynamo_routes)
        .layer(middleware::from_fn(auth_middleware))
}
