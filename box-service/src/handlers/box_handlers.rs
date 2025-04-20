use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};
use lockbox_shared::store::BoxStore;
use serde_json;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, Result};
// Import models from shared crate
use crate::shared_models::{
    BoxRecord, Document, Guardian, now_str,
};
// Import request/response types from local models
use crate::models::{
    BoxResponse, CreateBoxRequest, DocumentUpdateRequest,
    DocumentUpdateResponse, GuardianUpdateRequest, GuardianUpdateResponse,
    OptionalField, UpdateBoxRequest,
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
            documents: b.documents.clone(),
            guardians: b.guardians.clone(),
            lead_guardians: b.lead_guardians.clone(),
            owner_id: b.owner_id.clone(),
            owner_name: b.owner_name.clone(),
            unlock_request: b.unlock_request.clone(),
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
        return Err(AppError::unauthorized(
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
            documents: box_rec.documents.clone(),
            guardians: box_rec.guardians.clone(),
            lead_guardians: box_rec.lead_guardians.clone(),
            owner_id: box_rec.owner_id.clone(),
            owner_name: box_rec.owner_name.clone(),
            unlock_request: box_rec.unlock_request.clone(),
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
        documents: created_box.documents.clone(),
        guardians: created_box.guardians.clone(),
        lead_guardians: created_box.lead_guardians.clone(),
        owner_id: created_box.owner_id.clone(),
        owner_name: created_box.owner_name.clone(),
        unlock_request: created_box.unlock_request.clone(),
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
        return Err(AppError::unauthorized(
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

    // For unlock_instructions, we need to handle both the case of setting it to a value
    // or explicitly clearing it by setting it to None
    if let Some(field) = &payload.unlock_instructions {
        match field {
            OptionalField::Value(val) => box_rec.unlock_instructions = Some(val.clone()),
            OptionalField::Null => box_rec.unlock_instructions = None,
        }
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
        documents: updated_box.documents.clone(),
        guardians: updated_box.guardians.clone(),
        lead_guardians: updated_box.lead_guardians.clone(),
        owner_id: updated_box.owner_id.clone(),
        owner_name: updated_box.owner_name.clone(),
        unlock_request: updated_box.unlock_request.clone(),
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
        return Err(AppError::unauthorized(
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
        return Err(AppError::unauthorized(
            "You don't have permission to update this box".into(),
        ));
    }

    // Check if the guardian already exists in the box
    let guardian_index = box_rec.guardians.iter().position(|g| g.id == guardian.id);

    let was_updated = if let Some(index) = guardian_index {
        // Update existing guardian
        box_rec.guardians[index] = guardian.clone();

        // Update lead_guardians array if needed
        if guardian.lead_guardian {
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
        if guardian.lead_guardian {
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
        return Err(AppError::unauthorized(
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

// Helper function to delete a document from a box
// Returns updated box after deletion
async fn delete_document_from_box<S>(
    store: &S,
    box_id: &str,
    owner_id: &str,
    document_id: &str,
) -> Result<BoxRecord>
where
    S: BoxStore,
{
    // Get the current box from store
    let mut box_rec = store.get_box(box_id).await?;

    // Check if the user is the owner
    if box_rec.owner_id != owner_id {
        return Err(AppError::unauthorized(
            "You don't have permission to delete documents from this box".into(),
        ));
    }

    // Check if the document exists in the box
    let document_index = box_rec.documents.iter().position(|d| d.id == document_id);

    // Return not found if document doesn't exist
    if document_index.is_none() {
        return Err(AppError::not_found(format!(
            "Document with ID {} not found in box {}",
            document_id, box_id
        )));
    }

    // Remove the document
    box_rec.documents.remove(document_index.unwrap());
    box_rec.updated_at = now_str();

    // Save the updated box
    let updated_box = store.update_box(box_rec).await?;

    Ok(updated_box)
}

// DELETE /boxes/owned/:id/document/:document_id
// This is a dedicated endpoint for deleting a single document
pub async fn delete_document<S>(
    State(store): State<Arc<S>>,
    Path((box_id, document_id)): Path<(String, String)>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Use the helper function to delete the document
    let updated_box = delete_document_from_box(&*store, &box_id, &user_id, &document_id).await?;

    // Create a response with all remaining documents
    let response = DocumentUpdateResponse {
        documents: updated_box.documents,
        updated_at: updated_box.updated_at,
    };

    Ok(Json(serde_json::json!({
        "message": "Document deleted successfully",
        "document": response
    })))
}

// Helper function to delete a guardian from a box
// Returns updated box after deletion
async fn delete_guardian_from_box<S>(
    store: &S,
    box_id: &str,
    owner_id: &str,
    guardian_id: &str,
) -> Result<BoxRecord>
where
    S: BoxStore,
{
    // Get the current box from store
    let mut box_rec = store.get_box(box_id).await?;

    // Check if the user is the owner
    if box_rec.owner_id != owner_id {
        return Err(AppError::unauthorized(
            "You don't have permission to delete guardians from this box".into(),
        ));
    }

    // Check if the guardian exists in the box
    let guardian_index = box_rec.guardians.iter().position(|g| g.id == guardian_id);

    // Return not found if guardian doesn't exist
    if guardian_index.is_none() {
        return Err(AppError::not_found(format!(
            "Guardian with ID {} not found in box {}",
            guardian_id, box_id
        )));
    }

    // Check if guardian is also a lead guardian and remove from lead_guardians if needed
    let is_lead = box_rec.guardians[guardian_index.unwrap()].lead_guardian;
    if is_lead {
        box_rec.lead_guardians.retain(|g| g.id != guardian_id);
    }

    // Remove the guardian
    box_rec.guardians.remove(guardian_index.unwrap());
    box_rec.updated_at = now_str();

    // Save the updated box
    let updated_box = store.update_box(box_rec).await?;

    Ok(updated_box)
}

// DELETE /boxes/owned/:id/guardian/:guardian_id
// This is a dedicated endpoint for deleting a single guardian
pub async fn delete_guardian<S>(
    State(store): State<Arc<S>>,
    Path((box_id, guardian_id)): Path<(String, String)>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>>
where
    S: BoxStore,
{
    // Use the helper function to delete the guardian
    let updated_box = delete_guardian_from_box(&*store, &box_id, &user_id, &guardian_id).await?;

    // Create a response with all remaining guardians
    let response = GuardianUpdateResponse {
        guardians: updated_box.guardians,
        updated_at: updated_box.updated_at,
    };

    Ok(Json(serde_json::json!({
        "message": "Guardian deleted successfully",
        "guardian": response
    })))
}
