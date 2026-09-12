//! HTTP route groups, split along the domain/namespace seams (subdomain →
//! namespace → router). Each exposes
//! a `*_router()`; a namespace boundary is also a policy boundary — the
//! cookie surface is wrapped by [`require_first_party_origin`] (CSRF), while
//! `health`/`wellknown` mount outside it.

use axum::{
    extract::{Request, State},
    http::{Method, header::ORIGIN},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::AppState;
use crate::problem::Problem;

mod accounts;
mod commissions;
mod health;
mod session;
mod wellknown;

pub(crate) use accounts::accounts_router;
pub(crate) use commissions::commissions_router;
pub(crate) use health::health_router;
pub(crate) use session::session_router;
pub(crate) use wellknown::wellknown_router;

/// CSRF defense-in-depth on the cookie surface: rejects a state-changing
/// request (`POST`/`PUT`/`PATCH`/`DELETE`) whose `Origin` header is present
/// and isn't [`Config::public_url`](crate::Config::public_url). A missing
/// `Origin` or a safe method passes.
pub(crate) async fn require_first_party_origin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let state_changing = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if state_changing
        && let Some(origin) = request.headers().get(ORIGIN)
        && origin.as_bytes() != state.config.public_url.trim_end_matches('/').as_bytes()
    {
        return Problem::cross_origin().into_response();
    }
    next.run(request).await
}
