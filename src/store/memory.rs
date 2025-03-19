use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use uuid::Uuid;

use super::BoxStore;
use crate::error::{AppError, Result};
use crate::models::{now_str, BoxRecord};

/// In-memory implementation of BoxStore for testing
pub struct MemoryBoxStore {
    boxes: Arc<RwLock<HashMap<String, BoxRecord>>>,
}

impl MemoryBoxStore {
    /// Creates a new empty in-memory box store
    // pub fn new() -> Self {
    //     Self {
    //         boxes: Arc::new(RwLock::new(HashMap::new())),
    //     }
    // }

    /// Creates a new in-memory box store with initial data
    pub fn with_data(initial_data: Vec<BoxRecord>) -> Self {
        let mut boxes = HashMap::new();
        for box_record in initial_data {
            boxes.insert(box_record.id.clone(), box_record);
        }

        Self {
            boxes: Arc::new(RwLock::new(boxes)),
        }
    }
}

impl Default for MemoryBoxStore {
    fn default() -> Self {
        // Create a default instance with one sample box
        let now = now_str();
        let sample_box = BoxRecord {
            id: Uuid::new_v4().to_string(),
            name: "Test Box".into(),
            description: "An in-memory test box".into(),
            is_locked: false,
            created_at: now.clone(),
            updated_at: now.clone(),
            owner_id: "test_user".into(),
            owner_name: Some("Test User".into()),
            documents: vec![],
            guardians: vec![],
            lead_guardians: vec![],
            unlock_instructions: None,
            unlock_request: None,
        };

        Self::with_data(vec![sample_box])
    }
}

#[async_trait]
impl BoxStore for MemoryBoxStore {
    async fn create_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        let mut boxes = self
            .boxes
            .write()
            .map_err(|_| AppError::InternalServerError("Failed to acquire write lock".into()))?;

        if boxes.contains_key(&box_record.id) {
            return Err(AppError::BadRequest(format!(
                "Box with ID {} already exists",
                box_record.id
            )));
        }

        boxes.insert(box_record.id.clone(), box_record.clone());
        Ok(box_record)
    }

    async fn get_box(&self, id: &str) -> Result<BoxRecord> {
        let boxes = self
            .boxes
            .read()
            .map_err(|_| AppError::InternalServerError("Failed to acquire read lock".into()))?;

        boxes
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("Box not found: {}", id)))
    }

    async fn get_boxes_by_owner(&self, owner_id: &str) -> Result<Vec<BoxRecord>> {
        let boxes = self
            .boxes
            .read()
            .map_err(|_| AppError::InternalServerError("Failed to acquire read lock".into()))?;

        let owner_boxes: Vec<BoxRecord> = boxes
            .values()
            .filter(|box_record| box_record.owner_id == owner_id)
            .cloned()
            .collect();

        Ok(owner_boxes)
    }

    async fn update_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        let mut boxes = self
            .boxes
            .write()
            .map_err(|_| AppError::InternalServerError("Failed to acquire write lock".into()))?;

        if !boxes.contains_key(&box_record.id) {
            return Err(AppError::NotFound(format!(
                "Box not found: {}",
                box_record.id
            )));
        }

        let updated_box = BoxRecord {
            updated_at: now_str(),
            ..box_record.clone()
        };

        boxes.insert(updated_box.id.clone(), updated_box.clone());
        Ok(updated_box)
    }

    async fn delete_box(&self, id: &str) -> Result<()> {
        let mut boxes = self
            .boxes
            .write()
            .map_err(|_| AppError::InternalServerError("Failed to acquire write lock".into()))?;

        if boxes.remove(id).is_none() {
            return Err(AppError::NotFound(format!("Box not found: {}", id)));
        }

        Ok(())
    }
}
