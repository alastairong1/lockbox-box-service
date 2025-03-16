use lambda_runtime::{service_fn, Error, LambdaEvent, run};
use aws_lambda_events::event::apigw::{ApiGatewayProxyRequest, ApiGatewayProxyResponse};
use aws_lambda_events::encodings::Body;
use aws_lambda_events::http;
use serde_json::json;
use chrono::Utc;
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use log::info;
use serde::{Serialize, Deserialize};
use http::StatusCode;

//
// Data models
//

#[derive(Serialize, Deserialize, Clone)]
struct Document {
    id: String,
    title: String,
    content: String,
    created_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct Guardian {
    id: String,
    name: String,
    email: String,
    lead: bool,
    status: String, // "pending", "accepted", "rejected"
    added_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct UnlockRequest {
    id: String,
    requested_at: String,
    status: String,
    message: Option<String>,
    initiated_by: Option<String>,
    approved_by: Vec<String>,
    rejected_by: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct BoxRecord {
    id: String,
    name: String,
    description: String,
    is_locked: bool,
    created_at: String,
    updated_at: String,
    owner_id: String,
    owner_name: Option<String>,
    documents: Vec<Document>,
    guardians: Vec<Guardian>,
    lead_guardians: Vec<Guardian>,
    unlock_instructions: Option<String>,
    unlock_request: Option<UnlockRequest>,
}

#[derive(Serialize, Deserialize)]
struct GuardianBox {
    id: String,
    name: String,
    description: String,
    is_locked: bool,
    created_at: String,
    updated_at: String,
    owner_id: String,
    owner_name: Option<String>,
    unlock_instructions: Option<String>,
    unlock_request: Option<UnlockRequest>,
    pending_guardian_approval: Option<bool>,
    guardians_count: usize,
    is_lead_guardian: bool,
}

//
// Global in-memory store (for sample purposes only)
//
static BOXES: Lazy<Mutex<Vec<BoxRecord>>> = Lazy::new(|| {
    let now = Utc::now().to_rfc3339();
    Mutex::new(vec![
        BoxRecord {
            id: Uuid::new_v4().to_string(),
            name: "Sample Box".into(),
            description: "A sample box".into(),
            is_locked: false,
            created_at: now.clone(),
            updated_at: now.clone(),
            owner_id: "user_1".into(),
            owner_name: Some("User One".into()),
            documents: vec![],
            guardians: vec![
                Guardian {
                    id: "guardian_1".into(),
                    name: "Guardian One".into(),
                    email: "guardian1@example.com".into(),
                    lead: false,
                    status: "pending".into(),
                    added_at: now.clone(),
                }
            ],
            lead_guardians: vec![],
            unlock_instructions: None,
            unlock_request: None,
        }
    ])
});

//
// Helper functions
//

// Returns current time in ISO8601 format
fn now_str() -> String {
    Utc::now().to_rfc3339()
}

// Helper to return a Json API response
fn response(status: StatusCode, body: serde_json::Value) -> ApiGatewayProxyResponse {
    let mut headers: http::HeaderMap = http::HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json")
    );
    ApiGatewayProxyResponse {
        status_code: status.as_u16() as i64,
        headers,
        multi_value_headers: http::HeaderMap::new(),
        body: Some(Body::Text(body.to_string())),
        is_base64_encoded: false,
    }
}

// Convert a BoxRecord into a GuardianBox, if the user is a guardian and not rejected
fn to_guardian_box(box_rec: &BoxRecord, user_id: &str) -> Option<GuardianBox> {
    if let Some(guardian) = box_rec.guardians.iter().find(|g| g.id == user_id && g.status != "rejected") {
        let pending = guardian.status == "pending";
        let is_lead = box_rec.lead_guardians.iter().any(|g| g.id == user_id);
        Some(GuardianBox {
            id: box_rec.id.clone(),
            name: box_rec.name.clone(),
            description: box_rec.description.clone(),
            is_locked: box_rec.is_locked,
            created_at: box_rec.created_at.clone(),
            updated_at: box_rec.updated_at.clone(),
            owner_id: box_rec.owner_id.clone(),
            owner_name: box_rec.owner_name.clone(),
            unlock_instructions: box_rec.unlock_instructions.clone(),
            unlock_request: box_rec.unlock_request.clone(),
            pending_guardian_approval: Some(pending),
            guardians_count: box_rec.guardians.len(),
            is_lead_guardian: is_lead,
        })
    } else {
        None
    }
}

//
// Lambda handler
//

async fn function_handler(event: LambdaEvent<ApiGatewayProxyRequest>) -> Result<ApiGatewayProxyResponse, Error> {
    info!("Received event: {:?}", event);
    
    let (event, _context) = event.into_parts();

    // Obtain user id from header
    let headers = event.headers;
    let user_id = if let Some(uid) = headers.get("x-user-id") {
        uid.to_str().unwrap().to_string()
    } else {
        return Ok(response(StatusCode::UNAUTHORIZED, json!({"error": "Unauthorized: Missing x-user-id header"})));
    };

    let method = event.http_method.to_string();
    let path = event.path.unwrap_or_default();

    let mut boxes_guard = BOXES.lock().unwrap();

    // Routing for My Boxes endpoints
    if method == "GET" && path == "/boxes" {
        // Return all boxes owned by user
        let my_boxes: Vec<_> = boxes_guard.iter()
            .filter(|b| b.owner_id == user_id)
            .map(|b| json!({
                "id": b.id,
                "name": b.name,
                "description": b.description,
                "created_at": b.created_at,
                "updated_at": b.updated_at
            }))
            .collect();
        return Ok(response(StatusCode::OK, json!({"boxes": my_boxes})));
    }
    else if method == "GET" && path == "/guardianBoxes" {
        // Return all boxes where the user is a guardian (and not rejected)
        let guardian_boxes: Vec<_> = boxes_guard.iter()
            .filter_map(|b| to_guardian_box(b, &user_id))
            .collect();
        return Ok(response(StatusCode::OK, json!({ "boxes": guardian_boxes })));
    }

    if method == "GET" && path.starts_with("/boxes/") {
        let id = path.trim_start_matches("/boxes/").to_string();
        if let Some(box_rec) = boxes_guard.iter().find(|b| b.id == id) {
            if box_rec.owner_id == user_id {
                // Return full box info for owner
                return Ok(response(StatusCode::OK, json!({ "box": {
                    "id": box_rec.id,
                    "name": box_rec.name,
                    "description": box_rec.description,
                    "created_at": box_rec.created_at,
                    "updated_at": box_rec.updated_at
                }})));
            } else if box_rec.guardians.iter().any(|g| g.id == user_id && g.status != "rejected") {
                // For guardian requests, return guardian view if applicable
                if let Some(guardian_box) = to_guardian_box(box_rec, &user_id) {
                    return Ok(response(StatusCode::OK, json!({ "box": guardian_box })));
                }
            }
        }
        return Ok(response(StatusCode::UNAUTHORIZED, json!({"error": "Unauthorized or Box not found"})));
    }

    if method == "POST" && path == "/boxes" {
        // Create a new box for the owner
        if let Some(body_str) = event.body {
            let payload: serde_json::Value = serde_json::from_str(&body_str)?;
            let name = payload.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let description = payload.get("description").and_then(|d| d.as_str()).unwrap_or("");
            let new_box = BoxRecord {
                id: Uuid::new_v4().to_string(),
                name: name.into(),
                description: description.into(),
                is_locked: false,
                created_at: now_str(),
                updated_at: now_str(),
                owner_id: user_id.clone(),
                owner_name: None,
                documents: vec![],
                guardians: vec![],
                lead_guardians: vec![],
                unlock_instructions: None,
                unlock_request: None,
            };
            boxes_guard.push(new_box.clone());
            return Ok(response(StatusCode::CREATED, json!({ "box": {
                "id": new_box.id,
                "name": new_box.name,
                "description": new_box.description,
                "created_at": new_box.created_at,
                "updated_at": new_box.updated_at
            }})));
        } else {
            return Ok(response(StatusCode::BAD_REQUEST, json!({"error": "Bad Request: Missing body"})));
        }
    }

    else if method == "PATCH" && path.starts_with("/boxes/guardian/") {
        let box_id = path.trim_start_matches("/boxes/guardian/").to_string();
        let box_index = boxes_guard.iter().position(|b| b.id == box_id);
        if let Some(idx) = box_index {
            let box_record = &mut boxes_guard[idx];
            if box_record.guardians.iter().find(|g| g.id == user_id && g.status != "rejected").is_none() {
                return Ok(response(StatusCode::UNAUTHORIZED, json!({"error": "Unauthorized: Not a guardian for this box."})));
            }
            let is_lead = box_record.lead_guardians.iter().any(|g| g.id == user_id);
            let payload: serde_json::Value = if let Some(body) = &event.body {
                if let Body::Text(text) = body {
                    serde_json::from_str(text).unwrap_or(json!({}))
                } else {
                    json!({})
                }
            } else {
                json!({})
            };
            if is_lead {
                if let Some(message) = payload.get("message").and_then(|v| v.as_str()) {
                    let new_unlock = UnlockRequest {
                        id: Uuid::new_v4().to_string(),
                        requested_at: now_str(),
                        status: "pending".into(),
                        message: Some(message.to_string()),
                        initiated_by: Some(user_id.clone()),
                        approved_by: vec![],
                        rejected_by: vec![],
                    };
                    box_record.unlock_request = Some(new_unlock);
                } else {
                    return Ok(response(StatusCode::BAD_REQUEST, json!({"error": "Missing 'message' field for lead guardian update"})));
                }
            } else {
                if box_record.unlock_request.is_none() {
                    return Ok(response(StatusCode::BAD_REQUEST, json!({"error": "No unlock request exists to update"})));
                }
                if let Some(unlock) = &mut box_record.unlock_request {
                    let mut updated = false;
                    if let Some(approve) = payload.get("approve").and_then(|v| v.as_bool()) {
                        if approve && !unlock.approved_by.contains(&user_id) {
                            unlock.approved_by.push(user_id.clone());
                            updated = true;
                        }
                    }
                    if let Some(reject) = payload.get("reject").and_then(|v| v.as_bool()) {
                        if reject && !unlock.rejected_by.contains(&user_id) {
                            unlock.rejected_by.push(user_id.clone());
                            updated = true;
                        }
                    }
                    if !updated {
                        return Ok(response(StatusCode::BAD_REQUEST, json!({"error": "No valid update field provided"})));
                    }
                }
            }
            box_record.updated_at = now_str();
            if let Some(guard_box) = to_guardian_box(box_record, &user_id) {
                return Ok(response(StatusCode::OK, json!({"box": guard_box})));
            } else {
                return Ok(response(StatusCode::INTERNAL_SERVER_ERROR, json!({"error": "Failed to render guardian box"})));
            }
        } else {
            return Ok(response(StatusCode::NOT_FOUND, json!({"error": "Box not found"}))); 
        }
    }

    else if method == "PATCH" && path.starts_with("/boxes/") {
        let id = path.trim_start_matches("/boxes/").to_string();
        if let Some(box_rec) = boxes_guard.iter_mut().find(|b| b.id == id && b.owner_id == user_id) {
            if let Some(body_str) = event.body {
                let payload: serde_json::Value = if let Body::Text(text) = body_str {
                    serde_json::from_str(&text).unwrap_or(json!({}))
                } else {
                    json!({})
                };
                if let Some(name) = payload.get("name").and_then(|n| n.as_str()) {
                    box_rec.name = name.to_string();
                }
                if let Some(description) = payload.get("description").and_then(|d| d.as_str()) {
                    box_rec.description = description.to_string();
                }
                box_rec.updated_at = now_str();
                return Ok(response(StatusCode::OK, json!({
                    "box": {
                        "id": box_rec.id,
                        "name": box_rec.name,
                        "description": box_rec.description,
                        "created_at": box_rec.created_at,
                        "updated_at": box_rec.updated_at
                    }
                })));
            } else {
                return Ok(response(StatusCode::BAD_REQUEST, json!({"error": "Bad Request: Missing body"})));
            }
        } else {
            return Ok(response(StatusCode::UNAUTHORIZED, json!({"error": "Unauthorized or Box not found"})));
        }
    }

    if method == "DELETE" && path.starts_with("/boxes/") {
        let id = path.trim_start_matches("/boxes/").to_string();
        if let Some(pos) = boxes_guard.iter().position(|b| b.id == id && b.owner_id == user_id) {
            boxes_guard.remove(pos);
            return Ok(response(StatusCode::OK, json!({ "message": "Box deleted successfully." })));
        } else {
            return Ok(response(StatusCode::UNAUTHORIZED, json!({"error": "Unauthorized or Box not found"})));
        }
    }

    // Routing for Guardian Boxes endpoints
    if method == "GET" && path.starts_with("/guardianBoxes/") {
        let id = path.trim_start_matches("/guardianBoxes/").to_string();
        if let Some(box_rec) = boxes_guard.iter().find(|b| b.id == id) {
            if let Some(guardian_box) = to_guardian_box(box_rec, &user_id) {
                return Ok(response(StatusCode::OK, json!({ "box": guardian_box })));
            }
        }
        return Ok(response(StatusCode::UNAUTHORIZED, json!({"error": "Unauthorized or Box not found"})));
    }

    // If no route matches, return 404
    Ok(response(StatusCode::NOT_FOUND, json!({"error": "Not Found"})))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    simple_logger::init_with_level(log::Level::Info)?;
    let func = service_fn(function_handler);
    run(func).await?;
    Ok(())
}
