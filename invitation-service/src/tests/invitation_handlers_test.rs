use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::{Path, State};
use axum::Json;
use async_trait::async_trait;

use lockbox_shared::error::StoreError;
use lockbox_shared::models::Invitation;
use lockbox_shared::store::InvitationStore;

use crate::handlers::invitation_handlers::{create_invitation, handle_invitation, refresh_invitation};
use crate::models::{ConnectToUserRequest, CreateInvitationRequest};

// Mock implementation of InvitationStore for testing
struct MockInvitationStore {
    invitations: Mutex<HashMap<String, Invitation>>,
    invitation_codes: Mutex<HashMap<String, String>>, // Maps invite_code -> id
}

impl MockInvitationStore {
    fn new() -> Self {
        Self {
            invitations: Mutex::new(HashMap::new()),
            invitation_codes: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl InvitationStore for MockInvitationStore {
    async fn create_invitation(&self, invitation: Invitation) -> lockbox_shared::error::Result<Invitation> {
        let id = invitation.id.clone();
        let invite_code = invitation.invite_code.clone();
        
        // Store by ID
        self.invitations.lock().unwrap().insert(id.clone(), invitation.clone());
        
        // Store by invite code for lookups
        self.invitation_codes.lock().unwrap().insert(invite_code, id);
        
        Ok(invitation)
    }

    async fn get_invitation(&self, id: &str) -> lockbox_shared::error::Result<Invitation> {
        self.invitations
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("Invitation not found: {}", id)))
    }

    async fn get_invitation_by_code(&self, invite_code: &str) -> lockbox_shared::error::Result<Invitation> {
        let id = self
            .invitation_codes
            .lock()
            .unwrap()
            .get(invite_code)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("Invitation not found with code: {}", invite_code)))?;
        
        self.get_invitation(&id).await
    }

    async fn update_invitation(&self, invitation: Invitation) -> lockbox_shared::error::Result<Invitation> {
        let id = invitation.id.clone();
        let old_invite_code = self
            .invitations
            .lock()
            .unwrap()
            .get(&id)
            .map(|inv| inv.invite_code.clone());
        
        // If invite code changed, update the code mapping
        if let Some(old_code) = old_invite_code {
            if old_code != invitation.invite_code {
                self.invitation_codes.lock().unwrap().remove(&old_code);
                self.invitation_codes
                    .lock()
                    .unwrap()
                    .insert(invitation.invite_code.clone(), id.clone());
            }
        }
        
        // Update the invitation
        self.invitations.lock().unwrap().insert(id, invitation.clone());
        
        Ok(invitation)
    }

    async fn delete_invitation(&self, id: &str) -> lockbox_shared::error::Result<()> {
        if let Some(invitation) = self.invitations.lock().unwrap().remove(id) {
            self.invitation_codes
                .lock()
                .unwrap()
                .remove(&invitation.invite_code);
        }
        
        Ok(())
    }

    async fn get_invitations_by_box_id(&self, box_id: &str) -> lockbox_shared::error::Result<Vec<Invitation>> {
        let invitations = self
            .invitations
            .lock()
            .unwrap()
            .values()
            .filter(|inv| inv.box_id == box_id)
            .cloned()
            .collect();
        
        Ok(invitations)
    }
}

#[tokio::test]
async fn test_create_invitation() {
    // Setup
    let store = Arc::new(MockInvitationStore::new());
    let request = CreateInvitationRequest {
        invited_name: "Test User".to_string(),
        box_id: "box-123".to_string(),
    };

    // Execute
    let result = create_invitation(State(store.clone()), Json(request)).await;

    // Verify
    assert!(result.is_ok());
    let response = result.unwrap();
    
    // Verify the response contains an invite code and expiry
    assert!(!response.0.invite_code.is_empty());
    assert!(!response.0.expires_at.is_empty());
    
    // Verify invitation was stored
    let invitations = store.invitations.lock().unwrap();
    assert_eq!(invitations.len(), 1);
    
    let invitation = invitations.values().next().unwrap();
    assert_eq!(invitation.invited_name, "Test User");
    assert_eq!(invitation.box_id, "box-123");
    assert_eq!(invitation.opened, false);
    assert_eq!(invitation.linked_user_id, None);
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
    
    let create_result = create_invitation(State(store.clone()), Json(invite_request)).await.unwrap();
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
    
    let create_result = create_invitation(State(store.clone()), Json(invite_request)).await.unwrap();
    
    // Get the ID of the created invitation
    let invitation_id = {
        let invitations = store.invitations.lock().unwrap();
        invitations.keys().next().unwrap().clone()
    };
    
    // Connect it to a user
    let original_code = create_result.0.invite_code.clone();
    let handle_request = ConnectToUserRequest {
        user_id: "user-456".to_string(),
        invite_code: original_code.clone(),
    };
    
    let _ = handle_invitation(State(store.clone()), Json(handle_request)).await.unwrap();
    
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