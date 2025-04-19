use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;

// Import shared models for direct use in response types
use crate::shared_models::{Document, Guardian, UnlockRequest};

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
    #[serde(rename = "unlockInstructions")]
    pub unlock_instructions: NullableField<String>,
    #[serde(rename = "isLocked")]
    pub is_locked: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct DocumentUpdateRequest {
    pub document: Document,
}

#[derive(Deserialize, Debug)]
pub struct GuardianUpdateRequest {
    pub guardian: Guardian,
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
    pub unlock_instructions: Option<String>,
    #[serde(rename = "isLocked")]
    pub is_locked: bool,
    pub documents: Vec<Document>,
    pub guardians: Vec<Guardian>,
    #[serde(rename = "leadGuardians")]
    pub lead_guardians: Vec<Guardian>,
    #[serde(rename = "ownerId")]
    pub owner_id: String,
    #[serde(rename = "ownerName")]
    pub owner_name: Option<String>,
    #[serde(rename = "unlockRequest")]
    pub unlock_request: Option<UnlockRequest>,
}

#[derive(Serialize, Debug)]
pub struct DocumentUpdateResponse {
    pub documents: Vec<Document>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Serialize, Debug)]
pub struct GuardianUpdateResponse {
    pub guardians: Vec<Guardian>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

// Helper for null vs. not-present in JSON
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum NullableField<T> {
    Null,
    Value(T),
    #[serde(skip_deserializing)]
    NotPresent,
}

impl<T> Default for NullableField<T> {
    fn default() -> Self {
        NullableField::NotPresent
    }
}

impl<T> NullableField<T> {
    pub fn into_option(self) -> Option<T> {
        match self {
            NullableField::Value(v) => Some(v),
            _ => None,
        }
    }

    pub fn was_present(&self) -> bool {
        match self {
            NullableField::NotPresent => false,
            _ => true,
        }
    }
}

impl<T: fmt::Debug> fmt::Display for NullableField<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NullableField::Null => write!(f, "null"),
            NullableField::Value(v) => write!(f, "{:?}", v),
            NullableField::NotPresent => write!(f, "[not present]"),
        }
    }
}

// Additional request/response types
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

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub box_id: Option<String>,
}

// Utility functions
pub fn now_str() -> String {
    Utc::now().to_rfc3339()
}
