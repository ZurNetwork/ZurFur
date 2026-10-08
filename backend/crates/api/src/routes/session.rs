//! The session route group: the browser OAuth sign-in flow — `POST /signin`,
//! `GET /signin-callback`, `GET /me`, `POST /logout`. `/signin` and
//! `/signin-callback` redirect the browser; `/me` is JSON whoami (`401` on no
//! session, not a redirect). Callback failures redirect to `/login` with a
//! stable `error=<code>`, never the PDS-supplied reason.

use application::user::me::{self, MeError, MeQuery};
use axum::{
    Form, Json, Router,
    extract::{Query, State, rejection::FormRejection},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use domain::elements::handle::AtHandle;
use domain::ports::{AccountMismatch, UnitOfWork};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    AppState, SESSION_USER_KEY, extract::CallingUser, generated::GetMeResponse, problem::Problem,
};

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
/// `you.bsky.social`).
#[derive(Deserialize)]
struct SigninForm {
    handle: String,
}

/// The PDS's redirect-back query params, all optional: success carries `code`
/// (+ `state`/`iss`); denial carries `error` and no `code`.
#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
    error: Option<String>,
}

/// `POST /signin` (form body) — trims what the visitor typed, parses it as a
/// handle, and redirects the browser to the PDS authorization URL the
/// Authenticator returns.
///
/// - `303` → PDS authorize URL
/// - `422 invalid_request` — a body that is not a form carrying `handle`, not a
///   handle (a URL or a DID included), or a handle that could not begin
///   sign-in; one identical body for every cause
async fn signin(
    State(state): State<AppState>,
    form: Result<Form<SigninForm>, FormRejection>,
) -> Result<Redirect, Problem> {
    // An unreadable body is refused like every other cause, never with axum's own text.
    let Form(form) = form.map_err(|_| signin_refused())?;
    // The handle type is strict; trimming what a person typed happens here.
    let handle = form
        .handle
        .trim()
        .parse::<AtHandle>()
        .map_err(|_| signin_refused())?;
    let url = state
        .auth
        .start(&handle)
        .await
        .map_err(|_| signin_refused())?;
    Ok(Redirect::to(&url))
}

/// The one answer a failed `POST /signin` gets, whatever failed: an anonymous
/// visitor learns nothing about why.
fn signin_refused() -> Problem {
    Problem::invalid_request("That handle could not be used to sign in. Check it and try again.")
}

/// `GET /signin-callback` — completes sign-in: exchanges `code` for a DID,
/// provisions the User (mint-or-return), rotates the session id, and stores
/// the User's id in the session.
///
/// - `303 /` — success (`Set-Cookie` on the response)
/// - `303 /login?error=denied|invalid_callback|exchange_failed|account_mismatch`
///   — failure modes; `account_mismatch` when the visitor signed in to a
///   different account than the handle they typed
/// - `500 internal_error` — provisioning or session-write failure
async fn signin_callback(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<CallbackQuery>,
) -> Response {
    // Denied: no crash, no blank page — the PDS-supplied reason isn't echoed.
    if q.error.is_some() {
        return Redirect::to("/login?error=denied").into_response();
    }
    let Some(code) = q.code else {
        return Redirect::to("/login?error=invalid_callback").into_response();
    };

    let completed = state.auth.complete(code, q.state, q.iss).await;
    let did = match completed {
        Ok(did) => did,
        Err(error) => return Redirect::to(callback_failure_location(&error)).into_response(),
    };

    // Mint-or-return: recognizes rather than registers (idempotent, one DID = one User).
    let provisioned = state
        .transaction(async move |uow: &mut dyn UnitOfWork| uow.users().provision(&did).await)
        .await;
    let Ok(user) = provisioned else {
        return Problem::internal_error(
            "Your sign-in succeeded but your account couldn't be set up. Please try again.",
        )
        .into_response();
    };

    // Rotate the session id at this privilege change (session-fixation hardening).
    if session.cycle_id().await.is_err()
        || session
            .insert(SESSION_USER_KEY, user.id.did())
            .await
            .is_err()
    {
        return Problem::internal_error(
            "Your sign-in succeeded but the session couldn't be saved. Please try again.",
        )
        .into_response();
    }
    Redirect::to("/").into_response()
}

/// Where a failed code exchange sends the visitor: its own code for the wrong
/// account, the generic one for everything else.
fn callback_failure_location(error: &anyhow::Error) -> &'static str {
    if error.is::<AccountMismatch>() {
        "/login?error=account_mismatch"
    } else {
        "/login?error=exchange_failed"
    }
}

/// `GET /me` — the JSON whoami: resolves the session to a User, no PDS round trip.
///
/// - `200` — DID plus profile fields (unresolved profile omits the keys, not an error)
/// - `401 not_authenticated` — no/expired session, or its User no longer exists
async fn me(
    State(state): State<AppState>,
    CallingUser(user_id): CallingUser,
) -> Result<Json<GetMeResponse>, Problem> {
    let query = MeQuery { user_id };

    let me = state
        .app()
        .users()
        .me(query, &*state.profile_cache, &*state.profile_source)
        .await
        .map_err(|err| match err {
            MeError::UnknownUser(_) => Problem::not_authenticated(),
            MeError::Store(err) => Problem::from(err),
        })?;

    let body = GetMeResponse::from(me);
    Ok(Json(body))
}

/// The `GET /me` projection: a resolved profile contributes handle/name/avatar;
/// no profile degrades to the bare DID (absence is not an error).
impl From<me::Output> for GetMeResponse {
    fn from(me: me::Output) -> Self {
        let did = me.id.to_string();
        match me.profile {
            Some(profile) => GetMeResponse {
                did,
                handle: Some(profile.handle.to_string()),
                display_name: profile.display_name,
                avatar_url: profile.avatar_url,
            },
            None => GetMeResponse {
                did,
                handle: None,
                display_name: None,
                avatar_url: None,
            },
        }
    }
}

/// `POST /logout` — destroys the session server-side (store row + cookie), so a
/// stolen cookie dies with it. A stale/already-gone session still succeeds.
///
/// - `303 /` — success (including a repeat sign-out) · `500` — store failure
async fn logout(session: Session) -> Response {
    if session.flush().await.is_err() {
        return Problem::internal_error("Sign-out couldn't be completed. Please try again.")
            .into_response();
    }
    Redirect::to("/").into_response()
}
