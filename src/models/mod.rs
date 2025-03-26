use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;

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
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub owner_id: String,
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
}

// Request DTOs
#[derive(Deserialize, Debug)]
pub struct CreateBoxRequest {
    pub name: String,
    pub description: String,
}

// Add this custom struct for handling nullable fields
#[derive(Debug, Clone, Default)]
pub struct NullableField<T> {
    value: Option<T>,
    present_in_request: bool,
}

impl<T> NullableField<T> {
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }
    
    pub fn was_present(&self) -> bool {
        self.present_in_request
    }
    
    pub fn into_option(self) -> Option<T> {
        self.value
    }
}

// Add custom Deserialize implementation
impl<'de, T> Deserialize<'de> for NullableField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // This helps us track whether the field was present
        Ok(NullableField {
            value: Option::<T>::deserialize(deserializer)?,
            present_in_request: true,
        })
    }
}

impl<T: fmt::Debug> fmt::Display for NullableField<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NullableField({:?}, present={})", self.value, self.present_in_request)
    }
}

// Update the UpdateBoxRequest struct to use NullableField instead of Option<Option<>>
#[derive(Deserialize, Debug)]
pub struct UpdateBoxRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "unlockInstructions")]
    #[serde(default)]
    pub unlock_instructions: NullableField<String>,
    #[serde(rename = "isLocked")]
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
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "unlockInstructions")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlock_instructions: Option<String>,
    #[serde(rename = "isLocked")]
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
