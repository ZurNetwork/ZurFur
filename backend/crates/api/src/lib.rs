use std::net::SocketAddr;

use adapter_atproto::{complete_callback, get_oauth_url};
use adapter_pg::PgPool;
use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;

// The OAuth client type is owned by the adapter (it names jacquard's resolver +
// store); the composition root re-exports it rather than restating those types.
pub use adapter_atproto::Oauth;

/// Session key under which the signed-in visitor's DID is stored.
const SESSION_DID_KEY: &str = "did";

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
    pub oauth: Oauth,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(form))
        .route("/signin", post(signin))
        .route("/signin-callback", get(signin_callback))
        .route("/me", get(me))
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
    match get_oauth_url(&state.oauth, &f.handle).await {
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

    let did = match complete_callback(&state.oauth, code, q.state, q.iss).await {
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

    // "Signed in" at this ticket's scope means a session holding the DID exists.
    // Persist it, then hand off to the greeting route; the cookie now survives reload.
    if session.insert(SESSION_DID_KEY, did).await.is_err() {
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

/// Greets the signed-in visitor by the DID held in their session. An anonymous
/// visitor — no session, or one that expired — is sent back to the sign-in page.
async fn me(session: Session) -> Response {
    match session.get::<String>(SESSION_DID_KEY).await {
        Ok(Some(did)) => Html(format!("Signed in as {}", escape(&did))).into_response(),
        _ => Redirect::to("/").into_response(),
    }
}
