use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, KeySchemaElement, KeyType, GlobalSecondaryIndex,
    Projection, ProjectionType, ProvisionedThroughput, ScalarAttributeType,
    AttributeValue, TableStatus, IndexStatus,
};
use std::error::Error;

// Constants for DynamoDB tests
pub const DYNAMO_LOCAL_URI: &str = "http://localhost:8000";

// Helper to check if DynamoDB integration tests should be used
pub fn use_dynamodb() -> bool {
    std::env::var("USE_DYNAMODB").unwrap_or_default() == "true"
}

// Helper to set up a DynamoDB client for local testing
pub async fn create_dynamo_client() -> Client {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .endpoint_url(DYNAMO_LOCAL_URI)
        .load()
        .await;
    
    Client::new(&config)
}

// Helper to create a table with the given name and GSIs
pub async fn create_dynamo_table(
    client: &Client, 
    table_name: &str,
    gsi_configs: Vec<(&str, &str, KeyType)>,
) -> Result<(), Box<dyn Error>> {
    // Check if table already exists
    let tables = client.list_tables().send().await?;
    let table_names = tables.table_names();
    if table_names.contains(&table_name.to_string()) {
        // Delete table if it exists
        client.delete_table().table_name(table_name).send().await?;
        // Wait for table deletion to complete
        loop {
            let tables = client.list_tables().send().await?;
            if !tables.table_names().contains(&table_name.to_string()) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    // Define primary key (always using 'id' as the hash key)
    let id_key = KeySchemaElement::builder()
        .attribute_name("id")
        .key_type(KeyType::Hash)
        .build()?;

    // Define id attribute
    let id_attr = AttributeDefinition::builder()
        .attribute_name("id")
        .attribute_type(ScalarAttributeType::S)
        .build()?;

    // Create the table create request with primary key
    let mut create_table_req = client
        .create_table()
        .table_name(table_name)
        .key_schema(id_key)
        .attribute_definitions(id_attr);

    // Add all GSI attributes and indices
    let mut attribute_definitions = Vec::new();
    let mut global_secondary_indices = Vec::new();

    for (gsi_name, attr_name, key_type) in gsi_configs {
        // Add attribute definition
        let attr_def = AttributeDefinition::builder()
            .attribute_name(attr_name)
            .attribute_type(ScalarAttributeType::S)
            .build()?;
        attribute_definitions.push(attr_def);

        // Create GSI
        let key_schema = KeySchemaElement::builder()
            .attribute_name(attr_name)
            .key_type(key_type)
            .build()?;

        let gsi = GlobalSecondaryIndex::builder()
            .index_name(gsi_name)
            .key_schema(key_schema)
            .projection(
                Projection::builder()
                    .projection_type(ProjectionType::All)
                    .build()
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()?
            )
            .build()?;
        
        global_secondary_indices.push(gsi);
    }

    // Add all attribute definitions to the request
    for attr_def in attribute_definitions {
        create_table_req = create_table_req.attribute_definitions(attr_def);
    }

    // Add all GSIs to the request
    for gsi in global_secondary_indices {
        create_table_req = create_table_req.global_secondary_indexes(gsi);
    }

    // Add provisioned throughput
    create_table_req = create_table_req.provisioned_throughput(
        ProvisionedThroughput::builder()
            .read_capacity_units(5)
            .write_capacity_units(5)
            .build()?
    );

    // Create the table
    create_table_req.send().await?;

    // Wait for the table (and GSIs) to become ACTIVE before running tests
    loop {
        let resp = client.describe_table().table_name(table_name).send().await?;
        if let Some(table_desc) = resp.table() {
            if table_desc.table_status() == Some(&TableStatus::Active) {
                // ensure all global secondary indexes are active
                let gsi_descs = table_desc.global_secondary_indexes();
                if gsi_descs.is_empty() || gsi_descs.iter().all(|idx| idx.index_status() == Some(&IndexStatus::Active)) {
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}

// Helper to clean the DynamoDB table between tests
pub async fn clear_dynamo_table(client: &Client, table_name: &str) {
    // Scan all items
    let scan_resp = client.scan().table_name(table_name).send().await.unwrap();
    
    // Delete each item
    let items = scan_resp.items();
    for item in items {
        if let Some(id) = item.get("id") {
            if let Some(id_str) = id.as_s().ok() {
                let _ = client
                    .delete_item()
                    .table_name(table_name)
                    .key("id", AttributeValue::S(id_str.to_string()))
                    .send()
                    .await;
            }
        }
    }
}

// Helper to create the invitation table for testing
pub async fn create_invitation_table(client: &Client, table_name: &str) -> Result<(), Box<dyn Error>> {
    let gsi_configs = vec![
        ("box_id-index", "box_id", KeyType::Hash),
        ("invite_code-index", "invite_code", KeyType::Hash),
        ("creator_id-index", "creator_id", KeyType::Hash),
    ];
    
    create_dynamo_table(client, table_name, gsi_configs).await
}

// Helper to create the box table for testing
pub async fn create_box_table(client: &Client, table_name: &str) -> Result<(), Box<dyn Error>> {
    println!("Creating box table '{}' for testing...", table_name);
    
    // Check if table already exists
    let tables = client.list_tables().send().await?;
    let table_names = tables.table_names();
    
    if table_names.contains(&table_name.to_string()) {
        println!("Table '{}' already exists, deleting it first...", table_name);
        // Delete table if it exists
        match client.delete_table().table_name(table_name).send().await {
            Ok(_) => println!("Successfully deleted existing table '{}'", table_name),
            Err(e) => println!("Error deleting table '{}': {}", table_name, e),
        }
        
        // Wait for table deletion to complete
        println!("Waiting for table '{}' to be deleted...", table_name);
        loop {
            let tables = client.list_tables().send().await?;
            if !tables.table_names().contains(&table_name.to_string()) {
                println!("Table '{}' successfully deleted!", table_name);
                break;
            }
            println!("Table '{}' still exists, waiting...", table_name);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    println!("Creating new table '{}'...", table_name);
    
    // Define GSI configurations
    let gsi_configs = vec![
        ("owner_id-index", "ownerId", KeyType::Hash),
    ];
    
    // Define primary key (always using 'id' as the hash key)
    let id_key = KeySchemaElement::builder()
        .attribute_name("id")
        .key_type(KeyType::Hash)
        .build()?;

    // Define id attribute
    let id_attr = AttributeDefinition::builder()
        .attribute_name("id")
        .attribute_type(ScalarAttributeType::S)
        .build()?;

    // Create the table create request with primary key
    let mut create_table_req = client
        .create_table()
        .table_name(table_name)
        .key_schema(id_key)
        .attribute_definitions(id_attr);

    // Add all GSI attributes and indices
    let mut attribute_definitions = Vec::new();
    let mut global_secondary_indices = Vec::new();

    for (gsi_name, attr_name, key_type) in gsi_configs {
        // Add attribute definition
        let attr_def = AttributeDefinition::builder()
            .attribute_name(attr_name)
            .attribute_type(ScalarAttributeType::S)
            .build()?;
        attribute_definitions.push(attr_def);

        // Create GSI
        let key_schema = KeySchemaElement::builder()
            .attribute_name(attr_name)
            .key_type(key_type)
            .build()?;

        let gsi = GlobalSecondaryIndex::builder()
            .index_name(gsi_name)
            .key_schema(key_schema)
            .projection(
                Projection::builder()
                    .projection_type(ProjectionType::All)
                    .build()
            )
            .provisioned_throughput(
                ProvisionedThroughput::builder()
                    .read_capacity_units(5)
                    .write_capacity_units(5)
                    .build()?
            )
            .build()?;
        
        global_secondary_indices.push(gsi);
    }

    // Add all attribute definitions to the request
    for attr_def in attribute_definitions {
        create_table_req = create_table_req.attribute_definitions(attr_def);
    }

    // Add all GSIs to the request
    for gsi in global_secondary_indices {
        create_table_req = create_table_req.global_secondary_indexes(gsi);
    }

    // Add provisioned throughput
    create_table_req = create_table_req.provisioned_throughput(
        ProvisionedThroughput::builder()
            .read_capacity_units(5)
            .write_capacity_units(5)
            .build()?
    );

    // Create the table
    println!("Sending create table request...");
    let create_result = create_table_req.send().await;
    match &create_result {
        Ok(_) => println!("Table '{}' creation request successful", table_name),
        Err(e) => println!("Error creating table '{}': {}", table_name, e),
    }
    create_result?;

    // Wait for the table (and GSIs) to become ACTIVE before running tests
    println!("Waiting for table '{}' to become ACTIVE...", table_name);
    loop {
        match client.describe_table().table_name(table_name).send().await {
            Ok(resp) => {
                if let Some(table_desc) = resp.table() {
                    if table_desc.table_status() == Some(&TableStatus::Active) {
                        // ensure all global secondary indexes are active
                        let gsi_descs = table_desc.global_secondary_indexes();
                        if gsi_descs.is_empty() || gsi_descs.iter().all(|idx| idx.index_status() == Some(&IndexStatus::Active)) {
                            println!("Table '{}' and all GSIs are now ACTIVE!", table_name);
                            break;
                        } else {
                            println!("Table '{}' is ACTIVE but waiting for GSIs...", table_name);
                        }
                    } else {
                        println!("Table '{}' status: {:?}", table_name, table_desc.table_status());
                    }
                }
            },
            Err(e) => println!("Error checking table status: {}", e),
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    println!("Table '{}' is ready for testing!", table_name);
    Ok(())
} 