use axum::{
    extract::{Path, State, Extension},
    Json,
};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    models::{GuardianResponseRequest, LeadGuardianUpdateRequest, UnlockRequest, now_str},
    store::{BoxStore, convert_to_guardian_box},
};

// GET /guardianBoxes
pub async fn get_guardian_boxes(
    State(store): State<BoxStore>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>> {
    let boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    let guardian_boxes: Vec<_> = boxes_guard
        .iter()
        .filter_map(|b| convert_to_guardian_box(b, &user_id))
        .collect();

    Ok(Json(serde_json::json!({ "boxes": guardian_boxes })))
}

// GET /guardianBoxes/:id
pub async fn get_guardian_box(
    State(store): State<BoxStore>,
    Path(id): Path<String>,
    Extension(user_id): Extension<String>,
) -> Result<Json<serde_json::Value>> {
    let boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    if let Some(box_rec) = boxes_guard.iter().find(|b| b.id == id) {
        if let Some(guardian_box) = convert_to_guardian_box(box_rec, &user_id) {
            return Ok(Json(serde_json::json!({ "box": guardian_box })));
        }
    }

    Err(AppError::Unauthorized("Unauthorized or Box not found".into()))
}

// PATCH /boxes/guardian/:id/request - For lead guardian to initiate unlock request
pub async fn request_unlock(
    State(store): State<BoxStore>,
    Path(box_id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<LeadGuardianUpdateRequest>,
) -> Result<Json<serde_json::Value>> {
    let mut boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    let box_index = boxes_guard.iter().position(|b| b.id == box_id);
    
    if box_index.is_none() {
        return Err(AppError::NotFound("Box not found".into()));
    }
    
    let box_index = box_index.unwrap();
    let box_record = &mut boxes_guard[box_index];
    
    // Check if user is a guardian and not rejected
    let is_guardian = box_record.guardians.iter().find(|g| g.id == user_id && g.status != "rejected").is_some();
    
    if !is_guardian {
        return Err(AppError::Unauthorized("Not a guardian for this box".into()));
    }
    
    // Check if user is a lead guardian
    let is_lead = box_record.lead_guardians.iter().any(|g| g.id == user_id);
    
    if is_lead {
        // Lead guardian is initiating an unlock request
        let new_unlock = UnlockRequest {
            id: Uuid::new_v4().to_string(),
            requested_at: now_str(),
            status: "pending".into(),
            message: Some(payload.message),
            initiated_by: Some(user_id.clone()),
            approved_by: vec![],
            rejected_by: vec![],
        };
        
        box_record.unlock_request = Some(new_unlock);
        box_record.updated_at = now_str();
        
        if let Some(guard_box) = convert_to_guardian_box(box_record, &user_id) {
            return Ok(Json(serde_json::json!({ "box": guard_box })));
        } else {
            return Err(AppError::InternalServerError("Failed to render guardian box".into()));
        }
    }
    
    Err(AppError::BadRequest("User is not a lead guardian for this box".into()))
}

// PATCH /boxes/guardian/:id/respond - For guardians to respond to unlock request
pub async fn respond_to_unlock_request(
    State(store): State<BoxStore>,
    Path(box_id): Path<String>,
    Extension(user_id): Extension<String>,
    Json(payload): Json<GuardianResponseRequest>,
) -> Result<Json<serde_json::Value>> {
    let mut boxes_guard = store.lock().map_err(|_| {
        AppError::InternalServerError("Failed to acquire lock".into())
    })?;

    let box_index = boxes_guard.iter().position(|b| b.id == box_id)
        .ok_or_else(|| AppError::NotFound("Box not found".into()))?;

    let box_record = &mut boxes_guard[box_index];
    
    // Check if user is a guardian and not rejected
    if box_record.guardians.iter().find(|g| g.id == user_id && g.status != "rejected").is_none() {
        return Err(AppError::Unauthorized("Not a guardian for this box".into()));
    }
    
    // Check if there's an unlock request to respond to
    if box_record.unlock_request.is_none() {
        return Err(AppError::BadRequest("No unlock request exists to update".into()));
    }
    
    if let Some(unlock) = &mut box_record.unlock_request {
        let mut updated = false;
        
        if let Some(approve) = payload.approve {
            if approve && !unlock.approved_by.contains(&user_id) {
                unlock.approved_by.push(user_id.clone());
                updated = true;
            }
        }
        
        if let Some(reject) = payload.reject {
            if reject && !unlock.rejected_by.contains(&user_id) {
                unlock.rejected_by.push(user_id.clone());
                updated = true;
            }
        }
        
        if !updated {
            return Err(AppError::BadRequest("No valid update field provided".into()));
        }
    }
    
    box_record.updated_at = now_str();
    
    if let Some(guard_box) = convert_to_guardian_box(box_record, &user_id) {
        return Ok(Json(serde_json::json!({ "box": guard_box })));
    } else {
        return Err(AppError::InternalServerError("Failed to render guardian box".into()));
    }
}
