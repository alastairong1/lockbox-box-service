use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Invitation {
    pub id: String,
    pub invite_code: String,         // Unique code for the deep link
    #[serde(rename = "invitedName")]
    pub invited_name: String,
    #[serde(rename = "boxId")]
    pub box_id: String,              // Associated BoxRecord
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,          // 48-hour expiry time
    pub opened: bool,                
    #[serde(rename = "linkedUserId")]
    pub linked_user_id: Option<String>, // To be filled upon open
}

// Request DTOs
#[derive(Deserialize, Debug)]
pub struct CreateInvitationRequest {
    #[serde(rename = "invitedName")]
    pub invited_name: String,
    #[serde(rename = "boxId")]
    pub box_id: String,
}

#[derive(Deserialize, Debug)]
pub struct ConnectToUserRequest {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "inviteCode")]
    pub invite_code: String,
}

// Response DTOs

// Minimal response with just the code and expiry
#[derive(Serialize, Debug)]
pub struct InvitationCodeResponse {
    #[serde(rename = "inviteCode")]
    pub invite_code: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
}

// Helper function to get current timestamp as string
pub fn now_str() -> String {
    Utc::now().to_rfc3339()
} 