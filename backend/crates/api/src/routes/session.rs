//! The session route group: the browser-facing sign-in flow.
//!
//! These endpoints (`GET /`, `POST /signin`, `GET /signin-callback`, `GET /me`,
//! `POST /logout`) speak HTML and redirect — an unrecognized visitor lands back
//! on the sign-in page rather than getting a `401`, because the visitor *browses*
//! to them. This is part of the cookie surface, so [`crate::app`] mounts the group
//! under the first-party-`Origin` (CSRF) layer.
//!
//! References: ZMVP-8 through ZMVP-11; DESIGN/Account.

use axum::{
    Form, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use domain::{
    elements::{did::Did, profile::Profile, user::UserId},
    ports::{ProfileCache, ProfileSource},
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, SESSION_USER_KEY};

/// The session route group: the HTML sign-in flow and the signed-in greeting.
/// Each route here is on the cookie surface; the composition root wraps the
/// group with the CSRF [`require_first_party_origin`](super::require_first_party_origin)
/// layer.
pub(crate) fn session_router() -> Router<AppState> {
    Router::new()
        .route("/", get(form))
        .route("/signin", post(signin))
        .route("/signin-callback", get(signin_callback))
        .route("/me", get(me))
        .route("/logout", post(logout))
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
/// for a DID via the [`Authenticator`], provisions the [`UserWrites`](domain::ports::UserWrites) User for that
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
/// References: [`Authenticator`], [`UserStore`](domain::ports::UserStore), [`me`], [`SESSION_USER_KEY`].
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
    // Recognition is a private-store write, so it goes through one unit of work.
    let provisioned = async {
        let mut uow = state.database.begin().await?;
        let user = uow.users().provision(&did).await?;
        uow.commit().await?;
        anyhow::Ok(user)
    }
    .await;
    let user = match provisioned {
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

/// The signed-in greeting (`GET /me`): resolves the session's [`UserId`] to a
/// User via the [`UserStore`](domain::ports::UserStore) (no PDS round trip — ZMVP-9 Criterion 3), then
/// renders the handle and avatar via [`resolve_profile`]/[`render_me`] (ZMVP-10).
///
/// Caveats: an anonymous visitor — no session, an expired one, or one whose User
/// no longer exists — is redirected to [`form`], not erred. A page, not an API:
/// it redirects rather than returning `401`. An unreachable PDS with nothing
/// cached still renders, falling back to the DID (absence is not an error).
///
/// References: [`UserStore`](domain::ports::UserStore), [`resolve_profile`], [`render_me`].
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
    let user = match state.users.find(UserId::new(id)).await {
        Ok(Some(user)) => user,
        _ => return Redirect::to("/").into_response(),
    };
    let profile = resolve_profile(&*state.profile_cache, &*state.profile_source, &user.did).await;
    Html(render_me(&user.did, profile.as_ref())).into_response()
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
