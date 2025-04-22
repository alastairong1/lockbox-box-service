use chrono::Utc;
use serde::{Deserialize, Serialize};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "unlockInstructions", skip_serializing_if = "Option::is_none", default, with = "optional_field_serde")]
    pub unlock_instructions: Option<OptionalField<String>>,
    #[serde(rename = "isLocked", skip_serializing_if = "Option::is_none")]
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
// Custom wrapper type to differentiate between field not present and field present but null
#[derive(Debug)]
pub enum OptionalField<T> {
    Value(T),
    Null,
}

// Custom serde module for optional fields that need to distinguish between null and absent
mod optional_field_serde {
    use super::OptionalField;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<OptionalField<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let option = Option::<T>::deserialize(deserializer)?;
        match option {
            Some(val) => Ok(Some(OptionalField::Value(val))),
            None => Ok(Some(OptionalField::Null)), // null was explicitly provided
        }
    }

    pub fn serialize<S, T>(value: &Option<OptionalField<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: serde::Serialize,
    {
        match value {
            Some(OptionalField::Value(val)) => serializer.serialize_some(val),
            Some(OptionalField::Null) => serializer.serialize_none(),
            None => serializer.serialize_none(),
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
