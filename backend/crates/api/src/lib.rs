//! The composition root and HTTP surface of the Zurfur backend.
//!
//! The HTTP driving adapter. Which adapters are live is decided once, in the
//! shared `composition` crate ([`Config`], the [`AppState`] bag of ports —
//! `composition::Runtime` re-exported — and its live wiring); this crate owns
//! only the axum [`app`] router, the session layer, and the HTTP↔port
//! translation. Domain logic lives in `domain`; persistence and the PDS live
//! behind the `adapter-*` crates.
//!
//! The HTTP surface is split into per-domain route groups under [`mod@routes`]
//! (`health`, `session`, `accounts`, `commissions`), each exposing a `*_router()` builder;
//! [`app`] is pure composition that merges them. Two shapes of endpoint coexist.
//! The browser-facing sign-in flow (`/signin`, `/signin-callback`, `/me`,
//! `/logout`) redirects or speaks JSON; the human-facing HTML lives in the
//! SvelteKit frontend (ZMVP-151). The account/membership API (`POST /accounts`,
//! `.../members`, `.../invitations`) speaks JSON and returns status codes — an
//! unrecognized caller gets a `401`, never a redirect, because the frontend calls
//! these rather than browsing to them.
//!
//! References: DESIGN "Domains and Applications" (ports and adapters);
//! DESIGN/Account, DESIGN/Roles; ZMVP-8 through ZMVP-16; ZMVP-151.

use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
};
use tower_http::set_header::SetResponseHeaderLayer;

pub(crate) use application::transaction;
/// The composition root is shared with the CLI (ZMVP-200): the runtime
/// [`Config`], the custody guard, and the live-port bag — re-exported so this
/// crate's handlers and tests keep naming the bag `AppState`.
pub use composition::{Config, Environment, Runtime as AppState};

/// The contract's generated message types (DD 40992770; `@generated` by
/// `contract-gen`, regenerate with `just gen-contract`). Drift from
/// `contract/zurfur/api/v1/*.proto` fails the `contract_current` test.
pub mod generated;

mod problem;
mod routes;
mod sweep;

/// The canonical-ProtoJSON wire instant the generated types carry for every
/// `google.protobuf.Timestamp` field (`extern_path`'d there by `contract-gen`).
pub mod wire_time;

pub use sweep::{run_deadline_sweeper, sweep_deadlines};

/// Session key under which the recognized visitor's `UserId` is stored. The
/// session carries our own key, not the DID: subsequent requests resolve
/// session → User through the repo, never re-asking the PDS (ZMVP-9 Criterion 3).
pub(crate) const SESSION_USER_KEY: &str = "user_id";

/// Builds the axum [`Router`] over an [`AppState`], composing the per-domain route
/// groups from [`mod@routes`]. This is the canonical route table; the e2e tests and
/// `main` both mount it. `main` additionally layers the session middleware (the
/// [`Session`](tower_sessions::Session) extractor handlers rely on comes from that
/// layer, applied outside this fn).
///
/// Composition follows DESIGN "Domains and Applications": each area exposes a
/// `*_router()` builder and this fn merges them. A namespace boundary is also a
/// **policy boundary**, so the CSRF [`require_first_party_origin`](routes::require_first_party_origin)
/// guard is layered over the **cookie surface only** — `session` + `accounts` — and
/// not over `/health`, nor (in future) over the bearer `/plugin/v1` namespace, which
/// authenticates by `app_key` and is exempt by construction (ZMVP-23, DD "Auth
/// Surfaces, the Plugin Trust Boundary & CSRF"). That same cookie surface also carries
/// a `Cache-Control: no-store` response layer so authenticated identity/PII JSON is
/// never cached by a browser or shared intermediary (CWE-525, ZMVP-151); the public
/// `/health` and `/.well-known` GETs stay cacheable.
///
/// Routes: `GET /health`; `GET /.well-known/atproto-did` (handle resolution, also
/// top-level and CSRF-exempt); the sign-in flow (`POST /signin`,
/// `GET /signin-callback`, `GET /me`, `POST /logout`); the accounts tree
/// (`POST /accounts`, `POST`/`DELETE /accounts/{id}/members`,
/// `DELETE /accounts/{id}/members/me`, `POST`/`DELETE /accounts/{id}/invitations`,
/// `POST /accounts/{id}/invitations/decline`, `POST /accounts/{id}/invitations/accept`);
/// and the commissions tree (`POST /commissions` — user-scoped, no Account
/// required; `GET /commissions/{id}/changelog`, `POST /commissions/{id}/notes`,
/// `PUT`/`DELETE /commissions/{id}/channel` — participant-gated behind the
/// closed-door uniform 404, ZMVP-87).
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
/// References: [`AppState`]; the per-group docs under [`mod@routes`].
///
/// ```ignore
/// let router = api::app(state).layer(session_layer);
/// ```
pub fn app(state: AppState) -> Router {
    // Stamp `Cache-Control: no-store` on every cookie-surface response so a browser
    // or shared intermediary never caches authenticated identity/PII JSON — e.g.
    // `GET /me` (CWE-525, ZMVP-151 security review). `if_not_present` yields to any
    // handler that sets its own cache policy (none do today). Scoped to the cookie
    // surface deliberately: the public `/health` and `/.well-known` GETs are left
    // cacheable — over-scoping to them was called out in review.
    let no_store_cookie_surface = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );

    // The cookie surface: the browser/session flow and the account API, both reached
    // with the ambient session cookie. The first-party-`Origin` (CSRF) guard wraps
    // this surface once — a state-changing request from a foreign `Origin` is refused —
    // and the no-store layer sits outermost so even a CSRF rejection is uncacheable.
    let cookie_surface = routes::session_router()
        .merge(routes::accounts_router())
        .merge(routes::commissions_router(
            // Checked, not `as`: on a 32-bit target an oversized configured cap
            // saturates to usize::MAX instead of silently truncating the limit.
            usize::try_from(state.config.max_upload_bytes).unwrap_or(usize::MAX),
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::require_first_party_origin,
        ))
        .layer(no_store_cookie_surface);

    // `/health` and the atproto `/.well-known/atproto-did` resolver are mounted
    // top-level, deliberately outside the CSRF layer (they bear no cookie and change
    // no state — the resolver is a public unauthenticated GET). The future bearer
    // `/plugin/v1` namespace nests here too, exempt by construction rather than by a
    // remembered carve-out.
    Router::new()
        .merge(routes::health_router())
        .merge(routes::wellknown_router())
        .merge(cookie_surface)
        .with_state(state)
}
