//! HTTP route groups, split along the domain/namespace seams that DESIGN
//! "Domains and Applications" (`11763713`) prescribes: subdomain → namespace →
//! router. Each group exposes a `*_router()` builder returning a
//! [`Router<AppState>`](axum::Router); [`crate::app`] is pure composition that
//! merges them and attaches state.
//!
//! The split is the intermediate step toward the eventual per-domain crates
//! (`identity`, `gallery`, `workflow`, `plugin`): keeping each area in its own
//! module here means the future crate extraction is a *move*, not a redesign, and
//! a new domain adds its own module + a line in [`crate::app`] without touching
//! the others (the flat router was becoming a merge-conflict hotspot — ZMVP-39).
//!
//! A namespace boundary is also a **policy boundary**. The cookie surface
//! ([`session_router`] + [`accounts_router`]) is wrapped by
//! [`require_first_party_origin`] (CSRF defense-in-depth, ZMVP-23); [`health_router`]
//! is mounted *outside* it, and the future bearer `/plugin/v1` namespace — which
//! authenticates by `app_key`, not cookie, and so cannot be CSRF'd — will likewise
//! nest top-level, exempt by construction rather than by a remembered carve-out.
//!
//! References: DESIGN "Domains and Applications"; DESIGN "Auth Surfaces, the Plugin
//! Trust Boundary & CSRF".

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

/// CSRF defense-in-depth on the cookie surface (ZMVP-23): on a state-changing
/// method, reject a request whose `Origin` header is present and is **not** our
/// first-party origin ([`Config::public_url`](crate::Config::public_url)). A missing
/// `Origin` — a non-browser client, which carries no ambient cookie and so cannot be
/// CSRF'd — passes, as do safe methods (`GET`/`HEAD`/…). This layers on top of the
/// session cookie's `SameSite=Lax`; together they keep a forged cross-site request
/// from acting with the user's session.
///
/// [`crate::app`] applies this as a layer over the cookie sub-routers only — not over
/// [`health_router`], and not over the future bearer public API (`/plugin/v1`), which
/// is exempt by construction (no ambient cookie). See DESIGN "Auth Surfaces, the
/// Plugin Trust Boundary & CSRF".
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
