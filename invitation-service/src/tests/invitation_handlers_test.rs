use std::sync::Arc;
use std::env;

use axum::{http::StatusCode, response::Response, Router};
use serde_json::{json, Value};
use tower::ServiceExt;

use lockbox_shared::test_utils::mock_invitation_store::MockInvitationStore;
use lockbox_shared::error::StoreError;
use lockbox_shared::store::InvitationStore;
use lockbox_shared::auth::create_test_request;

use crate::routes::create_router_with_store;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
use lockbox_shared::models::Invitation;

// Helper to convert an Axum response into JSON for assertions
async fn response_to_json(response: Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// Helper to build a test application with a given InvitationStore implementation
fn create_test_app<S>(store: Arc<S>) -> Router
where
    S: InvitationStore + 'static,
{
    create_router_with_store(store, "")
}

// Setup the test environment, including a mock store and required env vars
fn setup_test_env() -> Arc<MockInvitationStore> {
    env::set_var("SNS_TOPIC_ARN", "arn:aws:sns:us-east-1:123456789012:test-topic");
    Arc::new(MockInvitationStore::new())
}

// A store that simulates an expired invitation
struct ExpiredMockInvitationStore;
#[async_trait::async_trait]
impl InvitationStore for ExpiredMockInvitationStore {
    async fn create_invitation(&self, _i: lockbox_shared::models::Invitation) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { unimplemented!() }
    async fn get_invitation(&self, _id: &str) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { Err(StoreError::InvitationExpired) }
    async fn get_invitation_by_code(&self, _c: &str) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { Err(StoreError::InvitationExpired) }
    async fn update_invitation(&self, _i: lockbox_shared::models::Invitation) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { unimplemented!() }
    async fn delete_invitation(&self, _id: &str) -> lockbox_shared::error::Result<()> { unimplemented!() }
    async fn get_invitations_by_box_id(&self, _b: &str) -> lockbox_shared::error::Result<Vec<lockbox_shared::models::Invitation>> { unimplemented!() }
    async fn get_invitations_by_creator_id(&self, _c: &str) -> lockbox_shared::error::Result<Vec<lockbox_shared::models::Invitation>> { unimplemented!() }
}

#[tokio::test]
async fn test_create_invitation() {
    let store = setup_test_env();
    let app = create_test_app(store.clone());

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

    // Verify stored invitation
    let invitations = store
        .get_invitations_by_creator_id("test-user-id")
        .await
        .unwrap();
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
    let _ = setup_test_env();
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
    let store = Arc::new(MockInvitationStore::with_data(vec![invitation.clone()]));
    let app = create_test_app(store.clone());

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

    let updated_inv = store.get_invitation_by_code(&invite_code).await.unwrap();
    assert!(updated_inv.opened);
    assert_eq!(updated_inv.linked_user_id, Some("user-456".to_string()));
}

#[tokio::test]
async fn test_handle_invitation_expired_code() {
    let store = Arc::new(ExpiredMockInvitationStore);
    let app = create_test_app(store.clone());

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
    let _ = setup_test_env();
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    let old_code = "OLDCODE1".to_string();
    let invitation = Invitation {
        id: id.clone(),
        invite_code: old_code.clone(),
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
        created_at: now.to_rfc3339(),
        expires_at: (now - Duration::hours(2)).to_rfc3339(),
        opened: true,
        linked_user_id: Some("user-456".to_string()),
        creator_id: "test-user-id".to_string(),
    };
    let store = Arc::new(MockInvitationStore::with_data(vec![invitation.clone()]));
    let app = create_test_app(store.clone());

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

    let refreshed = store.get_invitation(&id).await.unwrap();
    assert_eq!(refreshed.invite_code, new_code.to_string());
    assert!(!refreshed.opened);
    assert!(refreshed.linked_user_id.is_none());
}

#[tokio::test]
async fn test_refresh_invitation_invalid_id() {
    let _ = setup_test_env();
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
    let store = Arc::new(MockInvitationStore::with_data(vec![invitation.clone()]));
    let app = create_test_app(store.clone());

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
    let _ = setup_test_env();
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
    let store = Arc::new(MockInvitationStore::with_data(vec![invitation.clone()]));
    let app = create_test_app(store.clone());

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
    let store = setup_test_env();
    let app = create_test_app(store.clone());

    // Seed multiple invitations
    for (name, box_id, user) in [
        ("User 1", "box-123", "test-user-id"),
        ("User 2", "box-456", "test-user-id"),
        ("User 3", "box-789", "other-user-id"),
    ] {
        let payload = json!({"invitedName": name, "boxId": box_id});
        app.clone()
            .oneshot(create_test_request("POST", "/invitation", user, Some(payload)))
            .await
            .unwrap();
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
    let store = setup_test_env();
    let app = create_test_app(store.clone());

    let response = app
        .clone()
        .oneshot(create_test_request("GET", "/invitations/me", "test-user-id", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json_resp = response_to_json(response).await;
    assert!(json_resp.as_array().unwrap().is_empty());
}

// A store that always errors, to test error paths
struct ErrorMockInvitationStore;
#[async_trait::async_trait]
impl InvitationStore for ErrorMockInvitationStore {
    async fn create_invitation(&self, _i: lockbox_shared::models::Invitation) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { Err(StoreError::InternalError("Mock".into())) }
    async fn get_invitation(&self, _id: &str) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { Err(StoreError::InternalError("Mock".into())) }
    async fn get_invitation_by_code(&self, _c: &str) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { Err(StoreError::InternalError("Mock".into())) }
    async fn update_invitation(&self, _i: lockbox_shared::models::Invitation) -> lockbox_shared::error::Result<lockbox_shared::models::Invitation> { Err(StoreError::InternalError("Mock".into())) }
    async fn delete_invitation(&self, _id: &str) -> lockbox_shared::error::Result<()> { Err(StoreError::InternalError("Mock".into())) }
    async fn get_invitations_by_box_id(&self, _b: &str) -> lockbox_shared::error::Result<Vec<lockbox_shared::models::Invitation>> { Err(StoreError::InternalError("Mock".into())) }
    async fn get_invitations_by_creator_id(&self, _c: &str) -> lockbox_shared::error::Result<Vec<lockbox_shared::models::Invitation>> { Err(StoreError::InternalError("Mock".into())) }
}

#[tokio::test]
async fn test_get_my_invitations_error() {
    let store = Arc::new(ErrorMockInvitationStore);
    let app = create_test_app(store.clone());

    let response = app
        .oneshot(create_test_request("GET", "/invitations/me", "test-user-id", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
