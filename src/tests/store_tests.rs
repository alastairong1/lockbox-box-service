#[cfg(test)]
mod tests {
    use crate::models::BoxRecord;
    use crate::store::{BoxStore, memory::MemoryBoxStore};
    use uuid::Uuid;
    
    // Helper function to create a test box
    fn create_test_box(name: &str, owner_id: &str) -> BoxRecord {
        let now = crate::models::now_str();
        BoxRecord {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: "Test box description".to_string(),
            is_locked: false,
            created_at: now.clone(),
            updated_at: now,
            owner_id: owner_id.to_string(),
            owner_name: Some("Test Owner".to_string()),
            documents: vec![],
            guardians: vec![],
            lead_guardians: vec![],
            unlock_instructions: None,
            unlock_request: None,
        }
    }
    
    #[tokio::test]
    async fn memory_store_create_box() {
        let store = MemoryBoxStore::new();
        let test_box = create_test_box("Test Box", "test_owner");
        
        let result = store.create_box(test_box.clone()).await;
        assert!(result.is_ok());
        
        let created_box = result.unwrap();
        assert_eq!(created_box.id, test_box.id);
        assert_eq!(created_box.name, "Test Box");
    }
    
    #[tokio::test]
    async fn memory_store_get_box() {
        let store = MemoryBoxStore::new();
        let test_box = create_test_box("Test Box", "test_owner");
        
        store.create_box(test_box.clone()).await.unwrap();
        
        let result = store.get_box(&test_box.id).await;
        assert!(result.is_ok());
        
        let fetched_box = result.unwrap();
        assert_eq!(fetched_box.id, test_box.id);
        assert_eq!(fetched_box.name, "Test Box");
    }
    
    #[tokio::test]
    async fn memory_store_get_boxes_by_owner() {
        let store = MemoryBoxStore::new();
        
        let box1 = create_test_box("Box 1", "owner1");
        let box2 = create_test_box("Box 2", "owner1");
        let box3 = create_test_box("Box 3", "owner2");
        
        store.create_box(box1).await.unwrap();
        store.create_box(box2).await.unwrap();
        store.create_box(box3).await.unwrap();
        
        let result = store.get_boxes_by_owner("owner1").await;
        assert!(result.is_ok());
        
        let boxes = result.unwrap();
        assert_eq!(boxes.len(), 2);
        assert!(boxes.iter().any(|b| b.name == "Box 1"));
        assert!(boxes.iter().any(|b| b.name == "Box 2"));
    }
    
    #[tokio::test]
    async fn memory_store_update_box() {
        let store = MemoryBoxStore::new();
        let test_box = create_test_box("Test Box", "test_owner");
        
        store.create_box(test_box.clone()).await.unwrap();
        
        let mut updated_box = test_box.clone();
        updated_box.name = "Updated Box".to_string();
        
        let result = store.update_box(updated_box).await;
        assert!(result.is_ok());
        
        let box_after_update = result.unwrap();
        assert_eq!(box_after_update.id, test_box.id);
        assert_eq!(box_after_update.name, "Updated Box");
        
        // Verify it's actually updated in the store
        let fetched = store.get_box(&test_box.id).await.unwrap();
        assert_eq!(fetched.name, "Updated Box");
    }
    
    #[tokio::test]
    async fn memory_store_delete_box() {
        let store = MemoryBoxStore::new();
        let test_box = create_test_box("Test Box", "test_owner");
        
        store.create_box(test_box.clone()).await.unwrap();
        
        let result = store.delete_box(&test_box.id).await;
        assert!(result.is_ok());
        
        let fetch_result = store.get_box(&test_box.id).await;
        assert!(fetch_result.is_err()); // Should not find the deleted box
    }
    
    // For DynamoDB tests, you would setup a LocalStack container or mock
    // and implement similar tests with the DynamoBoxStore
    
    // Example (commented out as it requires LocalStack setup):
    /*
    #[tokio::test]
    async fn dynamo_store_operations() {
        // Setup LocalStack DynamoDB
        let endpoint_url = "http://localhost:4566";  // LocalStack default port
        
        let config = aws_sdk_dynamodb::config::Builder::new()
            .endpoint_url(endpoint_url)
            .build();
        
        let client = aws_sdk_dynamodb::Client::from_conf(config);
        
        let table_name = format!("test-table-{}", Uuid::new_v4());
        
        // Create test table
        client.create_table()
            .table_name(&table_name)
            .key_schema(
                aws_sdk_dynamodb::types::KeySchemaElement::builder()
                    .attribute_name("id")
                    .key_type(aws_sdk_dynamodb::types::KeyType::Hash)
                    .build()
            )
            .attribute_definitions(
                aws_sdk_dynamodb::types::AttributeDefinition::builder()
                    .attribute_name("id")
                    .attribute_type(aws_sdk_dynamodb::types::ScalarAttributeType::S)
                    .build()
            )
            .provisioned_throughput(
                aws_sdk_dynamodb::types::ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()
            )
            .send()
            .await
            .unwrap();
        
        // Create DynamoBoxStore instance
        let store = DynamoBoxStore {
            client,
            table_name,
        };
        
        // Now run the same tests as for MemoryBoxStore
        let test_box = create_test_box("DynamoDB Test Box", "test_owner");
        
        let result = store.create_box(test_box.clone()).await;
        assert!(result.is_ok());
        
        // [Rest of the tests...]
        
        // Clean up - delete the test table
        client.delete_table()
            .table_name(&table_name)
            .send()
            .await
            .unwrap();
    }
    */
}