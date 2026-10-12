//! The atproto well-known route group: `GET /.well-known/atproto-did`,
//! handle→DID resolution for the configured handle namespace.
//! Reads `Host`, validates it's a claimable subdomain of `handle_domain`, and
//! returns the bare DID as `text/plain`, or `404`. No auth, no cookie; a single
//! private-store read (no PDS touch).

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use domain::elements::handle::{Handle, HandleDomain};

use crate::AppState;

/// The well-known route group: just `GET /.well-known/atproto-did`, mounted
/// top-level alongside (not under) the cookie-surface CSRF layer.
pub(crate) fn wellknown_router() -> Router<AppState> {
    Router::new().route("/.well-known/atproto-did", get(atproto_did))
}

/// Parses a request `Host` into the account [`Handle`] it addresses, or
/// `None` if the host isn't ours to resolve (not a subdomain of
/// `handle_domain`, a handle the namespace withholds, or malformed).
fn handle_from_host(host: &str, handle_domain: &HandleDomain) -> Option<Handle> {
    // Drop an optional :port; more than one colon is not a valid authority (fail closed).
    let host = match host.split_once(':') {
        None => host,
        Some((h, port)) if !h.is_empty() && !port.contains(':') => h,
        Some(_) => return None,
    };
    // Drop a trailing FQDN-root dot, consistent with Handle's FromStr normalization.
    let host = host.strip_suffix('.').unwrap_or(host);
    let handle = host.parse::<Handle>().ok()?;
    // Never the apex or a foreign authority — a BYO-domain handle resolves at its own domain.
    if !handle.is_in_namespace(handle_domain) {
        return None;
    }
    // Never a reserved label: no account can hold one, so no lookup is made.
    handle.ensure_unreserved(handle_domain).ok()?;
    Some(handle)
}

/// `GET /.well-known/atproto-did` — resolves the `Host` header's handle to its
/// account's `did:plc`, `text/plain` (`200`).
///
/// - `404` — `Host` not a claimable subdomain of `handle_domain`, or no live account holds it
async fn atproto_did(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let Some(handle) = handle_from_host(host, &state.config.handle_domain) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    match state.accounts.find_did_by_handle(&handle).await {
        Ok(Some(did)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            did.as_ref().to_owned(),
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        // A store failure is a 500 (the request was fine); the resolver may retry.
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests;
