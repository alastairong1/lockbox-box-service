use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{now_str, BoxRecord, BoxResponse, CreateBoxRequest, UpdateBoxRequest},
    store::BoxStore,
};

// GET /boxes
pub async fn get_boxes<S>(
    State(store): State<Arc<S>>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Get boxes from store
    let boxes = store.get_boxes_by_owner(&user_id).await?;

    let my_boxes: Vec<_> = boxes
        .iter()
        .map(|b| BoxResponse {
            id: b.id.clone(),
            name: b.name.clone(),
            description: b.description.clone(),
            created_at: b.created_at.clone(),
            updated_at: b.updated_at.clone(),
        })
        .collect();

    Ok(Json(serde_json::json!({ "boxes": my_boxes })))
}

// GET /boxes/:id
pub async fn get_box<S>(
    State(store): State<Arc<S>>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Get box from store
    let box_rec = store.get_box(&id).await?;

    // TODO: Is it safe to check here or should we do filter in the db query?
    if box_rec.owner_id != user_id {
        return Err(AppError::Unauthorized(
            "You don't have permission to view this box".into(),
        ));
    }

    // Return full box info for owner
    Ok(Json(serde_json::json!({
        "box": BoxResponse {
            id: box_rec.id.clone(),
            name: box_rec.name.clone(),
            description: box_rec.description.clone(),
            created_at: box_rec.created_at.clone(),
            updated_at: box_rec.updated_at.clone(),
        }
    })))
}

// POST /boxes
pub async fn create_box<S>(
    State(store): State<Arc<S>>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateBoxRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)>
where
    S: BoxStore,
{
    let now = now_str();
    let new_box = BoxRecord {
        id: Uuid::new_v4().to_string(),
        name: payload.name,
        description: payload.description,
        is_locked: false,
        created_at: now.clone(),
        updated_at: now.clone(),
        owner_id: user_id,
        owner_name: None,
        documents: vec![],
        guardians: vec![],
        lead_guardians: vec![],
        unlock_instructions: None,
        unlock_request: None,
    };

    // Create the box in store
    let created_box = store.create_box(new_box).await?;

    let response = BoxResponse {
        id: created_box.id.clone(),
        name: created_box.name.clone(),
        description: created_box.description.clone(),
        created_at: created_box.created_at.clone(),
        updated_at: created_box.updated_at.clone(),
    };

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "box": response })),
    ))
}

// PATCH /boxes/:id
pub async fn update_box<S>(
    State(store): State<Arc<S>>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateBoxRequest>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Get the current box from store
    let mut box_rec = store.get_box(&id).await?;

    // Check if the user is the owner
    if box_rec.owner_id != user_id {
        return Err(AppError::Unauthorized(
            "You don't have permission to update this box".into(),
        ));
    }

    // Update fields if provided
    if let Some(name) = payload.name {
        box_rec.name = name;
    }

    if let Some(description) = payload.description {
        box_rec.description = description;
    }

    box_rec.updated_at = now_str();

    // Save the updated box
    let updated_box = store.update_box(box_rec).await?;

    let response = BoxResponse {
        id: updated_box.id.clone(),
        name: updated_box.name.clone(),
        description: updated_box.description.clone(),
        created_at: updated_box.created_at.clone(),
        updated_at: updated_box.updated_at.clone(),
    };

    Ok(Json(serde_json::json!({ "box": response })))
}

// DELETE /boxes/:id
pub async fn delete_box<S>(
    State(store): State<Arc<S>>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Get the box to check ownership
    let box_rec = store.get_box(&id).await?;

    // Check if the user is the owner
    if box_rec.owner_id != user_id {
        return Err(AppError::Unauthorized(
            "You don't have permission to delete this box".into(),
        ));
    }

    // Delete the box
    store.delete_box(&id).await?;

    Ok(Json(
        serde_json::json!({ "message": "Box deleted successfully." }),
    ))
}
