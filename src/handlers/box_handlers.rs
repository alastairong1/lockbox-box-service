use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{
        now_str, BoxRecord, BoxResponse, CreateBoxRequest, Document, DocumentUpdateRequest,
        DocumentUpdateResponse, Guardian, GuardianUpdateRequest, GuardianUpdateResponse,
        UpdateBoxRequest,
    },
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
            unlock_instructions: b.unlock_instructions.clone(),
            is_locked: b.is_locked,
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
            unlock_instructions: box_rec.unlock_instructions.clone(),
            is_locked: box_rec.is_locked,
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
        unlock_instructions: created_box.unlock_instructions.clone(),
        is_locked: created_box.is_locked,
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

    // Handle unlock_instructions with our new NullableField
    // If the field was present in the request, update it (even if null)
    if payload.unlock_instructions.was_present() {
        println!(
            "unlockInstructions was present in request: {:?}",
            payload.unlock_instructions
        );
        box_rec.unlock_instructions = payload.unlock_instructions.into_option();
    }

    if let Some(is_locked) = payload.is_locked {
        box_rec.is_locked = is_locked;
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
        unlock_instructions: updated_box.unlock_instructions.clone(),
        is_locked: updated_box.is_locked,
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

// Helper function to update a guardian in a box
// Returns (updated_box, was_guardian_updated)
async fn update_or_add_guardian<S>(
    store: &S,
    box_id: &str,
    owner_id: &str,
    guardian: &Guardian,
) -> Result<(BoxRecord, bool)>
where
    S: BoxStore,
{
    // Get the current box from store
    let mut box_rec = store.get_box(box_id).await?;

    // Check if the user is the owner
    if box_rec.owner_id != owner_id {
        return Err(AppError::Unauthorized(
            "You don't have permission to update this box".into(),
        ));
    }

    // Check if the guardian already exists in the box
    let guardian_index = box_rec.guardians.iter().position(|g| g.id == guardian.id);

    let was_updated = if let Some(index) = guardian_index {
        // Update existing guardian
        box_rec.guardians[index] = guardian.clone();

        // Update lead_guardians array if needed
        if guardian.lead {
            if !box_rec.lead_guardians.iter().any(|g| g.id == guardian.id) {
                box_rec.lead_guardians.push(guardian.clone());
            }
        } else {
            // Remove from lead guardians if needed
            box_rec.lead_guardians.retain(|g| g.id != guardian.id);
        }
        true
    } else {
        // Add new guardian
        box_rec.guardians.push(guardian.clone());

        // Add to lead_guardians if needed
        if guardian.lead {
            box_rec.lead_guardians.push(guardian.clone());
        }
        true
    };

    box_rec.updated_at = now_str();

    // Save the updated box
    let updated_box = store.update_box(box_rec).await?;

    Ok((updated_box, was_updated))
}

// PATCH /boxes/owned/:id/guardian
// This is a dedicated endpoint for updating a single guardian
pub async fn update_guardian<S>(
    State(store): State<Arc<S>>,
    Path(box_id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<GuardianUpdateRequest>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Let the helper function do the work
    let (updated_box, _) =
        update_or_add_guardian(&*store, &box_id, &user_id, &payload.guardian).await?;

    // Create a specialized response with all guardians
    let response = GuardianUpdateResponse {
        guardians: updated_box.guardians,
        updated_at: updated_box.updated_at,
    };

    Ok(Json(serde_json::json!({ "guardian": response })))
}

// Helper function to update a document in a box
// Returns (updated_box, was_document_updated)
async fn update_or_add_document<S>(
    store: &S,
    box_id: &str,
    owner_id: &str,
    document: &Document,
) -> Result<(BoxRecord, bool)>
where
    S: BoxStore,
{
    // Get the current box from store
    let mut box_rec = store.get_box(box_id).await?;

    // Check if the user is the owner
    if box_rec.owner_id != owner_id {
        return Err(AppError::Unauthorized(
            "You don't have permission to update this box".into(),
        ));
    }

    // Check if the document already exists in the box
    let document_index = box_rec.documents.iter().position(|d| d.id == document.id);

    let was_updated = if let Some(index) = document_index {
        // Update existing document
        box_rec.documents[index] = document.clone();
        true
    } else {
        // Add new document
        box_rec.documents.push(document.clone());
        true
    };

    box_rec.updated_at = now_str();

    // Save the updated box
    let updated_box = store.update_box(box_rec).await?;

    Ok((updated_box, was_updated))
}

// PATCH /boxes/owned/:id/document
// This is a dedicated endpoint for updating a single document
pub async fn update_document<S>(
    State(store): State<Arc<S>>,
    Path(box_id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<DocumentUpdateRequest>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Let the helper function do the work
    let (updated_box, _) =
        update_or_add_document(&*store, &box_id, &user_id, &payload.document).await?;

    // Create a specialized response with all documents
    let response = DocumentUpdateResponse {
        documents: updated_box.documents,
        updated_at: updated_box.updated_at,
    };

    Ok(Json(serde_json::json!({ "document": response })))
}
