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
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        profile::Profile,
        role::Role,
        user::UserId,
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

/// Session key under which the recognized visitor's `UserId` is stored. The
/// session carries our own key, not the DID: subsequent requests resolve
/// session → User through the repo, never re-asking the PDS (ZMVP-9 Criterion 3).
const SESSION_USER_KEY: &str = "user_id";

#[derive(Clone, Debug, Deserialize)]
pub enum Environment {
    DEV,
    STG,
    PROD,
}

#[derive(Clone, Deserialize)]
pub struct Config {
    pub env: Environment,
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    /// Externally-visible origin (scheme + host + port) used to build OAuth redirect URIs.
    pub public_url: String,
    pub database_url: String,
    pub log_level: String,
}

fn default_http_addr() -> SocketAddr {
    "127.0.0.1:3621".parse().unwrap()
}

impl Config {
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

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub pool: PgPool,
    /// Authenticates visitors against their PDS. A trait object so the composition
    /// root chooses the live adapter (atproto in `main`, a fake PDS in e2e tests).
    pub auth: Arc<dyn Authenticator>,
    /// Zurfur's record of recognized visitors. A trait object so the composition
    /// root chooses the live adapter (pg in `main`, mem in tests).
    pub user_repo: Arc<dyn UserRepo>,
    /// Reads public profiles from the PDS (atproto in `main`, a fake in tests).
    pub profile_source: Arc<dyn ProfileSource>,
    /// Private read-through cache of those profiles (pg in `main`, mem in tests).
    pub profile_cache: Arc<dyn ProfileCache>,
    /// Zurfur's record of accounts and their owners (pg in `main`, mem in tests).
    /// An account and its founder's Owner membership are persisted together here
    /// (ZMVP-14).
    pub account_repo: Arc<dyn AccountRepo>,
    /// Mints a sovereign `did:plc` for a newly founded account. The live adapter
    /// is the floor stub (`StubDidMinter`); dressing it for real minting is an
    /// adapter swap, invisible to this handler layer.
    pub did_minter: Arc<dyn DidMinter>,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(form))
        .route("/signin", post(signin))
        .route("/signin-callback", get(signin_callback))
        .route("/me", get(me))
        .route("/accounts", post(create_account))
        .route("/accounts/{id}/members", post(grant_role))
        .route("/logout", post(logout))
        .with_state(state)
}

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

async fn form() -> Html<String> {
    sign_in_page(None)
}

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

    // The session carries our own UserId, not the DID, so later requests resolve
    // to the User through the repo without re-asking the PDS. The cookie now
    // survives reload; hand off to the greeting route.
    if session.insert(SESSION_USER_KEY, *user.id).await.is_err() {
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

/// Greets the signed-in visitor, resolving their session's `UserId` to a User via
/// the repo (no PDS round trip). An anonymous visitor — no session, an expired one,
/// or one whose User no longer exists — is sent back to the sign-in page.
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
/// - `{ "name": "   " }` or no body → `422 { "error" }`, nothing minted
async fn create_account(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateAccountBody>, JsonRejection>,
) -> Response {
    // Founding is a write, so it requires a recognized visitor (DESIGN/Account: "a
    // user without any accounts must create one before any write"). No session, an
    // expired one, or a vanished User → 401 JSON, not a redirect: this is an API
    // endpoint the frontend calls, not a page.
    let Ok(Some(id)) = session.get::<Uuid>(SESSION_USER_KEY).await else {
        return unauthorized();
    };
    let user = match state.user_repo.find(UserId::new(id)).await {
        Ok(Some(user)) => user,
        _ => return unauthorized(),
    };

    // A missing/malformed body, or a name that fails validation, is rejected before
    // anything is minted. Both map to 422 — the request was understood but unusable.
    let Ok(Json(body)) = body else {
        return unprocessable("Provide a name for the account, e.g. {\"name\": \"Acme Studio\"}.");
    };
    let name = match AccountName::try_new(body.name) {
        Ok(name) => name,
        Err(err) => return unprocessable(&err.to_string()),
    };

    // Mint the account's sovereign DID before touching the private store. A mint
    // failure (the real adapter writes to the PLC directory) aborts with nothing
    // persisted; the client may retry.
    let did = match state.did_minter.mint().await {
        Ok(did) => did,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "We couldn't mint an identity for the account. Please try again."
                })),
            )
                .into_response();
        }
    };

    // The founding invariant: the account and the creator's Owner membership are
    // minted together (`Account::open`) and persisted atomically.
    let (account, owner) = Account::open(user.id, did, name, chrono::Utc::now());
    if state.account_repo.create(&account, &owner).await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "The account couldn't be created. Please try again."
            })),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(json!({
            "id": account.id.to_string(),
            "did": account.did.as_str(),
            "name": account.name.as_str(),
        })),
    )
        .into_response()
}

/// The 422 a write endpoint returns when the request is understood but its data
/// won't do — a blank name, say. Carries a human-readable reason.
fn unprocessable(reason: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "error": reason })),
    )
        .into_response()
}

/// The 401 a write endpoint returns to a visitor we don't recognize — JSON, not a
/// redirect, since these endpoints are called by the frontend, not browsed to.
fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "You must be signed in to do that." })),
    )
        .into_response()
}

/// The 403 a write endpoint returns when the visitor is recognized but lacks the
/// authority for the action — here, granting a role without being the account's
/// Owner (the floor rule; DESIGN/Roles).
fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({ "error": "You don't have permission to grant that role on this account." })),
    )
        .into_response()
}

/// The 404 a write endpoint returns when the addressed account doesn't exist (or
/// has been soft-deleted) — the target of the grant isn't there to act on.
fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "No such account." })),
    )
        .into_response()
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
) -> Response {
    // Granting is a write, so it requires a recognized visitor — the actor whose
    // authority we are about to check.
    let Ok(Some(id)) = session.get::<Uuid>(SESSION_USER_KEY).await else {
        return unauthorized();
    };
    let actor = match state.user_repo.find(UserId::new(id)).await {
        Ok(Some(user)) => user,
        _ => return unauthorized(),
    };

    // A missing/malformed body, or a role string that isn't one of the four known
    // discriminants, is rejected before anything is touched — understood but unusable.
    let Ok(Json(body)) = body else {
        return unprocessable(
            "Provide a member to grant, e.g. {\"user\": \"did:plc:…\", \"role\": \"admin\"}.",
        );
    };
    let new_role = match Role::try_from(body.role) {
        Ok(role) => role,
        Err(err) => return unprocessable(&err.to_string()),
    };

    // The grant must address a real, live account. A soft-deleted or unknown id is
    // a 404 — there's nothing to act on — kept distinct from "you may not" (403).
    let account = AccountId::new(account_id);
    match state.account_repo.find(account).await {
        Ok(Some(_)) => {}
        Ok(None) => return not_found(),
        Err(_) => return internal_error("The grant couldn't be completed. Please try again."),
    }

    // Authorization, at the seam: the actor's standing in *this* account decides
    // whether the grant is allowed. A non-member has no role and so no authority.
    let actor_role = match state.account_repo.role_of(actor.id, account).await {
        Ok(Some(role)) => role,
        Ok(None) => return forbidden(),
        Err(_) => return internal_error("The grant couldn't be completed. Please try again."),
    };
    if !actor_role.can_grant(&new_role) {
        return forbidden();
    }

    // Recognize the grantee by their DID (idempotent — mints them on first contact,
    // returns the existing User otherwise). Granting a role to someone who has never
    // signed in is how an Owner adds them; they resolve to the same User when they do.
    let grantee = match state.user_repo.provision(&Did::new(body.user)).await {
        Ok(user) => user,
        Err(_) => return internal_error("The grant couldn't be completed. Please try again."),
    };

    // The guard above bounds the role being *granted*; this bounds the *grantee*.
    // An account's Owner is never demoted through a grant — ownership only moves via
    // the separate transfer seam ("an Owner never has a parent, even when
    // transferred", DESIGN/Roles). Without this, an Admin could grant Manager to the
    // Owner's DID and quietly unseat them.
    match state.account_repo.role_of(grantee.id, account).await {
        Ok(Some(Role::Owner(_))) => return forbidden(),
        Ok(_) => {}
        Err(_) => return internal_error("The grant couldn't be completed. Please try again."),
    }

    // Settle the grant: upsert the membership in the private store.
    let member = UserAccount(grantee.id, account, new_role);
    if state.account_repo.grant_role(&member).await.is_err() {
        return internal_error("The grant couldn't be completed. Please try again.");
    }

    (
        StatusCode::OK,
        Json(json!({
            "account": account.to_string(),
            "user": grantee.did.as_str(),
            "role": member.get_role().as_str(),
        })),
    )
        .into_response()
}

/// The 500 a write endpoint returns when a dependency (the store, the recognizer)
/// fails — the request was fine, our side wasn't. Carries a steady, retryable message.
fn internal_error(reason: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": reason })),
    )
        .into_response()
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
