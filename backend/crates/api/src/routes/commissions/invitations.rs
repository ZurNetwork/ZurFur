//! `POST`/`DELETE /commissions/{id}/invitations` — the owner invites a User to
//! a vacant Seat, or revokes a pending offer (issue + revoke only; accept and
//! decline are separate).

use application::commission::invitations::{self, issue::InvitationOutput};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::{
    commission::{CommissionId, element::SeatId},
    user::UserId,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// `POST /commissions/{id}/invitations`'s body: the seat offer, in full — see
/// [`invite_to_seat`].
#[derive(Serialize)]
struct InviteToSeatResponse {
    commission: String,
    id: String,
    seat: String,
    state: &'static str,
    user: String,
}

impl From<InvitationOutput> for InviteToSeatResponse {
    fn from(invitation: InvitationOutput) -> Self {
        Self {
            commission: invitation.commission_id.to_string(),
            id: invitation.invitation_id.to_string(),
            seat: invitation.seat_id.to_string(),
            state: invitation.invitation_state.as_str(),
            user: invitation.invited_user_id.to_string(),
        }
    }
}

/// `DELETE /commissions/{id}/invitations`'s `200` body — see
/// [`revoke_seat_invitation`].
#[derive(Serialize)]
struct RevokeSeatInvitationResponse {
    commission: String,
    seat: String,
    user: String,
}

/// The `POST /commissions/{id}/invitations` request body: the `seat` to offer
/// (its element id) and the `user` to invite, named by their `did`.
#[derive(Deserialize)]
pub(super) struct InviteToSeatBody {
    seat: Uuid,
    user: String,
}

/// Issues a pending invitation offering a vacant seat to a User (by `did`).
/// Owner-only; `404 element_not_found` for a seat not in this commission,
/// `409 seat_filled` for an occupied seat. Idempotent — re-inviting the same
/// pending user returns the existing offer (`200`); otherwise `201 Created`.
pub(super) async fn invite_to_seat(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<InviteToSeatBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a seat and a user to invite, e.g. {\"seat\": \"…\", \"user\": \"did:plc:…\"}.",
        )
    })?;
    let command = invitations::issue::Command {
        commission_id,
        actor_id,
        target_id: body
            .user
            .parse::<UserId>()
            .map_err(|_| Problem::invalid_request("Invalid target user DID"))?,
        seat_id: SeatId::new(body.seat),
    };

    let issued = state
        .app()
        .commissions()
        .invitations()
        .issue(command, Utc::now())
        .await?;

    let (status, offer) = match issued {
        invitations::issue::Output::Created(invitation) => {
            (StatusCode::CREATED, InviteToSeatResponse::from(invitation))
        }
        invitations::issue::Output::PreExisting(invitation) => {
            (StatusCode::OK, InviteToSeatResponse::from(invitation))
        }
    };

    let response = (status, Json(offer)).into_response();
    Ok(response)
}

/// The `DELETE /commissions/{id}/invitations` request body: the `seat` and
/// the invited User's `did` identify the pending offer.
#[derive(Deserialize)]
pub(super) struct RevokeSeatInvitationBody {
    seat: Uuid,
    user: String,
}

/// Revokes a pending seat invitation so it can no longer be accepted.
/// Owner-only and idempotent — an unknown DID or no pending offer is a `200`
/// no-op. Every path echoes `{ commission, seat, user }`.
pub(super) async fn revoke_seat_invitation(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<RevokeSeatInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the seat and invited user to revoke, e.g. {\"seat\": \"…\", \"user\": \"did:plc:…\"}.",
        )
    })?;
    let command = invitations::revoke::Command {
        commission_id,
        target_id: body
            .user
            .parse::<UserId>()
            .map_err(|_| Problem::invalid_request("Invalid request"))?,
        actor_id,
        seat_id: SeatId::new(body.seat),
    };

    let output = state
        .app()
        .commissions()
        .invitations()
        .revoke(command, Utc::now())
        .await?;

    let response = RevokeSeatInvitationResponse {
        seat: output.seat_id.to_string(),
        commission: output.commission_id.to_string(),
        user: output.user_id.to_string(),
    };

    Ok((StatusCode::OK, Json(response)).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins the two response bodies' wire shapes.

    use super::*;

    #[test]
    fn invite_to_seat_response_serializes_every_field_as_a_string() {
        let body = InviteToSeatResponse {
            commission: "commission-id".to_string(),
            id: "offer-id".to_string(),
            seat: "seat-id".to_string(),
            state: "pending",
            user: "did:plc:invitee".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"commission":"commission-id","id":"offer-id","seat":"seat-id","state":"pending","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn revoke_seat_invitation_response_serializes_every_field_as_a_string() {
        let body = RevokeSeatInvitationResponse {
            commission: "commission-id".to_string(),
            seat: "seat-id".to_string(),
            user: "did:plc:invitee".to_string(),
        };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"commission":"commission-id","seat":"seat-id","user":"did:plc:invitee"}"#
        );
    }
}
