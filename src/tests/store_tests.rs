#[cfg(test)]
mod dynamo_tests {
    use crate::models::BoxRecord;
    use crate::store::{dynamo::DynamoBoxStore, BoxStore};
    use aws_sdk_dynamodb::Client;
    use uuid::Uuid;

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

    // Helper function to create a DynamoDB client for local testing
    async fn create_local_dynamo_client() -> Client {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .endpoint_url("http://localhost:8000")
            .region("us-east-1")
            .load()
            .await;

        Client::new(&config)
    }

    // Helper function to create a test table for DynamoDB
    async fn create_test_table(client: &Client, table_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        use aws_sdk_dynamodb::types::{
            AttributeDefinition, GlobalSecondaryIndex, KeySchemaElement, KeyType, Projection, 
            ProjectionType, ScalarAttributeType,
        };
        
        // First check if table already exists (in case a previous test didn't clean up)
        let existing_tables = client.list_tables().send().await?;
        let table_names = existing_tables.table_names();
        if table_names.contains(&table_name.to_string()) {
            // Table exists, let's delete it first
            client.delete_table()
                .table_name(table_name)
                .send()
                .await?;
            
            // Wait for the table to be deleted
            let mut is_deleted = false;
            while !is_deleted {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let tables = client.list_tables().send().await?;
                let names = tables.table_names();
                is_deleted = !names.contains(&table_name.to_string());
            }
        }
        
        // Create the test table with GSIs
        // ID attribute (primary key)
        let id_attr = AttributeDefinition::builder()
            .attribute_name("id")
            .attribute_type(ScalarAttributeType::S)
            .build()?;
        
        // Owner ID attribute (for GSI)
        let owner_id_attr = AttributeDefinition::builder()
            .attribute_name("owner_id")
            .attribute_type(ScalarAttributeType::S)
            .build()?;
        
        // Primary key schema
        let id_key = KeySchemaElement::builder()
            .attribute_name("id")
            .key_type(KeyType::Hash)
            .build()?;
        
        // GSI key schema for owner_id
        let owner_id_key = KeySchemaElement::builder()
            .attribute_name("owner_id")
            .key_type(KeyType::Hash)
            .build()?;
        
        // Create owner_id GSI
        let owner_id_gsi = GlobalSecondaryIndex::builder()
            .index_name("owner_id-index")
            .key_schema(owner_id_key)
            .projection(
                Projection::builder()
                    .projection_type(ProjectionType::All)
                    .build()
            )
            .provisioned_throughput(
                aws_sdk_dynamodb::types::ProvisionedThroughput::builder()
                    .read_capacity_units(1)
                    .write_capacity_units(1)
                    .build()?
            )
            .build()?;
        
        // Now create the table with GSI
        client.create_table()
            .table_name(table_name)
            .attribute_definitions(id_attr.clone())
            .attribute_definitions(owner_id_attr)
            .key_schema(id_key)
            .global_secondary_indexes(owner_id_gsi)
            .provisioned_throughput(
                aws_sdk_dynamodb::types::ProvisionedThroughput::builder()
                    .read_capacity_units(1)
                    .write_capacity_units(1)
                    .build()?
            )
            .send()
            .await?;
        
        // Wait for the table to become active
        let mut is_active = false;
        while !is_active {
            let describe_response = client.describe_table()
                .table_name(table_name)
                .send()
                .await?;
            
            if let Some(table) = describe_response.table() {
                if table.table_status() == Some(&aws_sdk_dynamodb::types::TableStatus::Active) {
                    is_active = true;
                }
            }
            
            if !is_active {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        }
        
        Ok(())
    }

    // Helper function to delete a test table
    async fn delete_test_table(client: &Client, table_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        client.delete_table()
            .table_name(table_name)
            .send()
            .await?;
        
        // Wait for the table to be deleted
        let mut is_deleted = false;
        while !is_deleted {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            
            let tables = client.list_tables().send().await?;
            let names = tables.table_names();
            is_deleted = !names.contains(&table_name.to_string());
        }
        
        Ok(())
    }

    // Helper function to create a DynamoBoxStore for testing
    async fn create_test_store() -> (DynamoBoxStore, Client, String) {
        let client = create_local_dynamo_client().await;
        let table_name = format!("test-table-{}", Uuid::new_v4());
        
        // Create the test table
        create_test_table(&client, &table_name).await.expect("Failed to create test table");
        
        // Create the store with our client and table
        let store = DynamoBoxStore::with_client_and_table(client.clone(), table_name.clone());
        
        (store, client, table_name)
    }

    // Test for creating a box
    #[tokio::test]
    async fn dynamo_store_create_box() {
        // Check if DynamoDB local is running
        if !is_dynamodb_local_running() {
            println!("Skipping test: DynamoDB Local is not running");
            return;
        }
        
        // Create the test store
        let (store, client, table_name) = create_test_store().await;
        
        // Create a test box
        let test_box = create_test_box("DynamoDB Test Box", "test_owner");
        
        // Test creation
        let result = store.create_box(test_box.clone()).await;
        assert!(result.is_ok());
        
        let created_box = result.unwrap();
        assert_eq!(created_box.id, test_box.id);
        assert_eq!(created_box.name, "DynamoDB Test Box");
        
        // Clean up
        delete_test_table(&client, &table_name).await.expect("Failed to delete test table");
    }

    // Test for getting a box by ID
    #[tokio::test]
    async fn dynamo_store_get_box() {
        // Check if DynamoDB local is running
        if !is_dynamodb_local_running() {
            println!("Skipping test: DynamoDB Local is not running");
            return;
        }
        
        // Create the test store
        let (store, client, table_name) = create_test_store().await;
        
        // Create a test box
        let test_box = create_test_box("DynamoDB Test Box", "test_owner");
        store.create_box(test_box.clone()).await.unwrap();
        
        // Test retrieval
        let result = store.get_box(&test_box.id).await;
        assert!(result.is_ok());
        
        let fetched_box = result.unwrap();
        assert_eq!(fetched_box.id, test_box.id);
        assert_eq!(fetched_box.name, "DynamoDB Test Box");
        
        // Clean up
        delete_test_table(&client, &table_name).await.expect("Failed to delete test table");
    }

    // Test for getting boxes by owner
    #[tokio::test]
    async fn dynamo_store_get_boxes_by_owner() {
        // Check if DynamoDB local is running
        if !is_dynamodb_local_running() {
            println!("Skipping test: DynamoDB Local is not running");
            return;
        }
        
        // Create the test store
        let (store, client, table_name) = create_test_store().await;
        
        // Create test boxes with different owners
        let box1 = create_test_box("Box 1", "owner1");
        let box2 = create_test_box("Box 2", "owner1");
        let box3 = create_test_box("Box 3", "owner2");
        
        store.create_box(box1).await.unwrap();
        store.create_box(box2).await.unwrap();
        store.create_box(box3).await.unwrap();
        
        // Test retrieval by owner
        let result = store.get_boxes_by_owner("owner1").await;
        assert!(result.is_ok());
        
        let boxes = result.unwrap();
        println!("Found {} boxes for owner1", boxes.len());
        assert_eq!(boxes.len(), 2); // Should only return boxes where owner is owner1
        
        // Clean up
        delete_test_table(&client, &table_name).await.expect("Failed to delete test table");
    }

    // Test for updating a box
    #[tokio::test]
    async fn dynamo_store_update_box() {
        // Check if DynamoDB local is running
        if !is_dynamodb_local_running() {
            println!("Skipping test: DynamoDB Local is not running");
            return;
        }
        
        // Create the test store
        let (store, client, table_name) = create_test_store().await;
        
        // Create a test box
        let test_box = create_test_box("Test Box", "test_owner");
        store.create_box(test_box.clone()).await.unwrap();
        
        // Update the box
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
        
        // Clean up
        delete_test_table(&client, &table_name).await.expect("Failed to delete test table");
    }

    // Test for deleting a box
    #[tokio::test]
    async fn dynamo_store_delete_box() {
        // Check if DynamoDB local is running
        if !is_dynamodb_local_running() {
            println!("Skipping test: DynamoDB Local is not running");
            return;
        }
        
        // Create the test store
        let (store, client, table_name) = create_test_store().await;
        
        // Create a test box
        let test_box = create_test_box("Test Box", "test_owner");
        store.create_box(test_box.clone()).await.unwrap();
        
        // Delete the box
        let result = store.delete_box(&test_box.id).await;
        assert!(result.is_ok());
        
        // Verify it's actually deleted
        let fetch_result = store.get_box(&test_box.id).await;
        assert!(fetch_result.is_err()); // Should not find the deleted box
        
        // Clean up
        delete_test_table(&client, &table_name).await.expect("Failed to delete test table");
    }

    // Test for getting boxes by guardian ID
    // TODO: Refactor this test once we implement a proper guardian index solution
    // See GUARDIAN_INDEX_IMPLEMENTATION.md for the detailed implementation plan
    #[tokio::test]
    async fn dynamo_store_get_boxes_by_guardian_id() {
        // Check if DynamoDB local is running
        if !is_dynamodb_local_running() {
            println!("Skipping test: DynamoDB Local is not running");
            return;
        }
        
        // Create the test store
        let (store, client, table_name) = create_test_store().await;
        
        // Create test boxes with different guardians
        let mut box1 = create_test_box("Box with Guardian", "owner1");
        box1.guardians = vec![
            crate::models::Guardian {
                id: "guardian1".to_string(),
                name: "Guardian One".to_string(),
                email: "guardian1@example.com".to_string(),
                lead: false,
                status: "accepted".to_string(),
                added_at: crate::models::now_str(),
            }
        ];
        
        let mut box2 = create_test_box("Another Box with Guardian", "owner2");
        box2.guardians = vec![
            crate::models::Guardian {
                id: "guardian1".to_string(),
                name: "Guardian One".to_string(),
                email: "guardian1@example.com".to_string(),
                lead: false,
                status: "accepted".to_string(),
                added_at: crate::models::now_str(),
            }
        ];
        
        let mut box3 = create_test_box("Box with Rejected Guardian", "owner3");
        box3.guardians = vec![
            crate::models::Guardian {
                id: "guardian1".to_string(),
                name: "Guardian One".to_string(),
                email: "guardian1@example.com".to_string(),
                lead: false,
                status: "rejected".to_string(), // This should be filtered out
                added_at: crate::models::now_str(),
            }
        ];
        
        store.create_box(box1).await.unwrap();
        store.create_box(box2).await.unwrap();
        store.create_box(box3).await.unwrap();
        
        // Test retrieval by guardian
        let result = store.get_boxes_by_guardian_id("guardian1").await;
        assert!(result.is_ok());
        
        let boxes = result.unwrap();
        println!("Found {} boxes for guardian1", boxes.len());
        assert_eq!(boxes.len(), 2); // Should only return boxes where guardian1 is accepted
        
        // Clean up
        delete_test_table(&client, &table_name).await.expect("Failed to delete test table");
    }
    
    // Helper function to check if DynamoDB local is running
    fn is_dynamodb_local_running() -> bool {
        let response = std::process::Command::new("sh")
            .arg("-c")
            .arg("curl -s http://localhost:8000 > /dev/null && echo 'success' || echo 'failed'")
            .output()
            .expect("Failed to execute command");
            
        let output = String::from_utf8_lossy(&response.stdout);
        output.trim() == "success"
    }
}
