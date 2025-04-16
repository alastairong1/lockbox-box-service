use aws_lambda_events::event::sns::{SnsEvent, SnsMessage};
use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{info, error};

// Import shared models and store
use lockbox_shared::models::{BoxRecord, Guardian};
use lockbox_shared::store::BoxStore;
use lockbox_shared::store::dynamo::DynamoBoxStore;

// Event received from SNS
#[derive(Deserialize, Debug)]
struct InvitationAcceptedEvent {
    event_type: String,
    invitation_id: String,
    box_id: String,
    user_id: Option<String>,
    invite_code: String,
    timestamp: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    info!("Starting Box Invitation Handler Lambda");

    // Run the Lambda service function
    lambda_runtime::run(service_fn(handler)).await?;
    Ok(())
}

// Lambda handler function
async fn handler(event: LambdaEvent<SnsEvent>) -> Result<(), Error> {
    // Get the SNS event
    let sns_event = event.payload;
    
    // Process each record (message) in the SNS event
    for record in sns_event.records {
        // Extract and parse the SNS message
        let message = record.sns;
        
        // Try to parse the message as an InvitationAcceptedEvent
        if let Ok(invitation_event) = serde_json::from_str::<InvitationAcceptedEvent>(&message.message) {
            if invitation_event.event_type == "invitation_accepted" {
                info!(
                    "Processing invitation_accepted event for box_id={}, user_id={:?}",
                    invitation_event.box_id, invitation_event.user_id
                );
                
                // Process invitation acceptance
                if let Err(err) = process_invitation_acceptance(&invitation_event).await {
                    error!("Error processing invitation: {:?}", err);
                }
            }
        } else {
            error!("Failed to parse SNS message: {}", message.message);
        }
    }
    
    Ok(())
}

// Core business logic to update the box when an invitation is accepted
async fn process_invitation_acceptance(event: &InvitationAcceptedEvent) -> Result<(), Error> {
    // Check if user_id is present
    let user_id = match &event.user_id {
        Some(id) => id,
        None => {
            error!("User ID is missing in the event");
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "User ID is required",
            )));
        }
    };
    
    // Initialize DynamoBoxStore
    let box_store = DynamoBoxStore::new().await;
    
    // Get the box record
    let mut box_record = box_store.get_box(&event.box_id).await
        .map_err(|e| {
            error!("Failed to get box: {}", e);
            Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
        })?;
    
    // Find the guardian with matching invitation_id
    let guardian_updated = update_guardian_in_box(&mut box_record, &event.invitation_id, user_id);
    
    if guardian_updated {
        // Update the box in DynamoDB
        box_store.update_box(box_record).await
            .map_err(|e| {
                error!("Failed to update box: {}", e);
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            })?;
        
        info!("Box guardian updated successfully for box_id={}", event.box_id);
    } else {
        info!("No matching guardian found for invitation_id={}", event.invitation_id);
    }
    
    Ok(())
}

// Updates a guardian in the box with the user_id
fn update_guardian_in_box(box_record: &mut BoxRecord, invitation_id: &str, user_id: &str) -> bool {
    // Find the guardian with matching invitation_id
    for guardian in box_record.guardians.iter_mut() {
        if guardian.invitation_id == invitation_id {
            info!(
                "Found guardian with invitation_id={}, current status={}",
                invitation_id, guardian.status
            );
            
            // Update the guardian with the user_id
            guardian.id = user_id.to_string();
            
            // Update the guardian status to accepted
            guardian.status = "accepted".to_string();
            
            // If it's a lead guardian, update in lead_guardians array too
            if guardian.lead {
                for lead in box_record.lead_guardians.iter_mut() {
                    if lead.invitation_id == invitation_id {
                        lead.id = user_id.to_string();
                        lead.status = "accepted".to_string();
                    }
                }
            }
            
            // Update the box's updated_at timestamp
            box_record.updated_at = lockbox_shared::models::now_str();
            
            info!(
                "Updated guardian with invitation_id={} to user_id={}, status=accepted",
                invitation_id, user_id
            );
            
            return true;
        }
    }
    
    error!("No guardian found with invitation_id={}", invitation_id);
    false
} 