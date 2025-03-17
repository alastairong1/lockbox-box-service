use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{BoxRecord, BoxResponse, CreateBoxRequest, UpdateBoxRequest, now_str},
    store::BoxStore,
};

// GET /boxes
pub async fn get_boxes(
    State(store): State<BoxStore>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>> {
    let boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    let my_boxes: Vec<_> = boxes_guard
        .iter()
        .filter(|b| b.owner_id == user_id)
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
pub async fn get_box(
    State(store): State<BoxStore>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>> {
    let boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    if let Some(box_rec) = boxes_guard.iter().find(|b| b.id == id) {
        if box_rec.owner_id == user_id {
            // Return full box info for owner
            return Ok(Json(serde_json::json!({
                "box": BoxResponse {
                    id: box_rec.id.clone(),
                    name: box_rec.name.clone(),
                    description: box_rec.description.clone(),
                    created_at: box_rec.created_at.clone(),
                    updated_at: box_rec.updated_at.clone(),
                }
            })));
        }
    }

    Err(AppError::Unauthorized("Unauthorized or Box not found".into()))
}

// POST /boxes
pub async fn create_box(
    State(store): State<BoxStore>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<CreateBoxRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>)> {
    let mut boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

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

    let response = BoxResponse {
        id: new_box.id.clone(),
        name: new_box.name.clone(),
        description: new_box.description.clone(),
        created_at: new_box.created_at.clone(),
        updated_at: new_box.updated_at.clone(),
    };

    boxes_guard.push(new_box);

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "box": response }))))
}

// PATCH /boxes/:id
pub async fn update_box(
    State(store): State<BoxStore>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<UpdateBoxRequest>,
) -> Result<Json<serde_json::Value>> {
    let mut boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    if let Some(box_rec) = boxes_guard.iter_mut().find(|b| b.id == id && b.owner_id == user_id) {
        if let Some(name) = payload.name {
            box_rec.name = name;
        }

        if let Some(description) = payload.description {
            box_rec.description = description;
        }

        box_rec.updated_at = now_str();

        let response = BoxResponse {
            id: box_rec.id.clone(),
            name: box_rec.name.clone(),
            description: box_rec.description.clone(),
            created_at: box_rec.created_at.clone(),
            updated_at: box_rec.updated_at.clone(),
        };

        return Ok(Json(serde_json::json!({ "box": response })));
    }

    Err(AppError::Unauthorized("Unauthorized or Box not found".into()))
}

// DELETE /boxes/:id
pub async fn delete_box(
    State(store): State<BoxStore>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>> {
    let mut boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    if let Some(pos) = boxes_guard.iter().position(|b| b.id == id && b.owner_id == user_id) {
        boxes_guard.remove(pos);
        return Ok(Json(serde_json::json!({ "message": "Box deleted successfully." })));
    }

    Err(AppError::Unauthorized("Unauthorized or Box not found".into()))
}
