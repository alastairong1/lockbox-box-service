use async_trait::async_trait;

use crate::error::Result;
use crate::models::Invitation;

// Expose the DynamoDB store module
pub mod dynamo;

/// InvitationStore trait defining the interface for invitation storage implementations
#[async_trait]
pub trait InvitationStore: Send + Sync + 'static {
    /// Creates a new invitation
    async fn create_invitation(&self, invitation: Invitation) -> Result<Invitation>;

    /// Gets an invitation by ID
    async fn get_invitation(&self, id: &str) -> Result<Invitation>;

    /// Gets an invitation by invite code
    async fn get_invitation_by_code(&self, invite_code: &str) -> Result<Invitation>;

    /// Updates an invitation
    async fn update_invitation(&self, invitation: Invitation) -> Result<Invitation>;

    /// Deletes an invitation
    async fn delete_invitation(&self, id: &str) -> Result<()>;
    
    /// Gets all invitations for a box
    async fn get_invitations_by_box_id(&self, box_id: &str) -> Result<Vec<Invitation>>;
} 