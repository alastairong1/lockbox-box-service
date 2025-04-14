use std::env;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::AttributeValue;
use serde_dynamo::{from_item, to_item};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use std::collections::HashMap;

use crate::error::{
    Result, ServiceError, 
    map_get_dynamo_error, map_query_dynamo_error, 
    map_put_dynamo_error, map_delete_dynamo_error
};
use crate::models::Invitation;

const TABLE_NAME: &str = "invitation-table";
const GSI_BOX_ID: &str = "box_id-index";
const GSI_INVITE_CODE: &str = "invite_code-index";

pub struct DynamoInvitationStore {
    client: Client,
    table_name: String,
}

impl DynamoInvitationStore {
    pub async fn new() -> Self {
        // Use the recommended defaults() function with latest behavior version
        let config = aws_config::defaults(BehaviorVersion::latest())
            .load()
            .await;
        
        let client = Client::new(&config);
        
        // Use environment variable for table name if available
        let table_name = env::var("DYNAMODB_INVITATION_TABLE").unwrap_or_else(|_| TABLE_NAME.to_string());
        
        Self { client, table_name }
    }
    
    /// Creates a new DynamoDB store with the specified client and table name.
    /// This is mainly useful for testing with a local DynamoDB instance.
    #[allow(dead_code)]
    pub fn with_client_and_table(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }
}

#[async_trait]
impl super::InvitationStore for DynamoInvitationStore {
    async fn create_invitation(&self, mut invitation: Invitation) -> Result<Invitation> {
        // Set created_at and expires_at if not already set
        if invitation.created_at.is_empty() {
            invitation.created_at = Utc::now().to_rfc3339();
        }
        
        if invitation.expires_at.is_empty() {
            // Set expiration to 48 hours from now
            invitation.expires_at = (Utc::now() + Duration::hours(48)).to_rfc3339();
        }
        
        // Convert to DynamoDB item
        let item = to_item(invitation.clone())?;
        
        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| map_put_dynamo_error(e))?;
            
        Ok(invitation)
    }
    
    async fn get_invitation(&self, id: &str) -> Result<Invitation> {
        let key = HashMap::from([("id".to_string(), AttributeValue::S(id.to_string()))]);
        
        let result = self.client
            .get_item()
            .table_name(&self.table_name)
            .set_key(Some(key))
            .send()
            .await
            .map_err(|e| map_get_dynamo_error(e, id))?;
            
        let item = result.item()
            .ok_or_else(|| ServiceError::NotFound(format!("Invitation with id {} not found", id)))?;
        
        let invitation: Invitation = from_item(item.clone())?;
        
        // Check if the invitation has expired
        let expires_at = chrono::DateTime::parse_from_rfc3339(&invitation.expires_at)
            .map_err(|_| ServiceError::InternalError("Invalid expiration date format".to_string()))?;
            
        if Utc::now() > expires_at {
            return Err(ServiceError::InvitationExpired);
        }
        
        Ok(invitation)
    }
    
    async fn get_invitation_by_code(&self, invite_code: &str) -> Result<Invitation> {
        // Create expression attribute values
        let expr_attr_values = HashMap::from([
            (":invite_code".to_string(), AttributeValue::S(invite_code.to_string()))
        ]);
        
        let result = self.client
            .query()
            .table_name(&self.table_name)
            .index_name(GSI_INVITE_CODE)
            .key_condition_expression("invite_code = :invite_code")
            .set_expression_attribute_values(Some(expr_attr_values))
            .send()
            .await
            .map_err(|e| map_query_dynamo_error(e))?;
            
        let items = result.items();
        
        if items.is_empty() {
            return Err(ServiceError::NotFound(format!("Invitation with code {} not found", invite_code)));
        }
        
        let invitation: Invitation = from_item(items[0].clone())?;
        
        // Check if the invitation has expired
        let expires_at = chrono::DateTime::parse_from_rfc3339(&invitation.expires_at)
            .map_err(|_| ServiceError::InternalError("Invalid expiration date format".to_string()))?;
            
        if Utc::now() > expires_at {
            return Err(ServiceError::InvitationExpired);
        }
        
        Ok(invitation)
    }
    
    async fn update_invitation(&self, invitation: Invitation) -> Result<Invitation> {
        // Verify invitation exists first
        self.get_invitation(&invitation.id).await?;
        
        // Convert to DynamoDB item
        let item = to_item(invitation.clone())?;
        
        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| map_put_dynamo_error(e))?;
            
        Ok(invitation)
    }
    
    async fn delete_invitation(&self, id: &str) -> Result<()> {
        // Verify invitation exists first
        self.get_invitation(id).await?;
        
        let key = HashMap::from([("id".to_string(), AttributeValue::S(id.to_string()))]);
        
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .set_key(Some(key))
            .send()
            .await
            .map_err(|e| map_delete_dynamo_error(e))?;
            
        Ok(())
    }
    
    async fn get_invitations_by_box_id(&self, box_id: &str) -> Result<Vec<Invitation>> {
        // Create expression attribute values
        let expr_attr_values = HashMap::from([
            (":box_id".to_string(), AttributeValue::S(box_id.to_string()))
        ]);
        
        let result = self.client
            .query()
            .table_name(&self.table_name)
            .index_name(GSI_BOX_ID)
            .key_condition_expression("box_id = :box_id")
            .set_expression_attribute_values(Some(expr_attr_values))
            .send()
            .await
            .map_err(|e| map_query_dynamo_error(e))?;
            
        let items = result.items();
        
        let mut invitations = Vec::new();
        for item in items {
            let invitation: Invitation = from_item(item.clone())?;
            // Filter out expired invitations
            let expires_at = chrono::DateTime::parse_from_rfc3339(&invitation.expires_at)
                .map_err(|_| ServiceError::InternalError("Invalid expiration date format".to_string()))?;
                
            if Utc::now() <= expires_at {
                invitations.push(invitation);
            }
        }
        
        Ok(invitations)
    }
}

// Add Default impl for convenience
impl Default for DynamoInvitationStore {
    fn default() -> Self {
        // For the default implementation, we'll need to use the tokio runtime to run the async new() function
        // This is not ideal, but it's a reasonable fallback for Default
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
            
        runtime.block_on(Self::new())
    }
} 