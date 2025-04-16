use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::Json;
use async_trait::async_trait;

use lockbox_shared::models::Invitation;
use lockbox_shared::test_utils::mock_invitation_store::MockInvitationStore;
use lockbox_shared::error::StoreError;
use lockbox_shared::store::InvitationStore;

use crate::handlers::invitation_handlers::{
    create_invitation, get_my_invitations, handle_invitation, refresh_invitation,
};
use crate::models::{ConnectToUserRequest, CreateInvitationRequest};

#[tokio::test]
async fn test_create_invitation() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());
    let request = CreateInvitationRequest {
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
    };

    // Execute
    let result = create_invitation(
        State(store.clone()),
        Extension("test-user-id".to_string()),
        Json(request),
    )
    .await;

    // Verify
    assert!(result.is_ok());
    let response = result.unwrap();

    // Verify the response contains an invite code and expiry
    assert!(!response.0.invite_code.is_empty());
    assert!(!response.0.expires_at.is_empty());

    // Verify invitation was stored by creator ID
    let invitations = store.get_invitations_by_creator_id("test-user-id").await.unwrap();
    assert_eq!(invitations.len(), 1);

    let invitation = &invitations[0];
    assert_eq!(invitation.invited_name, "Test User");
    assert_eq!(invitation.box_id, "box-123");
    assert_eq!(invitation.opened, false);
    assert_eq!(invitation.linked_user_id, None);
    assert_eq!(invitation.creator_id, "test-user-id");
}

#[tokio::test]
async fn test_handle_invitation() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());

    // Create an invitation first
    let invite_request = CreateInvitationRequest {
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
    };

    let create_result = create_invitation(
        State(store.clone()),
        Extension("test-user-id".to_string()),
        Json(invite_request),
    )
    .await
    .unwrap();
    let invite_code = create_result.0.invite_code;

    // Now handle the invitation (connect to a user)
    let handle_request = ConnectToUserRequest {
        user_id: "user-456".to_string(),
        invite_code: invite_code.clone(),
    };

    // Execute
    let result = handle_invitation(State(store.clone()), Json(handle_request)).await;

    // Verify
    assert!(result.is_ok());

    // Check that invitation was updated
    let invitation = store.get_invitation_by_code(&invite_code).await.unwrap();
    assert_eq!(invitation.opened, true);
    assert_eq!(invitation.linked_user_id, Some("user-456".to_string()));
}

#[tokio::test]
async fn test_refresh_invitation() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());

    // Create an invitation first
    let invite_request = CreateInvitationRequest {
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
    };

    let create_result = create_invitation(
        State(store.clone()),
        Extension("test-user-id".to_string()),
        Json(invite_request),
    )
    .await
    .unwrap();

    // Get the ID of the created invitation
    let invitations = store.get_invitations_by_creator_id("test-user-id").await.unwrap();
    let invitation_id = invitations[0].id.clone();

    // Connect it to a user
    let original_code = create_result.0.invite_code.clone();
    let handle_request = ConnectToUserRequest {
        user_id: "user-456".to_string(),
        invite_code: original_code.clone(),
    };

    let _ = handle_invitation(State(store.clone()), Json(handle_request))
        .await
        .unwrap();

    // Now refresh the invitation
    let result = refresh_invitation(State(store.clone()), Path(invitation_id.clone())).await;

    // Verify
    assert!(result.is_ok());
    let response = result.unwrap();

    // Verify that a new code was generated
    assert_ne!(response.0.invite_code, original_code);

    // Verify invitation was updated properly
    let invitation = store.get_invitation(&invitation_id).await.unwrap();
    assert_eq!(invitation.opened, false);
    assert_eq!(invitation.linked_user_id, None);
}

#[tokio::test]
async fn test_handle_invitation_invalid_code() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());

    // Try to handle an invitation with an invalid code
    let handle_request = ConnectToUserRequest {
        user_id: "user-456".to_string(),
        invite_code: "INVALID".to_string(),
    };

    // Execute
    let result = handle_invitation(State(store.clone()), Json(handle_request)).await;

    // Verify
    assert!(result.is_err());
}

#[tokio::test]
async fn test_refresh_invitation_invalid_id() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());

    // Try to refresh an invitation with an invalid ID
    let result = refresh_invitation(State(store.clone()), Path("invalid-id".to_string())).await;

    // Verify
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_my_invitations() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());

    // Create first invitation
    let invite_request1 = CreateInvitationRequest {
        invited_name: "Test User 1".to_string(),
        box_id: "box-123".to_string(),
    };

    let _ = create_invitation(
        State(store.clone()),
        Extension("test-user-id".to_string()),
        Json(invite_request1),
    )
    .await
    .unwrap();

    // Create second invitation with same creator
    let invite_request2 = CreateInvitationRequest {
        invited_name: "Test User 2".to_string(),
        box_id: "box-456".to_string(),
    };

    let _ = create_invitation(
        State(store.clone()),
        Extension("test-user-id".to_string()),
        Json(invite_request2),
    )
    .await
    .unwrap();

    // Create third invitation with different creator
    let invite_request3 = CreateInvitationRequest {
        invited_name: "Test User 3".to_string(),
        box_id: "box-789".to_string(),
    };

    let _ = create_invitation(
        State(store.clone()),
        Extension("other-user-id".to_string()),
        Json(invite_request3),
    )
    .await
    .unwrap();

    // Execute - get invitations for test-user-id
    let result =
        get_my_invitations(State(store.clone()), Extension("test-user-id".to_string())).await;

    // Verify
    assert!(result.is_ok());
    let invitations = result.unwrap().0;

    // Should only return the 2 invitations created by test-user-id
    assert_eq!(invitations.len(), 2);

    // All should have the correct creator_id
    for invitation in invitations {
        assert_eq!(invitation.creator_id, "test-user-id");
        assert!(invitation.box_id == "box-123" || invitation.box_id == "box-456");
    }
}

#[tokio::test]
async fn test_get_my_invitations_empty() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());

    // Create invitations for a different user only
    let invite_request = CreateInvitationRequest {
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
    };

    let _ = create_invitation(
        State(store.clone()),
        Extension("other-user-id".to_string()),
        Json(invite_request),
    )
    .await
    .unwrap();

    // Execute - get invitations for a user with no invitations
    let result =
        get_my_invitations(State(store.clone()), Extension("test-user-id".to_string())).await;

    // Verify
    assert!(result.is_ok());
    let invitations = result.unwrap().0;

    // Should return an empty list, not null
    assert_eq!(invitations.len(), 0);
}

// Mock store that returns an error for get_invitations_by_creator_id
struct ErrorMockInvitationStore;

#[async_trait]
impl InvitationStore for ErrorMockInvitationStore {
    async fn create_invitation(
        &self,
        _invitation: Invitation,
    ) -> lockbox_shared::error::Result<Invitation> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }

    async fn get_invitation(&self, _id: &str) -> lockbox_shared::error::Result<Invitation> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }

    async fn get_invitation_by_code(
        &self,
        _invite_code: &str,
    ) -> lockbox_shared::error::Result<Invitation> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }

    async fn update_invitation(
        &self,
        _invitation: Invitation,
    ) -> lockbox_shared::error::Result<Invitation> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }

    async fn delete_invitation(&self, _id: &str) -> lockbox_shared::error::Result<()> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }

    async fn get_invitations_by_box_id(
        &self,
        _box_id: &str,
    ) -> lockbox_shared::error::Result<Vec<Invitation>> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }

    async fn get_invitations_by_creator_id(
        &self,
        _creator_id: &str,
    ) -> lockbox_shared::error::Result<Vec<Invitation>> {
        Err(StoreError::InternalError("Mock error".to_string()))
    }
}

#[tokio::test]
async fn test_get_my_invitations_error() {
    // Setup
    let store = Arc::new(ErrorMockInvitationStore);

    // Execute - attempt to get invitations
    let result = get_my_invitations(State(store), Extension("test-user-id".to_string())).await;

    // Verify the error is properly handled
    assert!(result.is_err());
}
