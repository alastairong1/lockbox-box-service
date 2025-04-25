// Import shared models and store
use lockbox_shared::store::BoxStore;
use lockbox_shared::models::events::InvitationEvent;
use std::sync::Arc; // Add Arc for shared state
use std::error::Error; // Ensure Error trait is in scope

use tracing::{info, error};

// Import our custom error type
use crate::errors::InvitationEventError;
use crate::errors::AppError; // Add AppError import

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

    // Extract user_id from event
    let user_id = match &event.user_id {
        Some(id) => id,
        None => {
            error!("User ID is missing in the event");
            return Err(InvitationEventError::MissingField("user_id".to_string()).into());
        }
    };

    // Process invitation viewing (connecting user to invitation)
    match process_invitation_viewing(state, &event.box_id, &event.invitation_id, user_id).await {
        Ok(_) => Ok(()),
        Err(app_error) => Err(Box::new(app_error))
    }
}

// Increase the maximum number of retries for better handling of high contention scenarios
const MAX_RETRIES: usize = 10;

pub async fn process_invitation_viewing(
    store: SharedBoxStore,
    box_id: &str,
    invitation_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    log::info!(
        "Processing invitation viewing: box_id={}, invitation_id={}, user_id={}",
        box_id, invitation_id, user_id
    );

    if user_id.is_empty() {
        return Err(AppError::from(anyhow::anyhow!("User ID cannot be empty")));
    }

    let mut retries = 0;
    let mut last_error = None;
    
    while retries < MAX_RETRIES {
        match update_guardian_in_box(&store, box_id, invitation_id, user_id).await {
            Ok(_) => {
                log::info!(
                    "Successfully updated guardian for invitation: box_id={}, invitation_id={}, user_id={}",
                    box_id, invitation_id, user_id
                );
                return Ok(());
            }
            Err(err) => {
                // Check if this is a version conflict error that we should retry
                if let Some(AppError::VersionConflict(_)) = err.downcast_ref::<AppError>() {
                    retries += 1;
                    last_error = Some(err);

                    // Exponential backoff with jitter for better concurrency handling
                    // Base delay of 25ms with exponential growth and randomized jitter
                    let base_delay_ms = 25;
                    let max_jitter_ms = 20;
                    let exp_factor = 1.5f64.powi(retries as i32);
                    let delay_ms = (base_delay_ms as f64 * exp_factor) as u64;
                    let jitter = rand::random::<u64>() % max_jitter_ms;
                    let total_delay = delay_ms + jitter;

                    log::info!(
                        "Version conflict when updating guardian (retry {}/{}): box_id={}, invitation_id={}, waiting {}ms",
                        retries, MAX_RETRIES, box_id, invitation_id, total_delay
                    );
                    
                    tokio::time::sleep(tokio::time::Duration::from_millis(total_delay)).await;
                    continue;
                } else {
                    return Err(AppError::from(err));
                }
            }
        }
    }

    // If we reached max retries, log detailed information and return the last error
    if let Some(err) = last_error {
        log::error!(
            "Failed to update guardian after {} retries: box_id={}, invitation_id={}, user_id={}",
            MAX_RETRIES, box_id, invitation_id, user_id
        );
        
        // Final check - verify current box state to provide better diagnostics
        match store.get_box(box_id).await {
            Ok(box_record) => {
                // Check if guardian was actually updated by another concurrent process
                let guardian = box_record.guardians.iter().find(|g| g.invitation_id == invitation_id);
                match guardian {
                    Some(g) => {
                        if g.id == user_id && g.status == "viewed" {
                            log::info!(
                                "Guardian was actually updated by another process: box_id={}, invitation_id={}, user_id={}",
                                box_id, invitation_id, user_id
                            );
                            return Ok(());
                        }
                        log::error!(
                            "Current guardian state: id={}, status={}, invitation_id={}",
                            g.id, g.status, g.invitation_id
                        );
                    }
                    None => log::error!("Guardian with invitation_id={} not found in box", invitation_id),
                }
                
                log::debug!("Current box state: version={}, guardian_count={}", 
                    box_record.version, box_record.guardians.len());
            }
            Err(e) => log::error!("Failed to retrieve current box state: {}", e),
        }
        
        Err(AppError::from(err))
    } else {
        // This shouldn't happen, but just in case
        Err(AppError::from(anyhow::anyhow!("Failed to update guardian after max retries")))
    }
}

async fn update_guardian_in_box(
    store: &SharedBoxStore,
    box_id: &str,
    invitation_id: &str,
    user_id: &str,
) -> anyhow::Result<()> {
    // Get the box record
    let mut box_record = store.get_box(box_id).await?;
    
    // Find the guardian with the matching invitation ID
    let guardian_idx = box_record
        .guardians
        .iter()
        .position(|g| g.invitation_id == invitation_id);
    
    // Check if we found a matching guardian
    match guardian_idx {
        Some(idx) => {
            // Check if this guardian has already been updated
            let guardian = &box_record.guardians[idx];
            if guardian.status == "viewed" && guardian.id == user_id {
                log::info!(
                    "Guardian already updated, skipping: box_id={}, invitation_id={}, user_id={}",
                    box_id, invitation_id, user_id
                );
                return Ok(());
            }
            
            // Update the guardian's ID and status
            box_record.guardians[idx].id = user_id.to_string();
            box_record.guardians[idx].status = "viewed".to_string();
            
            // Update the box record in the store
            store.update_box(box_record).await?;
            Ok(())
        }
        None => Err(anyhow::anyhow!(
            "No guardian found with invitation ID: {}",
            invitation_id
        )),
    }
} 