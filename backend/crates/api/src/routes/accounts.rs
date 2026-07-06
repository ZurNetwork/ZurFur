//! The accounts route group: the account/membership/invitation JSON API.
//!
//! These endpoints (`POST /accounts`, the `.../members` and `.../invitations`
//! trees) speak JSON and return status codes — an unrecognized caller gets a
//! `401`, never a redirect, because the frontend *calls* these rather than
//! browsing to them. This is part of the cookie surface, so [`crate::app`] mounts
//! the group under the first-party-`Origin` (CSRF) layer.
//!
//! The shared write-path seam lives here. [`AccountRole`] is the account-scope
//! authorization **extractor** every account-scoped write declares: it resolves the
//! actor, loads the `{id}` account, and enforces the membership floor in one place a
//! route cannot compile without (generalizing the former per-handler
//! [`require_user`] → [`load_account`] → [`actor_role`] chain, which those helpers
//! still back). User-scoped writes (e.g. founding an account) and public account
//! reads stay off the seam — the gate is writes-against-a-target-account only
//! (ZMVP-47; DD 26247170 §5).
//!
//! References: ZMVP-14 through ZMVP-21, ZMVP-32, ZMVP-47; DESIGN/Account,
//! DESIGN/Roles; DD "User as Actor & On-Demand Accounts" (26247170).

use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, State, rejection::JsonRejection},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{delete, patch, post},
};
use chrono::{Duration, Utc};
use domain::elements::{
    account::{Account, AccountId, AccountName},
    did::Did,
    handle::Handle,
    invitation::Invitation,
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
};
use domain::ports::{HandleTaken, transaction};
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

/// The account-scope authorization seam: the extractor every **account-scoped
/// write** flows through (ZMVP-47). It resolves the acting [`User`], loads the
/// target [`Account`] named by the `{id}` path parameter, and confirms the actor
/// holds *some* role on it — the shared membership floor, generalizing the former
/// per-handler `require_user` → [`load_account`] → [`actor_role`] chain into one
/// place that cannot be forgotten.
///
/// **Why an extractor.** Declaring `AccountRole` in a handler's argument list is
/// what makes the floor unskippable: an account-scoped route *cannot compile*
/// without it, so no future route can quietly omit the membership check. This is
/// the "make unsoundness unreachable" move — one enforced path over per-site checks
/// that drift — the same instinct as the compile-enforced Unit of Work (DD 24150017).
///
/// **Floor, not rank.** It yields the actor, the loaded account, and the actor's
/// [`Role`]; the handler then applies its own capability-specific rank rule
/// ([`Role::can_grant`], Owner-only) on that `Role`. The floor is *membership*; the
/// rank stays the handler's (DD 26247170 §5 — capability-scoped, flat membership
/// floor returning `Role`; DESIGN/Roles).
///
/// **Writes only.** Public reads of an account are anonymous-readable (discovery)
/// and must **not** extract this — gating a read path would be a regression
/// (DD 26247170 §5). User-scoped writes (Characters, reviews, commission
/// participation, founding an account) likewise sit at the auth-only floor and do
/// not extract it.
///
/// Rejections mirror the chain it replaces, in the same order (auth before any
/// account lookup, so an anonymous caller is a `401` even on a missing account):
/// - no/unreadable session, or a vanished User → `401` `not_authenticated`
/// - the `{id}` names no live account (unknown, soft-deleted, or non-uuid) → `404`
///   `account_not_found`
/// - the actor holds no role on the account → `403` `forbidden`
pub(crate) struct AccountRole {
    /// The acting, recognized [`User`] (the session resolved to a live User).
    pub actor: User,
    /// The live target [`Account`] named by the `{id}` path parameter.
    pub account: Account,
    /// The actor's [`Role`] on [`account`](AccountRole::account) — always `Some` by
    /// construction (a non-member is rejected `403` before this is built). The
    /// handler applies its own rank rule on it.
    pub role: Role,
}

impl FromRequestParts<AppState> for AccountRole {
    type Rejection = Problem;

    /// Resolve `(actor, account, role)` for an account-scoped write, or reject.
    ///
    /// Order matters and mirrors the former handler chain: the acting User is
    /// resolved *first*, so an anonymous caller is turned away at `401` before any
    /// account is loaded (never leaking a `404`/`403` to the signed-out). Only then
    /// is the `{id}` account loaded (`404` if it names none) and the membership floor
    /// applied (`403` for a non-member).
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Problem> {
        // Resolve the acting visitor first — an anonymous caller is a 401 before any
        // account lookup. A missing session layer (infra misconfig, never in practice)
        // is likewise "not authenticated" rather than a leaked 500.
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| Problem::not_authenticated())?;
        let actor = require_user(state, &session).await?;

        // The `{id}` path segment names the target account. A non-uuid segment names
        // no account — a 404 (nothing to act on), the same outcome as an unknown id —
        // never a bare 400/500.
        let Path(account_id) = Path::<Uuid>::from_request_parts(parts, state)
            .await
            .map_err(|_| Problem::account_not_found())?;
        let account = load_account(state, AccountId::new(account_id)).await?;

        // The membership floor: a non-member has no role and so no authority (403).
        // The per-capability rank rule stays the handler's (flat floor, DD 26247170 §5).
        let role = actor_role(state, actor.id, account.id).await?;

        Ok(AccountRole {
            actor,
            account,
            role,
        })
    }
}

/// The light anti-abuse ceiling on handle changes per account within
/// [`handle_change_window`] (DD "Account Handle Change Flow" `27852802` §3 — Bluesky's
/// ~10-per-5-minutes spirit: a burst throttle, **not** a long cooldown, since the
/// anti-impersonation weight lives on the quarantine, not the cadence). A build-time
/// number the DD leaves to implementation.
const HANDLE_CHANGE_LIMIT: i64 = 10;

/// The rolling window [`HANDLE_CHANGE_LIMIT`] is counted over (DD `27852802` §3).
fn handle_change_window() -> Duration {
    Duration::minutes(5)
}

/// How long a vacated `*.zurfur.app` handle stays reserved (quarantined) to the account
/// that left it before it frees for anyone else (DD `27852802` §4) — the
/// anti-impersonation knob, a build-time number the DD leaves to implementation.
fn handle_quarantine_window() -> Duration {
    Duration::days(30)
}

/// Whether `handle` is in the Zurfur-issued namespace (a subdomain of `handle_domain`)
/// rather than a brought (BYO) domain. Quarantine reserves this namespace only, and v1
/// ships the *change* flow for it only — a BYO target needs bidirectional
/// verify-before-commit that isn't built yet (DD `27852802` §4/§6).
///
/// `handle.as_str()` is already normalized (lowercase, no trailing dot) by
/// [`Handle::try_new`]; the configured `handle_domain` is not, so we normalize it the
/// same way before comparing — otherwise a mixed-case or trailing-dot domain (via
/// config/env) would misclassify a Zurfur handle as BYO and silently disable both the
/// quarantine and the change flow. Mirrors `handle_from_host` in `routes/wellknown.rs`.
fn in_zurfur_namespace(handle: &Handle, handle_domain: &str) -> bool {
    let domain = handle_domain
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    handle.as_str().ends_with(&format!(".{domain}"))
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

/// The body of `POST /accounts`. Founding takes real input, not a bare click:
/// the account's display `name` and its public `handle` (the atproto-style name it
/// is reached by; DD "The Account Handle" 24870914).
///
/// Example: `{ "name": "Acme Studio", "handle": "acme.zurfur.app" }`.
#[derive(Deserialize)]
struct CreateAccountBody {
    name: String,
    handle: String,
}

/// Founds a new Account for the signed-in visitor and makes them its Owner
/// (ZMVP-14: "User creates an Account and becomes its Owner"). Onboarding
/// *sequencing* — when to prompt, how to nudge a user who has none — is a frontend
/// concern; this endpoint is the capability the frontend calls. An account is a
/// sovereign entity, so founding first mints the account's own `did:plc` (the live
/// `RealDidMinter`: generates rotation keys, signs an identity-only genesis
/// operation, custodies the keys, and submits to a — no-op in v1 — directory).
/// That mint is kept off the sign-in critical path precisely because it is a
/// fallible, key-generating step. The account and the founder's Owner membership are then
/// persisted together in one private-store transaction — never a cross-store dual
/// write. Per DESIGN/Account a user may own several accounts, so this founds a fresh
/// one on every call rather than being idempotent.
///
/// The caller must supply a name and a handle. Examples:
/// - `{ "name": "Acme Studio", "handle": "acme.zurfur.app" }` → `201 { "id", "did",
///   "handle", "name" }`
/// - missing/malformed body (e.g. no `handle`) → `422` (`invalid_request`), nothing minted
/// - a blank name → `422` (`invalid_request`), nothing minted
/// - a malformed/reserved/punycode handle → `422` (`invalid_request`), nothing minted
/// - a handle already claimed by a live **or tombstoned** account → `409`
///   (`handle_taken`), nothing minted
async fn create_account(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateAccountBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // Founding is a write, so it requires a recognized visitor (DESIGN/Account: "a
    // user without any accounts must create one before any write").
    let user = require_user(&state, &session).await?;

    // A missing/malformed body (e.g. no `handle` field, or non-JSON), or a
    // name/handle that fails validation, is rejected before anything is minted. All
    // map to 422 — the request was understood but unusable. The `Handle` newtype is
    // the one shared claim-validation gate (normalize + punycode/reserved-label
    // rejects; ZMVP-48/45, DD/24870914 §6).
    let Json(body) =
        body.map_err(|_| Problem::invalid_request("A name and handle are required."))?;
    let name =
        AccountName::try_new(body.name).map_err(|err| Problem::invalid_request(err.to_string()))?;
    let handle =
        Handle::try_new(body.handle).map_err(|err| Problem::invalid_request(err.to_string()))?;

    // Fast path: reject a handle already claimed by a *live* account up front with a
    // friendly 409 — nothing minted. This can't see a handle reserved by a
    // soft-deleted account, nor win against a concurrent claim; the global unique
    // index (mapped to `HandleTaken` below) is the authoritative backstop for both.
    if state.accounts.find_did_by_handle(&handle).await?.is_some() {
        return Err(Problem::handle_taken());
    }

    // A handle a *different* account vacated recently is quarantined to them for a
    // window — a squatter must not be able to found a fresh account on a just-freed
    // identity (DD 27852802 §4). Both handle-claim sites (this and the change flow)
    // honor the quarantine, so the reservation can't be sidestepped by founding instead
    // of renaming. Only the Zurfur namespace is quarantined (a BYO domain is the user's
    // own DNS); `excluding = None` because a founder claims fresh, never reclaiming.
    if in_zurfur_namespace(&handle, &state.config.handle_domain)
        && state
            .accounts
            .handle_reserved_for_other(&handle, None, Utc::now() - handle_quarantine_window())
            .await?
    {
        return Err(Problem::handle_taken());
    }

    // Mint the account's sovereign DID before touching the private store. The real
    // minter generates the account's rotation keys, signs an identity-only genesis
    // operation binding `alsoKnownAs = at://<handle>`, custodies the keys, and
    // submits the operation. A mint failure aborts with nothing persisted; the
    // client may retry.
    let did = state.did_minter.mint(&handle).await.map_err(|_| {
        Problem::service_unavailable(
            "We couldn't mint an identity for the account. Please try again.",
        )
    })?;

    // The founding invariant: the account and the creator's Owner membership are
    // minted together (`Account::open`) and persisted atomically.
    let (account, owner) = Account::open(user.id, did, handle, name, chrono::Utc::now());
    // One unit of work: the account row and the founder's Owner membership commit
    // together or not at all — reached through the transaction-bound write view. A
    // handle collision surfaces as `HandleTaken` (the global unique index — live or
    // tombstoned, DD 23003138); map it to a 409 rather than a 500. On any error the
    // `transaction` helper rolls the unit back and preserves *this* error (never the
    // rollback's), so the 409 downcast below still sees `HandleTaken`.
    // The boxed transaction future owns what it writes (it cannot borrow this stack
    // frame across the `for<'a>` boundary), so `account`/`owner` move in and the
    // committed `account` is handed back out for the response body.
    let account = match transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.accounts().create(&account, &owner).await?;
            Ok(account)
        })
    })
    .await
    {
        Ok(account) => account,
        Err(err) => {
            if err.downcast_ref::<HandleTaken>().is_some() {
                return Err(Problem::handle_taken());
            }
            return Err(err.into());
        }
    };

    Ok(created_json(json!({
        "id": account.id.to_string(),
        "did": account.did.as_str(),
        "handle": account.handle.as_str(),
        "name": account.name.as_str(),
    })))
}

// Mirror of the `adapter_pg::ACCOUNT_FACT_TABLES` compile guard, placed right beside
// the seam it protects (ZMVP-57 AC4): the constant-`false` body of `account_has_facts`
// below is sound ONLY while no account-anchored fact store exists. The moment the
// account-fact registry gains its first table, this fails to compile at the exact seam
// that must change — forcing whoever wires that store to replace the constant with a
// real query over it in the same change (and remove this guard and its sibling in
// `adapter_pg::account`). A test can be skipped or deleted; a compile guard cannot.
const _: () = assert!(
    adapter_pg::ACCOUNT_FACT_TABLES.is_empty(),
    "an account-anchored fact store was registered: replace the constant-`false` body \
     of account_has_facts with a real query over it (an account bearing such a fact must \
     be soft-deleted, never hard-deleted), then remove this guard and its sibling in \
     adapter_pg::account"
);

/// Whether an account holds any **account-anchored fact** — evidence that would be
/// *orphaned* by removing the account. This is the single seam that decides soft-vs-hard
/// deletion: an account holding any such fact is **soft-deleted** (its row kept, handle
/// reserved, `did:plc` live) and never escalates; an empty one is **hard-deleted**.
///
/// The enumeration of account-anchored fact classes is owned by the Account Deletion DD
/// (`23003138`), not this seam — this seam enforces the gate, whatever that list becomes.
/// Commissions are **not** among them: a commission is **User-owned** and survives account
/// deletion by design (Ownership Separation DD `29130754`); an account holds only a
/// commission's *placement* and revocable *view grants*, which are severed with the
/// account (never facts). So placing a commission in an account never forces a soft-delete.
///
/// Today **no** account-anchored fact store exists (the account-fact registry
/// `adapter_pg::ACCOUNT_FACT_TABLES` is empty), so no account can hold a queryable fact
/// and this is `false` — every deletion is currently a hard-delete. When the first such
/// store lands it wires its query here **in the same change**; nothing else in the delete
/// path changes.
///
/// SAFETY — this constant `false` is the ONLY thing keeping a fact-bearing account from
/// being *hard*-deleted (handle freed for reuse, `did:plc` tombstoned), and it is sound
/// only while the account-fact registry is empty. That soundness is not left to vigilance:
/// the compile guard above (and its sibling in `adapter_pg::account`) breaks the build the
/// moment a fact table is registered, and the schema tripwire test refuses any
/// `accounts`-referencing table that skips classification — so this body becomes a real
/// query in the same change that mints the first account-anchored fact (ZMVP-34 tombstone
/// review F1; ZMVP-57).
async fn account_has_facts(_state: &AppState, _account: AccountId) -> Result<bool, Problem> {
    Ok(false)
}

/// `DELETE /accounts/{id}` — the Owner deletes their account (ZMVP-34). Owner-only and
/// live-account-only: the acting user must hold `Owner` in this account, and the account
/// must not already be soft-deleted/unknown.
///
/// The account is **soft-deleted** if it holds any account-anchored fact (per the Account
/// Deletion DD `23003138` — **not** commissions, which are User-owned and survive) and
/// **hard-deleted** if it is empty. Soft keeps the row — the handle stays reserved, the `did:plc` stays
/// live, the account's surface is hidden — and never escalates to hard. Hard removes the
/// account (freeing its handle for reuse) and, as a **separate retryable atproto step**
/// (never inside the private transaction), tombstones the DID on the native ~72h PLC
/// recovery window (DD 23003138; in v1 the DID is identity-only, DD 26935298).
///
/// Outcomes:
/// - `204` — the account was soft- or hard-deleted
/// - `401` — not signed in
/// - `403` — signed in but not this account's Owner (a non-member or a non-Owner member)
/// - `404` — no such live account
async fn delete_account(
    State(state): State<AppState>,
    account_role: AccountRole,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam already settled the write floor: a recognized
    // visitor (else 401), a real live account named by `{id}` (else 404), and a role
    // on it (else 403). It hands back the loaded account and the actor's role.
    let AccountRole { account, role, .. } = account_role;

    // Owner-only (DD 23003138), the handler's rank rule on top of the membership
    // floor: a member who is not the Owner is forbidden — deletion is the Owner's
    // alone, unlike the grant/revoke seam which any sufficiently-ranked member reaches.
    if !matches!(role, Role::Owner(_)) {
        return Err(Problem::forbidden());
    }

    // Soft if the account holds facts (it then never escalates to hard); hard if it is
    // empty. Both are one private-store transaction on the write view.
    if account_has_facts(&state, account.id).await? {
        let account_id = account.id;
        transaction(&*state.database, |uow| {
            Box::pin(async move { uow.accounts().soft_delete(account_id).await })
        })
        .await?;
    } else {
        let account_id = account.id;
        transaction(&*state.database, |uow| {
            Box::pin(async move { uow.accounts().hard_delete(account_id).await })
        })
        .await?;

        // The private hard-delete above already freed the handle. Tombstoning the
        // account's `did:plc` is a separate, retryable **public** step — never a
        // cross-store transaction with the private delete (the mint path's mirror). The
        // custody keys and the operation log deliberately outlive the hard-delete so
        // this can run (and be retried) afterward, and so a higher-authority key can
        // reverse it within the native ~72h window. A failure here does not undo the
        // delete — the account is gone and its handle freed — so we log and still return
        // success rather than resurrecting a deleted account; the tombstone is
        // re-submittable. In v1 the directory is a gated no-op, so this signs and logs
        // but registers nowhere.
        if let Err(err) = state.did_minter.tombstone(&account.did).await {
            tracing::warn!(
                did = %account.did.as_str(),
                error = %err,
                "did:plc tombstone failed after hard-delete; account is deleted and its \
                 handle freed — the tombstone is retryable"
            );
        }
    }

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// The body of `PATCH /accounts/{id}/handle`: the `handle` to change to (the
/// atproto-style name the account will be reached by). Re-validated through the same
/// [`Handle`] gate as founding.
///
/// Example: `{ "handle": "renamed.zurfur.app" }`.
#[derive(Deserialize)]
struct ChangeHandleBody {
    handle: String,
}

/// `PATCH /accounts/{id}/handle` — the Owner changes the account's handle after
/// onboarding (ZMVP-46, DD "Account Handle Change Flow" `27852802`). Owner-only, the
/// new handle re-validated to the *same* guarantees as the initial claim
/// ([`Handle::try_new`]), with both resolution halves brought into agreement without a
/// cross-store transaction.
///
/// The order is the DD's (§7): **the DID document first, the private store second.**
/// We re-point the DID's `alsoKnownAs` via the ZMVP-50 signed `did:plc` UPDATE op — its
/// own retryable, idempotent step, never inside the private transaction — and only then
/// commit `accounts.handle` (which flips the `*.zurfur.app` handle→DID resolver) and
/// record the change. If the private write fails after the DID-doc succeeded the worst
/// transient is `handle.invalid` (the new handle simply not-yet-valid), never a handle
/// resolving to the wrong DID; the op is idempotent, so a client retry is safe.
///
/// Policy (all DD-decided): **Owner-only** (§2); a light **rate limit** (§3); the
/// vacated `*.zurfur.app` handle is **quarantined** to this account (§4 — recorded by
/// the change itself, enforced at every claim site); `alsoKnownAs` is **REPLACED** (§5,
/// owned by the ZMVP-50 op). **BYO targets are deferred** (§6): changing *to* a
/// non-`*.zurfur.app` handle is refused until bidirectional verify-before-commit ships
/// (changing *from* a BYO handle *to* a Zurfur one is fine — the target is ours).
///
/// Outcomes:
/// - `200 { "id", "did", "handle", "name" }` — the handle changed; resolution follows
/// - `401` — not signed in · `403` — signed in but not this account's Owner
/// - `404` — no such live account
/// - `409` (`handle_taken`) — the target is held by another account (live or tombstoned)
///   or quarantined to another
/// - `422` — malformed body, an invalid handle, the account's own current handle, or a
///   BYO target (`unsupported_handle`)
/// - `429` (`rate_limited`) — too many recent changes · `503` — the DID minter is down
async fn change_handle(
    State(state): State<AppState>,
    account_role: AccountRole,
    body: Result<Json<ChangeHandleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam settled the write floor (401/404/403) and loaded the
    // live account plus the actor's role in it.
    let AccountRole { account, role, .. } = account_role;

    // Owner-only (DD §2), the handler's rank rule on the membership floor — the same
    // authority bar as delete/transfer, above the grant/revoke seam any ranked member
    // reaches.
    if !matches!(role, Role::Owner(_)) {
        return Err(Problem::forbidden());
    }

    // Re-validate through the one shared claim gate — punycode/reserved/normalization
    // all re-enforced on a change, exactly as at founding (Done-when: "same validation
    // guarantees as the initial claim").
    let Json(body) = body.map_err(|_| Problem::invalid_request("A new handle is required."))?;
    let new =
        Handle::try_new(body.handle).map_err(|err| Problem::invalid_request(err.to_string()))?;

    // Changing to the account's own current handle is a no-op: reject it as unusable
    // rather than burn a rate-limit slot and sign a redundant chain op.
    if new == account.handle {
        return Err(Problem::invalid_request(
            "That is already this account's handle.",
        ));
    }

    // BYO deferred (DD §6): v1 ships the change flow for the Zurfur namespace only. A
    // brought-domain target needs bidirectional verify-before-commit (so we never
    // persist a handle the user hasn't proved they control) — a capability carved into
    // a follow-up ticket. Migrating *from* a BYO handle *to* a `*.zurfur.app` one is
    // allowed: the target resolves under our control.
    if !in_zurfur_namespace(&new, &state.config.handle_domain) {
        return Err(Problem::unsupported_handle(
            "Changing to a brought (non-*.zurfur.app) handle isn't supported yet.",
        ));
    }

    let now = Utc::now();

    // Rate limit (DD §3): a light anti-abuse throttle on how often an account renames.
    if state
        .accounts
        .count_handle_changes_since(account.id, now - handle_change_window())
        .await?
        >= HANDLE_CHANGE_LIMIT
    {
        return Err(Problem::rate_limited(
            "You've changed this account's handle too many times recently. Please wait a bit and try again.",
        ));
    }

    // Availability (DD §4). Fast path: a handle held by a *live* account is taken.
    if state.accounts.find_did_by_handle(&new).await?.is_some() {
        return Err(Problem::handle_taken());
    }
    // Quarantine: a handle another account vacated recently is reserved to them — a
    // squatter can't grab a just-freed identity. The asking account is excluded, so it
    // may reclaim its OWN vacated handle within the window. (The global unique index is
    // the write-time backstop for the tombstoned/race cases the reads can't see.)
    if state
        .accounts
        .handle_reserved_for_other(&new, Some(account.id), now - handle_quarantine_window())
        .await?
    {
        return Err(Problem::handle_taken());
    }

    // DID-doc FIRST (DD §7): re-point the DID document's `alsoKnownAs` to the new handle
    // — the ZMVP-50 signed `did:plc` UPDATE op (REPLACE), its own retryable/idempotent
    // step, NEVER inside the private transaction below (no cross-store dual write). A
    // failure here changes nothing in Postgres, so the account keeps its old handle and
    // the caller may retry.
    state
        .did_minter
        .update_handle(&account.did, &new)
        .await
        .map_err(|_| {
            Problem::service_unavailable(
                "We couldn't update the account's identity right now. Please try again.",
            )
        })?;

    // Private half (DD §7): repoint `accounts.handle` (which flips the handle→DID
    // resolver) and record the change (the rate-limit + quarantine source) in ONE unit
    // of work. A collision with the global handle index maps to 409, exactly as founding
    // does. `old`/`new` move into the boxed future; `new` is cloned so the response can
    // still name it.
    let old = account.handle.clone();
    let account_id = account.id;
    let new_stored = new.clone();
    if let Err(err) = transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.accounts()
                .change_handle(account_id, &old, &new_stored, now)
                .await
        })
    })
    .await
    {
        if err.downcast_ref::<HandleTaken>().is_some() {
            return Err(Problem::handle_taken());
        }
        return Err(err.into());
    }

    Ok(ok_json(json!({
        "id": account.id.to_string(),
        "did": account.did.as_str(),
        "handle": new.as_str(),
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
    let accepted = transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.accounts()
                .accept_invitation(invitation, body.listed_on_profile)
                .await
        })
    })
    .await?;

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
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.accounts().leave(leaving_user.id, account).await })
    })
    .await?;

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
    account_role: AccountRole,
    body: Result<Json<GrantRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam already settled the write floor (401/404/403) and
    // loaded the account plus the actor's role in it.
    let AccountRole {
        account,
        role: actor_role,
        ..
    } = account_role;

    // A missing/malformed body, or a role string that isn't one of the four known
    // discriminants, is rejected as understood-but-unusable (422).
    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a member to grant, e.g. {\"user\": \"did:plc:…\", \"role\": \"admin\"}.",
        )
    })?;
    let new_role =
        Role::try_from(body.role).map_err(|err| Problem::unknown_role(err.to_string()))?;

    // Authorization, the handler's rank rule on the membership floor: the actor's
    // standing in this account decides whether *this* grant is allowed.
    if !actor_role.can_grant(&new_role) {
        return Err(Problem::forbidden());
    }

    // Recognize the grantee by their DID (idempotent — mints them on first contact,
    // returns the existing User otherwise). Granting a role to someone who has never
    // signed in is how an Owner adds them; they resolve to the same User when they do.
    // Recognition is its own unit of work, settled before the grant (as before the
    // Unit-of-Work refactor): an idempotent recognize, independent of the grant.
    let grantee = transaction(&*state.database, |uow| {
        Box::pin(async move { uow.users().provision(&Did::new(body.user)).await })
    })
    .await?;

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
    // `member` moves into the boxed future (it can't be borrowed across `for<'a>`);
    // the granted role is returned back out for the response body.
    let granted_role = transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.accounts().grant_role(&member).await?;
            Ok(member.role)
        })
    })
    .await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": grantee.did.as_str(),
        "role": granted_role.as_str(),
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
    account_role: AccountRole,
    body: Result<Json<RevokeRoleBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam already settled the write floor (401/404/403) and
    // loaded the account plus the actor's standing in it — what decides the revoke.
    let AccountRole {
        account,
        role: actor_role,
        ..
    } = account_role;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide a member to revoke, e.g. {\"user\": \"did:plc:…\"}.")
    })?;

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
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.accounts().revoke_role(target.id, account.id).await })
    })
    .await?;

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
    account_role: AccountRole,
    body: Result<Json<InviteUserToAccountBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam settled the write floor (401/404/403) and loaded
    // the account plus the inviter's own standing (`actor` is the issuing member).
    let AccountRole {
        actor,
        account,
        role: inviting_user_role,
    } = account_role;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide a user to invite and a role, e.g. {\"user\": \"did:plc:…\", \"role\": \"member\"}.",
        )
    })?;
    let role = Role::try_from(body.role).map_err(|err| Problem::unknown_role(err.to_string()))?;

    if !inviting_user_role.can_grant(&role) {
        return Err(Problem::forbidden());
    }

    // Recognize the invitee (idempotent), its own unit of work — settled before
    // the offer is issued, as before the Unit-of-Work refactor.
    let invited = transaction(&*state.database, |uow| {
        Box::pin(async move { uow.users().provision(&Did::new(body.user)).await })
    })
    .await?;

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
    // `invitation` moves into the boxed future and is handed back out for the body.
    let invitation = transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.accounts().create_invitation(&invitation).await?;
            Ok(invitation)
        })
    })
    .await?;

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
    account_role: AccountRole,
    body: Result<Json<RevokeInvitationBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam settled the write floor (401/404/403) and loaded
    // the account plus the actor's standing — kept to apply the authority rule once
    // the invitation is loaded.
    let AccountRole {
        account,
        role: actor_role,
        ..
    } = account_role;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request(
            "Provide the invited user to revoke, e.g. {\"user\": \"did:plc:…\"}.",
        )
    })?;
    // Keep the invited DID by value: the response echoes it on every path (mirroring
    // `revoke_role`), including the idempotent no-ops where no invitation row — and so
    // no id/state — is available to report.
    let invited_did = body.user;

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
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.accounts().revoke_invitation(invitation.id).await })
    })
    .await?;

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
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.accounts().revoke_invitation(invitation.id).await })
    })
    .await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": actor.did.as_str(),
    })))
}

/// The body of `POST /accounts/{id}/transfer`. The incoming Owner is named by their
/// public `new_owner` DID — the same identity convention as grant/revoke: we address
/// a member by the DID they own, never by our internal id.
///
/// Example: `{ "new_owner": "did:plc:abc123" }`.
#[derive(Deserialize)]
struct TransferOwnershipBody {
    new_owner: String,
}

/// Transfers Account ownership to another existing member (ZMVP-33; DESIGN/Roles
/// rule 8). Ownership is singular — the role tree's root — so handing it off is its
/// own seam, distinct from a grant (which never grants Owner) and from leaving. The
/// transfer is immediate and unilateral (no recipient acceptance): in one
/// private-store transaction the named member becomes the sole `Owner` with no parent
/// (rule 5) and the outgoing Owner is demoted to `Admin`, re-homed under the new
/// Owner. The Account's `did:plc` is stable — only the human Owner pointer moves, so
/// there is no PLC write. This is the precondition that lets a former sole Owner then
/// leave (ZMVP-21).
///
/// Authority is settled at the handler seam, like grant/revoke/leave, so the outcomes
/// are problem+json rather than `500`s:
/// - `200 { "account", "owner", "previous_owner" }` — ownership moved
/// - `401` — not signed in
/// - `403` — signed in but not the account's current Owner (only the Owner may transfer)
/// - `404` — no such account, or the named `new_owner` is not a member of it
/// - `422` — malformed body, or transferring to oneself (Roles rule 8: to *another* member)
async fn transfer_ownership(
    State(state): State<AppState>,
    account_role: AccountRole,
    body: Result<Json<TransferOwnershipBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // The shared `AccountRole` seam settled the write floor (401/404/403) and loaded
    // the account plus the acting member's standing (`old_owner` is the actor).
    let AccountRole {
        actor: old_owner,
        account,
        role: actor_role,
    } = account_role;

    let Json(body) = body.map_err(|_| {
        Problem::invalid_request("Provide the new owner, e.g. {\"new_owner\": \"did:plc:…\"}.")
    })?;

    // Only the current Owner may transfer ownership (the handler's rank rule on the
    // membership floor): a member who isn't the Owner is forbidden.
    if !matches!(actor_role, Role::Owner(_)) {
        return Err(Problem::forbidden());
    }

    // Resolve the incoming Owner by DID *without minting* — like revoke, a transfer
    // must not recognize a brand-new visitor as a side effect. An unknown DID, or a
    // known user who holds no role here, is not a member (404).
    let new_owner = state
        .users
        .find_by_did(&Did::new(body.new_owner))
        .await?
        .ok_or_else(Problem::member_not_found)?;
    if state
        .accounts
        .role_of(new_owner.id, account.id)
        .await?
        .is_none()
    {
        return Err(Problem::member_not_found());
    }

    // Ownership moves to *another* member (Roles rule 8). Transferring to oneself is a
    // no-op the domain doesn't model — reject it as an unusable request rather than
    // churning the Owner's own row through Admin and back.
    if new_owner.id == old_owner.id {
        return Err(Problem::invalid_request(
            "You already own this account; transfer ownership to another member.",
        ));
    }

    // Demote the outgoing Owner to Admin and promote the incoming member to Owner —
    // atomically, on the transaction-bound write view. Only the `Copy` ids are
    // captured by the future, so the `User`/`Account` structs stay owned here for the
    // response body below (the same shape as `leave_account`).
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.accounts()
                .transfer_ownership(old_owner.id, new_owner.id, account.id)
                .await
        })
    })
    .await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "owner": new_owner.did.as_str(),
        "previous_owner": old_owner.did.as_str(),
    })))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the account-scope authorization floor the [`AccountRole`]
    //! extractor is built on (ZMVP-47). These pin the *mapping* the extractor
    //! delegates to — "no role → 403", "unknown account → 404", "a member's role is
    //! returned" — independent of the HTTP stack, so the invariant survives a future
    //! change to how routes are wired. The extractor's request-ordering (auth *before*
    //! account lookup, so an anonymous caller is 401 even on a missing account) and the
    //! 401 floor itself are proven end-to-end in `tests/account_scope_gate.rs`, since
    //! they need a real request (session + matched path).

    use std::sync::Arc;

    use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
    use chrono::Utc;
    use domain::elements::{account::Account, profile::Profile};

    use super::*;
    use crate::{Config, Environment};

    /// A fully in-memory [`AppState`] plus the backing [`MemBackend`], so a unit test
    /// can seed accounts/memberships and then call the shared floor helpers directly —
    /// the same fakes `spawn_app` wires, without binding a socket.
    fn mem_state() -> (AppState, MemBackend) {
        let backend = MemBackend::new();
        let state = AppState {
            config: Config {
                env: Environment::DEV,
                http_addr: "127.0.0.1:0".parse().unwrap(),
                public_url: "http://127.0.0.1".to_string(),
                database_url: "postgres://unused".to_string(),
                log_level: "info".to_string(),
                handle_domain: "zurfur.app".to_string(),
                did_key_root_key: "unused-in-tests".to_string(),
                plc_directory_endpoint: "https://plc.directory".to_string(),
                plc_directory_submit: false,
                deadline_sweep_interval_secs: 60,
            },
            pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
            auth: Arc::new(MemAuthenticator::new(Did::new("did:plc:unit".to_string()))),
            users: backend.user_store(),
            profile_source: Arc::new(MemProfileSource::new(Profile {
                did: Did::new("did:plc:unit".to_string()),
                handle: "unit.bsky.social".to_string(),
                display_name: None,
                avatar_url: None,
            })),
            profile_cache: backend.profile_cache(),
            database: backend.database(),
            accounts: backend.account_store(),
            commissions: backend.commission_store(),
            changelog: backend.changelog_store(),
            did_minter: Arc::new(MemDidMinter::new()),
        };
        (state, backend)
    }

    /// Seed an account owned by a freshly-provisioned user; returns the account and
    /// its owner's [`UserId`].
    async fn seed_account(
        backend: &MemBackend,
        owner_did: &str,
        handle: &str,
    ) -> (Account, UserId) {
        let owner = backend
            .provision(&Did::new(owner_did.to_string()))
            .await
            .expect("provision owner");
        let (account, membership) = Account::open(
            owner.id,
            Did::new(format!("{owner_did}:acct")),
            Handle::try_new(handle).expect("valid handle"),
            AccountName::try_new("Seed Studio").expect("valid name"),
            Utc::now(),
        );
        backend
            .create(&account, &membership)
            .await
            .expect("seed account");
        (account, owner.id)
    }

    // The floor's core mapping: a user with NO role on the account is turned away with
    // a 403 `forbidden` — the exact rejection the extractor surfaces for a non-member.
    #[tokio::test]
    async fn actor_role_maps_a_non_member_to_forbidden() {
        let (state, backend) = mem_state();
        let (account, _owner) = seed_account(&backend, "did:plc:owner", "seed.zurfur.app").await;
        // A provisioned user who was never granted a role on the account.
        let stranger = backend
            .provision(&Did::new("did:plc:stranger".to_string()))
            .await
            .expect("provision stranger");

        let err = actor_role(&state, stranger.id, account.id)
            .await
            .expect_err("a non-member has no authority");
        assert_eq!(err.status, 403, "no role on the account is a 403");
        assert_eq!(err.code, "forbidden");
    }

    // The floor's positive path: a seated member's actual [`Role`] is returned, for the
    // handler to apply its own rank rule on (flat membership floor, DD 26247170 §5).
    #[tokio::test]
    async fn actor_role_returns_a_members_role() {
        let (state, backend) = mem_state();
        let (account, owner_id) = seed_account(&backend, "did:plc:owner", "seed.zurfur.app").await;

        let owner_role = actor_role(&state, owner_id, account.id)
            .await
            .expect("the owner holds a role");
        assert_eq!(owner_role, Role::Owner(None), "the founder is the Owner");

        // A non-owner member's own rank comes back unchanged — the floor never flattens
        // the role it returns.
        let member = backend
            .provision(&Did::new("did:plc:member".to_string()))
            .await
            .expect("provision member");
        backend
            .grant_role(&UserAccount {
                user_id: member.id,
                account_id: account.id,
                role: Role::Manager(None),
            })
            .await
            .expect("seat a manager");
        let member_role = actor_role(&state, member.id, account.id)
            .await
            .expect("the member holds a role");
        assert_eq!(member_role, Role::Manager(None));
    }

    // The account-load floor: an unknown (or soft-deleted) account id is a 404, kept
    // distinct from the 403 authority failure — the extractor's `{id}` → account step.
    #[tokio::test]
    async fn load_account_maps_an_unknown_id_to_not_found() {
        let (state, _backend) = mem_state();

        let Err(err) = load_account(&state, AccountId::new(Uuid::now_v7())).await else {
            panic!("an unknown account id has nothing to act on");
        };
        assert_eq!(err.status, 404, "an unknown account is a 404");
        assert_eq!(err.code, "account_not_found");
    }
}
