use std::sync::{Arc, Mutex};
use uuid::Uuid;
use once_cell::sync::Lazy;

use crate::models::{BoxRecord, Guardian, GuardianBox, now_str};

// Global in-memory store (for sample purposes only)
pub static BOXES: Lazy<Mutex<Vec<BoxRecord>>> = Lazy::new(|| {
    let now = now_str();
    Mutex::new(vec![
        BoxRecord {
            id: Uuid::new_v4().to_string(),
            name: "Sample Box".into(),
            description: "A sample box".into(),
            is_locked: false,
            created_at: now.clone(),
            updated_at: now.clone(),
            owner_id: "user_1".into(),
            owner_name: Some("User One".into()),
            documents: vec![],
            guardians: vec![
                Guardian {
                    id: "guardian_1".into(),
                    name: "Guardian One".into(),
                    email: "guardian1@example.com".into(),
                    lead: false,
                    status: "pending".into(),
                    added_at: now.clone(),
                }
            ],
            lead_guardians: vec![],
            unlock_instructions: None,
            unlock_request: None,
        }
    ])
});

pub type BoxStore = Arc<Mutex<Vec<BoxRecord>>>;

// Store utility functions
pub fn convert_to_guardian_box(box_rec: &BoxRecord, user_id: &str) -> Option<GuardianBox> {
    if let Some(guardian) = box_rec.guardians.iter().find(|g| g.id == user_id && g.status != "rejected") {
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
