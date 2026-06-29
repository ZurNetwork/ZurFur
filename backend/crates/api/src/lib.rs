//! The composition root and HTTP surface of the Zurfur backend.
//!
//! This crate is the one place that knows which adapters are live. It owns
//! [`Config`] (figment-loaded), the shared [`AppState`] (a bag of trait
//! objects, one per port), the axum [`app`] router, and every HTTP handler.
//! Domain logic lives in `domain`; persistence and the PDS live behind the
//! `adapter-*` crates; this crate only wires them together and translates
//! between HTTP and those ports.
//!
//! Two shapes of endpoint coexist here. The browser-facing sign-in flow
//! (`/`, `/signin`, `/signin-callback`, `/me`, `/logout`) speaks HTML and
//! redirects — an unrecognized visitor lands back on the sign-in page. The
//! account/membership API (`POST /accounts`, `.../members`) speaks JSON and
//! returns status codes — an unrecognized caller gets a `401`, never a
//! redirect, because the frontend calls these rather than browsing to them.
//!
//! References: DESIGN "Domains and Applications" (ports and adapters);
//! DESIGN/Account, DESIGN/Roles; ZMVP-8 through ZMVP-16.

use std::net::SocketAddr;
use std::sync::Arc;

use adapter_pg::PgPool;
use axum::{
    Form, Json, Router,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        invitation::Invitation,
        profile::Profile,
        role::Role,
        user::{User, UserId},
        user_account::UserAccount,
    },
    ports::{AccountRepo, Authenticator, DidMinter, ProfileCache, ProfileSource, UserRepo},
};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

mod problem;
use problem::Problem;

/// Session key under which the recognized visitor's `UserId` is stored. The
/// session carries our own key, not the DID: subsequent requests resolve
/// session → User through the repo, never re-asking the PDS (ZMVP-9 Criterion 3).
const SESSION_USER_KEY: &str = "user_id";

/// The deployment profile, selected by `ZURFUR_ENV` (`dev` → [`DEV`]). The only
/// behavioral fork it drives today is cookie security: [`STG`] and [`PROD`] set
/// the session cookie `Secure` (HTTPS-only) in `main`, while [`DEV`] leaves it
/// off so loopback HTTP doesn't drop the cookie.
///
/// Caveats: deserialized from config/env, so the spelling must match a variant
/// exactly (`DEV`/`STG`/`PROD`). New environments are an enum change, not config.
///
/// [`DEV`]: Environment::DEV
/// [`STG`]: Environment::STG
/// [`PROD`]: Environment::PROD
#[derive(Clone, Debug, Deserialize)]
pub enum Environment {
    /// Local development: plain HTTP on loopback, non-`Secure` cookies.
    DEV,
    /// Staging: HTTPS, `Secure` cookies — a production-shaped environment.
    STG,
    /// Production: HTTPS, `Secure` cookies.
    PROD,
}

/// The fully-resolved runtime configuration, produced by [`Config::load`] and
/// then moved into [`AppState`]. Every field is required at boot except
/// [`http_addr`], which defaults to `127.0.0.1:3621`.
///
/// Caveats: figment layers config/{profile}.toml first, then `DATABASE_URL`,
/// then `ZURFUR_*` env (env wins); a missing required key fails the load.
/// [`database_url`] is read from the unprefixed `DATABASE_URL` on purpose — sqlx
/// tooling reads that exact name. [`public_url`] is the externally-visible
/// origin and must be a parseable URI: `main` builds the OAuth redirect URI from
/// it and aborts boot if it can't.
///
/// References: CLAUDE.md "Configuration"; [`Config::load`].
///
/// [`http_addr`]: Config::http_addr
/// [`database_url`]: Config::database_url
/// [`public_url`]: Config::public_url
#[derive(Clone, Deserialize)]
pub struct Config {
    /// The deployment profile; see [`Environment`].
    pub env: Environment,
    /// The socket the HTTP server binds. Defaults to `127.0.0.1:3621`
    /// (`default_http_addr`); dev.toml overrides to `127.0.0.1:8080`.
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    /// Externally-visible origin (scheme + host + port) used to build OAuth redirect URIs.
    pub public_url: String,
    /// Postgres connection string for the pool built at boot. Read from the
    /// unprefixed `DATABASE_URL` (the name sqlx tooling expects), not `ZURFUR_*`.
    pub database_url: String,
    /// Default tracing filter, applied when `RUST_LOG` is unset (see `main`).
    pub log_level: String,
}

/// Serde default for [`Config::http_addr`]: `127.0.0.1:3621`. The literal is a
/// known-valid socket, so the parse can't fail.
fn default_http_addr() -> SocketAddr {
    "127.0.0.1:3621".parse().unwrap()
}

impl Config {
    /// Loads and validates the runtime [`Config`] from the layered figment
    /// sources, selecting the profile from `ZURFUR_ENV` (default `dev`).
    ///
    /// Layering, lowest precedence first: `config/{profile}.toml`, then the
    /// unprefixed `DATABASE_URL`, then all `ZURFUR_*` env vars — so environment
    /// always wins over the file. The config directory is anchored to
    /// `CARGO_MANIFEST_DIR` (overridable via `ZURFUR_CONFIG_DIR`) because cargo,
    /// cargo-watch, and `just` each run from a different CWD.
    ///
    /// Caveats: returns a boxed [`figment::Error`] if a required key is missing
    /// or a value fails to deserialize (e.g. a malformed `http_addr`, or an
    /// `env` that isn't one of [`Environment`]'s variants). The TOML file is
    /// optional — env alone can satisfy every required key — but the keys
    /// themselves are not.
    ///
    /// References: CLAUDE.md "Configuration".
    pub fn load() -> Result<Self, Box<figment::Error>> {
        let profile = std::env::var("ZURFUR_ENV").unwrap_or_else(|_| "dev".into());

        // Anchor the config directory to this crate rather than the current working
        // directory: cargo, cargo-watch, and `just` all run from different CWDs, so a
        // relative `config/...` path resolves inconsistently. `CARGO_MANIFEST_DIR` is
        // `backend/crates/api`; the config lives at `backend/config`. A deployed binary
        // can point elsewhere via `ZURFUR_CONFIG_DIR`.
        let config_dir = std::env::var("ZURFUR_CONFIG_DIR")
            .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../config").into());

        Figment::new()
            .merge(Toml::file(format!("{config_dir}/{profile}.toml")))
            .merge(Env::raw().only(&["DATABASE_URL"]))
            .merge(Env::prefixed("ZURFUR_"))
            .extract()
            .map_err(Box::new)
    }
}
/// The form body of `POST /signin`: the visitor's AT Protocol `handle` (e.g.
/// `you.bsky.social`), the only thing sign-in needs. It is handed straight to
/// the [`Authenticator`] to resolve into the PDS authorization URL; an unknown
/// or malformed handle fails politely back on the sign-in page (ZMVP-8).
#[derive(Deserialize)]
struct SigninForm {
    handle: String,
}

/// The query parameters a PDS may send back to the redirect URI. All optional: a
/// successful authorization carries `code` (+ `state`/`iss`), while a denial carries
/// `error` (+ `error_description`) and no `code`. Parsing a neutral struct rather
/// than jacquard's strict `CallbackParams` is what lets a denial reach the handler
/// instead of being rejected by the extractor as a 400 — so we can render the
/// sign-in page rather than a crash.
#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// The shared application state every handler receives via axum's [`State`]
/// extractor — the composition root's bag of dependencies. It is `Clone` (the
/// pool and every port are cheaply clonable, behind [`PgPool`]/[`Arc`]), so axum
/// can hand each request its own copy.
///
/// Each port is an `Arc<dyn Trait>` precisely so the wiring picks the live
/// adapter once, in `main`, and the handlers stay ignorant of it: pg/atproto in
/// production, the in-process fakes (mem + a fake PDS) in the e2e tests. Adding a
/// capability is adding a field here plus a line in `main` — never a handler
/// rewrite.
///
/// References: DESIGN "Domains and Applications"; [`app`].
#[derive(Clone)]
pub struct AppState {
    /// The resolved runtime [`Config`]. Kept whole so handlers and `main` read
    /// the same values (e.g. cookie security keys off [`Config::env`]).
    pub config: Config,
    /// The Postgres connection pool. Shared directly (not behind a port) because
    /// it backs both the adapters built over it and the `health` probe.
    pub pool: PgPool,
    /// The [`Authenticator`] port: drives the OAuth handshake with a visitor's
    /// PDS — `start` yields the authorization URL, `complete` exchanges the
    /// callback for a DID. A trait object so the composition root chooses the
    /// live adapter (atproto's `AtprotoAuthenticator` in `main`, a fake PDS in
    /// e2e tests). Used by the `signin` and `signin_callback` handlers.
    pub auth: Arc<dyn Authenticator>,
    /// The [`UserRepo`] port: Zurfur's record of recognized visitors, keyed by
    /// DID. `provision` mints-or-returns one User per DID (idempotent); `find`
    /// resolves a session's id. pg in `main`, mem in tests.
    pub user_repo: Arc<dyn UserRepo>,
    /// The [`ProfileSource`] port: reads public profiles from the PDS. atproto
    /// in `main`, a fake in tests. A failure here degrades the `me` page to the
    /// DID rather than erroring.
    pub profile_source: Arc<dyn ProfileSource>,
    /// The [`ProfileCache`] port: private read-through cache fronting
    /// [`profile_source`](AppState::profile_source). pg in `main` (entries
    /// expire after an hour, set in `main`), mem in tests. See
    /// `resolve_profile`.
    pub profile_cache: Arc<dyn ProfileCache>,
    /// The [`AccountRepo`] port: Zurfur's record of accounts and their
    /// memberships. `create` persists an account and its founder's Owner
    /// membership in one transaction (ZMVP-14); `role_of`/`grant_role`/
    /// `revoke_role` back the membership API. pg in `main`, mem in tests.
    pub account_repo: Arc<dyn AccountRepo>,
    /// The [`DidMinter`] port: mints a sovereign `did:plc` for a newly founded
    /// account. The live adapter is the floor stub (`StubDidMinter`); the real
    /// PLC-directory write lands later as an adapter swap, invisible to the
    /// handler layer. Used by the `create_account` handler.
    pub did_minter: Arc<dyn DidMinter>,
}

/// Builds the axum [`Router`] over an [`AppState`], wiring every route to its
/// handler. This is the canonical route table; the e2e tests and `main` both
/// mount it. `main` additionally layers the session middleware (the [`Session`]
/// extractor handlers rely on comes from that layer, applied outside this fn).
///
/// Routes: `GET /health`, `GET /` and `POST /signin` and
/// `GET /signin-callback` (the sign-in flow), `GET /me`, `POST /accounts`,
/// `POST`/`DELETE /accounts/{id}/members`, `POST /logout`.
///
/// Cross-persona unlinkability (ZMVP-17): this table is the public surface, and
/// no route on it may correlate one person's separate handles — join one
/// handle's User/Account graph to another's *as the same human*. The separation
/// holds by construction (separate handles → separate Users → separate DIDs/
/// logins); the only sanctioned correlation, opt-in User-Linking ("alts"), is
/// post-MVP. Before adding a read route that enumerates Users or returns the set
/// of handles/accounts tied to a person, weigh it against that invariant — a
/// single-account member roster (DESIGN/1DD decision 5) is fine, a person-level
/// "their other personas" surface is not. Guarded by
/// `tests/cross_persona_unlinkability.rs`.
///
/// References: [`AppState`]; the per-handler docs below.
///
/// ```ignore
/// let router = api::app(state).layer(session_layer);
/// ```
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(form))
        .route("/signin", post(signin))
        .route("/signin-callback", get(signin_callback))
        .route("/me", get(me))
        .route("/accounts", post(create_account))
        .route(
            "/accounts/{id}/members",
            post(grant_role).delete(revoke_role),
        )
        .route(
            "/accounts/{id}/invitations",
            post(invite_user_to_account).delete(revoke_invitation_to_account),
        )
        .route(
            "/accounts/{id}/invitations/decline",
            post(decline_invitation),
        )
        .route("/accounts/{id}/invitations/accept", post(accept_invitation))
        .route("/logout", post(logout))
        .with_state(state)
}

/// Liveness/readiness probe (`GET /health`). Reports `200` with the database
/// `up` when the pool can reach Postgres, `503 degraded` when it can't — the one
/// endpoint that intentionally fails when a dependency is down, so an
/// orchestrator can gate traffic. No auth.
///
/// Caveats: only the database is probed; a healthy `200` doesn't certify the PDS
/// or any other adapter. References: CLAUDE.md "Database"; [`adapter_pg::is_reachable`].
///
/// ```text
/// GET /health
/// → 200 { "status": "ok",       "database": "up"   }
/// → 503 { "status": "degraded", "database": "down" }
/// ```
async fn health(state: State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    if adapter_pg::is_reachable(&state.pool).await {
        (
            StatusCode::OK,
            Json(json!({ "status": "ok", "database": "up" })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "degraded", "database": "down" })),
        )
    }
}

/// Renders the bare sign-in page — ugly on purpose (ZMVP-8). When `error` is set,
/// a message is shown above the form, so a denied authorization or a bad handle
/// lands the visitor back here rather than on a crash or a blank page.
fn sign_in_page(error: Option<&str>) -> Html<String> {
    let banner = match error {
        Some(msg) => format!(r#"<p style="color:red">{}</p>"#, escape(msg)),
        None => String::new(),
    };
    Html(format!(
        r#"{banner}<form method="post" action="/signin">
        <input name="handle" placeholder="you.bsky.social">
        <button>Sign in</button>
        </form>
        "#
    ))
}

/// Minimal HTML-escaping for error text echoed into the page. The PDS-supplied
/// `error_description` is outside our control, so escape before rendering.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The sign-in landing page (`GET /`): the bare form with no error banner. No
/// auth; this is where an anonymous visitor — or one bounced from [`me`] or
/// [`logout`] — begins. See [`sign_in_page`].
///
/// ```text
/// GET /
/// → 200 (HTML: the sign-in form)
/// ```
async fn form() -> Html<String> {
    sign_in_page(None)
}

/// Begins sign-in (`POST /signin`): hands the submitted [`SigninForm`] handle to
/// the [`Authenticator`] and redirects the browser to the PDS authorization URL
/// it returns (ZMVP-8). The visitor returns to [`signin_callback`].
///
/// Caveats: no auth (this is how a visitor becomes recognized). An unknown or
/// malformed handle is a `400` rendering the sign-in page with a steady message
/// — the underlying error can be noisy/internal, so it is not echoed. Expects a
/// form-encoded body, not JSON.
///
/// References: [`SigninForm`], [`Authenticator`], [`signin_callback`].
///
/// ```text
/// POST /signin   (form)   handle=you.bsky.social
/// → 303 Location: https://pds.example/oauth/authorize?...
///
/// POST /signin   (form)   handle=not a handle
/// → 400 (HTML: sign-in page with an error banner)
/// ```
async fn signin(
    State(state): State<AppState>,
    Form(f): Form<SigninForm>,
) -> Result<Redirect, (StatusCode, Html<String>)> {
    match state.auth.start(&f.handle).await {
        Ok(url) => Ok(Redirect::to(&url)),
        // An unknown or malformed handle fails politely back on the sign-in page.
        // The underlying error can be noisy/internal, so show a steady message.
        Err(_) => Err((
            StatusCode::BAD_REQUEST,
            sign_in_page(Some(
                "That handle could not be used to sign in. Check it and try again.",
            )),
        )),
    }
}

/// The OAuth redirect target (`GET /signin-callback`): completes sign-in and
/// establishes the session. On success it exchanges the [`CallbackQuery`] `code`
/// for a DID via the [`Authenticator`], provisions the [`UserRepo`] User for that
/// DID (mint-or-return; first contact *recognizes*, it doesn't register —
/// ZMVP-9), rotates the session id so a pre-auth id cannot carry into the
/// authenticated session (session-fixation hardening — ZMVP-24), stores the
/// User's id under [`SESSION_USER_KEY`], and redirects to [`me`]. The session
/// carries our own id, never the DID, so later requests resolve without
/// re-asking the PDS.
///
/// Caveats / failure modes: a denied authorization arrives with `error` and no
/// `code` — handled as `200` back on the sign-in page, not a crash (this is why
/// [`CallbackQuery`] is a lax struct, not jacquard's strict params). A missing
/// `code` with no `error` is a `400`. A failed code exchange is `400`. A
/// provision, session-rotation, or session-insert failure is `500`, each rendering the sign-in page
/// with a steady message. No prior auth required (this *creates* the session).
///
/// References: [`Authenticator`], [`UserRepo`], [`me`], [`SESSION_USER_KEY`].
///
/// ```text
/// GET /signin-callback?code=abc&state=xyz&iss=https://pds.example
/// → 303 Location: /me   (Set-Cookie: zurfur.sid=...)
///
/// GET /signin-callback?error=access_denied&error_description=...
/// → 200 (HTML: sign-in page, "Sign-in was not completed: ...")
/// ```
async fn signin_callback(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<CallbackQuery>,
) -> Response {
    // A denied authorization returns with `error` and no `code`. Send the visitor
    // back to the sign-in page with the reason — not a crash, not a blank page.
    if let Some(err) = q.error {
        let reason = q.error_description.unwrap_or(err);
        return (
            StatusCode::OK,
            sign_in_page(Some(&format!("Sign-in was not completed: {reason}"))),
        )
            .into_response();
    }
    let Some(code) = q.code else {
        return (
            StatusCode::BAD_REQUEST,
            sign_in_page(Some(
                "The sign-in response was incomplete. Please try again.",
            )),
        )
            .into_response();
    };

    let did = match state.auth.complete(code, q.state, q.iss).await {
        Ok(did) => did,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                sign_in_page(Some(
                    "We couldn't complete sign-in with your PDS. Please try again.",
                )),
            )
                .into_response();
        }
    };

    // First contact recognizes rather than registers: provisioning mints a User on
    // the first sign-in for this DID and returns the existing one on every repeat
    // (idempotent — one DID, one User, forever). The human fills out nothing.
    let user = match state.user_repo.provision(&did).await {
        Ok(user) => user,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                sign_in_page(Some(
                    "Your sign-in succeeded but your account couldn't be set up. Please try again.",
                )),
            )
                .into_response();
        }
    };

    // Rotate the session id at this privilege change, then store the identity.
    // `cycle_id` mints a fresh id (preserving session data) so a pre-auth id — one
    // an attacker may have fixed in the victim's browser — cannot carry into the
    // authenticated session (session-fixation hardening, ZMVP-24). The session then
    // carries our own UserId, not the DID, so later requests resolve to the User
    // through the repo without re-asking the PDS. The cookie now survives reload;
    // hand off to the greeting route.
    if session.cycle_id().await.is_err()
        || session.insert(SESSION_USER_KEY, *user.id).await.is_err()
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            sign_in_page(Some(
                "Your sign-in succeeded but the session couldn't be saved. Please try again.",
            )),
        )
            .into_response();
    }
    Redirect::to("/me").into_response()
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
/// private-store transaction inside [`AccountRepo::accept_invitation`]; a revoke or
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
        .account_repo
        .find_pending_invitation(AccountId::new(account_id), invited_user.id)
        .await?
        .ok_or_else(Problem::no_pending_invitation)?;

    let accepted = state
        .account_repo
        .accept_invitation(invitation, body.listed_on_profile)
        .await?;

    Ok(ok_json(json!({
        "account": accepted.account_id.to_string(),
        "user": invited_user.did.as_str(),
        "role": accepted.role.as_str(),
    })))
}

/// The signed-in greeting (`GET /me`): resolves the session's [`UserId`] to a
/// User via the [`UserRepo`] (no PDS round trip — ZMVP-9 Criterion 3), then
/// renders the handle and avatar via [`resolve_profile`]/[`render_me`] (ZMVP-10).
///
/// Caveats: an anonymous visitor — no session, an expired one, or one whose User
/// no longer exists — is redirected to [`form`], not erred. A page, not an API:
/// it redirects rather than returning `401`. An unreachable PDS with nothing
/// cached still renders, falling back to the DID (absence is not an error).
///
/// References: [`UserRepo`], [`resolve_profile`], [`render_me`].
///
/// ```text
/// GET /me   (Cookie: zurfur.sid=...)
/// → 200 (HTML: "Signed in as @you (You)" + sign-out control)
///
/// GET /me   (no/expired session)
/// → 303 Location: /
/// ```
async fn me(State(state): State<AppState>, session: Session) -> Response {
    let Ok(Some(id)) = session.get::<Uuid>(SESSION_USER_KEY).await else {
        return Redirect::to("/").into_response();
    };
    let user = match state.user_repo.find(UserId::new(id)).await {
        Ok(Some(user)) => user,
        _ => return Redirect::to("/").into_response(),
    };
    let profile = resolve_profile(&*state.profile_cache, &*state.profile_source, &user.did).await;
    Html(render_me(&user.did, profile.as_ref())).into_response()
}

/// The body of `POST /accounts`. Founding takes real input, not a bare click.
///
/// Example: `{ "name": "Acme Studio" }`.
#[derive(Deserialize)]
struct CreateAccountBody {
    name: String,
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
        .user_repo
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
        .account_repo
        .find(id)
        .await?
        .ok_or_else(Problem::account_not_found)
}

/// The actor's role in an account, or `403` — a non-member has no authority. The
/// authority *floor* shared by grant/revoke/invite (DESIGN/Roles); the per-action
/// rank rule (`Role::can_grant`) is the caller's. A store error is a `500`.
async fn actor_role(state: &AppState, user: UserId, account: AccountId) -> Result<Role, Problem> {
    state
        .account_repo
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
    state.account_repo.create(&account, &owner).await?;

    Ok(created_json(json!({
        "id": account.id.to_string(),
        "did": account.did.as_str(),
        "name": account.name.as_str(),
    })))
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

    let invited = state.user_repo.provision(&Did::new(body.user)).await?;

    // An invitation is the path *to* membership; someone who already holds a role has
    // nowhere to be invited. This is a state conflict (409), not an authority failure
    // (403) or a malformed request (422) — the actor may invite, just not this person.
    if state
        .account_repo
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
        .account_repo
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
    state.account_repo.create_invitation(&invitation).await?;

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
        .user_repo
        .find_by_did(&Did::new(invited_did.clone()))
        .await?
    {
        Some(user) => user,
        None => return Ok(revoked()),
    };

    let mut invitation = match state
        .account_repo
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
    state.account_repo.revoke_invitation(invitation.id).await?;

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
        .account_repo
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
    state.account_repo.revoke_invitation(invitation.id).await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": actor.did.as_str(),
    })))
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
    let grantee = state.user_repo.provision(&Did::new(body.user)).await?;

    // The guard above bounds the role being *granted*; this bounds the *grantee*.
    // An account's Owner is never demoted through a grant — ownership only moves via
    // the separate transfer seam ("an Owner never has a parent, even when
    // transferred", DESIGN/Roles). Without this, an Admin could grant Manager to the
    // Owner's DID and quietly unseat them.
    if let Some(Role::Owner(_)) = state.account_repo.role_of(grantee.id, account.id).await? {
        return Err(Problem::forbidden());
    }

    // Settle the grant: upsert the membership in the private store.
    let member = UserAccount {
        user_id: grantee.id,
        account_id: account.id,
        role: new_role,
    };
    state.account_repo.grant_role(&member).await?;

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
        .user_repo
        .find_by_did(&Did::new(body.user))
        .await?
        .ok_or_else(Problem::member_not_found)?;

    // The member's *current* rank is what the actor must be allowed to act on — the
    // same predicate as grant. An Owner outranks everyone, so they're never revocable
    // here; an Admin can't revoke a peer Admin. Someone with no role isn't a member.
    let target_role = state
        .account_repo
        .role_of(target.id, account.id)
        .await?
        .ok_or_else(Problem::member_not_found)?;
    if !actor_role.can_grant(&target_role) {
        return Err(Problem::forbidden());
    }

    // Settle the revoke: remove the membership.
    state
        .account_repo
        .revoke_role(target.id, account.id)
        .await?;

    Ok(ok_json(json!({
        "account": account.id.to_string(),
        "user": target.did.as_str(),
    })))
}

/// The exit door (ZMVP-11). Destroys the session server-side: `flush` removes the
/// Postgres row through the store and drops the cookie, so a stolen cookie dies
/// with the session rather than merely being cleared on the client. A second
/// sign-out from a stale tab carries a session id whose row is already gone — the
/// `DELETE` matches nothing and still succeeds — so the visitor lands back on the
/// sign-in page, not an error (Criterion 2). On the rare store failure we report it
/// honestly rather than claim a sign-out that didn't reach the server.
async fn logout(session: Session) -> Response {
    if session.flush().await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            sign_in_page(Some("Sign-out couldn't be completed. Please try again.")),
        )
            .into_response();
    }
    Redirect::to("/").into_response()
}

/// Read-through resolution of a visitor's profile: a fresh cache hit is served
/// without waking the PDS (ZMVP-10 criterion 2); a miss reads the PDS and caches
/// the result; a PDS failure degrades to `None` rather than erroring (criterion 3).
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
            // Best-effort cache write: a cache failure must not fail the page.
            let _ = cache.put(&profile).await;
            Some(profile)
        }
        Err(_) => None,
    }
}

/// The sign-out control (ZMVP-11). A POST so the exit door is a deliberate action,
/// not something a prefetch or a stray GET can trip.
const SIGN_OUT_FORM: &str =
    r#"<form method="post" action="/logout"><button>Sign out</button></form>"#;

/// Renders the signed-in greeting. With a profile, shows the avatar (if any) and
/// the handle plus display name. Without one — an unreachable PDS and nothing
/// cached — the DID still proves who is signed in; absence is not an error. Either
/// way the greeting carries the sign-out control.
fn render_me(did: &Did, profile: Option<&Profile>) -> String {
    let greeting = match profile {
        Some(p) => {
            let display = p.display_name.as_deref().unwrap_or(&p.handle);
            let avatar = match &p.avatar_url {
                Some(url) => format!(r#"<img src="{}" alt="avatar" width="80">"#, escape(url)),
                None => String::new(),
            };
            format!(
                "{avatar}<p>Signed in as @{} ({})</p>",
                escape(&p.handle),
                escape(display),
            )
        }
        None => format!("<p>Signed in as {}</p>", escape(did)),
    };
    format!("{greeting}{SIGN_OUT_FORM}")
}
