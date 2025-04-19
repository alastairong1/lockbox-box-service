use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::delete_item::DeleteItemError;
use aws_sdk_dynamodb::operation::get_item::GetItemError;
use aws_sdk_dynamodb::operation::query::QueryError;
use aws_sdk_dynamodb::operation::scan::ScanError;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use chrono::{Duration, Utc};
use serde_dynamo::{from_item, to_item};
use std::collections::HashMap;
use std::env;

use crate::error::{map_dynamo_error, Result, StoreError};
use crate::models::{BoxRecord, Invitation, now_str};

// Invitation Store Constants
const TABLE_NAME: &str = "invitation-table";
const GSI_BOX_ID: &str = "box_id-index";
const GSI_INVITE_CODE: &str = "invite_code-index";

// Box Store Constants
const BOX_TABLE_NAME: &str = "box-table";
const GSI_OWNER_ID: &str = "owner_id-index";

// DynamoInvitationStore

pub struct DynamoInvitationStore {
    client: Client,
    table_name: String,
}

impl DynamoInvitationStore {
    pub async fn new() -> Self {
        // Use the recommended defaults() function with latest behavior version
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;

        let client = Client::new(&config);

        // Use environment variable for table name if available
        let table_name =
            env::var("DYNAMODB_INVITATION_TABLE").unwrap_or_else(|_| TABLE_NAME.to_string());

        Self { client, table_name }
    }

    /// Creates a new DynamoDB store with the specified client and table name.
    /// This is mainly useful for testing with a local DynamoDB instance.
    #[allow(dead_code)]
    pub fn with_client_and_table(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }
}

// DynamoBoxStore

/// DynamoDB store for boxes
pub struct DynamoBoxStore {
    client: Client,
    table_name: String,
}

impl DynamoBoxStore {
    /// Creates a new DynamoDB store
    pub async fn new() -> Self {
        // Use the recommended defaults() function with latest behavior version
        let config = aws_config::defaults(BehaviorVersion::latest()).load().await;

        let client = Client::new(&config);

        // Use environment variable for table name if available
        let table_name =
            env::var("DYNAMODB_TABLE").unwrap_or_else(|_| BOX_TABLE_NAME.to_string());

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
impl super::BoxStore for DynamoBoxStore {
    /// Creates a new box record in DynamoDB
    async fn create_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        let item = to_item(&box_record)?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| map_dynamo_error("put_item", e))?;

        Ok(box_record)
    }

    /// Gets a box by ID
    async fn get_box(&self, id: &str) -> Result<BoxRecord> {
        let key = HashMap::from([("id".to_string(), AttributeValue::S(id.to_string()))]);

        let response = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .set_key(Some(key))
            .send()
            .await
            .map_err(|e| map_get_dynamo_error(e, id))?;

        let item = response
            .item()
            .ok_or_else(|| StoreError::NotFound(format!("Box not found: {}", id)))?;

        let box_record = from_item(item.clone())?;
        Ok(box_record)
    }

    /// Gets all boxes owned by a user
    async fn get_boxes_by_owner(&self, owner_id: &str) -> Result<Vec<BoxRecord>> {
        let expr_attr_names = HashMap::from([("#owner_id".to_string(), "ownerId".to_string())]);

        let expr_attr_values = HashMap::from([(
            ":owner_id".to_string(),
            AttributeValue::S(owner_id.to_string()),
        )]);

        let response = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name(GSI_OWNER_ID) // Use the GSI
            .key_condition_expression("#owner_id = :owner_id")
            .set_expression_attribute_names(Some(expr_attr_names))
            .set_expression_attribute_values(Some(expr_attr_values))
            .send()
            .await
            .map_err(|e| map_query_dynamo_error(e))?;

        // items() returns a reference to a slice, which could be empty but not None
        let items = response.items();

        let mut boxes = Vec::new();
        for item in items {
            let box_record = from_item(item.clone())?;
            boxes.push(box_record);
        }

        Ok(boxes)
    }

    /// Updates a box
    async fn update_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        // For updates, simply use put_item to replace the entire item
        // In a production app, you might want to use update_item with expressions for efficiency
        let updated_box = BoxRecord {
            updated_at: now_str(),
            ..box_record
        };

        let item = to_item(&updated_box)?;

        self.client
            .put_item()
            .table_name(&self.table_name)
            .set_item(Some(item))
            .send()
            .await
            .map_err(|e| map_dynamo_error("put_item", e))?;

        Ok(updated_box)
    }

    /// Deletes a box
    async fn delete_box(&self, id: &str) -> Result<()> {
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

    /// Gets all boxes where the given user is a guardian (with status not rejected)
    ///
    /// Implementation notes:
    /// - Currently uses a full table scan since guardians are stored in nested arrays within the BoxRecord
    /// - For production systems with many boxes, this could be improved by:
    ///   1. Creating a new GSI with a composite key or
    ///   2. Creating a separate guardian-to-box mapping table with a GSI
    ///   3. Using DynamoDB's new document path capabilities for filtering
    async fn get_boxes_by_guardian_id(&self, guardian_id: &str) -> Result<Vec<BoxRecord>> {
        // Currently we perform a full table scan as guardian information is stored in an array within
        // the box document, not as a separate attribute that can be indexed. In the future, we could
        // create a separate table or GSI for guardian relationships.

        let response = self
            .client
            .scan()
            .table_name(&self.table_name)
            .send()
            .await
            .map_err(|e| map_scan_dynamo_error(e))?;

        let items = response.items();

        let mut boxes = Vec::new();
        for item in items {
            let box_record: BoxRecord = from_item(item.clone())?;

            // Check if the user is a guardian for this box
            let is_guardian = box_record
                .guardians
                .iter()
                .any(|guardian| guardian.id == guardian_id && guardian.status != "rejected");

            if is_guardian {
                boxes.push(box_record);
            }
        }

        Ok(boxes)
    }
}

// INVITATION STORE IMPLEMENTATION
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
            .map_err(|e| map_dynamo_error("put_item", e))?;

        Ok(invitation)
    }

    async fn get_invitation(&self, id: &str) -> Result<Invitation> {
        let key = HashMap::from([("id".to_string(), AttributeValue::S(id.to_string()))]);

        let result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .set_key(Some(key))
            .send()
            .await
            .map_err(|e| map_dynamo_error("get_item", e))?;

        let item = result
            .item()
            .ok_or_else(|| StoreError::NotFound(format!("Invitation with id {} not found", id)))?;

        let invitation: Invitation = from_item(item.clone())?;

        // Check if the invitation has expired
        let expires_at = chrono::DateTime::parse_from_rfc3339(&invitation.expires_at)
            .map_err(|_| StoreError::InternalError("Invalid expiration date format".to_string()))?;

        if Utc::now() > expires_at {
            return Err(StoreError::InvitationExpired);
        }

        Ok(invitation)
    }

    async fn get_invitation_by_code(&self, invite_code: &str) -> Result<Invitation> {
        // Create expression attribute values
        let expr_attr_values = HashMap::from([(
            ":invite_code".to_string(),
            AttributeValue::S(invite_code.to_string()),
        )]);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name(GSI_INVITE_CODE)
            .key_condition_expression("invite_code = :invite_code")
            .set_expression_attribute_values(Some(expr_attr_values))
            .send()
            .await
            .map_err(|e| map_dynamo_error("query", e))?;

        let items = result.items();

        if items.is_empty() {
            return Err(StoreError::NotFound(format!(
                "Invitation with code {} not found",
                invite_code
            )));
        }

        let invitation: Invitation = from_item(items[0].clone())?;

        // Check if the invitation has expired
        let expires_at = chrono::DateTime::parse_from_rfc3339(&invitation.expires_at)
            .map_err(|_| StoreError::InternalError("Invalid expiration date format".to_string()))?;

        if Utc::now() > expires_at {
            return Err(StoreError::InvitationExpired);
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
            .map_err(|e| map_dynamo_error("put_item", e))?;

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
            .map_err(|e| map_dynamo_error("delete_item", e))?;

        Ok(())
    }

    async fn get_invitations_by_box_id(&self, box_id: &str) -> Result<Vec<Invitation>> {
        // Create expression attribute values
        let expr_attr_values =
            HashMap::from([(":box_id".to_string(), AttributeValue::S(box_id.to_string()))]);

        let result = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name(GSI_BOX_ID)
            .key_condition_expression("box_id = :box_id")
            .set_expression_attribute_values(Some(expr_attr_values))
            .send()
            .await
            .map_err(|e| map_dynamo_error("query", e))?;

        let items = result.items();

        let mut invitations = Vec::new();
        for item in items {
            let invitation: Invitation = from_item(item.clone())?;
            // Filter out expired invitations
            let expires_at =
                chrono::DateTime::parse_from_rfc3339(&invitation.expires_at).map_err(|_| {
                    StoreError::InternalError("Invalid expiration date format".to_string())
                })?;

            if Utc::now() <= expires_at {
                invitations.push(invitation);
            }
        }

        Ok(invitations)
    }

    async fn get_invitations_by_creator_id(&self, creator_id: &str) -> Result<Vec<Invitation>> {
        // Scan the entire table with strong consistency and parse items
        let result = self
            .client
            .scan()
            .table_name(&self.table_name)
            .consistent_read(true)
            .send()
            .await
            .map_err(|e| map_scan_dynamo_error(e))?;
        let items = result.items();
        let mut invitations = Vec::new();
        for item in items {
            let invitation: Invitation = from_item(item.clone())?;
            // Only include invitations created by this user
            if invitation.creator_id == creator_id {
                invitations.push(invitation);
            }
        }
        Ok(invitations)
    }
}

// Helper functions for DynamoDB error mapping
fn map_get_dynamo_error(err: SdkError<GetItemError>, id: &str) -> StoreError {
    match err {
        SdkError::ServiceError(ref service_err) => {
            if let GetItemError::ResourceNotFoundException(_) = service_err.err() {
                StoreError::NotFound(format!("Box not found: {}", id))
            } else {
                StoreError::InternalError(format!("DynamoDB get_item error: {}", err))
            }
        }
        _ => StoreError::InternalError(format!("DynamoDB get_item error: {}", err)),
    }
}

fn map_delete_dynamo_error(err: SdkError<DeleteItemError>) -> StoreError {
    StoreError::InternalError(format!("DynamoDB delete_item error: {}", err))
}

fn map_query_dynamo_error(err: SdkError<QueryError>) -> StoreError {
    StoreError::InternalError(format!("DynamoDB query error: {}", err))
}

fn map_scan_dynamo_error(err: SdkError<ScanError>) -> StoreError {
    StoreError::InternalError(format!("DynamoDB scan error: {}", err))
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
