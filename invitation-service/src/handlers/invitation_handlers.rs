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
    error::{map_dynamo_error, Result},
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
pub async fn create_invitation<S: InvitationStore>(
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
pub async fn handle_invitation<S: InvitationStore>(
    State(store): State<Arc<S>>,
    Json(request): Json<ConnectToUserRequest>,
) -> Result<Json<MessageResponse>> {
    // Fetch the invitation by code
    let mut invitation = store
        .get_invitation_by_code(&request.invite_code)
        .await
        .map_err(|e| map_dynamo_error("get_invitation_by_code", e))?;

    // Set as opened and connect to user
    invitation.opened = true;
    invitation.linked_user_id = Some(request.user_id.clone());

    // Save the updated invitation
    let updated_invitation = store
        .update_invitation(invitation.clone())
        .await
        .map_err(|e| map_dynamo_error("update_invitation", e))?;

    // Publish event to SNS
    publish_invitation_accepted_event(&updated_invitation).await?;

    // Return response with box_id to help frontend
    let response = MessageResponse {
        message: format!("User successfully bound to invitation for box {}", updated_invitation.box_id),
        box_id: Some(updated_invitation.box_id),
    };

    Ok(Json(response))
}

// Helper function to publish an invitation accepted event to SNS
async fn publish_invitation_accepted_event(invitation: &Invitation) -> Result<()> {
    // Get SNS topic ARN from environment variable
    let topic_arn = env::var("SNS_TOPIC_ARN").map_err(|_| {
        map_dynamo_error(
            "publish_invitation_accepted_event", 
            anyhow::anyhow!("SNS_TOPIC_ARN environment variable not set")
        )
    })?;

    // Create SNS client
    let config = aws_config::load_from_env().await;
    let sns_client = SnsClient::new(&config);

    // Create the event payload
    let event_payload = json!({
        "event_type": "invitation_accepted",
        "invitation_id": invitation.id,
        "box_id": invitation.box_id,
        "user_id": invitation.linked_user_id,
        "invite_code": invitation.invite_code,
        "timestamp": Utc::now().to_rfc3339()
    });

    // Convert to string
    let message = serde_json::to_string(&event_payload).map_err(|e| {
        map_dynamo_error(
            "publish_invitation_accepted_event",
            anyhow::anyhow!("Failed to serialize event payload: {}", e)
        )
    })?;

    // Publish to SNS
    sns_client
        .publish()
        .topic_arn(topic_arn)
        .message(message)
        .subject("Invitation Accepted")
        .send()
        .await
        .map_err(|e| {
            map_dynamo_error(
                "publish_invitation_accepted_event",
                anyhow::anyhow!("Failed to publish to SNS: {}", e)
            )
        })?;

    Ok(())
}

// POST /invitations/:inviteId/refresh - Refresh the invitation
pub async fn refresh_invitation<S: InvitationStore>(
    State(store): State<Arc<S>>,
    Path(invite_id): Path<String>,
) -> Result<Json<InvitationCodeResponse>> {
    // Fetch the invitation
    let mut invitation = store
        .get_invitation(&invite_id)
        .await
        .map_err(|e| map_dynamo_error("get_invitation", e))?;

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
        .await
        .map_err(|e| map_dynamo_error("update_invitation", e))?;

    // Return minimal response with just the new code and expiry
    let response = InvitationCodeResponse {
        invite_code: updated_invitation.invite_code,
        expires_at: updated_invitation.expires_at,
    };

    Ok(Json(response))
}

// GET /invitations/me - Get all invitations created by the current user
pub async fn get_my_invitations<S: InvitationStore>(
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
