use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use crate::{
    routes,
    models::{BoxRecord, Guardian, UnlockRequest, now_str},
    store::BoxStore,
};

// Create mock data for testing
fn setup_test_data() -> BoxStore {
    let now = now_str();
    
    // Box 1: Regular guardian (guardian_1)
    let box_1_id = "11111111-1111-1111-1111-111111111111".to_string();
    let box_1 = BoxRecord {
        id: box_1_id,
        name: "Guardian Test Box 1".into(),
        description: "Box for guardian tests".into(),
        is_locked: true,
        created_at: now.clone(),
        updated_at: now.clone(),
        owner_id: "owner_1".into(),
        owner_name: Some("Owner One".into()),
        documents: vec![],
        guardians: vec![
            Guardian {
                id: "guardian_1".into(),
                name: "Guardian One".into(),
                email: "guardian1@example.com".into(),
                lead: false,
                status: "accepted".into(),
                added_at: now.clone(),
            },
            Guardian {
                id: "guardian_2".into(),
                name: "Guardian Two".into(),
                email: "guardian2@example.com".into(),
                lead: false,
                status: "accepted".into(),
                added_at: now.clone(),
            },
            Guardian {
                id: "lead_guardian_1".into(),
                name: "Lead Guardian One".into(),
                email: "leadguardian1@example.com".into(),
                lead: true,
                status: "accepted".into(),
                added_at: now.clone(),
            }
        ],
        lead_guardians: vec![
            Guardian {
                id: "lead_guardian_1".into(),
                name: "Lead Guardian One".into(),
                email: "leadguardian1@example.com".into(),
                lead: true,
                status: "accepted".into(),
                added_at: now.clone(),
            }
        ],
        unlock_instructions: Some("Contact all guardians".into()),
        unlock_request: None,
    };
    
    // Box 2: With pending unlock request
    let box_2_id = "22222222-2222-2222-2222-222222222222".to_string();
    let unlock_request = UnlockRequest {
        id: "unlock-111".into(),
        requested_at: now.clone(),
        status: "pending".into(),
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
        created_at: now.clone(),
        updated_at: now.clone(),
        owner_id: "owner_1".into(),
        owner_name: Some("Owner One".into()),
        documents: vec![],
        guardians: vec![
            Guardian {
                id: "guardian_1".into(),
                name: "Guardian One".into(),
                email: "guardian1@example.com".into(),
                lead: false,
                status: "accepted".into(),
                added_at: now.clone(),
            },
            Guardian {
                id: "guardian_3".into(),
                name: "Guardian Three".into(),
                email: "guardian3@example.com".into(),
                lead: false,
                status: "accepted".into(),
                added_at: now.clone(),
            }
        ],
        lead_guardians: vec![
            Guardian {
                id: "lead_guardian_1".into(),
                name: "Lead Guardian One".into(),
                email: "leadguardian1@example.com".into(),
                lead: true,
                status: "accepted".into(),
                added_at: now.clone(),
            }
        ],
        unlock_instructions: Some("Call emergency contact".into()),
        unlock_request: Some(unlock_request),
    };
    
    // Box 3: Not associated with guardian_1
    let box_3_id = "33333333-3333-3333-3333-333333333333".to_string();
    let box_3 = BoxRecord {
        id: box_3_id,
        name: "Guardian Test Box 3".into(),
        description: "Box without guardian_1".into(),
        is_locked: true,
        created_at: now.clone(),
        updated_at: now.clone(),
        owner_id: "owner_2".into(),
        owner_name: Some("Owner Two".into()),
        documents: vec![],
        guardians: vec![
            Guardian {
                id: "guardian_2".into(),
                name: "Guardian Two".into(),
                email: "guardian2@example.com".into(),
                lead: false,
                status: "accepted".into(),
                added_at: now.clone(),
            }
        ],
        lead_guardians: vec![],
        unlock_instructions: None,
        unlock_request: None,
    };
    
    Arc::new(Mutex::new(vec![box_1, box_2, box_3]))
}

// Inject the test data into the router
fn create_test_app() -> axum::Router {
    let store = setup_test_data();
    routes::create_router_with_store(store)
}

// Helper function to create test request
fn create_request(method: &str, uri: &str, user_id: &str, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(uri)
        .method(method)
        .header("x-user-id", user_id);
    
    if let Some(json_body) = body {
        builder = builder
            .header(header::CONTENT_TYPE, "application/json");
        builder.body(Body::from(json_body.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

// Helper function to extract JSON from response
async fn response_to_json(response: axum::response::Response) -> Value {
    let body = response.into_body();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_get_guardian_boxes() {
    // Setup with mock data
    let app = create_test_app();
    
    // Execute
    let response = app
        .oneshot(create_request("GET", "/guardianBoxes", "guardian_1", None))
        .await
        .unwrap();
    
    // Verify
    assert_eq!(response.status(), StatusCode::OK);
    
    let json_response = response_to_json(response).await;
    let boxes = json_response.get("boxes").unwrap().as_array().unwrap();
    
    // Guardian_1 should have 2 boxes (box_1 and box_2)
    assert_eq!(boxes.len(), 2);
    
    // Verify box ids
    let box_ids: Vec<&str> = boxes.iter()
        .map(|b| b.get("id").unwrap().as_str().unwrap())
        .collect();
    
    assert!(box_ids.contains(&"11111111-1111-1111-1111-111111111111"));
    assert!(box_ids.contains(&"22222222-2222-2222-2222-222222222222"));
}

#[tokio::test]
async fn test_get_guardian_boxes_empty_for_non_guardian() {
    // Setup with mock data
    let app = create_test_app();
    
    // Execute
    let response = app
        .oneshot(create_request("GET", "/guardianBoxes", "not_a_guardian", None))
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
    // Setup with mock data
    let app = create_test_app();
    let box_id = "11111111-1111-1111-1111-111111111111";
    
    // Execute
    let response = app
        .oneshot(create_request(
            "GET", 
            &format!("/guardianBoxes/{}", box_id), 
            "guardian_1", 
            None
        ))
        .await
        .unwrap();
    
    // Verify
    assert_eq!(response.status(), StatusCode::OK);
    
    let json_response = response_to_json(response).await;
    let box_data = json_response.get("box").unwrap();
    
    assert_eq!(box_data.get("id").unwrap().as_str().unwrap(), box_id);
    assert_eq!(box_data.get("name").unwrap().as_str().unwrap(), "Guardian Test Box 1");
}

#[tokio::test]
async fn test_get_guardian_box_unauthorized() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "11111111-1111-1111-1111-111111111111";
    
    // Execute with a non-guardian user
    let response = app
        .oneshot(create_request(
            "GET", 
            &format!("/guardianBoxes/{}", box_id), 
            "not_a_guardian", 
            None
        ))
        .await
        .unwrap();
    
    // Should be UNAUTHORIZED
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_guardian_box_not_found() {
    // Setup with mock data
    let app = create_test_app();
    let non_existent_box_id = "99999999-9999-9999-9999-999999999999";
    
    // Execute with a non-existent box ID
    let response = app
        .oneshot(create_request(
            "GET", 
            &format!("/guardianBoxes/{}", non_existent_box_id), 
            "guardian_1", 
            None
        ))
        .await
        .unwrap();
    
    // Verify
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED); // Returns unauthorized for not found when guardian is valid
}

#[tokio::test]
async fn test_lead_guardian_unlock_request() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "11111111-1111-1111-1111-111111111111";
    
    // Create unlock request payload
    let request_payload = json!({
        "message": "Emergency access needed for testing"
    });
    
    // Execute the PATCH request to initiate unlock
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/request", box_id), 
            "lead_guardian_1", 
            Some(request_payload)
        ))
        .await
        .unwrap();
    
    // Verify 
    assert_eq!(response.status(), StatusCode::OK);
    
    let json_response = response_to_json(response).await;
    let box_data = json_response.get("box").unwrap();
    
    // Verify unlock request was created
    let unlock_request = box_data.get("unlock_request").unwrap();
    assert_eq!(unlock_request.get("status").unwrap().as_str().unwrap(), "pending");
    assert_eq!(unlock_request.get("message").unwrap().as_str().unwrap(), "Emergency access needed for testing");
    assert_eq!(unlock_request.get("initiated_by").unwrap().as_str().unwrap(), "lead_guardian_1");
}

#[tokio::test]
async fn test_non_lead_guardian_cannot_initiate_unlock() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "11111111-1111-1111-1111-111111111111";
    
    // Create unlock request payload
    let request_payload = json!({
        "message": "This should not work"
    });
    
    // Execute the PATCH request with a non-lead guardian
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/request", box_id), 
            "guardian_1", // Not a lead guardian
            Some(request_payload)
        ))
        .await
        .unwrap();
    
    // Should be UNAUTHORIZED or BAD_REQUEST
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_accept_unlock_request() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request
    
    // Create response payload
    let response_payload = json!({
        "approve": true
    });
    
    // Execute the PATCH request to respond to an unlock request
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/respond", box_id), 
            "guardian_1", 
            Some(response_payload)
        ))
        .await
        .unwrap();
    
    // Verify
    assert_eq!(response.status(), StatusCode::OK);
    
    let json_response = response_to_json(response).await;
    let box_data = json_response.get("box").unwrap();
    
    // Verify guardian was added to approved_by
    let unlock_request = box_data.get("unlock_request").unwrap();
    let approved_by = unlock_request.get("approved_by").unwrap().as_array().unwrap();
    
    assert!(approved_by.iter().any(|id| id.as_str().unwrap() == "guardian_1"));
}

#[tokio::test]
async fn test_reject_unlock_request() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request
    
    // Create response payload to reject
    let response_payload = json!({
        "reject": true
    });
    
    // Execute the PATCH request to reject an unlock request
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/respond", box_id), 
            "guardian_1", 
            Some(response_payload)
        ))
        .await
        .unwrap();
    
    // Verify
    assert_eq!(response.status(), StatusCode::OK);
    
    let json_response = response_to_json(response).await;
    let box_data = json_response.get("box").unwrap();
    
    // Verify guardian was added to rejected_by
    let unlock_request = box_data.get("unlock_request").unwrap();
    let rejected_by = unlock_request.get("rejected_by").unwrap().as_array().unwrap();
    
    assert!(rejected_by.iter().any(|id| id.as_str().unwrap() == "guardian_1"));
}

#[tokio::test]
async fn test_respond_to_unlock_request_invalid_payload() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request
    
    // Send an invalid response payload (missing both approve and reject)
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/respond", box_id), 
            "guardian_1",
            Some(json!({
                // Missing required fields
                "message": "Invalid payload"
            }))
        ))
        .await
        .unwrap();
    
    // Should result in a client error
    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_respond_without_unlock_request() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "11111111-1111-1111-1111-111111111111"; // Box WITHOUT unlock request
    
    // Create response payload
    let response_payload = json!({
        "approve": true
    });
    
    // Execute the PATCH request to respond when no request exists
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/respond", box_id), 
            "guardian_1", 
            Some(response_payload)
        ))
        .await
        .unwrap();
    
    // Should return bad request since there's no unlock request
    assert!(response.status().is_client_error());
}

#[tokio::test]
async fn test_non_guardian_cannot_respond() {
    // Setup with mock data
    let app = create_test_app();
    let box_id = "22222222-2222-2222-2222-222222222222"; // Box with existing unlock request
    
    // Create response payload
    let response_payload = json!({
        "approve": true
    });
    
    // Execute the PATCH request as a non-guardian
    let response = app
        .oneshot(create_request(
            "PATCH", 
            &format!("/boxes/guardian/{}/respond", box_id), 
            "not_a_guardian", 
            Some(response_payload)
        ))
        .await
        .unwrap();
    
    // Should be UNAUTHORIZED
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
