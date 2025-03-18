use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::error::Result;
use crate::models::{now_str, BoxRecord, Guardian, GuardianBox};

// Expose the DynamoDB store module
pub mod dynamo;
// Add the memory store implementation
pub mod memory;

/// BoxStore trait defining the interface for box storage implementations
pub trait BoxStore: Send + Sync + 'static {
    /// Creates a new box
    async fn create_box(&self, box_record: BoxRecord) -> Result<BoxRecord>;
    
    /// Gets a box by ID
    async fn get_box(&self, id: &str) -> Result<BoxRecord>;
    
    /// Gets all boxes owned by a user
    async fn get_boxes_by_owner(&self, owner_id: &str) -> Result<Vec<BoxRecord>>;
    
    /// Updates a box
    async fn update_box(&self, box_record: BoxRecord) -> Result<BoxRecord>;
    
    /// Deletes a box
    async fn delete_box(&self, id: &str) -> Result<()>;
}

// Global in-memory store (for sample purposes only)
pub static BOXES: Lazy<Mutex<Vec<BoxRecord>>> = Lazy::new(|| {
    let now = now_str();
    Mutex::new(vec![BoxRecord {
        id: Uuid::new_v4().to_string(),
        name: "Sample Box".into(),
        description: "A sample box".into(),
        is_locked: false,
        created_at: now.clone(),
        updated_at: now.clone(),
        owner_id: "user_1".into(),
        owner_name: Some("User One".into()),
        documents: vec![],
        guardians: vec![Guardian {
            id: "guardian_1".into(),
            name: "Guardian One".into(),
            email: "guardian1@example.com".into(),
            lead: false,
            status: "pending".into(),
            added_at: now.clone(),
        }],
        lead_guardians: vec![],
        unlock_instructions: None,
        unlock_request: None,
    }])
});

// We're keeping BoxStore type for backward compatibility with other routes
// This type will be deprecated in favor of the BoxStore trait
pub type LegacyBoxStore = Arc<Mutex<Vec<BoxRecord>>>;

// Store utility functions
pub fn convert_to_guardian_box(box_rec: &BoxRecord, user_id: &str) -> Option<GuardianBox> {
    if let Some(guardian) = box_rec
        .guardians
        .iter()
        .find(|g| g.id == user_id && g.status != "rejected")
    {
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
