use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
    #[serde(rename = "boxId", skip_serializing_if = "Option::is_none")]
    pub box_id: Option<String>,
}
