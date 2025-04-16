use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use crate::error::{Result, StoreError};
use crate::models::BoxRecord;
use crate::store::BoxStore;

/// MockBoxStore is a simple in-memory implementation of BoxStore for testing
pub struct MockBoxStore {
    boxes: Mutex<HashMap<String, BoxRecord>>,
    owner_indexes: Mutex<HashMap<String, Vec<String>>>, // owner_id -> [box_id]
}

impl MockBoxStore {
    /// Create a new empty MockBoxStore
    pub fn new() -> Self {
        Self {
            boxes: Mutex::new(HashMap::new()),
            owner_indexes: Mutex::new(HashMap::new()),
        }
    }

    /// Create a MockBoxStore with initial test data
    pub fn with_data(box_records: Vec<BoxRecord>) -> Self {
        let store = Self::new();
        
        // Initialize with data
        for box_record in box_records {
            let owner_id = box_record.owner_id.clone();
            let box_id = box_record.id.clone();
            
            // Add to main storage
            store.boxes.lock().unwrap().insert(box_id.clone(), box_record);
            
            // Add to owner index
            store.owner_indexes
                .lock()
                .unwrap()
                .entry(owner_id)
                .or_insert_with(Vec::new)
                .push(box_id);
        }
        
        store
    }
}

#[async_trait]
impl BoxStore for MockBoxStore {
    async fn create_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        let box_id = box_record.id.clone();
        let owner_id = box_record.owner_id.clone();
        
        // Store the box
        self.boxes.lock().unwrap().insert(box_id.clone(), box_record.clone());
        
        // Update owner index
        self.owner_indexes
            .lock()
            .unwrap()
            .entry(owner_id)
            .or_insert_with(Vec::new)
            .push(box_id);
        
        Ok(box_record)
    }

    async fn get_box(&self, id: &str) -> Result<BoxRecord> {
        self.boxes
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("Box with id {} not found", id)))
    }

    async fn get_boxes_by_owner(&self, owner_id: &str) -> Result<Vec<BoxRecord>> {
        let owner_boxes = self
            .owner_indexes
            .lock()
            .unwrap()
            .get(owner_id)
            .cloned()
            .unwrap_or_default();
        
        let boxes = self.boxes.lock().unwrap();
        let result: Vec<BoxRecord> = owner_boxes
            .iter()
            .filter_map(|id| boxes.get(id).cloned())
            .collect();
        
        Ok(result)
    }

    async fn get_boxes_by_guardian_id(&self, guardian_id: &str) -> Result<Vec<BoxRecord>> {
        let boxes = self.boxes.lock().unwrap();
        
        let guardian_boxes: Vec<BoxRecord> = boxes
            .values()
            .filter(|b| {
                b.guardians
                    .iter()
                    .any(|guardian| guardian.id == guardian_id && guardian.status != "rejected")
            })
            .cloned()
            .collect();
        
        Ok(guardian_boxes)
    }

    async fn update_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        let box_id = box_record.id.clone();
        
        // Check if box exists
        if !self.boxes.lock().unwrap().contains_key(&box_id) {
            return Err(StoreError::NotFound(format!(
                "Box with id {} not found",
                box_id
            )));
        }
        
        // Update the box
        self.boxes
            .lock()
            .unwrap()
            .insert(box_id, box_record.clone());
        
        Ok(box_record)
    }

    async fn delete_box(&self, id: &str) -> Result<()> {
        // Check if box exists and get owner_id
        let owner_id = {
            let boxes = self.boxes.lock().unwrap();
            let box_record = boxes.get(id).ok_or_else(|| {
                StoreError::NotFound(format!("Box with id {} not found", id))
            })?;
            box_record.owner_id.clone()
        };
        
        // Remove from boxes
        self.boxes.lock().unwrap().remove(id);
        
        // Update owner index
        if let Some(box_ids) = self.owner_indexes.lock().unwrap().get_mut(&owner_id) {
            box_ids.retain(|box_id| box_id != id);
        }
        
        Ok(())
    }
} 