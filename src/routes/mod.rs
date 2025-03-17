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
    store::{BoxStore, BOXES},
};

/// Creates a router with the default store
pub fn create_router() -> Router {
    // Create store with initial data
    let store = Arc::new(Mutex::new(BOXES.lock().unwrap().clone()));
    create_router_with_store(store)
}

/// Creates a router with a custom store (for testing)
pub fn create_router_with_store(store: BoxStore) -> Router {
    // Create routes
    Router::new()
        // Box routes
        .route("/boxes", get(get_boxes))
        .route("/boxes", post(create_box))
        .route("/boxes/:id", get(get_box))
        .route("/boxes/:id", patch(update_box))
        .route("/boxes/:id", delete(delete_box))
        // Guardian box routes
        .route("/guardianBoxes", get(get_guardian_boxes))
        .route("/guardianBoxes/:id", get(get_guardian_box))
        .route("/boxes/guardian/:id/request", patch(request_unlock))
        .route(
            "/boxes/guardian/:id/respond",
            patch(respond_to_unlock_request),
        )
        // Apply middleware
        .layer(middleware::from_fn(auth_middleware))
        // Add store as state
        .with_state(store)
}
