//! The accounts route group: the account/membership/invitation JSON API.
//!
//! These endpoints (`POST /accounts`, the `.../members` and `.../invitations`
//! trees) speak JSON and return status codes — an unrecognized caller gets a
//! `401`, never a redirect, because the frontend *calls* these rather than
//! browsing to them. This is part of the cookie surface, so [`crate::app`] mounts
//! the group under the first-party-`Origin` (CSRF) layer.
//!
//! The shared write-path helpers ([`require_user`], [`load_account`],
//! [`actor_role`]) live here: they are the reusable auth seam every account write
//! consults, so the authority rule isn't reinvented per handler.
//!
//! References: ZMVP-14 through ZMVP-21, ZMVP-32; DESIGN/Account, DESIGN/Roles.

use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, post},
};
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    did::Did,
    invitation::Invitation,
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::problem::Problem;
use crate::{AppState, SESSION_USER_KEY};

/// The accounts route group: founding, membership (grant/revoke/leave) and the
/// invitation lifecycle (invite/revoke/decline/accept). Every route here is on
/// the cookie surface; the composition root wraps the group with the CSRF
/// [`require_first_party_origin`](super::require_first_party_origin) layer.
pub(crate) fn accounts_router() -> Router<AppState> {
    Router::new()
        .route("/accounts", post(create_account))
        .route(
            "/accounts/{id}/members",
            post(grant_role).delete(revoke_role),
        )
        .route("/accounts/{id}/members/me", delete(leave_account))
        .route(
            "/accounts/{id}/invitations",
            post(invite_user_to_account).delete(revoke_invitation_to_account),
        )
        .route(
            "/accounts/{id}/invitations/decline",
            post(decline_invitation),
        )
        .route("/accounts/{id}/invitations/accept", post(accept_invitation))
}

/// Resolve the session to the acting [`User`], or `401` — the shared opening of
/// every JSON write endpoint. Both an absent/unreadable session and a vanished User
/// are "not authenticated": these endpoints are called by the frontend, so an
/// unrecognized caller gets a problem+json `401`, never a redirect.
async fn require_user(state: &AppState, session: &Session) -> Result<User, Problem> {
    let id = session
        .get::<Uuid>(SESSION_USER_KEY)
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)?;
    state
        .users
        .find(UserId::new(id))
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)
}

/// Load a live account by id, or `404` — a soft-deleted/unknown id has nothing to
/// act on. A store error becomes a `500` via the `?`/`From<anyhow::Error>` seam.
async fn load_account(state: &AppState, id: AccountId) -> Result<Account, Problem> {
    state
        .accounts
        .find(id)
        .await?
        .ok_or_else(Problem::account_not_found)
}

/// The actor's role in an account, or `403` — a non-member has no authority. The
/// authority *floor* shared by grant/revoke/invite (DESIGN/Roles); the per-action
/// rank rule (`Role::can_grant`) is the caller's. A store error is a `500`.
async fn actor_role(state: &AppState, user: UserId, account: AccountId) -> Result<Role, Problem> {
    state
        .accounts
        .role_of(user, account)
        .await?
        .ok_or_else(Problem::forbidden)
}

/// `200 OK` carrying a bare JSON resource body (success bodies are not enveloped;
/// see the RFC 9457 response-shape decision).
fn ok_json(body: serde_json::Value) -> Response {
    (StatusCode::OK, Json(body)).into_response()
}

/// `201 Created` carrying a bare JSON resource body.
fn created_json(body: serde_json::Value) -> Response {
    (StatusCode::CREATED, Json(body)).into_response()
}

/// The body of `POST /accounts`. Founding takes real input, not a bare click.
///
/// Example: `{ "name": "Acme Studio" }`.
#[derive(Deserialize)]
struct CreateAccountBody {
    name: String,
}

/// Founds a new Account for the signed-in visitor and makes them its Owner
/// (ZMVP-14: "User creates an Account and becomes its Owner"). Onboarding
/// *sequencing* — when to prompt, how to nudge a user who has none — is a frontend
/// concern; this endpoint is the capability the frontend calls. An account is a
/// sovereign entity, so founding first mints the account's own `did:plc` (the floor
/// `StubDidMinter`; the real PLC directory write lands later as an adapter swap).
/// That mint is kept off the sign-in critical path precisely because it is a
/// fallible network step. The account and the founder's Owner membership are then
/// persisted together in one private-store transaction — never a cross-store dual
/// write. Per DESIGN/Account a user may own several accounts, so this founds a fresh
/// one on every call rather than being idempotent.
///
/// The caller must supply a name (the anti-spam gate). Examples:
/// - `{ "name": "Acme Studio" }` → `201 { "id", "did", "name" }`
/// - `{ "name": "   " }` or no body → `422` problem+json (`invalid_request` /
///   `name_required`), nothing minted
async fn create_account(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateAccountBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // Founding is a write, so it requires a recognized visitor (DESIGN/Account: "a
    // user without any accounts must create one before any write").
    let user = require_user(&state, &session).await?;

    // A missing/malformed body, or a name that fails validation, is rejected before
    // anything is minted. Both map to 422 — the request was understood but unusable.
    let Json(body) = body.map_err(|_| Problem::name_required())?;
    let name =
        AccountName::try_new(body.name).map_err(|err| Problem::invalid_request(err.to_string()))?;

    // Mint the account's sovereign DID before touching the private store. A mint
    // failure (the real adapter writes to the PLC directory) aborts with nothing
    // persisted; the client may retry.
    let did = state.did_minter.mint().await.map_err(|_| {
        Problem::service_unavailable(
            "We couldn't mint an identity for the account. Please try again.",
        )
    })?;

    // The founding invariant: the account and the creator's Owner membership are
    // minted together (`Account::open`) and persisted atomically.
    let (account, owner) = Account::open(user.id, did, name, chrono::Utc::now());
    // One unit of work: the account row and the founder's Owner membership commit
    // together or not at all — reached through the transaction-bound write view.
    let mut uow = state.database.begin().await?;
    uow.accounts().create(&account, &owner).await?;
    uow.commit().await?;

    Ok(created_json(json!({
        "id": account.id.to_string(),
        "did": account.did.as_str(),
        "name": account.name.as_str(),
    })))
}

/// The accept-invitation request body: the invitee's choice of whether this new
/// membership is shown on their public profile (`account_members.listed_on_profile`).
#[derive(Deserialize)]
struct AcceptInvitationBody {
    pub listed_on_profile: bool,
}

/// Accepts a pending invitation (ZMVP-20): the invited User takes up their own offer
/// and becomes a member. Symmetric with [`decline_invitation`] — keyed by the session
/// User, not a DID in the body, so authority is implicit: we only ever look up the
/// signed-in User's own pending invitation, so no one can accept another's.
///
/// Flipping the offer to accepted and seating the member (with `parent = inviter`,
/// DESIGN/Roles rule 4a, and the body's `listed_on_profile` choice) happen in one
/// private-store transaction inside [`AccountWrites::accept_invitation`](domain::ports::AccountWrites::accept_invitation); a revoke or
/// accept that wins the race there seats no member ("a revoked invitation yields no
/// membership"). With no pending offer to accept this is a `404`
/// (`no_pending_invitation`); a malformed body is a `400`.
async fn accept_invitation(
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    session: Session,
    body: Result<Json<AcceptInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed JSON"))?;
    let invited_user = require_user(&state, &session).await?;

    // The invitee accepts their own offer; an absent/spent one is a 404.
    let invitation = state
        .accounts
        .find_pending_invitation(AccountId::new(account_id), invited_user.id)
        .await?
        .ok_or_else(Problem::no_pending_invitation)?;

    // Flip the offer to accepted and seat the member in one transaction; a revoke
    // or accept that wins the race inside the write view seats no member.
    let mut uow = state.database.begin().await?;
    let accepted = uow
        .accounts()
        .accept_invitation(invitation, body.listed_on_profile)
        .await?;
    uow.commit().await?;

    Ok(ok_json(json!({
        "account": accepted.account_id.to_string(),
        "user": invited_user.did.as_str(),
        "role": accepted.role.as_str(),
    })))
}

/// `DELETE /accounts/{id}/members/me` — the signed-in member leaves the account on
/// their own action (ZMVP-21). Self-removal needs no authority check (you're acting
/// on your own membership), but two preconditions gate it, resolved handler-side like
/// grant/revoke so the outcomes are problem+json rather than `500`s: you must be a
/// member (else `404`), and the `Owner` can't walk out while still `Owner` (`409` —
/// the sole-Owner root has nowhere to re-home its members; transfer or delete first).
/// On success the repo re-homes the leaver's children, deletes the membership, and
/// revokes the leaver's pending issued invitations in one transaction, and we return
/// `204 No Content`.
async fn leave_account(
    State(state): State<AppState>,
    Path(account_id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let leaving_user = require_user(&state, &session).await?;
    let account = AccountId::new(account_id);

    match state.accounts.role_of(leaving_user.id, account).await? {
        None => return Err(Problem::member_not_found()),
        Some(Role::Owner(_)) => return Err(Problem::owner_cannot_leave()),
        Some(_) => {}
    }

    // Re-home the leaver's children, delete the membership, and revoke their
    // pending issued invitations — atomically, on the transaction-bound view.
    let mut uow = state.database.begin().await?;
    uow.accounts().leave(leaving_user.id, account).await?;
    uow.commit().await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// The body of `POST /accounts/{id}/members`. The grantee is named by their public
/// `did` (identity precedes us — we recognize by DID, never by our internal id),
/// and `role` is the discriminant to grant: `"admin" | "manager" | "member"`.
/// `"owner"` is understood but never grantable through this seam.
///
/// Example: `{ "user": "did:plc:abc123", "role": "admin" }`.
#[derive(Deserialize)]
struct GrantRoleBody {
    user: String,
    role: String,
}

/// Grants a role to a user on an account, seating them as a member if they aren't
/// one yet (ZMVP-15: "Owner grants a role on their Account" — on this platform a
/// grant *is* how a user joins, DESIGN/Roles). This is the seam where reusable role
/// checks are born: the authority decision lives in `Role::can_grant`, so every
/// later role-gated action consults the same rule rather than reinventing it.
///
/// The floor enforces only what DESIGN/Roles settles for now — an Owner may grant
/// Admin/Manager/Member, and Owner is never grantable here (transfer is its own
/// seam). The richer rules (Admin granting up to its rank, the parent/child tree)
/// are deferred dressing and intentionally absent.
///
/// Outcomes:
/// - `200 { "account", "user", "role" }` — the grant settled (created or changed)
/// - `401` — not signed in
/// - `403` — signed in but not allowed to grant that role
/// - `404` — no such account
/// - `422` — malformed body or an unknown role discriminant
async fn grant_role(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
    body: Result<Json<GrantRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // Granting is a write, so it requires a recognized visitor — the actor whose
    // authority we are about to check.
    let actor = require_user(&state, &session).await?;

    // A missing/malformed body, or a role string that isn't one of the four known
    // discriminants, is rejected before anything is touched — understood but unusable.
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a member to grant, e.g. {\"user\": \"did:plc:…\", \"role\": \"admin\"}.",
        )
    })?;
    let new_role =
        Role::try_from(body.role).map_err(|err| Problem::unknown_role(err.to_string()))?;

    // The grant must address a real, live account. A soft-deleted or unknown id is
    // a 404 — there's nothing to act on — kept distinct from "you may not" (403).
    let account = load_account(&state, AccountId::new(account_id)).await?;

    // Authorization, at the seam: the actor's standing in *this* account decides
    // whether the grant is allowed. A non-member has no role and so no authority.
    let actor_role = actor_role(&state, actor.id, account.id).await?;
    if !actor_role.can_grant(&new_role) {
        return Err(Problem::forbidden());
    }

    // Recognize the grantee by their DID (idempotent — mints them on first contact,
    // returns the existing User otherwise). Granting a role to someone who has never
    // signed in is how an Owner adds them; they resolve to the same User when they do.
    // Recognition is its own unit of work, settled before the grant (as before the
    // Unit-of-Work refactor): an idempotent recognize, independent of the grant.
    let mut uow = state.database.begin().await?;
    let grantee = uow.users().provision(&Did::new(body.user)).await?;
    uow.commit().await?;

    // The guard above bounds the role being *granted*; this bounds the *grantee*.
    // An account's Owner is never demoted through a grant — ownership only moves via
    // the separate transfer seam ("an Owner never has a parent, even when
    // transferred", DESIGN/Roles). Without this, an Admin could grant Manager to the
    // Owner's DID and quietly unseat them.
    if let Some(Role::Owner(_)) = state.accounts.role_of(grantee.id, account.id).await? {
        return Err(Problem::forbidden());
    }

    // Settle the grant: upsert the membership in the private store.
    let member = UserAccount {
        user_id: grantee.id,
        account_id: account.id,
        role: new_role,
    };
    let mut uow = state.database.begin().await?;
    uow.accounts().grant_role(&member).await?;
    uow.commit().await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": grantee.did.as_str(),
        "role": member.role.as_str(),
    })))
}

/// The body of `DELETE /accounts/{id}/members`. The member to revoke is named by
/// their public `did` — the same identity convention as the grant. No role: a
/// revoke removes the membership whatever role it holds.
///
/// Example: `{ "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct RevokeRoleBody {
    user: String,
}

/// Revokes a user's role on an account — removes their membership, the inverse of
/// `grant_role` (ZMVP-16). Authorization reuses the same seam: an actor may revoke a
/// member only if `can_grant` would let them act on that member's *current* rank — so
/// an Owner revokes Admin/Manager/Member, an Admin revokes Manager/Member (never a
/// peer Admin), and an Owner is never revocable here. That last point keeps a sole
/// Owner safe for free: ownership only leaves via the separate transfer seam.
///
/// Outcomes:
/// - `200 { "account", "user" }` — the member was revoked
/// - `401` — not signed in
/// - `403` — signed in but not allowed to revoke that member
/// - `404` — no such account, or the user is not a member of it
/// - `422` — malformed body
async fn revoke_role(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
    body: Result<Json<RevokeRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // Revoking is a write — it requires a recognized visitor, the acting authority.
    let actor = require_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide a member to revoke, e.g. {\"user\": \"did:plc:…\"}.")
    })?;

    // The revoke must address a real, live account.
    let account = load_account(&state, AccountId::new(account_id)).await?;

    // The actor's standing in this account decides what they may do; a non-member
    // has none.
    let actor_role = actor_role(&state, actor.id, account.id).await?;

    // Resolve the target by DID *without minting* — unlike a grant, a revoke must not
    // recognize a brand-new visitor as a side effect. An unknown DID is not a member.
    let target = state
        .users
        .find_by_did(&Did::new(body.user))
        .await?
        .ok_or_else(Problem::member_not_found)?;

    // The member's *current* rank is what the actor must be allowed to act on — the
    // same predicate as grant. An Owner outranks everyone, so they're never revocable
    // here; an Admin can't revoke a peer Admin. Someone with no role isn't a member.
    let target_role = state
        .accounts
        .role_of(target.id, account.id)
        .await?
        .ok_or_else(Problem::member_not_found)?;
    if !actor_role.can_grant(&target_role) {
        return Err(Problem::forbidden());
    }

    // Settle the revoke: remove the membership (and re-home children + revoke the
    // member's pending issued invitations) atomically, on the write view.
    let mut uow = state.database.begin().await?;
    uow.accounts().revoke_role(target.id, account.id).await?;
    uow.commit().await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": target.did.as_str(),
    })))
}

/// The body of `POST /accounts/{id}/invitations`. The invitee is named by their
/// public `did` (identity precedes us — we recognize by DID, never by our internal
/// id), and `role` is the discriminant to offer: `"admin" | "manager" | "member"`.
/// `"owner"` is understood but never offerable by invitation (that would be a
/// transfer, not an invite).
///
/// Example: `{ "user": "did:plc:abc123", "role": "member" }`.
#[derive(Deserialize)]
struct InviteUserToAccountBody {
    user: String,
    role: String,
}

/// Issues a pending invitation for a User to join an account (ZMVP-32 — the
/// issuing half of invite-then-accept; acceptance is ZMVP-20). Authority reuses the
/// grant rule: only an Owner/Admin may invite, and the offered role must sit
/// strictly below the inviter's own rank (`Role::can_grant`) — the same seam as
/// [`grant_role`].
///
/// The invitee is provisioned by DID (idempotent, like a grant) so the offer can
/// reference a real `UserId` even for someone who has never visited. Inviting an
/// existing member is a `409` (there's nothing to invite them to); re-inviting an
/// already-pending User is idempotent — the existing offer is returned (`200`),
/// never a second row (handler check plus the partial-unique-index backstop).
/// Otherwise a fresh pending offer is created (`201`).
async fn invite_user_to_account(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
    body: Result<Json<InviteUserToAccountBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let actor = require_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a user to invite and a role, e.g. {\"user\": \"did:plc:…\", \"role\": \"member\"}.",
        )
    })?;
    let role = Role::try_from(body.role).map_err(|err| Problem::unknown_role(err.to_string()))?;

    let account = load_account(&state, AccountId::new(account_id)).await?;
    let inviting_user_role = actor_role(&state, actor.id, account.id).await?;

    if !inviting_user_role.can_grant(&role) {
        return Err(Problem::forbidden());
    }

    // Recognize the invitee (idempotent), its own unit of work — settled before
    // the offer is issued, as before the Unit-of-Work refactor.
    let mut uow = state.database.begin().await?;
    let invited = uow.users().provision(&Did::new(body.user)).await?;
    uow.commit().await?;

    // An invitation is the path *to* membership; someone who already holds a role has
    // nowhere to be invited. This is a state conflict (409), not an authority failure
    // (403) or a malformed request (422) — the actor may invite, just not this person.
    if state
        .accounts
        .role_of(invited.id, account.id)
        .await?
        .is_some()
    {
        return Err(Problem::already_member(
            "That user is already a member of this account.",
        ));
    }

    // Idempotent re-invite: an existing pending offer is returned, not a second row.
    if let Some(existing_invitation) = state
        .accounts
        .find_pending_invitation(account.id, invited.id)
        .await?
    {
        return Ok(ok_json(json!({
            "id": existing_invitation.id.to_string(),
            "account": account.id.to_string(),
            "user": invited.did.as_str(),
            "role": existing_invitation.role.as_str(),
            "state": existing_invitation.state.as_str()
        })));
    }

    let invitation = Invitation::issue(account.id, invited.id, role, actor.id, Utc::now());
    let mut uow = state.database.begin().await?;
    uow.accounts().create_invitation(&invitation).await?;
    uow.commit().await?;

    Ok(created_json(json!({
        "id": invitation.id.to_string(),
        "account": account.id.to_string(),
        "user": invited.did.as_str(),
        "role": invitation.role.as_str(),
        "state": invitation.state.as_str()
    })))
}

/// The body of `DELETE /accounts/{id}/invitations`. The invitation is addressed by
/// the invited User's `did` (not an invitation id): there is at most one pending
/// offer per (account, user), so the pair identifies it — keeping revoke symmetric
/// with issue and with [`revoke_role`].
///
/// Example: `{ "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct RevokeInvitationBody {
    user: String,
}

/// Revokes a pending invitation so it can no longer be accepted (ZMVP-32). The
/// invited User is named by DID in the body and resolved *without minting* (like
/// [`revoke_role`], a revoke must not recognize a brand-new visitor as a side
/// effect). Authority is the issuing seam again — the actor must be able to
/// `can_grant` the offered role.
///
/// Idempotent: an unknown DID, or no pending offer, is a `200` no-op rather than a
/// 404. Every path — success or no-op — echoes `{ account, user }` (the
/// always-available request inputs), since the no-op paths have no invitation row
/// to report an id or state from.
async fn revoke_invitation_to_account(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
    body: Result<Json<RevokeInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let actor = require_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the invited user to revoke, e.g. {\"user\": \"did:plc:…\"}.",
        )
    })?;
    // Keep the invited DID by value: the response echoes it on every path (mirroring
    // `revoke_role`), including the idempotent no-ops where no invitation row — and so
    // no id/state — is available to report.
    let invited_did = body.user;

    let account = load_account(&state, AccountId::new(account_id)).await?;

    // The actor's standing in this account decides what they may do; a non-member has
    // none. We keep the role to apply the authority rule once the invitation is loaded.
    let actor_role = actor_role(&state, actor.id, account.id).await?;

    // Resolve the invited user by DID *without minting* — like revoke_role, a revoke
    // must not recognize a brand-new visitor as a side effect. An unknown DID was never
    // invited, so there is nothing pending to revoke (idempotent no-op).
    let revoked = || {
        (
            StatusCode::OK,
            Json(json!({
                "account": account.id.to_string(),
                "user": invited_did.as_str(),
            })),
        )
            .into_response()
    };

    let invited_user = match state
        .users
        .find_by_did(&Did::new(invited_did.clone()))
        .await?
    {
        Some(user) => user,
        None => return Ok(revoked()),
    };

    let mut invitation = match state
        .accounts
        .find_pending_invitation(account.id, invited_user.id)
        .await?
    {
        Some(invitation) => invitation,
        None => return Ok(revoked()),
    };

    // Authority, the same seam as issuing/granting: an actor may revoke only an
    // invitation they could have issued — Owner/Admin, the offered role strictly below
    // their own rank (`can_grant`). A non-member was already turned away above.
    if !actor_role.can_grant(&invitation.role) {
        return Err(Problem::forbidden());
    }

    // Run the domain transition first as a guard — it enforces the pending → revoked
    // rule (the offer is pending by construction here, the lookup filtered on state),
    // keeping the state machine the single arbiter of legality — then persist it.
    if invitation.revoke(Utc::now()).is_err() {
        return Err(Problem::internal_error(
            "Could not revoke invitation. Please try again.",
        ));
    }
    let mut uow = state.database.begin().await?;
    uow.accounts().revoke_invitation(invitation.id).await?;
    uow.commit().await?;

    Ok(revoked())
}

/// Declines a pending invitation (ZMVP-20). The invitee actively kills their *own*
/// offer — symmetric with the issuer's revoke, but keyed by the session User rather
/// than a DID in the body, so authority is implicit: we only ever look up the
/// signed-in User's own pending invitation. Reuses the `pending → revoked`
/// transition (a declined offer is a dead offer; re-invite stays possible).
///
/// Declining when there is no pending offer is a `404` (`no_pending_invitation`) —
/// there is nothing for this User to decline.
async fn decline_invitation(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
) -> Result<Response, Problem> {
    let actor = require_user(&state, &session).await?;
    let account = load_account(&state, AccountId::new(account_id)).await?;

    // The invitee declines their own offer; an absent/spent one is a 404.
    let mut invitation = state
        .accounts
        .find_pending_invitation(account.id, actor.id)
        .await?
        .ok_or_else(Problem::no_pending_invitation)?;

    // Reuse the pending → revoked transition (pending by construction here), then
    // persist it — exactly the issuer-revoke path, just initiated by the invitee.
    if invitation.revoke(Utc::now()).is_err() {
        return Err(Problem::internal_error(
            "Could not decline the invitation. Please try again.",
        ));
    }
    let mut uow = state.database.begin().await?;
    uow.accounts().revoke_invitation(invitation.id).await?;
    uow.commit().await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": actor.did.as_str(),
    })))
}
