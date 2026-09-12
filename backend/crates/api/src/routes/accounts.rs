//! HTTP driver for the account routes over `application::account` —
//! founding, membership, and the invitation lifecycle. Mounted under the
//! first-party-`Origin` (CSRF) layer.

use application::account::{
    self, AccountEntity, AccountError,
    invitation::{self, issue::InviteOutcome},
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use chrono::Utc;
use domain::elements::{
    account::{AccountId, AccountName},
    handle::Handle,
    role::Role,
    user::UserId,
};
use serde::{Deserialize, Serialize};

use crate::problem::Problem;
use crate::{
    AppState,
    extract::CallingUser,
    generated::{
        AccountMembership, ChangeHandleRequest, ChangeHandleResponse, CreateAccountRequest,
        CreateAccountResponse, DeleteAccountResponse, ListAccountsResponse,
    },
};

/// Maps an account use-case error onto the wire problem — the mapping every
/// account handler shares.
impl From<AccountError> for Problem {
    fn from(err: AccountError) -> Self {
        match err {
            AccountError::Infrastructure(err) => Problem::from(err),
            AccountError::HandleTaken => Problem::handle_taken(),
            AccountError::UnsupportedHandle => Problem::unsupported_handle(
                "Changing to a handle outside the Zurfur namespace isn't supported yet.",
            ),
            AccountError::HandleUnchanged => {
                Problem::invalid_request("That is already the account's handle.")
            }
            AccountError::RenamedTooRecently => Problem::rate_limited(
                "Too many handle changes recently. Please wait before changing it again.",
            ),
            AccountError::IncorrectRole => Problem::forbidden(),
            AccountError::NotFound(AccountEntity::Account) => Problem::account_not_found(),
            AccountError::NotFound(AccountEntity::Commission) => Problem::commission_not_found(),
            AccountError::NotFound(AccountEntity::Workflow) => Problem::workflow_not_found(),
            AccountError::NotFound(AccountEntity::Column) => Problem::column_not_found(),
            AccountError::NoPendingInvitation => Problem::no_pending_invitation(),
            AccountError::NotAMember => Problem::member_not_found(),
            AccountError::OwnerCannotLeave => Problem::owner_cannot_leave(),
            AccountError::IncorrectTransferOfAccount => Problem::forbidden(),
            AccountError::DidBelongsToAnotherActor => Problem::did_belongs_to_another_actor(),
            AccountError::AlreadyMember => {
                Problem::already_member("That user is already a member of this account.")
            }
            AccountError::CannotTransferToSelf => Problem::invalid_request(
                "You already own this account; transfer ownership to another member.",
            ),
            // Never 404 here (actor-existence oracle over DIDs; DD 57081857 F1).
            AccountError::NotFound(AccountEntity::User) => Problem::forbidden(),
            // TODO(Engineer): status/code for this variant is unruled (409?); no use case produces it yet.
            AccountError::InvitationAlreadyPending => {
                Problem::invalid_request("An invitation for that user is already pending.")
            }
            AccountError::ContainsCommissions => Problem::invalid_request(
                "That column still holds commissions; move them off it first.",
            ),
            AccountError::DuplicateName => {
                Problem::invalid_request("Something on this board already has that name.")
            }
            AccountError::IncorrectNumberOfColumns => {
                Problem::invalid_request("That board cannot hold any more columns.")
            }
            AccountError::IndexOutOfRange(_) => {
                Problem::invalid_request("That position is past the end of the list.")
            }
            AccountError::NothingToDo => {
                Problem::invalid_request("That change would leave everything as it is.")
            }
            AccountError::SystemError(err) => Problem::internal_error(err.to_string()),
        }
    }
}

/// One account membership row. `id` and `did` carry the same value — an
/// Account is addressed by its DID alone, with no separate surrogate id.
impl From<account::list::Listing> for AccountMembership {
    fn from(account: account::list::Listing) -> Self {
        let did = account.id.to_string();
        AccountMembership {
            id: did.clone(),
            did,
            role: account.role.to_string(),
            handle: account.handle.as_str().to_owned(),
            name: account.name.as_str().to_owned(),
            alias: account.alias.map(|alias| alias.to_string()),
        }
    }
}

/// The founded account, as `POST /accounts` renders it. Pinned against the
/// CLI's hand copy by `tests/create_account_parity.rs`, so the two drivers'
/// projections of the same use case never drift apart.
impl From<account::create::Output> for CreateAccountResponse {
    fn from(founded: account::create::Output) -> Self {
        let did = founded.account_id.to_string();
        CreateAccountResponse {
            id: did.clone(),
            did,
            handle: founded.handle.as_str().to_owned(),
            name: founded.name.as_str().to_owned(),
        }
    }
}

/// The deletion's outcome (`soft` | `hard`), as `DELETE /accounts/{id}`
/// renders it. Pinned by `tests/delete_account_parity.rs`.
impl From<account::delete::Output> for DeleteAccountResponse {
    fn from(deleted: account::delete::Output) -> Self {
        DeleteAccountResponse {
            outcome: deleted.outcome.to_string(),
        }
    }
}

/// The renamed account, as `PATCH /accounts/{id}/handle` renders it — the
/// same envelope [`CreateAccountResponse`] carries.
impl From<account::change_handle::Output> for ChangeHandleResponse {
    fn from(renamed: account::change_handle::Output) -> Self {
        let did = renamed.id.to_string();
        ChangeHandleResponse {
            id: did.clone(),
            did,
            handle: renamed.handle.as_str().to_owned(),
            name: renamed.name.as_str().to_owned(),
        }
    }
}

/// The accounts route group: founding, membership (grant/revoke/leave), and
/// the invitation lifecycle (invite/revoke/decline/accept).
pub(crate) fn accounts_router() -> Router<AppState> {
    Router::new()
        .route("/accounts", get(list_accounts).post(create_account))
        .route("/accounts/{id}", delete(delete_account))
        .route("/accounts/{id}/handle", patch(change_handle))
        .route(
            "/accounts/{id}/members",
            post(grant_role).delete(revoke_role),
        )
        .route("/accounts/{id}/members/me", delete(leave_account))
        .route("/accounts/{id}/transfer", post(transfer_ownership))
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

/// `GET /accounts` — every live account the signed-in visitor holds a role in
/// (not owned-only).
///
/// - `200 { "accounts": [ { "id", "did", "handle", "name", "role", "alias" }, … ] }`
/// - `401` — not signed in
async fn list_accounts(
    State(state): State<AppState>,
    CallingUser(user_id): CallingUser,
) -> Result<Response, Problem> {
    let query = account::list::Query { user_id };

    let listed = state.app().accounts().list(query).await?;
    let accounts = listed
        .accounts
        .into_iter()
        .map(AccountMembership::from)
        .collect();

    let body = ListAccountsResponse { accounts };
    Ok((StatusCode::OK, Json(body)).into_response())
}

/// `POST /accounts` — founds a new Account for the signed-in visitor as its
/// Owner.
///
/// - `201 { "id", "did", "handle", "name" }`
/// - `401` — not signed in
/// - `422 invalid_request` — missing/malformed body, blank name, or bad handle
/// - `409 handle_taken` — handle already claimed (live or tombstoned)
async fn create_account(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<CreateAccountRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) =
        body.map_err(|_| Problem::invalid_request("A name and handle are required."))?;
    let name = body
        .name
        .parse::<AccountName>()
        .map_err(|err| Problem::invalid_request(err.to_string()))?;
    let handle = body
        .handle
        .parse::<Handle>()
        .map_err(|err| Problem::invalid_request(err.to_string()))?;

    let command = account::create::Command {
        actor_id,
        name,
        handle,
    };

    let founded = state
        .app()
        .accounts()
        .create(command, &state.config.handle_domain, Utc::now())
        .await?;

    let body = CreateAccountResponse::from(founded);
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}

// Fails to compile once the first account-fact table registers (assert below).
const _: () = assert!(
    adapter_pg::ACCOUNT_FACT_TABLES.is_empty(),
    "an account-anchored fact store was registered: replace the constant-`false` body \
     of application::account::facts::exist::run with a real query over it (an account \
     bearing such a fact must be soft-deleted, never hard-deleted), then remove this \
     guard and its sibling in adapter_pg::account"
);

/// Parses a body's `user` field — an actor DID — into a [`UserId`], rejecting a
/// malformed value as `422 invalid_request`.
fn parse_user_did(raw: &str) -> Result<UserId, Problem> {
    raw.parse::<UserId>()
        .map_err(|_| Problem::invalid_request("The user must be a DID, e.g. \"did:plc:…\"."))
}

/// `DELETE /accounts/{id}` — the Owner deletes their account.
///
/// - `200 { "outcome": "soft" | "hard" }`
/// - `401` — not signed in
/// - `403` — not this account's Owner
/// - `404` — no such live account
async fn delete_account(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let cmd = account::delete::Command {
        actor_id,
        account_id,
    };

    let deleted = state.app().accounts().delete(cmd).await.map_err(|err| {
        if let AccountError::Infrastructure(cause) = &err {
            tracing::error!(error = ?cause, "deleting the account failed in the store");
        }
        Problem::from(err)
    })?;

    let body = DeleteAccountResponse::from(deleted);
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// `PATCH /accounts/{id}/handle` — the Owner changes the account's handle
/// post-onboarding.
///
/// - `200 { "id", "did", "handle", "name" }`
/// - `401` — not signed in · `403` — not this account's Owner
/// - `404` — no such live account
/// - `409 handle_taken` — held by another account (live, tombstoned, or quarantined)
/// - `422` — malformed body, invalid handle, unchanged handle, or `unsupported_handle` (BYO target)
/// - `429 rate_limited` — too many recent changes · `503` — DID minter unavailable
async fn change_handle(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<ChangeHandleRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("A new handle is required."))?;
    let handle = body
        .handle
        .parse::<Handle>()
        .map_err(|err| Problem::invalid_request(err.to_string()))?;

    let command = account::change_handle::Command {
        actor_id,
        account_id,
        handle,
    };
    let renamed = state
        .app()
        .accounts()
        .change_handle(command, &state.config.handle_domain, Utc::now())
        .await?;

    let body = ChangeHandleResponse::from(renamed);
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The accept-invitation request body: whether the new membership is shown on
/// the invitee's public profile.
#[derive(Deserialize)]
struct AcceptInvitationBody {
    pub listed_on_profile: bool,
}

/// `POST /accounts/{id}/invitations/accept`'s `200` body.
#[derive(Serialize)]
struct AcceptInvitationResponse {
    account: String,
    role: String,
    user: String,
}

/// `POST /accounts/{id}/invitations/accept` — the invited User accepts their
/// own pending invitation and becomes a member.
///
/// - `200 { "account", "role", "user" }`
/// - `404 no_pending_invitation` — no pending offer for this user
/// - `400` — malformed body
async fn accept_invitation(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(invited_id): CallingUser,
    body: Result<Json<AcceptInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed JSON"))?;
    let command = invitation::accept::Command {
        account_id,
        target_id: invited_id,
        listed_on_profile: body.listed_on_profile,
    };

    let accepted = state.app().accounts().invitations().accept(command).await?;

    let body = AcceptInvitationResponse {
        account: accepted.account_id.to_string(),
        role: accepted.role.to_string(),
        user: accepted.user_id.to_string(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// `DELETE /accounts/{id}/members/me` — the signed-in member leaves the account.
///
/// - `204 No Content`
/// - `404` — not a member
/// - `409` — the sole Owner can't leave while still Owner (transfer or delete first)
async fn leave_account(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let cmd = account::leave::Command {
        account_id,
        leaving_user_id: actor_id,
    };

    state.app().accounts().leave(cmd).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// The body of `POST /accounts/{id}/members`: grantee by `did`, and `role`
/// (`"admin" | "manager" | "member"`; `"owner"` never grantable here).
///
/// Example: `{ "user": "did:plc:abc123", "role": "admin" }`.
#[derive(Deserialize)]
struct GrantRoleBody {
    user: String,
    role: String,
}

/// `POST /accounts/{id}/members`'s `200` body — see [`grant_role`].
#[derive(Serialize)]
struct GrantRoleResponse {
    account: String,
    role: String,
    user: String,
}

/// `POST /accounts/{id}/members` — grants a role, seating the grantee as a
/// member if they aren't one yet.
///
/// - `200 { "account", "user", "role" }`
/// - `401` — not signed in · `403` — not allowed to grant that role
/// - `404` — no such account
/// - `422` — malformed body or an unknown role discriminant
async fn grant_role(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<GrantRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a member to grant, e.g. {\"user\": \"did:plc:…\", \"role\": \"admin\"}.",
        )
    })?;
    let role = body
        .role
        .parse::<Role>()
        .map_err(|err| Problem::unknown_role(err.to_string()))?;

    let cmd = account::role::grant::Command {
        actor_id,
        target_id: parse_user_did(&body.user)?,
        account_id,
        role,
    };
    let granted = state.app().accounts().roles().grant(cmd).await?;

    let body = GrantRoleResponse {
        account: granted.account_id.to_string(),
        role: granted.role.to_string(),
        user: granted.user_id.to_string(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The body of `DELETE /accounts/{id}/members`: the member to revoke, named by
/// `did`. No role — a revoke removes the membership whatever role it holds.
///
/// Example: `{ "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct RevokeRoleBody {
    user: String,
}

/// `DELETE /accounts/{id}/members`'s `200` body — see [`revoke_role`].
#[derive(Serialize)]
struct RevokeRoleResponse {
    account: String,
    user: String,
}

/// `DELETE /accounts/{id}/members` — revokes a member's role, the inverse of
/// `grant_role`.
///
/// - `200 { "account", "user" }`
/// - `401` — not signed in · `403` — not allowed to revoke that member
/// - `404` — no such account, or not a member
/// - `422` — malformed body
async fn revoke_role(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<RevokeRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide a member to revoke, e.g. {\"user\": \"did:plc:…\"}.")
    })?;

    let cmd = account::role::revoke::Command {
        account_id,
        actor_id,
        target_id: parse_user_did(&body.user)?,
    };
    let revoked = state.app().accounts().roles().revoke(cmd).await?;

    let body = RevokeRoleResponse {
        account: revoked.account_id.to_string(),
        user: revoked.user_id.to_string(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The body of `POST /accounts/{id}/invitations`: invitee by `did`, and `role`
/// (`"admin" | "manager" | "member"`; `"owner"` never offerable by invitation).
///
/// Example: `{ "user": "did:plc:abc123", "role": "member" }`.
#[derive(Deserialize)]
struct InviteUserToAccountBody {
    user: String,
    role: String,
}

/// `POST /accounts/{id}/invitations`'s response body (both the idempotent
/// `200` re-invite and the minted `201`) — see [`invite_user_to_account`].
#[derive(Serialize)]
struct InviteUserToAccountResponse {
    account: String,
    id: String,
    role: String,
    state: String,
    user: String,
}

/// `POST /accounts/{id}/invitations` — issues a pending invitation. A
/// duplicate invite is idempotent (existing offer returned, `200`); a fresh
/// offer is `201`.
///
/// - `200`/`201 { "account", "id", "role", "state", "user" }`
/// - `401` — not signed in · `403` — not allowed to invite that role
/// - `409` — invitee is already a member
/// - `422` — malformed body or an unknown role discriminant
async fn invite_user_to_account(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<InviteUserToAccountBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a user to invite and a role, e.g. {\"user\": \"did:plc:…\", \"role\": \"member\"}.",
        )
    })?;
    let role = body
        .role
        .parse::<Role>()
        .map_err(|err| Problem::unknown_role(err.to_string()))?;

    let cmd = account::invitation::issue::Command {
        account_id,
        actor_id,
        role,
        target_id: parse_user_did(&body.user)?,
    };

    let invited = state
        .app()
        .accounts()
        .invitations()
        .issue(cmd, Utc::now())
        .await?;

    let status = match invited.outcome {
        InviteOutcome::Minted => StatusCode::CREATED,
        InviteOutcome::AlreadyPending => StatusCode::OK,
    };
    let offer = InviteUserToAccountResponse {
        id: invited.invitation_id.to_string(),
        account: invited.account_id.to_string(),
        user: invited.target_id.to_string(),
        role: invited.role.to_string(),
        state: invited.state.to_string(),
    };
    let response = (status, Json(offer)).into_response();
    Ok(response)
}

/// The body of `DELETE /accounts/{id}/invitations`: the invitation, addressed
/// by the invited User's `did` (at most one pending offer per account/user).
///
/// Example: `{ "user": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct RevokeInvitationBody {
    user: String,
}

/// `DELETE /accounts/{id}/invitations`'s `200` body — see
/// [`revoke_invitation_to_account`].
#[derive(Serialize)]
struct RevokeInvitationResponse {
    account: String,
    user: String,
}

/// `DELETE /accounts/{id}/invitations` — revokes a pending invitation.
/// Idempotent: unknown user or no pending offer is a `200` no-op.
///
/// - `200 { "account", "user" }`
/// - `401` — not signed in · `403` — not allowed to revoke that invitation
async fn revoke_invitation_to_account(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<RevokeInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the invited user to revoke, e.g. {\"user\": \"did:plc:…\"}.",
        )
    })?;

    let target_id = parse_user_did(&body.user)?;
    let response_body = RevokeInvitationResponse {
        account: account_id.to_string(),
        user: target_id.to_string(),
    };

    let cmd = account::invitation::revoke::Command {
        account_id,
        actor_id,
        target_id,
    };
    state.app().accounts().invitations().revoke(cmd).await?;

    let response = (StatusCode::OK, Json(response_body)).into_response();
    Ok(response)
}

/// `POST /accounts/{id}/invitations/decline`'s `200` body: the declined
/// offer's account and the declining user — see [`decline_invitation`].
#[derive(Serialize)]
struct DeclineInvitationResponse {
    account: String,
    user: String,
}

/// `POST /accounts/{id}/invitations/decline` — the invitee declines their own
/// pending invitation.
///
/// - `200 { "account", "user" }`
/// - `404 no_pending_invitation` — nothing pending for this user
async fn decline_invitation(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let body = DeclineInvitationResponse {
        account: account_id.to_string(),
        user: actor_id.to_string(),
    };

    let cmd = account::invitation::decline::Command {
        account_id,
        actor_id,
    };
    state
        .app()
        .accounts()
        .invitations()
        .decline(cmd, Utc::now())
        .await?;

    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

/// The body of `POST /accounts/{id}/transfer`: the incoming Owner, named by
/// `new_owner` DID.
///
/// Example: `{ "new_owner": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct TransferOwnershipBody {
    new_owner: String,
}

/// `POST /accounts/{id}/transfer`'s `200` body — see [`transfer_ownership`].
#[derive(Serialize)]
struct TransferOwnershipResponse {
    account: String,
    owner: String,
    previous_owner: String,
}

/// `POST /accounts/{id}/transfer` — transfers ownership to another existing
/// member, immediately and unilaterally (no recipient acceptance).
///
/// - `200 { "account", "owner", "previous_owner" }`
/// - `401` — not signed in · `403` — not the account's current Owner
/// - `404` — no such account, or `new_owner` is not a member
/// - `422` — malformed body, or transferring to oneself
async fn transfer_ownership(
    State(state): State<AppState>,
    Path(account_id): Path<AccountId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<TransferOwnershipBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide the new owner, e.g. {\"new_owner\": \"did:plc:…\"}.")
    })?;

    let target_id = parse_user_did(&body.new_owner)?;
    let cmd = account::transfer_ownership::Command {
        account_id,
        actor_id,
        target_id,
    };
    let transferred = state.app().accounts().transfer_ownership(cmd).await?;

    let body = TransferOwnershipResponse {
        account: transferred.account_id.to_string(),
        owner: transferred.owner_id.to_string(),
        previous_owner: transferred.previous_owner_id.to_string(),
    };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}

#[cfg(test)]
mod tests {
    //! Pins each response body's wire shape: every field a string.

    use super::*;

    #[test]
    fn accept_invitation_response_serializes_every_field_as_a_string() {
        let body = AcceptInvitationResponse {
            account: "account-id".to_string(),
            role: "member".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","role":"member","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn grant_role_response_serializes_every_field_as_a_string() {
        let body = GrantRoleResponse {
            account: "account-id".to_string(),
            role: "admin".to_string(),
            user: "did:plc:grantee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","role":"admin","user":"did:plc:grantee"}"#
        );
    }

    #[test]
    fn revoke_role_response_serializes_every_field_as_a_string() {
        let body = RevokeRoleResponse {
            account: "account-id".to_string(),
            user: "did:plc:target".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","user":"did:plc:target"}"#
        );
    }

    #[test]
    fn invite_user_to_account_response_serializes_every_field_as_a_string() {
        let body = InviteUserToAccountResponse {
            account: "account-id".to_string(),
            id: "offer-id".to_string(),
            role: "member".to_string(),
            state: "pending".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","id":"offer-id","role":"member","state":"pending","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn revoke_invitation_response_serializes_every_field_as_a_string() {
        let body = RevokeInvitationResponse {
            account: "account-id".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn decline_invitation_response_serializes_every_field_as_a_string() {
        let body = DeclineInvitationResponse {
            account: "account-id".to_string(),
            user: "did:plc:invitee".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","user":"did:plc:invitee"}"#
        );
    }

    #[test]
    fn transfer_ownership_response_serializes_every_field_as_a_string() {
        let body = TransferOwnershipResponse {
            account: "account-id".to_string(),
            owner: "did:plc:new-owner".to_string(),
            previous_owner: "did:plc:old-owner".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"account":"account-id","owner":"did:plc:new-owner","previous_owner":"did:plc:old-owner"}"#
        );
    }
}
