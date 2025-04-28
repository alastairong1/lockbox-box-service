use axum::http::StatusCode;
use axum::Router;
use lockbox_shared::auth::create_test_request;
use lockbox_shared::store::dynamo::DynamoBoxStore;
use lockbox_shared::store::BoxStore;
use lockbox_shared::test_utils::dynamo_test_utils::{
    clear_dynamo_table, create_box_table, create_dynamo_client, use_dynamodb,
};
use lockbox_shared::test_utils::http_test_utils::response_to_json;
use lockbox_shared::test_utils::mock_box_store::MockBoxStore;
use lockbox_shared::test_utils::test_logging::init_test_logging;
use log::{debug, info, trace};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use crate::{models::now_str, routes};
use lockbox_shared::models::{BoxRecord, Guardian, UnlockRequest};

// Constants for DynamoDB tests
const TEST_TABLE_NAME: &str = "guardian-test-table";

// Create mock data for testing
fn create_test_data(now: &str) -> Vec<BoxRecord> {
    // Box 1: Regular guardian (guardian_1)
    let box_1_id = "11111111-1111-1111-1111-111111111111".to_string();
    let box_1 = BoxRecord {
        id: box_1_id,
        name: "Guardian Test Box 1".into(),
        description: "Box for guardian tests".into(),
        is_locked: true,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        owner_id: "owner_1".into(),
        owner_name: Some("Owner One".into()),
        documents: vec![],
        guardians: vec![
            Guardian {
                id: "guardian_1".into(),
                name: "Guardian One".into(),
                lead_guardian: false,
                status: "accepted".into(),
                added_at: now.to_string(),
                invitation_id: "invitation_1".into(),
            },
            Guardian {
                id: "guardian_2".into(),
                name: "Guardian Two".into(),
                lead_guardian: false,
                status: "accepted".into(),
                added_at: now.to_string(),
                invitation_id: "invitation_2".into(),
            },
            Guardian {
                id: "lead_guardian_1".into(),
                name: "Lead Guardian One".into(),
                lead_guardian: true,
                status: "accepted".into(),
                added_at: now.to_string(),
                invitation_id: "invitation_3".into(),
            },
        ],
        unlock_instructions: Some("Contact all guardians".into()),
        unlock_request: None,
        version: 0,
    };

    // Box 2: With pending unlock request
    let box_2_id = "22222222-2222-2222-2222-222222222222".to_string();
    let unlock_request = UnlockRequest {
        id: "unlock-111".into(),
        requested_at: now.to_string(),
        status: "invited".into(),
        message: Some("Emergency access needed".into()),
        initiated_by: Some("lead_guardian_1".into()),
        approved_by: vec![],
        rejected_by: vec![],
    };

    let box_2 = BoxRecord {
        id: box_2_id,
        name: "Guardian Test Box 2".into(),
        description: "Box with unlock request".into(),
        is_locked: true,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        owner_id: "owner_1".into(),
        owner_name: Some("Owner One".into()),
        documents: vec![],
        guardians: vec![
            Guardian {
                id: "guardian_1".into(),
                name: "Guardian One".into(),
                lead_guardian: false,
                status: "accepted".into(),
                added_at: now.to_string(),
                invitation_id: "invitation_5".into(),
            },
            Guardian {
                id: "guardian_3".into(),
                name: "Guardian Three".into(),
                lead_guardian: false,
                status: "accepted".into(),
                added_at: now.to_string(),
                invitation_id: "invitation_6".into(),
            },
            Guardian {
                id: "lead_guardian_1".into(),
                name: "Lead Guardian One".into(),
                lead_guardian: true,
                status: "accepted".into(),
                added_at: now.to_string(),
                invitation_id: "invitation_7".into(),
            },
        ],
        unlock_instructions: Some("Call emergency contact".into()),
        unlock_request: Some(unlock_request),
        version: 0,
    };

    // Box 3: Not associated with guardian_1
    let box_3_id = "33333333-3333-3333-3333-333333333333".to_string();
    let box_3 = BoxRecord {
        id: box_3_id,
        name: "Guardian Test Box 3".into(),
        description: "Box without guardian_1".into(),
        is_locked: true,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        owner_id: "owner_2".into(),
        owner_name: Some("Owner Two".into()),
        documents: vec![],
        guardians: vec![Guardian {
            id: "guardian_2".into(),
            name: "Guardian Two".into(),
            lead_guardian: false,
            status: "accepted".into(),
            added_at: now.to_string(),
            invitation_id: "invitation_9".into(),
        }],
        unlock_instructions: None,
        unlock_request: None,
        version: 0,
    };

    vec![box_1, box_2, box_3]
}

enum TestStore {
    Mock(Arc<MockBoxStore>),
    DynamoDB(Arc<DynamoBoxStore>),
}

// Create test app with either mock or DynamoDB store
async fn create_test_app() -> (Router, TestStore) {
    // Initialize logging for tests
    init_test_logging();

    if use_dynamodb() {
        // Set up DynamoDB store
        info!("Using DynamoDB for guardian tests");
        let client = create_dynamo_client().await;

        // Create the table (ignore errors if table already exists)
        debug!("Setting up DynamoDB test table '{}'", TEST_TABLE_NAME);
        let _ = create_box_table(&client, TEST_TABLE_NAME).await;

        // Clean the table to start fresh
        debug!("Clearing DynamoDB test table");
        let _ = clear_dynamo_table(&client, TEST_TABLE_NAME).await;

        // Create the DynamoDB store with our test table
        let store = Arc::new(DynamoBoxStore::with_client_and_table(
            client.clone(),
            TEST_TABLE_NAME.to_string(),
        ));

        let app = routes::create_router_with_store(store.clone(), "");
        (app, TestStore::DynamoDB(store))
    } else {
        // Use mock store
        debug!("Using mock store for guardian tests");
        let store = Arc::new(MockBoxStore::new());
        let app = routes::create_router_with_store(store.clone(), "");
        (app, TestStore::Mock(store))
    }
}

// Helper function to add standard test data to the store
async fn add_test_data_to_store(store: &TestStore) {
    debug!("Adding test data to store for guardian tests");
    let now = now_str();
    let test_boxes = create_test_data(&now);

    for box_record in test_boxes {
        trace!("Creating test box with ID: {}", box_record.id);
        match store {
            TestStore::Mock(mock) => {
                mock.create_box(box_record.clone()).await.unwrap();
            }
            TestStore::DynamoDB(dynamo) => {
                dynamo.create_box(box_record.clone()).await.unwrap();
            }
        }
    }

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
}

#[tokio::test]
async fn test_get_guardian_boxes() {
    // Setup with test app
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    // Execute
    let response = app
        .oneshot(create_test_request(
            "GET",
            "/boxes/guardian",
            "guardian_1",
            None,
        ))
        .await
        .unwrap();

    // Verify
    assert_eq!(response.status(), StatusCode::OK);

    let json_response = response_to_json(response).await;
    let boxes = json_response.get("boxes").unwrap().as_array().unwrap();

    // Guardian_1 should have 2 boxes (box_1 and box_2)
    assert_eq!(boxes.len(), 2);

    // Verify box ids
    let box_ids: Vec<&str> = boxes
        .iter()
        .map(|b| b.get("id").unwrap().as_str().unwrap())
        .collect();

    assert!(box_ids.contains(&"11111111-1111-1111-1111-111111111111"));
    assert!(box_ids.contains(&"22222222-2222-2222-2222-222222222222"));

    // Check that the boxes have all the fields including the new ones
    let first_box = &boxes[0];
    assert!(
        first_box.get("documents").is_some(),
        "Box should include documents"
    );
    assert!(
        first_box.get("guardians").is_some(),
        "Box should include guardians"
    );

    // Verify the guardian-specific fields
    assert!(
        first_box.get("guardiansCount").is_some(),
        "Box should include guardiansCount"
    );
    assert!(
        first_box.get("isLeadGuardian").is_some(),
        "Box should include isLeadGuardian"
    );
    assert!(
        first_box.get("pendingGuardianApproval").is_some(),
        "Box should include pendingGuardianApproval"
    );
}

#[tokio::test]
async fn test_get_guardian_boxes_empty_for_non_guardian() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    // Execute
    let response = app
        .oneshot(create_test_request(
            "GET",
            "/boxes/guardian",
            "not_a_guardian",
            None,
        ))
        .await
        .unwrap();

    // Verify
    assert_eq!(response.status(), StatusCode::OK);

    let json_response = response_to_json(response).await;
    let boxes = json_response.get("boxes").unwrap().as_array().unwrap();
    assert!(boxes.is_empty());
}

#[tokio::test]
async fn test_get_guardian_box_found() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "11111111-1111-1111-1111-111111111111";

    // Execute
    let response = app
        .oneshot(create_test_request(
            "GET",
            &format!("/boxes/guardian/{}", box_id),
            "guardian_1",
            None,
        ))
        .await
        .unwrap();

    // Verify
    assert_eq!(response.status(), StatusCode::OK);

    let json_response = response_to_json(response).await;
    let box_data = json_response.get("box").unwrap();

    assert_eq!(box_data.get("id").unwrap().as_str().unwrap(), box_id);
    assert_eq!(
        box_data.get("name").unwrap().as_str().unwrap(),
        "Guardian Test Box 1"
    );
}

#[tokio::test]
async fn test_get_guardian_box_unauthorized() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "11111111-1111-1111-1111-111111111111";

    // Execute with a non-guardian user
    let response = app
        .oneshot(create_test_request(
            "GET",
            &format!("/boxes/guardian/{}", box_id),
            "not_a_guardian",
            None,
        ))
        .await
        .unwrap();

    // Should be UNAUTHORIZED
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_guardian_box_not_found() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let non_existent_box_id = "99999999-9999-9999-9999-999999999999";

    // Execute with a non-existent box ID
    let response = app
        .oneshot(create_test_request(
            "GET",
            &format!("/boxes/guardian/{}", non_existent_box_id),
            "guardian_1",
            None,
        ))
        .await
        .unwrap();

    // Verify
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_lead_guardian_unlock_request() {
    // Set up the app and store
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "11111111-1111-1111-1111-111111111111";

    // Verify box state before the test
    let initial_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };
    assert!(
        initial_box.unlock_request.is_none(),
        "Box should not have unlock request before test"
    );

    // Create unlock request payload
    let request_payload = json!({
        "message": "Emergency access needed for testing"
    });

    // Execute the PATCH request to initiate unlock
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/request", box_id),
            "lead_guardian_1",
            Some(request_payload),
        ))
        .await
        .unwrap();

    // Verify response
    assert_eq!(response.status(), StatusCode::OK);

    let json_response = response_to_json(response).await;
    debug!("Response JSON: {:?}", json_response);

    let box_data = json_response
        .get("box")
        .expect("Response should contain 'box' field");

    // Verify unlock request was created
    let unlock_request = box_data
        .get("unlockRequest")
        .expect("Box should have unlockRequest field");
    assert_eq!(
        unlock_request.get("status").unwrap().as_str().unwrap(),
        "invited"
    );
    assert_eq!(
        unlock_request.get("message").unwrap().as_str().unwrap(),
        "Emergency access needed for testing"
    );
    assert_eq!(
        unlock_request.get("initiatedBy").unwrap().as_str().unwrap(),
        "lead_guardian_1"
    );

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify directly in the store
    let updated_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        updated_box.unlock_request.is_some(),
        "Box should have unlock request in store"
    );
    let store_unlock_request = updated_box.unlock_request.unwrap();
    assert_eq!(store_unlock_request.status, "invited");
    assert_eq!(
        store_unlock_request.message,
        Some("Emergency access needed for testing".to_string())
    );
    assert_eq!(
        store_unlock_request.initiated_by,
        Some("lead_guardian_1".to_string())
    );
}

#[tokio::test]
async fn test_non_lead_guardian_cannot_initiate_unlock() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "11111111-1111-1111-1111-111111111111";

    // Create unlock request payload
    let request_payload = json!({
        "message": "This should not work"
    });

    // Execute the PATCH request with a non-lead guardian
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/request", box_id),
            "guardian_1", // Not a lead guardian
            Some(request_payload),
        ))
        .await
        .unwrap();

    // Should be BAD_REQUEST
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify the box still has no unlock request
    let final_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        final_box.unlock_request.is_none(),
        "Box should still not have unlock request after failed attempt"
    );
}

#[tokio::test]
async fn test_accept_unlock_request() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request

    // Verify the box has an unlock request
    let initial_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };
    assert!(
        initial_box.unlock_request.is_some(),
        "Box should have unlock request before test"
    );

    // Create response payload
    let response_payload = json!({
        "approve": true
    });

    // Execute the PATCH request to respond to an unlock request
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/respond", box_id),
            "guardian_1",
            Some(response_payload),
        ))
        .await
        .unwrap();

    // Verify
    assert_eq!(response.status(), StatusCode::OK);

    let json_response = response_to_json(response).await;
    debug!("Response JSON: {:?}", json_response);

    let box_data = json_response
        .get("box")
        .expect("Response should contain 'box' field");

    // Verify guardian was added to approvedBy
    let unlock_request = box_data
        .get("unlockRequest")
        .expect("Box should have unlockRequest field");
    let approved_by = unlock_request
        .get("approvedBy")
        .unwrap()
        .as_array()
        .unwrap();

    assert!(approved_by
        .iter()
        .any(|id| id.as_str().unwrap() == "guardian_1"));

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify directly in the store
    let updated_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        updated_box.unlock_request.is_some(),
        "Box should have unlock request in store"
    );
    let store_unlock_request = updated_box.unlock_request.unwrap();
    assert!(
        store_unlock_request
            .approved_by
            .contains(&"guardian_1".to_string()),
        "guardian_1 should be in approved_by list in store"
    );
}

#[tokio::test]
async fn test_reject_unlock_request() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request

    // Verify the box has an unlock request
    let initial_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };
    assert!(
        initial_box.unlock_request.is_some(),
        "Box should have unlock request before test"
    );
    assert!(
        initial_box
            .unlock_request
            .as_ref()
            .unwrap()
            .rejected_by
            .is_empty(),
        "Box unlock request should not have any rejections before test"
    );

    // Create response payload to reject
    let response_payload = json!({
        "reject": true
    });

    // Execute the PATCH request to reject an unlock request
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/respond", box_id),
            "guardian_1",
            Some(response_payload),
        ))
        .await
        .unwrap();

    // Verify
    assert_eq!(response.status(), StatusCode::OK);

    let json_response = response_to_json(response).await;
    debug!("Response JSON: {:?}", json_response);

    let box_data = json_response
        .get("box")
        .expect("Response should contain 'box' field");

    // Verify guardian was added to rejectedBy
    let unlock_request = box_data
        .get("unlockRequest")
        .expect("Box should have unlockRequest field");
    let rejected_by = unlock_request
        .get("rejectedBy")
        .unwrap()
        .as_array()
        .unwrap();

    assert!(rejected_by
        .iter()
        .any(|id| id.as_str().unwrap() == "guardian_1"));

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify directly in the store
    let updated_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        updated_box.unlock_request.is_some(),
        "Box should have unlock request in store"
    );
    let store_unlock_request = updated_box.unlock_request.unwrap();
    assert!(
        store_unlock_request
            .rejected_by
            .contains(&"guardian_1".to_string()),
        "guardian_1 should be in rejected_by list in store"
    );
}

#[tokio::test]
async fn test_respond_to_unlock_request_invalid_payload() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request

    // Send an invalid response payload (missing both approve and reject)
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/respond", box_id),
            "guardian_1",
            Some(json!({
                // Missing required fields
                "message": "Invalid payload"
            })),
        ))
        .await
        .unwrap();

    // Should result in a client error
    assert!(response.status().is_client_error());

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify the unlock request was not modified
    let final_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        final_box.unlock_request.is_some(),
        "Box should still have unlock request"
    );
    let final_request = final_box.unlock_request.unwrap();
    assert!(
        final_request.approved_by.is_empty(),
        "Approved by should still be empty"
    );
    assert!(
        final_request.rejected_by.is_empty(),
        "Rejected by should still be empty"
    );
}

#[tokio::test]
async fn test_respond_without_unlock_request() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "11111111-1111-1111-1111-111111111111"; // Box WITHOUT unlock request

    // Create response payload
    let response_payload = json!({
        "approve": true
    });

    // Execute the PATCH request to respond when no request exists
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/respond", box_id),
            "guardian_1",
            Some(response_payload),
        ))
        .await
        .unwrap();

    // Should return bad request since there's no unlock request
    assert!(response.status().is_client_error());

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify the box still has no unlock request
    let final_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        final_box.unlock_request.is_none(),
        "Box should still not have unlock request after attempt"
    );
}

#[tokio::test]
async fn test_non_guardian_cannot_respond() {
    // Setup with test data
    let (app, store) = create_test_app().await;

    // Add test data directly to the store
    add_test_data_to_store(&store).await;

    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request

    // Create response payload
    let response_payload = json!({
        "approve": true
    });

    // Execute the PATCH request as a non-guardian
    let response = app
        .oneshot(create_test_request(
            "PATCH",
            &format!("/boxes/guardian/{}/respond", box_id),
            "not_a_guardian",
            Some(response_payload),
        ))
        .await
        .unwrap();

    // Should be UNAUTHORIZED
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // Add delay for DynamoDB consistency
    if matches!(store, TestStore::DynamoDB(_)) {
        debug!("Adding delay for DynamoDB consistency");
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }

    // Verify the unlock request was not modified
    let final_box = match &store {
        TestStore::Mock(mock) => mock.get_box(&box_id).await.unwrap(),
        TestStore::DynamoDB(dynamo) => dynamo.get_box(&box_id).await.unwrap(),
    };

    assert!(
        final_box.unlock_request.is_some(),
        "Box should still have unlock request"
    );
    let final_request = final_box.unlock_request.unwrap();

    // Verify non-guardian was not added to approvers or rejecters
    assert!(
        !final_request
            .approved_by
            .contains(&"not_a_guardian".to_string()),
        "not_a_guardian should not be in approved_by list"
    );
    assert!(
        !final_request
            .rejected_by
            .contains(&"not_a_guardian".to_string()),
        "not_a_guardian should not be in rejected_by list"
    );
}
