//! The session route group: the browser-facing sign-in flow.
//!
//! These endpoints (`POST /signin`, `GET /signin-callback`, `GET /me`,
//! `POST /logout`) back the SvelteKit sign-in pages (ZMVP-151). The human-facing
//! HTML lives in the frontend now; the API speaks JSON and redirects. `/signin`
//! and `/signin-callback` still redirect — the browser *navigates* the OAuth
//! handshake — but `/me` is a JSON whoami: a live session returns the caller's
//! identity, an anonymous one gets a `401` problem+json (not a redirect), so the
//! frontend can branch. This is part of the cookie surface, so [`crate::app`]
//! mounts the group under the first-party-`Origin` (CSRF) layer.
//!
//! Callback failures never crash and never echo a PDS-supplied reason: each maps
//! to a redirect to the frontend `/login` carrying a stable `error=<code>` the
//! login page renders. The codes (`denied`, `invalid_callback`, `exchange_failed`)
//! are a contract.
//!
//! References: ZMVP-8 through ZMVP-11; ZMVP-151; DESIGN/Account.

use axum::{
    Form, Json, Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use domain::{
    elements::{did::Did, profile::Profile, user::UserId},
    ports::{ProfileCache, ProfileSource, UnitOfWork},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, SESSION_USER_KEY, problem::Problem};

/// The session route group: the OAuth sign-in flow and the JSON whoami. Each
/// route here is on the cookie surface; the composition root wraps the group with
/// the CSRF [`require_first_party_origin`](super::require_first_party_origin) layer.
pub(crate) fn session_router() -> Router<AppState> {
    Router::new()
        .route("/signin", post(signin))
        .route("/signin-callback", get(signin_callback))
        .route("/me", get(me))
        .route("/logout", post(logout))
}

/// The form body of `POST /signin`: the visitor's AT Protocol `handle` (e.g.
/// `you.bsky.social`), the only thing sign-in needs. It is handed straight to
/// the [`Authenticator`](domain::ports::Authenticator) to resolve into the PDS
/// authorization URL; an unknown or malformed handle fails as a problem+json the
/// frontend renders (ZMVP-8/ZMVP-151).
#[derive(Deserialize)]
struct SigninForm {
    handle: String,
}

/// The query parameters a PDS may send back to the redirect URI. All optional: a
/// successful authorization carries `code` (+ `state`/`iss`), while a denial carries
/// `error` (+ a description we deliberately drop) and no `code`. Parsing a neutral
/// struct rather than jacquard's strict `CallbackParams` is what lets a denial reach
/// the handler instead of being rejected by the extractor as a 400 — so we can
/// redirect to the login page rather than crash.
#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
}

/// The JSON body of `GET /me`: who the session belongs to. `did` is always present
/// (the session always resolves to a User with a DID); the profile fields are
/// `null` when the PDS profile can't be resolved — an unreachable PDS with nothing
/// cached is not an error, it degrades to a bare identity (the DID). See [`me`].
#[derive(Serialize)]
struct SessionUser {
    did: String,
    handle: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

/// Begins sign-in (`POST /signin`): hands the submitted [`SigninForm`] handle to
/// the [`Authenticator`](domain::ports::Authenticator) and redirects the browser to
/// the PDS authorization URL it returns (ZMVP-8). The visitor returns to
/// [`signin_callback`].
///
/// Caveats: no auth (this is how a visitor becomes recognized). An unknown or
/// malformed handle is an `invalid_request` problem+json with a steady message —
/// the underlying error can be noisy/internal, so it is not echoed. Expects a
/// form-encoded body, not JSON.
///
/// References: [`SigninForm`], [`signin_callback`].
///
/// ```text
/// POST /signin   (form)   handle=you.bsky.social
/// → 303 Location: https://pds.example/oauth/authorize?...
///
/// POST /signin   (form)   handle=not a handle
/// → 422 (application/problem+json, code invalid_request)
/// ```
async fn signin(
    State(state): State<AppState>,
    Form(f): Form<SigninForm>,
) -> Result<Redirect, Problem> {
    // An unknown or malformed handle fails as a problem the frontend renders. The
    // underlying error can be noisy/internal, so show a steady message, not its detail.
    let url = state.auth.start(&f.handle).await.map_err(|_| {
        Problem::invalid_request(
            "That handle could not be used to sign in. Check it and try again.",
        )
    })?;
    Ok(Redirect::to(&url))
}

/// The OAuth redirect target (`GET /signin-callback`): completes sign-in and
/// establishes the session. On success it exchanges the [`CallbackQuery`] `code`
/// for a DID via the [`Authenticator`](domain::ports::Authenticator), provisions
/// the [`UserWrites`](domain::ports::UserWrites) User for that DID (mint-or-return;
/// first contact *recognizes*, it doesn't register — ZMVP-9), rotates the session
/// id so a pre-auth id cannot carry into the authenticated session (session-fixation
/// hardening — ZMVP-24), stores the User's id under [`SESSION_USER_KEY`], and
/// redirects to the frontend root `/`. The session carries our own id, never the
/// DID, so later requests resolve without re-asking the PDS.
///
/// Caveats / failure modes, all mapped so the frontend can render them:
/// - a denied authorization (`error`, no `code`) → `303 /login?error=denied`;
/// - a missing `code` with no `error` → `303 /login?error=invalid_callback`;
/// - a failed code exchange → `303 /login?error=exchange_failed`;
/// - a provision or session-write failure → `500` `internal_error` problem+json.
///
/// The `error=<code>` values are a stable contract the login page branches on; the
/// PDS-supplied reason is deliberately not echoed. No prior auth required (this
/// *creates* the session).
///
/// References: [`Authenticator`](domain::ports::Authenticator), [`me`], [`SESSION_USER_KEY`].
///
/// ```text
/// GET /signin-callback?code=abc&state=xyz&iss=https://pds.example
/// → 303 Location: /   (Set-Cookie: zurfur.sid=...)
///
/// GET /signin-callback?error=access_denied&error_description=...
/// → 303 Location: /login?error=denied
/// ```
async fn signin_callback(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<CallbackQuery>,
) -> Response {
    // A denied authorization returns with `error` and no `code`. Send the visitor
    // to the login page with a stable code — not a crash, not a blank page. The
    // PDS-supplied reason is not echoed.
    if q.error.is_some() {
        return Redirect::to("/login?error=denied").into_response();
    }
    let Some(code) = q.code else {
        return Redirect::to("/login?error=invalid_callback").into_response();
    };

    let Ok(did) = state.auth.complete(code, q.state, q.iss).await else {
        return Redirect::to("/login?error=exchange_failed").into_response();
    };

    // First contact recognizes rather than registers: provisioning mints a User on
    // the first sign-in for this DID and returns the existing one on every repeat
    // (idempotent — one DID, one User, forever). The human fills out nothing.
    // Recognition is a private-store write, so it goes through one unit of work.
    let provisioned = state
        .transaction(async move |uow: &mut dyn UnitOfWork| uow.users().provision(&did).await)
        .await;
    let Ok(user) = provisioned else {
        return Problem::internal_error(
            "Your sign-in succeeded but your account couldn't be set up. Please try again.",
        )
        .into_response();
    };

    // Rotate the session id at this privilege change, then store the identity.
    // `cycle_id` mints a fresh id (preserving session data) so a pre-auth id — one
    // an attacker may have fixed in the victim's browser — cannot carry into the
    // authenticated session (session-fixation hardening, ZMVP-24). The session then
    // carries our own UserId, not the DID, so later requests resolve to the User
    // through the repo without re-asking the PDS. The cookie now survives reload;
    // land the visitor on the signed-in frontend root.
    if session.cycle_id().await.is_err()
        || session.insert(SESSION_USER_KEY, *user.id).await.is_err()
    {
        return Problem::internal_error(
            "Your sign-in succeeded but the session couldn't be saved. Please try again.",
        )
        .into_response();
    }
    Redirect::to("/").into_response()
}

/// The JSON whoami (`GET /me`): resolves the session's [`UserId`] to a User via the
/// [`UserStore`](domain::ports::UserStore) (no PDS round trip — ZMVP-9 Criterion 3),
/// then returns the caller's DID plus their resolved profile fields as JSON (ZMVP-10,
/// ZMVP-151).
///
/// Caveats: an anonymous visitor — no session, an expired one, or one whose User no
/// longer exists — gets a `401` `not_authenticated` problem+json, **not** a redirect
/// (an API, not a page: the frontend owns the redirect to `/login`). An unreachable
/// PDS with nothing cached still returns `200`, with the profile fields `null` and
/// the DID present (absence is not an error).
///
/// References: [`UserStore`](domain::ports::UserStore), [`SessionUser`], [`resolve_profile`].
///
/// ```text
/// GET /me   (Cookie: zurfur.sid=...)
/// → 200 (application/json: {"did":"did:plc:...","handle":"you.bsky.social",...})
///
/// GET /me   (no/expired session)
/// → 401 (application/problem+json, code not_authenticated)
/// ```
async fn me(State(state): State<AppState>, session: Session) -> Result<Json<SessionUser>, Problem> {
    let Ok(Some(id)) = session.get::<Uuid>(SESSION_USER_KEY).await else {
        return Err(Problem::not_authenticated());
    };
    let Ok(Some(user)) = state.users.find(UserId::new(id)).await else {
        return Err(Problem::not_authenticated());
    };
    let profile = resolve_profile(&*state.profile_cache, &*state.profile_source, &user.did).await;
    let did = user.did.to_string();
    let body = match profile {
        // A resolved profile: the handle is always present, display name and avatar
        // are the source's own optionals.
        Some(profile) => SessionUser {
            did,
            handle: Some(profile.handle),
            display_name: profile.display_name,
            avatar_url: profile.avatar_url,
        },
        // No profile (unreachable PDS, nothing cached): degrade to the bare DID —
        // absence is not an error, so the profile fields are simply null.
        None => SessionUser {
            did,
            handle: None,
            display_name: None,
            avatar_url: None,
        },
    };
    Ok(Json(body))
}

/// The exit door (ZMVP-11). Destroys the session server-side: `flush` removes the
/// Postgres row through the store and drops the cookie, so a stolen cookie dies
/// with the session rather than merely being cleared on the client. A second
/// sign-out from a stale tab carries a session id whose row is already gone — the
/// `DELETE` matches nothing and still succeeds — so the visitor lands back on `/`,
/// not an error (Criterion 2). On the rare store failure we report it honestly as a
/// `500` problem+json rather than claim a sign-out that didn't reach the server.
async fn logout(session: Session) -> Response {
    if session.flush().await.is_err() {
        return Problem::internal_error("Sign-out couldn't be completed. Please try again.")
            .into_response();
    }
    Redirect::to("/").into_response()
}

/// Read-through resolution of a visitor's profile: a fresh cache hit is served
/// without waking the PDS (ZMVP-10 criterion 2); a miss reads the PDS and caches
/// the result; a PDS failure degrades to `None` rather than erroring (criterion 3).
///
/// The cache fill is pool-backed and best-effort — a documented exception to the
/// compile-enforced Unit of Work (DD `24150017`): a read-through cache write on the
/// GET path has no transactional invariant, so it is not routed through a write
/// transaction (which would make a read endpoint open one for nothing). A `put`
/// failure is swallowed so a cache hiccup never fails the page.
async fn resolve_profile(
    cache: &dyn ProfileCache,
    source: &dyn ProfileSource,
    did: &Did,
) -> Option<Profile> {
    if let Ok(Some(profile)) = cache.get(did).await {
        return Some(profile);
    }
    match source.fetch(did).await {
        Ok(profile) => {
            // Best-effort, pool-backed cache write (guard exception): a cache failure
            // must not fail the page, so it is swallowed.
            let _ = cache.put(&profile).await;
            Some(profile)
        }
        Err(_) => None,
    }
}
