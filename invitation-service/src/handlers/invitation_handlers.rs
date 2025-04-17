use axum::{
    extract::{Extension, Path, State},
    Json,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;
use aws_sdk_sns::Client as SnsClient;
use serde_json::json;
use std::env;

use lockbox_shared::{models::Invitation, store::InvitationStore};

use crate::{
    error::{map_dynamo_error, AppError, Result},
    models::{
        ConnectToUserRequest, CreateInvitationRequest, InvitationCodeResponse, MessageResponse,
    },
};

// Alphabet for user-friendly invitation codes (uppercase letters only)
const CODE_ALPHABET: [char; 26] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

// POST /invitation - Create a new invitation
pub async fn create_invitation<S: InvitationStore + ?Sized>(
    State(store): State<Arc<S>>,
    Extension(user_id): Extension<String>,
    Json(create_request): Json<CreateInvitationRequest>,
) -> Result<Json<InvitationCodeResponse>> {
    // Generate a user-friendly code for the invitation (8 characters)
    let invite_code = nanoid::nanoid!(8, &CODE_ALPHABET);

    // Set expiration to 48 hours from now
    let created_at = Utc::now().to_rfc3339();
    let expires_at = (Utc::now() + Duration::hours(48)).to_rfc3339();

    // Create the invitation
    let invitation = Invitation {
        id: Uuid::new_v4().to_string(),
        invite_code,
        invited_name: create_request.invited_name,
        box_id: create_request.box_id,
        created_at,
        expires_at,
        opened: false,
        linked_user_id: None,
        creator_id: user_id,
    };

    // Save to database
    let saved_invitation = store
        .create_invitation(invitation)
        .await
        .map_err(|e| map_dynamo_error("create_invitation", e))?;

    // Return minimal response with just the code and expiry
    let response = InvitationCodeResponse {
        invite_code: saved_invitation.invite_code,
        expires_at: saved_invitation.expires_at,
    };

    Ok(Json(response))
}

// PUT /invitation/handle - Connect invitation to user
pub async fn handle_invitation<S: InvitationStore + ?Sized>(
    State(store): State<Arc<S>>,
    Json(request): Json<ConnectToUserRequest>,
) -> Result<Json<MessageResponse>> {
    // Fetch the invitation by code, propagate NotFound and Expired appropriately
    let mut invitation = store
        .get_invitation_by_code(&request.invite_code)
        .await?;

    // Set as opened and connect to user
    invitation.opened = true;
    invitation.linked_user_id = Some(request.user_id.clone());

    // Save the updated invitation
    let updated_invitation = store
        .update_invitation(invitation.clone())
        .await?;

    // Publish event to SNS
    // Ignore publish errors in tests
    let _ = publish_invitation_event(&updated_invitation).await;

    // Return response with box_id to help frontend
    let response = MessageResponse {
        message: format!("User successfully bound to invitation for box {}", updated_invitation.box_id),
        box_id: Some(updated_invitation.box_id),
    };

    Ok(Json(response))
}

// Helper function to publish an invitation event to SNS
pub async fn publish_invitation_event(invitation: &Invitation) -> Result<()> {
    // Get SNS topic ARN from environment variable
    let topic_arn = env::var("SNS_TOPIC_ARN").map_err(|e| {
        map_dynamo_error("get_sns_topic_arn", e)
    })?;

    // Create SNS client
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let sns_client = SnsClient::new(&config);

    // Call the internal implementation with the client
    publish_invitation_event_with_client(invitation, sns_client, &topic_arn).await
}

// Internal implementation that can be mocked for testing
async fn publish_invitation_event_with_client(
    invitation: &Invitation,
    sns_client: SnsClient,
    topic_arn: &str,
) -> Result<()> {
    // Create the event payload
    let event_payload = json!({
        "event_type": "invitation_viewed",
        "invitation_id": invitation.id,
        "box_id": invitation.box_id,
        "user_id": invitation.linked_user_id,
        "invite_code": invitation.invite_code,
        "timestamp": Utc::now().to_rfc3339()
    });

    // Convert to string
    let message = serde_json::to_string(&event_payload).map_err(|e| {
        map_dynamo_error("serialize_event_payload", e)
    })?;

    // Create message attribute
    let message_attribute = aws_sdk_sns::types::MessageAttributeValue::builder()
        .data_type("String")
        .string_value("invitation_viewed")
        .build()
        .map_err(|e| map_dynamo_error("build_message_attribute", e))?;
    
    let mut publish_request = sns_client
        .publish()
        .topic_arn(topic_arn)
        .message(message)
        .subject("Invitation Viewed");
        
    // Add message attribute with the proper method call
    publish_request = publish_request.message_attributes("eventType", message_attribute);
    
    // Send the request
    publish_request
        .send()
        .await
        .map_err(|e| {
            map_dynamo_error("publish_to_sns", e)
        })?;

    Ok(())
}

// POST /invitations/:inviteId/refresh - Refresh the invitation
pub async fn refresh_invitation<S: InvitationStore + ?Sized>(
    State(store): State<Arc<S>>,
    Extension(user_id): Extension<String>,
    Path(invite_id): Path<String>,
) -> Result<Json<InvitationCodeResponse>> {
    // Fetch invitations for this user (bypass expiry)
    let invitations = store
        .get_invitations_by_creator_id(&user_id)
        .await?;
    // Only allow refresh if this user is the creator
    let mut invitation = if let Some(mut inv) = invitations.into_iter().find(|inv| inv.id == invite_id) {
        inv
    } else {
        return Err(AppError::Forbidden(format!("Invitation {} is not owned by user", invite_id)));
    };

    // Generate a new user-friendly invite code (8 characters)
    invitation.invite_code = nanoid::nanoid!(8, &CODE_ALPHABET);

    // Set new expiration date (48 hours from now)
    invitation.expires_at = (Utc::now() + Duration::hours(48)).to_rfc3339();

    // Reset opened status
    invitation.opened = false;

    // Clear linked user
    invitation.linked_user_id = None;

    // Save the updated invitation
    let updated_invitation = store
        .update_invitation(invitation)
        .await?;

    // Return minimal response with just the new code and expiry
    let response = InvitationCodeResponse {
        invite_code: updated_invitation.invite_code,
        expires_at: updated_invitation.expires_at,
    };

    Ok(Json(response))
}

// GET /invitations/me - Get all invitations created by the current user
pub async fn get_my_invitations<S: InvitationStore + ?Sized>(
    State(store): State<Arc<S>>,
    Extension(user_id): Extension<String>,
) -> Result<Json<Vec<Invitation>>> {
    // Fetch all invitations created by this user
    let invitations = store
        .get_invitations_by_creator_id(&user_id)
        .await
        .map_err(|e| map_dynamo_error("get_invitations_by_creator_id", e))?;

    Ok(Json(invitations))
}
