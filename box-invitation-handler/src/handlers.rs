// Import shared models and store
use lockbox_shared::models::BoxRecord;
use lockbox_shared::store::BoxStore;
use lockbox_shared::models::events::InvitationEvent;
use std::sync::Arc; // Add Arc for shared state
use std::error::Error; // Ensure Error trait is in scope

use tracing::{info, error};

type SharedBoxStore = Arc<dyn BoxStore + Send + Sync>;

// Handler for invitation_created events
pub async fn handle_invitation_created(
    _state: SharedBoxStore, // Unused for now, prefixed with underscore
    event: &InvitationEvent
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!(
        "Processing invitation_created event for box_id={}",
        event.box_id
    );

    Ok(())
}

// Handler for invitation_viewed events
pub async fn handle_invitation_viewed(
    state: SharedBoxStore,
    event: &InvitationEvent
) -> Result<(), Box<dyn Error + Send + Sync>> {
    info!(
        "Processing invitation_viewed event for box_id={}, user_id={:?}",
        event.box_id, event.user_id
    );

    // Process invitation viewing (connecting user to invitation)
    process_invitation_viewing(state, event).await
}

// Core business logic to update the box when an invitation is viewed/connected to a user
async fn process_invitation_viewing(
    box_store: SharedBoxStore,
    event: &InvitationEvent
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Check if user_id is present
    let user_id = match &event.user_id {
        Some(id) => id,
        None => {
            error!("User ID is missing in the event");
            return Err(Box::new(std::io::Error::new( 
                std::io::ErrorKind::InvalidData,
                "User ID is required",
            )) as Box<dyn Error + Send + Sync>);
        }
    };

    // Get the box record using the passed-in store
    let mut box_record = box_store.get_box(&event.box_id).await
        .map_err(|e| {
            error!("Failed to get box: {}", e);
            Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string())) as Box<dyn Error + Send + Sync>
        })?;

    // Find the guardian with matching invitation_id
    let guardian_updated = update_guardian_in_box(&mut box_record, &event.invitation_id, user_id);

    if guardian_updated {
        // Update the box in DynamoDB using the passed-in store
        box_store.update_box(box_record).await
            .map_err(|e| {
                error!("Failed to update box: {}", e);
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())) as Box<dyn Error + Send + Sync>
            })?;

        info!("Box guardian updated successfully for box_id={}", event.box_id);
    } else {
        tracing::warn!("No matching guardian found for invitation_id={}", event.invitation_id);
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
            if guardian.lead_guardian {
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
    
    tracing::warn!("No guardian found with invitation_id={}", invitation_id);
    false
} 