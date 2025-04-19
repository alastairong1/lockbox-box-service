use std::sync::Arc;
use axum::{http::StatusCode, response::Response, Router};
use serde_json::{json, Value};
use tower::ServiceExt;

use lockbox_shared::store::InvitationStore;
use lockbox_shared::auth::create_test_request;
use lockbox_shared::test_utils::mock_invitation_store::MockInvitationStore;
use lockbox_shared::store::dynamo::DynamoInvitationStore;

use crate::routes::create_router_with_store;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
use lockbox_shared::models::Invitation;

// Required for DynamoDB integration tests
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, KeySchemaElement, KeyType, GlobalSecondaryIndex,
    Projection, ProjectionType, ProvisionedThroughput, ScalarAttributeType,
    AttributeValue, TableStatus, IndexStatus,
};

// Constants for DynamoDB tests
const DYNAMO_LOCAL_URI: &str = "http://localhost:8000";
const TEST_TABLE_NAME: &str = "invitation-test-table";
const GSI_BOX_ID: &str = "box_id-index";
const GSI_INVITE_CODE: &str = "invite_code-index";
const GSI_CREATOR_ID: &str = "creator_id-index";

// Helper to convert an Axum response into JSON for assertions
async fn response_to_json(response: Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// Helper to check if DynamoDB integration tests should be used
fn use_dynamodb() -> bool {
    std::env::var("USE_DYNAMODB").unwrap_or_default() == "true"
}

// Helper to set up a DynamoDB client for local testing
async fn create_dynamo_client() -> Client {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .endpoint_url(DYNAMO_LOCAL_URI)
        .load()
        .await;
    
    Client::new(&config)
}

// Helper to create the invitation table for testing
async fn create_invitation_table(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // Check if table already exists
    let tables = client.list_tables().send().await?;
    let table_names = tables.table_names();
    if table_names.contains(&TEST_TABLE_NAME.to_string()) {
        // Delete table if it exists
        client.delete_table().table_name(TEST_TABLE_NAME).send().await?;
        // Wait for table deletion to complete
        loop {
            let tables = client.list_tables().send().await?;
            if !tables.table_names().contains(&TEST_TABLE_NAME.to_string()) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    // Define primary key
    let id_key = KeySchemaElement::builder()
        .attribute_name("id")
        .key_type(KeyType::Hash)
        .build()?;

    // Define attributes
    let id_attr = AttributeDefinition::builder()
        .attribute_name("id")
        .attribute_type(ScalarAttributeType::S)
        .build()?;

    let box_id_attr = AttributeDefinition::builder()
        .attribute_name("box_id")
        .attribute_type(ScalarAttributeType::S)
        .build()?;

    let invite_code_attr = AttributeDefinition::builder()
        .attribute_name("invite_code")
        .attribute_type(ScalarAttributeType::S)
        .build()?;

    let creator_id_attr = AttributeDefinition::builder()
        .attribute_name("creator_id")
        .attribute_type(ScalarAttributeType::S)
        .build()?;

    // Create box_id GSI
    let box_id_index = GlobalSecondaryIndex::builder()
        .index_name(GSI_BOX_ID)
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("box_id")
                .key_type(KeyType::Hash)
                .build()?
        )
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

    // Create invite_code GSI
    let invite_code_index = GlobalSecondaryIndex::builder()
        .index_name(GSI_INVITE_CODE)
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("invite_code")
                .key_type(KeyType::Hash)
                .build()?
        )
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

    // Create creator_id GSI
    let creator_id_index = GlobalSecondaryIndex::builder()
        .index_name(GSI_CREATOR_ID)
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("creator_id")
                .key_type(KeyType::Hash)
                .build()?
        )
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

    // Create the table
    client
        .create_table()
        .table_name(TEST_TABLE_NAME)
        .key_schema(id_key)
        .attribute_definitions(id_attr)
        .attribute_definitions(box_id_attr)
        .attribute_definitions(invite_code_attr)
        .attribute_definitions(creator_id_attr)
        .global_secondary_indexes(box_id_index)
        .global_secondary_indexes(invite_code_index)
        .global_secondary_indexes(creator_id_index)
        .provisioned_throughput(
            ProvisionedThroughput::builder()
                .read_capacity_units(5)
                .write_capacity_units(5)
                .build()?
        )
        .send()
        .await?;

    // Wait for the table (and GSIs) to become ACTIVE before running tests
    loop {
        let resp = client.describe_table().table_name(TEST_TABLE_NAME).send().await?;
        if let Some(table_desc) = resp.table() {
            if table_desc.table_status() == Some(&TableStatus::Active) {
                // ensure all global secondary indexes are active
                let gsi_descs = table_desc.global_secondary_indexes();
                if gsi_descs.iter().all(|idx| idx.index_status() == Some(&IndexStatus::Active)) {
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}

// Helper to clean the DynamoDB table between tests
async fn clear_dynamo_table(client: &Client) {
    // Scan all items
    let scan_resp = client.scan().table_name(TEST_TABLE_NAME).send().await.unwrap();
    
    // Delete each item - items() returns a slice directly
    let items = scan_resp.items();
    for item in items {
        if let Some(id) = item.get("id") {
            if let Some(id_str) = id.as_s().ok() {
                let _ = client
                    .delete_item()
                    .table_name(TEST_TABLE_NAME)
                    .key("id", AttributeValue::S(id_str.to_string()))
                    .send()
                    .await;
            }
        }
    }
}

enum TestStore {
    Mock(Arc<MockInvitationStore>),
    DynamoDB(Arc<DynamoInvitationStore>),
}

// Helper to set up test application with the appropriate store based on environment
async fn create_test_app() -> (Router, TestStore) {
    if use_dynamodb() {
        // Set up DynamoDB store
        let client = create_dynamo_client().await;
        
        // Create the table (ignore errors if table already exists)
        let _ = create_invitation_table(&client).await;
        
        // Clean the table to start fresh
        clear_dynamo_table(&client).await;
        
        // Create the DynamoDB store with our test table
        let store = Arc::new(
            DynamoInvitationStore::with_client_and_table(client, TEST_TABLE_NAME.to_string())
        );
        
        let app = create_router_with_store(store.clone(), "");
        (app, TestStore::DynamoDB(store))
    } else {
        // Use mock store
        let store = Arc::new(MockInvitationStore::new_with_expiry());
        let app = create_router_with_store(store.clone(), "");
        (app, TestStore::Mock(store))
    }
}

#[tokio::test]
async fn test_create_invitation() {
    let (app, store) = create_test_app().await;

    let payload = json!({
        "invitedName": "Test User",
        "boxId": "box-123"
    });

    let response = app
        .clone()
        .oneshot(create_test_request(
            "POST",
            "/invitation",
            "test-user-id",
            Some(payload),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json_resp = response_to_json(response).await;
    let invite_code = json_resp["inviteCode"].as_str().unwrap();
    let expires_at = json_resp["expiresAt"].as_str().unwrap();
    assert_eq!(invite_code.len(), 8);
    assert!(!expires_at.is_empty());
    let expires_at_dt = DateTime::parse_from_rfc3339(expires_at).unwrap().with_timezone(&Utc);
    let now = Utc::now();
    let diff_secs = (expires_at_dt - now).num_seconds();
    assert!(diff_secs >= 47 * 3600 && diff_secs <= 49 * 3600, "Expiration time not within 47-49 hours, got {} seconds", diff_secs);

    // Add a small delay to allow for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify stored invitation
    let invitations = match &store {
        TestStore::Mock(mock) => mock.get_invitations_by_creator_id("test-user-id").await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_invitations_by_creator_id("test-user-id").await.unwrap(),
    };
    
    assert_eq!(invitations.len(), 1);
    let inv = &invitations[0];
    assert_eq!(inv.creator_id, "test-user-id");
    assert_eq!(inv.invited_name, "Test User");
    assert_eq!(inv.box_id, "box-123");
    assert!(!inv.opened);
    assert!(inv.linked_user_id.is_none());
}

#[tokio::test]
async fn test_handle_invitation() {
    let (app, store) = create_test_app().await;

    // seed an invitation directly
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let invite_code = "TESTCODE".to_string();
    let invitation = Invitation {
        id: id.clone(),
        invite_code: invite_code.clone(),
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
        created_at: now.to_rfc3339(),
        expires_at: (now + Duration::hours(2)).to_rfc3339(),
        opened: false,
        linked_user_id: None,
        creator_id: "creator-id".to_string(),
    };
    
    match &store {
        TestStore::Mock(mock) => mock.create_invitation(invitation.clone()).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.create_invitation(invitation.clone()).await.unwrap(),
    };

    let handle_payload = json!({
        "userId": "user-456",
        "inviteCode": invite_code
    });
    let response = app
        .clone()
        .oneshot(create_test_request(
            "PUT",
            "/invitation/handle",
            "user-456",
            Some(handle_payload),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json_resp = response_to_json(response).await;
    assert_eq!(json_resp["boxId"].as_str().unwrap(), "box-123");

    let updated_inv = match &store {
        TestStore::Mock(mock) => mock.get_invitation_by_code(&invite_code).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_invitation_by_code(&invite_code).await.unwrap(),
    };
    
    assert!(updated_inv.opened);
    assert_eq!(updated_inv.linked_user_id, Some("user-456".to_string()));
}

#[tokio::test]
async fn test_handle_invitation_expired_code() {
    let (app, store) = create_test_app().await;
    
    // seed an expired invitation
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let invite_code = "EXPIRED".to_string();
    let invitation = Invitation {
        id: id.clone(),
        invite_code: invite_code.clone(),
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
        created_at: now.to_rfc3339(),
        expires_at: (now - Duration::hours(1)).to_rfc3339(),
        opened: false,
        linked_user_id: None,
        creator_id: "creator-id".to_string(),
    };
    
    match &store {
        TestStore::Mock(mock) => mock.create_invitation(invitation.clone()).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.create_invitation(invitation.clone()).await.unwrap(),
    };

    let bad_payload = json!({
        "userId": "user-456",
        "inviteCode": "EXPIRED"
    });
    let response = app
        .clone()
        .oneshot(create_test_request(
            "PUT",
            "/invitation/handle",
            "user-456",
            Some(bad_payload),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn test_refresh_invitation() {
    let (app, store) = create_test_app().await;
    
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let old_code = "OLDCODE1".to_string();
    
    // Use the same dates for both mock and DynamoDB
    // Create time in the past, not yet expired (for both implementations)
    let create_time = now - Duration::hours(5); // Created 5 hours ago
    let expiry_time = now + Duration::hours(1); // Expires 1 hour from now
    
    let invitation = Invitation {
        id: id.clone(),
        invite_code: old_code.clone(),
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
        created_at: create_time.to_rfc3339(),
        expires_at: expiry_time.to_rfc3339(),
        opened: true,
        linked_user_id: Some("user-456".to_string()),
        creator_id: "test-user-id".to_string(),
    };
    
    match &store {
        TestStore::Mock(mock) => mock.create_invitation(invitation.clone()).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.create_invitation(invitation.clone()).await.unwrap(),
    };
    
    // Add a delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    let path = format!("/invitations/{}/refresh", id);
    let response = app
        .clone()
        .oneshot(create_test_request(
            "POST",
            &path,
            "test-user-id",
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json_resp = response_to_json(response).await;
    let new_code = json_resp["inviteCode"].as_str().unwrap();
    assert_ne!(new_code, old_code);

    let expires_at = json_resp["expiresAt"].as_str().unwrap();
    let expires_at_dt = DateTime::parse_from_rfc3339(expires_at).unwrap().with_timezone(&Utc);
    let now2 = Utc::now();
    let diff_secs = (expires_at_dt - now2).num_seconds();
    assert!(diff_secs >= 47 * 3600 && diff_secs <= 49 * 3600, "Expiration time not within 47-49 hours, got {} seconds", diff_secs);

    // Add a delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    let refreshed = match &store {
        TestStore::Mock(mock) => mock.get_invitation(&id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_invitation(&id).await.unwrap(),
    };
    
    assert_eq!(refreshed.invite_code, new_code.to_string());
    assert!(!refreshed.opened);
    assert!(refreshed.linked_user_id.is_none());
}

#[tokio::test]
async fn test_refresh_invitation_invalid_id() {
    let (app, store) = create_test_app().await;
    
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let invitation = Invitation {
        id: id.clone(),
        invite_code: "CODE1234".to_string(),
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
        created_at: now.to_rfc3339(),
        expires_at: (now + Duration::hours(2)).to_rfc3339(),
        opened: false,
        linked_user_id: None,
        creator_id: "owner-id".to_string(),
    };
    
    match &store {
        TestStore::Mock(mock) => mock.create_invitation(invitation.clone()).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.create_invitation(invitation.clone()).await.unwrap(),
    };

    let path = format!("/invitations/{}/refresh", id);
    let response = app
        .clone()
        .oneshot(create_test_request(
            "POST",
            &path,
            "other-user-id",
            None,
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_handle_invitation_invalid_code() {
    let (app, store) = create_test_app().await;
    
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let invitation = Invitation {
        id: id.clone(),
        invite_code: "VALID123".to_string(),
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
        created_at: now.to_rfc3339(),
        expires_at: (now + Duration::hours(2)).to_rfc3339(),
        opened: false,
        linked_user_id: None,
        creator_id: "creator-id".to_string(),
    };
    
    match &store {
        TestStore::Mock(mock) => mock.create_invitation(invitation.clone()).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.create_invitation(invitation.clone()).await.unwrap(),
    };

    let bad_payload = json!({
        "userId": "user-456",
        "inviteCode": "INVALID"
    });
    let response = app
        .clone()
        .oneshot(create_test_request(
            "PUT",
            "/invitation/handle",
            "user-456",
            Some(bad_payload),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_my_invitations() {
    let (app, store) = create_test_app().await;

    // Seed multiple invitations
    for (name, box_id, creator) in [
        ("User 1", "box-123", "test-user-id"),
        ("User 2", "box-456", "test-user-id"),
        ("User 3", "box-789", "other-user-id"),
    ] {
        let id = Uuid::new_v4().to_string();
        let invite_code = Uuid::new_v4().to_string().chars().take(8).collect::<String>().to_uppercase();
        let now = Utc::now();
        let invitation = Invitation {
            id,
            invite_code,
            invited_name: name.to_string(),
            box_id: box_id.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: (now + Duration::hours(48)).to_rfc3339(),
            opened: false,
            linked_user_id: None,
            creator_id: creator.to_string(),
        };
        
        match &store {
            TestStore::Mock(mock) => mock.create_invitation(invitation.clone()).await.unwrap(),
            TestStore::DynamoDB(dynamo) => dynamo.create_invitation(invitation.clone()).await.unwrap(),
        };
    }
    
    // Add a delay to allow for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    let response = app
        .clone()
        .oneshot(create_test_request("GET", "/invitations/me", "test-user-id", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json_resp = response_to_json(response).await;
    let arr = json_resp.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    for item in arr {
        assert_eq!(item["creatorId"].as_str().unwrap(), "test-user-id");
    }
}

#[tokio::test]
async fn test_get_my_invitations_empty() {
    let (app, _store) = create_test_app().await;

    let response = app
        .clone()
        .oneshot(create_test_request("GET", "/invitations/me", "test-user-id", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json_resp = response_to_json(response).await;
    assert!(json_resp.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_get_my_invitations_error() {
    // For this specific test, we'll use the mock store with errors
    // since it's testing error handling specifically
    let store = Arc::new(MockInvitationStore::new_error());
    let app = create_router_with_store(store.clone(), "");

    let response = app
        .oneshot(create_test_request("GET", "/invitations/me", "test-user-id", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
