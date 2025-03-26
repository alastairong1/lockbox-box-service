use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Guardian {
    pub id: String,
    pub name: String,
    pub email: String,
    pub lead: bool,
    pub status: String, // "pending", "accepted", "rejected"
    pub added_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UnlockRequest {
    pub id: String,
    pub requested_at: String,
    pub status: String,
    pub message: Option<String>,
    pub initiated_by: Option<String>,
    pub approved_by: Vec<String>,
    pub rejected_by: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BoxRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_locked: bool,
    pub created_at: String,
    pub updated_at: String,
    pub owner_id: String,
    pub owner_name: Option<String>,
    pub documents: Vec<Document>,
    pub guardians: Vec<Guardian>,
    pub lead_guardians: Vec<Guardian>,
    pub unlock_instructions: Option<String>,
    pub unlock_request: Option<UnlockRequest>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GuardianBox {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_locked: bool,
    pub created_at: String,
    pub updated_at: String,
    pub owner_id: String,
    pub owner_name: Option<String>,
    pub unlock_instructions: Option<String>,
    pub unlock_request: Option<UnlockRequest>,
    pub pending_guardian_approval: Option<bool>,
    pub guardians_count: usize,
    pub is_lead_guardian: bool,
}

// Request DTOs
#[derive(Deserialize, Debug)]
pub struct CreateBoxRequest {
    pub name: String,
    pub description: String,
}

#[derive(Deserialize, Debug)]
pub struct UpdateBoxRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub unlock_instructions: Option<String>,
    pub is_locked: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct LeadGuardianUpdateRequest {
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct GuardianResponseRequest {
    pub approve: Option<bool>,
    pub reject: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct GuardianInvitationResponse {
    pub accept: bool,
}

// Response DTOs
#[derive(Serialize, Debug)]
pub struct BoxResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub unlock_instructions: Option<String>,
    pub is_locked: bool,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// Utility functions
pub fn now_str() -> String {
    Utc::now().to_rfc3339()
}
