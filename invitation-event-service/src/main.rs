use aws_lambda_events::event::sns::SnsEvent;
use env_logger;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use lockbox_shared::models::events::InvitationEvent;
use lockbox_shared::store::{dynamo::DynamoBoxStore, BoxStore};
use log::{error, info};
use std::sync::Arc;

// Import the handlers module
mod handlers;
// Add the errors module
mod errors;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize env_logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Logging initialized with env_logger");

    info!("Starting Box Invitation Handler Lambda");

    // Create the DynamoDB Box Store
    let dynamo_store = Arc::new(DynamoBoxStore::new().await);

    // Run the Lambda service function with the store
    lambda_runtime::run(service_fn(|event| handler(event, dynamo_store.clone()))).await?;
    Ok(())
}

// Lambda handler function - make this public for testing
pub async fn handler<S>(event: LambdaEvent<SnsEvent>, store: Arc<S>) -> Result<(), Error>
where
    S: BoxStore + Send + Sync + 'static,
{
    // Get the SNS event
    let sns_event = event.payload;

    // Process each record (message) in the SNS event
    for record in sns_event.records {
        // Extract and parse the SNS message
        let message = record.sns;

        // Try to parse the message as an InvitationEvent
        if let Ok(invitation_event) = serde_json::from_str::<InvitationEvent>(&message.message) {
            match invitation_event.event_type.as_str() {
                "invitation_created" => {
                    handlers::handle_invitation_created(store.clone(), &invitation_event).await?
                }
                "invitation_viewed" => {
                    handlers::handle_invitation_opened(store.clone(), &invitation_event).await?
                }
                _ => {
                    error!("Unknown event type: {}", invitation_event.event_type);
                }
            }
        } else {
            error!("Failed to parse SNS message: {}", message.message);
            // Continue processing remaining records; rely on SNS DLQ for this one
            continue;
        }
    }

    Ok(())
}
