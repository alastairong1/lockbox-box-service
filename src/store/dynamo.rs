use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::delete_item::DeleteItemError;
use aws_sdk_dynamodb::operation::get_item::GetItemError;
use aws_sdk_dynamodb::operation::query::QueryError;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use serde_dynamo::{from_item, to_item};
use std::collections::HashMap;

use super::BoxStore;
use crate::error::{AppError, Result};
use crate::models::{now_str, BoxRecord};

/// DynamoDB store for boxes
pub struct DynamoBoxStore {
    client: Client,
    table_name: String,
}

impl DynamoBoxStore {
    /// Creates a new DynamoDB store
    pub async fn new() -> Self {
        // Use the recommended defaults() function instead of from_env()
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .load()
            .await;

        let client = Client::new(&config);

        // Get table name from environment or use a default
        let table_name =
            std::env::var("DYNAMODB_TABLE").unwrap_or_else(|_| "lockbox-boxes".to_string());

        Self { client, table_name }
    }
}

#[async_trait::async_trait]
impl BoxStore for DynamoBoxStore {
    /// Creates a new box record in DynamoDB
    async fn create_box(&self, box_record: BoxRecord) -> Result<BoxRecord> {
        let item = to_item(&box_record).map_err(|e| {
            AppError::InternalServerError(format!("Failed to serialize box: {}", e))
        })?;

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
            .ok_or_else(|| AppError::NotFound(format!("Box not found: {}", id)))?;

        from_item(item.clone())
            .map_err(|e| AppError::InternalServerError(format!("Failed to deserialize box: {}", e)))
    }

    /// Gets all boxes owned by a user
    async fn get_boxes_by_owner(&self, owner_id: &str) -> Result<Vec<BoxRecord>> {
        let expr_attr_names = HashMap::from([("#owner_id".to_string(), "owner_id".to_string())]);

        let expr_attr_values = HashMap::from([(
            ":owner_id".to_string(),
            AttributeValue::S(owner_id.to_string()),
        )]);

        let response = self
            .client
            .query()
            .table_name(&self.table_name)
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
            let box_record = from_item(item.clone()).map_err(|e| {
                AppError::InternalServerError(format!("Failed to deserialize box: {}", e))
            })?;
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

        let item = to_item(&updated_box).map_err(|e| {
            AppError::InternalServerError(format!("Failed to serialize box: {}", e))
        })?;

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
}

/// Default implementation for testing purposes
impl Default for DynamoBoxStore {
    fn default() -> Self {
        // Create a mock/placeholder client for testing
        // In a real testing scenario, you might want to use LocalStack or a mocking library
        let config = aws_sdk_dynamodb::config::Builder::new()
            .endpoint_url("http://localhost:8000") // Placeholder endpoint
            .build();

        Self {
            client: Client::from_conf(config),
            table_name: "test-table".to_string(),
        }
    }
}

/// Map DynamoDB put_item errors to application errors
fn map_dynamo_error<E>(operation: &str, err: SdkError<E>) -> AppError {
    AppError::InternalServerError(format!("DynamoDB {} error: {}", operation, err))
}

/// Map DynamoDB get_item errors to application errors
fn map_get_dynamo_error(err: SdkError<GetItemError>, id: &str) -> AppError {
    match err {
        SdkError::ServiceError(ref service_err) => {
            if let GetItemError::ResourceNotFoundException(_) = service_err.err() {
                AppError::NotFound(format!("Box not found: {}", id))
            } else {
                AppError::InternalServerError(format!("DynamoDB get_item error: {}", err))
            }
        }
        _ => AppError::InternalServerError(format!("DynamoDB get_item error: {}", err)),
    }
}

/// Map DynamoDB delete_item errors to application errors
fn map_delete_dynamo_error(err: SdkError<DeleteItemError>) -> AppError {
    AppError::InternalServerError(format!("DynamoDB delete_item error: {}", err))
}

/// Map DynamoDB query errors to application errors
fn map_query_dynamo_error(err: SdkError<QueryError>) -> AppError {
    AppError::InternalServerError(format!("DynamoDB query error: {}", err))
}
