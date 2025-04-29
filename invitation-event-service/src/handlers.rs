// Import shared models and store
use lockbox_shared::models::events::InvitationEvent;
use lockbox_shared::store::BoxStore;
use std::sync::Arc; // Add Arc for shared state

// use tracing::{error, info, warn}; // Remove tracing import
use log::{error, info, warn}; // Add log import

// Import our custom error type
use crate::errors::AppError;
use crate::errors::InvitationEventError; // Add AppError import

type SharedBoxStore = Arc<dyn BoxStore + Send + Sync>;

// Handler for invitation_created events
pub async fn handle_invitation_created(
    _state: SharedBoxStore, // Unused for now, prefixed with underscore
    event: &InvitationEvent,
) -> Result<(), AppError> {
    info!(
        "Processing invitation_created event for box_id={}",
        event.box_id
    );

    Ok(())
}

// Handler for invitation_opened events
pub async fn handle_invitation_opened(
    _state: SharedBoxStore,
    event: &InvitationEvent,
) -> Result<(), AppError> {
    info!(
        "Processing invitation_opened event for box_id={}",
        event.box_id
    );

    Ok(())
}

// Handler for invitation_viewed events
pub async fn handle_invitation_viewed(
    state: SharedBoxStore,
    event: &InvitationEvent,
) -> Result<(), AppError> {
    info!(
        "Processing invitation_viewed event for box_id={}",
        event.box_id
    );

    // We don't need to extract box_id separately since we use event.box_id directly

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
        Err(app_error) => {
            // Only propagate critical errors, log and ignore others
            // This is important for resilience - if a box doesn't exist or guardian not found,
            // we don't want to crash the handler
            match &app_error {
                AppError::GuardianNotFound(msg) => {
                    warn!("Ignoring event for non-existent guardian: {}", msg);
                    Ok(())
                }
                AppError::BoxNotFound(msg) => {
                    warn!("Ignoring event for non-existent box: {}", msg);
                    Ok(())
                }
                _ => Err(app_error),
            }
        }
    }
}

// Reasonable retry limit
const MAX_RETRIES: usize = 5;

pub async fn process_invitation_viewing(
    store: SharedBoxStore,
    box_id: &str,
    invitation_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    info!(
        "Processing invitation viewing: box_id={}, invitation_id={}, user_id={}",
        box_id, invitation_id, user_id
    );

    if user_id.is_empty() {
        return Err(AppError::from(anyhow::anyhow!("User ID cannot be empty")));
    }

    // First check if the box exists
    let box_result = store.get_box(box_id).await;
    if let Err(e) = box_result {
        return Err(AppError::BoxNotFound(format!(
            "Box not found: {}, error: {}",
            box_id, e
        )));
    }

    let mut retries = 0;
    let mut last_error = None;

    while retries < MAX_RETRIES {
        match update_specific_guardian(&store, box_id, invitation_id, user_id).await {
            Ok(_) => {
                info!(
                    "Successfully updated guardian for invitation: box_id={}, invitation_id={}, user_id={}",
                    box_id, invitation_id, user_id
                );
                return Ok(());
            }
            Err(err) => {
                retries += 1;
                last_error = Some(err);

                // Simple backoff with minimal jitter
                let delay_ms = 50 + (retries as u64 * 20);

                info!(
                    "Error updating guardian (retry {}/{}): box_id={}, invitation_id={}, waiting {}ms",
                    retries, MAX_RETRIES, box_id, invitation_id, delay_ms
                );

                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                continue;
            }
        }
    }

    // If we reached max retries, log and return the last error
    if let Some(err) = last_error {
        error!(
            "Failed to update guardian after {} retries: box_id={}, invitation_id={}, user_id={}",
            MAX_RETRIES, box_id, invitation_id, user_id
        );

        // Final check - verify current box state to provide better diagnostics
        match store.get_box(box_id).await {
            Ok(box_record) => {
                let guardian = box_record
                    .guardians
                    .iter()
                    .find(|g| g.invitation_id == invitation_id);
                match guardian {
                    Some(g) => {
                        if g.id == user_id && g.status == "viewed" {
                            info!(
                                "Guardian was actually updated by another process: box_id={}, invitation_id={}, user_id={}",
                                box_id, invitation_id, user_id
                            );
                            return Ok(());
                        }
                        log::error!(
                            "Current guardian state: id={}, status={}, invitation_id={}",
                            g.id,
                            g.status,
                            g.invitation_id
                        );
                    }
                    None => log::error!(
                        "Guardian with invitation_id={} not found in box",
                        invitation_id
                    ),
                }
            }
            Err(e) => log::error!("Failed to retrieve current box state: {}", e),
        }

        Err(AppError::from(err))
    } else {
        Err(AppError::from(anyhow::anyhow!(
            "Failed to update guardian after max retries"
        )))
    }
}

// New approach that updates only the specific guardian by invitation_id
// instead of updating the entire box at once
async fn update_specific_guardian(
    store: &SharedBoxStore,
    box_id: &str,
    invitation_id: &str,
    user_id: &str,
) -> anyhow::Result<()> {
    // Get the current box state
    let mut box_record = store.get_box(box_id).await?;

    // Find the guardian matching the invitation ID
    let guardian_idx = box_record
        .guardians
        .iter()
        .position(|g| g.invitation_id == invitation_id);

    // If no matching guardian found, return a specific error
    if guardian_idx.is_none() {
        return Err(anyhow::anyhow!(AppError::GuardianNotFound(format!(
            "No guardian found with invitation ID: {}",
            invitation_id
        ))));
    }

    let guardian_idx = guardian_idx.unwrap();
    let guardian = &box_record.guardians[guardian_idx];

    // Skip if already updated to viewed status with correct user ID
    if guardian.status == "viewed" && guardian.id == user_id {
        log::info!(
            "Guardian already updated, skipping: box_id={}, invitation_id={}, user_id={}",
            box_id,
            invitation_id,
            user_id
        );
        return Ok(());
    }

    // Only update if the guardian is still in "invited" state
    if guardian.status == "invited" {
        // Make a minimal update - only update this one guardian
        let now = chrono::Utc::now().to_rfc3339();
        box_record.guardians[guardian_idx].id = user_id.to_string();
        box_record.guardians[guardian_idx].status = "viewed".to_string();
        box_record.updated_at = now;

        // Don't increment version - let DynamoDB handle optimistic locking
        // Just use the version from the box we retrieved

        // Update using the store's update_box method
        match store.update_box(box_record).await {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!(
                    "Failed to update guardian: box_id={}, invitation_id={}, error={:?}",
                    box_id,
                    invitation_id,
                    e
                );
                Err(anyhow::anyhow!(e))
            }
        }
    } else {
        // Guardian is already in a different state, log and consider successful
        log::info!(
            "Guardian already in state {}, not updating: box_id={}, invitation_id={}, user_id={}",
            guardian.status,
            box_id,
            invitation_id,
            user_id
        );
        Ok(())
    }
}
