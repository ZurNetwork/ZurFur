//! `POST`/`DELETE /commissions/{id}/invitations` — the owner invites a User to a
//! vacant Seat, or revokes a pending offer (ZMVP-78; DESIGN/Commission,
//! Referenceable/Slot/Seat DD `28311564`).
//!
//! The Seat mirror of the account invitation seam (`routes/accounts.rs`): a Seat
//! is a structural participant position declared vacant (ZMVP-76), and filling
//! one is consensual — the owner *issues* a pending offer here, and the invited
//! User *accepts* to occupy it (ZMVP-79, out of this ticket's scope). This module
//! covers **issue + revoke only**; accept/decline and applications are later
//! tickets.
//!
//! Owner-only via the shared [`require_owner`] gate (the one managing-authority
//! path; ZMVP-83 activates its Admin arm), so a non-participant — and a truly
//! absent commission — gets the uniform
//! [`commission_not_found`](Problem::commission_not_found) 404, never a 403 (no
//! existence oracle). RFC 9457 throughout; success bodies are bare resources.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        commission::{CommissionId, ElementId, SeatInvitation},
        did::Did,
        invitation::InvitationState,
    },
    ports::{DidBelongsToAnotherActor, UnitOfWork},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// `POST /commissions/{id}/invitations`'s body (both the idempotent `200`
/// re-invite and the minted `201`): the seat offer, in full — see
/// [`invite_to_seat`].
#[derive(Serialize)]
struct InviteToSeatResponse {
    commission: String,
    id: String,
    seat: String,
    state: &'static str,
    user: String,
}

/// `DELETE /commissions/{id}/invitations`'s `200` body: the always-available
/// request inputs the revoke echoes on every path (success or idempotent
/// no-op) — see [`revoke_seat_invitation`].
#[derive(Serialize)]
struct RevokeSeatInvitationResponse {
    commission: String,
    seat: String,
    user: String,
}

/// The `POST /commissions/{id}/invitations` request body: the `seat` to offer
/// (its element id, from the seat-declaration `201`) and the `user` to invite,
/// named by their public `did` (identity precedes us — we recognize by DID,
/// never by our internal id).
///
/// Example: `{ "seat": "0192…", "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
pub(super) struct InviteToSeatBody {
    seat: Uuid,
    user: String,
}

/// Issue a pending invitation offering a vacant Seat to a User (ZMVP-78 — the
/// issuing half of seat invite-then-accept; acceptance is ZMVP-79). Owner-only.
///
/// The invitee is provisioned by DID (idempotent, like an account invite) so the
/// offer can reference a real `UserId` even for someone who has never visited.
/// Inviting to a seat that isn't one of **this** commission's seats — fabricated,
/// or belonging to another commission — is an
/// [`element_not_found`](Problem::element_not_found) 404 (the seats read is
/// scoped to the commission, so it is no cross-commission oracle). Inviting to an already-occupied seat is a
/// [`seat_filled`](Problem::seat_filled) 409 (a Seat holds at most one occupant,
/// ZMVP-76 AC3). Re-inviting an already-pending User to the same seat is
/// idempotent — the existing offer is returned (`200`), never a second row
/// (handler check plus the partial-unique-index backstop). Otherwise a fresh
/// pending offer is created (`201`). An already-participating User (including the
/// owner) MAY be invited to a *vacant* seat — a seat is structural position, not
/// membership, so there is no already-member check (Engineer ruling 2026-07-16).
///
/// **Golem invitees are satisfied by construction (AC4).** The rejected criterion
/// "a Golem cannot be invited to a Seat" needs no runtime check: an invitee is
/// resolved to a `User` by DID, and per the Identities DD (`34013187`) a seated
/// Golem *is* a User (the actor super-table has no `golem` ActorKind) — there is
/// no representable Golem-but-not-User invitee to reject. When the Golem epic
/// lands its own kind, that is the change that adds the guard; today it is
/// unreachable, so no dead code stands in for it.
pub(super) async fn invite_to_seat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<InviteToSeatBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    // Owner-only (the closed door hands a non-participant the uniform 404).
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a seat and a user to invite, e.g. {\"seat\": \"…\", \"user\": \"did:plc:…\"}.",
        )
    })?;
    let seat_id = body.seat;
    let seat = ElementId::new(seat_id);

    // The seat must be one of THIS commission's declared seats — a fabricated or
    // cross-commission id is `element_not_found` (the seats read is commission-scoped,
    // so it can't confirm a foreign seat) — and it must be vacant.
    let seats = state.commissions.seats(commission).await?;
    let target = seats
        .iter()
        .find(|s| s.id == seat)
        .ok_or_else(Problem::element_not_found)?;
    if target.occupant.is_some() {
        return Err(Problem::seat_filled());
    }

    // Recognize the invitee by DID (idempotent), its own unit of work — settled
    // before the offer is issued, as with the account invite.
    let invited = state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&Did::new(body.user)).await
        })
        .await
        .map_err(|err| match err.downcast_ref::<DidBelongsToAnotherActor>() {
            Some(_) => Problem::did_belongs_to_another_actor(),
            None => Problem::from(err),
        })?;

    // Idempotent re-invite: an existing pending offer to this seat is returned, not
    // a second row.
    if let Some(existing) = state
        .commissions
        .find_pending_seat_invitation(commission, seat, invited.id)
        .await?
    {
        let existing_offer = InviteToSeatResponse {
            commission: id.to_string(),
            id: existing.id.to_string(),
            seat: seat_id.to_string(),
            state: existing.state.as_str(),
            user: invited.did.as_str().to_owned(),
        };
        let response = (StatusCode::OK, Json(existing_offer)).into_response();
        return Ok(response);
    }

    let invitation = SeatInvitation::issue(commission, seat, invited.id, user.id, Utc::now());
    let minted = invitation.id;
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().create_seat_invitation(&invitation).await
        })
        .await?;

    // Answer from the row that actually survives: a duplicate invite racing past
    // the pending check above is dropped by the store (`ON CONFLICT … DO
    // NOTHING`), so the offer on record may be the racer's — report that one as
    // a 200, and claim 201 only when the minted offer is what the table holds.
    // (Residual window: the insert conflicted AND the surviving offer was
    // revoked, all between adjacent statements — then the minted body goes out
    // as created and the very next read shows the offer closed.)
    let (status, offer_id, offer_state) = match state
        .commissions
        .find_pending_seat_invitation(commission, seat, invited.id)
        .await?
    {
        Some(stored) if stored.id == minted => {
            let state = stored.state;
            (StatusCode::CREATED, stored.id, state)
        }
        Some(stored) => {
            let state = stored.state;
            (StatusCode::OK, stored.id, state)
        }
        None => (StatusCode::CREATED, minted, InvitationState::Pending),
    };
    let offer = InviteToSeatResponse {
        commission: id.to_string(),
        id: offer_id.to_string(),
        seat: seat_id.to_string(),
        state: offer_state.as_str(),
        user: invited.did.as_str().to_owned(),
    };
    let response = (status, Json(offer)).into_response();
    Ok(response)
}

/// The `DELETE /commissions/{id}/invitations` request body: the `seat` and the
/// invited User's `did`. There is at most one pending offer per (seat, user), so
/// the pair identifies it — keeping revoke symmetric with issue.
///
/// Example: `{ "seat": "0192…", "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
pub(super) struct RevokeSeatInvitationBody {
    seat: Uuid,
    user: String,
}

/// Revoke a pending seat invitation so it can no longer be accepted (ZMVP-78).
/// Owner-only.
///
/// The invited User is named by DID and resolved *without minting* (like the
/// account revoke, a revoke must not recognize a brand-new visitor as a side
/// effect). Idempotent: an unknown DID, or no pending offer for that
/// (seat, user), is a `200` no-op rather than a 404. Every path — success or
/// no-op — echoes `{ commission, seat, user }` (the always-available request
/// inputs), since the no-op paths have no invitation row to report an id from.
pub(super) async fn revoke_seat_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<RevokeSeatInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the seat and invited user to revoke, e.g. {\"seat\": \"…\", \"user\": \"did:plc:…\"}.",
        )
    })?;
    let seat = ElementId::new(body.seat);
    // Kept by value: the response echoes it on every path (including the idempotent
    // no-ops where no invitation row — and so no id — is available to report).
    let invited_did = body.user;

    let revoked = || {
        let response_body = RevokeSeatInvitationResponse {
            commission: id.to_string(),
            seat: body.seat.to_string(),
            user: invited_did.clone(),
        };
        (StatusCode::OK, Json(response_body)).into_response()
    };

    // Resolve the invited user by DID *without minting*. An unknown DID was never
    // invited, so there is nothing pending to revoke (idempotent no-op).
    let Some(invited_user) = state
        .users
        .find_by_did(&Did::new(invited_did.clone()))
        .await?
    else {
        return Ok(revoked());
    };

    let Some(mut invitation) = state
        .commissions
        .find_pending_seat_invitation(commission, seat, invited_user.id)
        .await?
    else {
        return Ok(revoked());
    };

    // Run the domain transition first as a guard — it enforces the pending →
    // revoked rule (the offer is pending by construction here, the lookup filtered
    // on state), keeping the state machine the single arbiter of legality — then
    // persist it.
    invitation.revoke(Utc::now()).map_err(|_| {
        Problem::internal_error("Could not revoke the invitation. Please try again.")
    })?;
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions()
                .revoke_seat_invitation(invitation.id)
                .await
        })
        .await?;

    Ok(revoked())
}

#[cfg(test)]
mod tests {
    //! Pins the two response bodies' wire shapes to the exact string form the
    //! retired `json!({…})` literals produced — every field a `String` (as the
    //! `.to_string()`/`.as_str()` calls already emitted), so swapping in the
    //! named structs moved no bytes (ZMVP-158 AC1/AC3).

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
