use axum::{
    middleware,
    routing::{get, patch},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::handlers::{
    auth_middleware,
    box_handlers::{
        create_box, delete_box, delete_document, delete_guardian, get_box, get_boxes, update_box, update_document,
        update_guardian,
    },
    guardian_handlers::{
        get_guardian_box, get_guardian_boxes, request_unlock, respond_to_invitation,
        respond_to_unlock_request,
    },
};
use crate::store::{dynamo::DynamoBoxStore, BoxStore};

/// Creates a router with the default store
pub async fn create_router() -> Router {
    // Create the DynamoDB store
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
        .route("/boxes/owned", get(get_boxes).post(create_box))
        .route(
            "/boxes/owned/:id",
            get(get_box).patch(update_box).delete(delete_box),
        )
        .route("/boxes/owned/:id/guardian", patch(update_guardian))
        .route("/boxes/owned/:id/guardian/:guardian_id", axum::routing::delete(delete_guardian))
        .route("/boxes/owned/:id/document", patch(update_document))
        .route("/boxes/owned/:id/document/:document_id", axum::routing::delete(delete_document))
        .route("/boxes/guardian", get(get_guardian_boxes))
        .route("/boxes/guardian/:id", get(get_guardian_box))
        .route("/boxes/guardian/:id/request", patch(request_unlock))
        .route(
            "/boxes/guardian/:id/respond",
            patch(respond_to_unlock_request),
        )
        .route(
            "/boxes/guardian/:id/invitation",
            patch(respond_to_invitation),
        )
        .layer(cors)
        .layer(middleware::from_fn(auth_middleware))
        .with_state(store)
}
