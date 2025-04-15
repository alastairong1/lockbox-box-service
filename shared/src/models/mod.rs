use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Invitation {
    pub id: String,
    pub invite_code: String, // Unique code for the deep link
    #[serde(rename = "invitedName")]
    pub invited_name: String,
    #[serde(rename = "boxId")]
    pub box_id: String, // Associated BoxRecord
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String, // 48-hour expiry time
    pub opened: bool,
    #[serde(rename = "linkedUserId")]
    pub linked_user_id: Option<String>, // To be filled upon open
    #[serde(rename = "creatorId")]
    pub creator_id: String, // ID of the user who created the invitation
}

// Response DTOs for general use across services
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
