//! The composition root and HTTP surface: axum [`app`] router, session layer,
//! HTTP↔port translation. Live adapters are wired in `composition`
//! ([`Config`], [`AppState`]); domain logic lives in `domain`.

use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
};
use tower_http::set_header::SetResponseHeaderLayer;

/// The composition root shared with the CLI: the runtime [`Config`] and the
/// live-port bag, re-exported here as `AppState`.
pub use composition::{Config, Environment, Runtime as AppState};

/// The contract's generated message types: prost structs plus canonical
/// ProtoJSON serde, `@generated` by `contract-gen` — regenerate with
/// `just gen-contract`. Drift from `contract/zurfur/api/v1/*.proto` fails
/// the `contract_current` test.
pub mod generated;

mod extract;
mod problem;
mod routes;
mod sweep;

/// The canonical-ProtoJSON wire instant the generated types carry for every
/// `google.protobuf.Timestamp` field (`extern_path`'d there by `contract-gen`).
pub mod wire_time;

pub use sweep::run_deadline_sweeper;

/// Session key holding the recognized visitor's `UserId` (never the DID).
pub(crate) const SESSION_USER_KEY: &str = "user_id";

/// Builds the axum [`Router`], composing route groups from `routes`; the
/// canonical route table for tests and `main` (`main` additionally layers the
/// session middleware). The first-party-`Origin` CSRF guard and a
/// `Cache-Control: no-store` layer wrap the cookie surface only — never
/// `/health` or `/.well-known`.
pub fn app(state: AppState) -> Router {
    // no-store on the cookie surface so authenticated JSON is never cached (CWE-525).
    let no_store_cookie_surface = SetResponseHeaderLayer::if_not_present(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );

    // Cookie surface: session + accounts + commissions, under CSRF; no-store outermost.
    let cookie_surface = routes::session_router()
        .merge(routes::accounts_router())
        .merge(routes::commissions_router(
            // Checked, not `as`: an oversized cap must saturate, never silently truncate.
            usize::try_from(state.config.max_upload_bytes).unwrap_or(usize::MAX),
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::require_first_party_origin,
        ))
        .layer(no_store_cookie_surface);

    // /health and /.well-known mount top-level, outside CSRF (no cookie, no state change).
    Router::new()
        .merge(routes::health_router())
        .merge(routes::wellknown_router())
        .merge(cookie_surface)
        .with_state(state)
}
