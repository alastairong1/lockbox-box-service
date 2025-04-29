use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub mod events;

// Invitation statuses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvitationStatus {
    Invited,
    Opened,
    Accepted,
    Rejected,
}

impl FromStr for InvitationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "invited" => Ok(InvitationStatus::Invited),
            "opened" => Ok(InvitationStatus::Opened),
            "accepted" => Ok(InvitationStatus::Accepted),
            "rejected" => Ok(InvitationStatus::Rejected),
            _ => Err(format!("Unknown invitation status: {}", s)),
        }
    }
}

impl fmt::Display for InvitationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str = match self {
            InvitationStatus::Invited => "invited",
            InvitationStatus::Opened => "opened",
            InvitationStatus::Accepted => "accepted",
            InvitationStatus::Rejected => "rejected",
        };
        write!(f, "{}", status_str)
    }
}

// Invitation-related models
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

// Box-related models
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Guardian {
    pub id: String, // user_id
    pub name: String,
    #[serde(rename = "leadGuardian")]
    pub lead_guardian: bool,
    pub status: String, // "invited", "viewed", "accepted", "rejected"
    #[serde(rename = "addedAt")]
    pub added_at: String,
    #[serde(rename = "invitationId")]
    pub invitation_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UnlockRequest {
    pub id: String,
    #[serde(rename = "requestedAt")]
    pub requested_at: String,
    pub status: String,
    pub message: Option<String>,
    #[serde(rename = "initiatedBy")]
    pub initiated_by: Option<String>,
    #[serde(rename = "approvedBy")]
    pub approved_by: Vec<String>,
    #[serde(rename = "rejectedBy")]
    pub rejected_by: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BoxRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "ownerId")]
    pub owner_id: String,
    #[serde(rename = "ownerName")]
    pub owner_name: Option<String>,
    pub documents: Vec<Document>,
    pub guardians: Vec<Guardian>,
    #[serde(rename = "unlockInstructions")]
    pub unlock_instructions: Option<String>,
    #[serde(rename = "unlockRequest")]
    pub unlock_request: Option<UnlockRequest>,
    #[serde(default)]
    pub version: u64, // Version for optimistic concurrency control
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GuardianBox {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "ownerId")]
    pub owner_id: String,
    #[serde(rename = "ownerName")]
    pub owner_name: Option<String>,
    #[serde(rename = "unlockInstructions")]
    pub unlock_instructions: Option<String>,
    #[serde(rename = "unlockRequest")]
    pub unlock_request: Option<UnlockRequest>,
    #[serde(rename = "pendingGuardianApproval")]
    pub pending_guardian_approval: Option<bool>,
    #[serde(rename = "guardiansCount")]
    pub guardians_count: usize,
    #[serde(rename = "isLeadGuardian")]
    pub is_lead_guardian: bool,
    // TODO we probably shouldn't be just returning them all for privacy reasons
    pub documents: Vec<Document>,
    pub guardians: Vec<Guardian>,
}

// Response DTOs for general use across services
#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Debug)]
pub struct MessageResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_id: Option<String>,
}

// Helper function to get current timestamp as string
pub fn now_str() -> String {
    Utc::now().to_rfc3339()
}
