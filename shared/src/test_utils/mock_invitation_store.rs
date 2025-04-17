use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use crate::error::{Result, StoreError};
use crate::models::Invitation;
use crate::store::InvitationStore;
use chrono::{DateTime, Utc};

/// Mock implementation of InvitationStore for testing
pub struct MockInvitationStore {
    invitations: Mutex<HashMap<String, Invitation>>,
    invitation_codes: Mutex<HashMap<String, String>>, // Maps invite_code -> id
    error_mode: bool,
    enforce_expiry: bool,
}

impl MockInvitationStore {
    /// Create a new empty MockInvitationStore
    pub fn new() -> Self {
        Self {
            invitations: Mutex::new(HashMap::new()),
            invitation_codes: Mutex::new(HashMap::new()),
            error_mode: false,
            enforce_expiry: false,
        }
    }
    
    /// Create a MockInvitationStore with initial data
    pub fn with_data(invitations: Vec<Invitation>) -> Self {
        let store = Self::new();
        
        for invitation in invitations {
            let id = invitation.id.clone();
            let invite_code = invitation.invite_code.clone();
            
            // Store by ID
            store.invitations
                .lock()
                .unwrap()
                .insert(id.clone(), invitation);
                
            // Store by invite code for lookups
            store.invitation_codes
                .lock()
                .unwrap()
                .insert(invite_code, id);
        }
        
        store
    }

    /// Create a new MockInvitationStore in error mode where all operations fail
    pub fn new_error() -> Self {
        Self {
            invitations: Mutex::new(HashMap::new()),
            invitation_codes: Mutex::new(HashMap::new()),
            error_mode: true,
            enforce_expiry: false,
        }
    }

    /// Create a new MockInvitationStore with expiry enforcement
    pub fn new_with_expiry() -> Self {
        Self {
            invitations: Mutex::new(HashMap::new()),
            invitation_codes: Mutex::new(HashMap::new()),
            error_mode: false,
            enforce_expiry: true,
        }
    }
}

#[async_trait]
impl InvitationStore for MockInvitationStore {
    async fn create_invitation(&self, invitation: Invitation) -> Result<Invitation> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
        let id = invitation.id.clone();
        let invite_code = invitation.invite_code.clone();

        // Store by ID
        self.invitations
            .lock()
            .unwrap()
            .insert(id.clone(), invitation.clone());

        // Store by invite code for lookups
        self.invitation_codes
            .lock()
            .unwrap()
            .insert(invite_code, id);

        Ok(invitation)
    }

    async fn get_invitation(&self, id: &str) -> Result<Invitation> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
        // Lookup invitation by ID
        let invitation = self.invitations
            .lock().unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(format!("Invitation not found: {}", id)))?;
        // Enforce expiry only if enabled
        if self.enforce_expiry {
            let expires_at = DateTime::parse_from_rfc3339(&invitation.expires_at)
                .map_err(|_| StoreError::InternalError("Invalid expiration date format".into()))?
                .with_timezone(&Utc);
            if Utc::now() > expires_at {
                return Err(StoreError::InvitationExpired);
            }
        }
        Ok(invitation)
    }

    async fn get_invitation_by_code(&self, invite_code: &str) -> Result<Invitation> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
        let id = self
            .invitation_codes
            .lock()
            .unwrap()
            .get(invite_code)
            .cloned()
            .ok_or_else(|| {
                StoreError::NotFound(format!("Invitation not found with code: {}", invite_code))
            })?;

        self.get_invitation(&id).await
    }

    async fn update_invitation(&self, invitation: Invitation) -> Result<Invitation> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
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
        self.invitations
            .lock()
            .unwrap()
            .insert(id, invitation.clone());

        Ok(invitation)
    }

    async fn delete_invitation(&self, id: &str) -> Result<()> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
        if let Some(invitation) = self.invitations.lock().unwrap().remove(id) {
            self.invitation_codes
                .lock()
                .unwrap()
                .remove(&invitation.invite_code);
        }

        Ok(())
    }

    async fn get_invitations_by_box_id(&self, box_id: &str) -> Result<Vec<Invitation>> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
        // Return invitations for the box, optionally filtering out expired
        let now = Utc::now();
        let invitations = self
            .invitations
            .lock().unwrap()
            .values()
            .filter(|inv| inv.box_id == box_id)
            .filter_map(|inv| {
                if self.enforce_expiry {
                    // parse expiration and include if not expired
                    DateTime::parse_from_rfc3339(&inv.expires_at)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                        .filter(|dt| &now <= dt)
                        .map(|_| inv.clone())
                } else {
                    Some(inv.clone())
                }
            })
            .collect();
        Ok(invitations)
    }

    async fn get_invitations_by_creator_id(&self, creator_id: &str) -> Result<Vec<Invitation>> {
        if self.error_mode {
            return Err(StoreError::InternalError("Mock".into()));
        }
        let invitations = self
            .invitations
            .lock()
            .unwrap()
            .values()
            .filter(|inv| inv.creator_id == creator_id)
            .cloned()
            .collect();

        Ok(invitations)
    }
} 