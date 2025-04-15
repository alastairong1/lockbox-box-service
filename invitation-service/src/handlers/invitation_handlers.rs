use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

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
    Json(request): Json<CreateInvitationRequest>,
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
        invited_name: request.invited_name,
        box_id: request.box_id,
        created_at,
        expires_at,
        opened: false,
        linked_user_id: None,
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
    invitation.linked_user_id = Some(request.user_id);

    // Save the updated invitation
    store
        .update_invitation(invitation)
        .await
        .map_err(|e| map_dynamo_error("update_invitation", e))?;

    // Return simple success message
    let response = MessageResponse {
        message: "User successfully connected to invitation".to_string(),
    };

    Ok(Json(response))
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
