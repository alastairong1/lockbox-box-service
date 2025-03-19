use async_trait::async_trait;

use crate::error::Result;
use crate::models::{BoxRecord, GuardianBox};

// Expose the DynamoDB store module
pub mod dynamo;
// Add the memory store implementation
pub mod memory;

/// BoxStore trait defining the interface for box storage implementations
#[async_trait]
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
