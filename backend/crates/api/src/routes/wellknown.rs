//! The atproto well-known route group: handle → DID resolution for the
//! Zurfur-issued `*.zurfur.app` namespace (ZMVP-44, DD/26607618 "Handle
//! Resolution for *.zurfur.app — HTTPS well-known").
//!
//! A client resolving the atproto handle `alice.zurfur.app` fetches
//! `https://alice.zurfur.app/.well-known/atproto-did`; the `Host` header carries
//! the handle. This endpoint reads that `Host`, confirms it is a subdomain of the
//! configured [`handle_domain`](crate::Config::handle_domain), normalizes it
//! through the shared [`Handle`] gate, looks up the account's `did:plc` in the
//! private store, and returns the **bare DID as `text/plain`** (HTTP 200), or
//! `404` when nothing resolves.
//!
//! It carries no auth, changes no state, and bears no cookie — so [`crate::app`]
//! mounts it top-level, deliberately *outside* the cookie-surface CSRF layer (like
//! `/health`). Resolution is a single private-store read: no PDS touch, so this
//! never becomes a cross-store transaction. BYO-domain handles resolve at the
//! owner's own domain, never here — the `handle_domain` suffix gate makes that
//! explicit (a request for any other authority is not ours to answer).

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use domain::elements::handle::Handle;

use crate::AppState;

/// The well-known route group: just `GET /.well-known/atproto-did`. Its own
/// builder so the composition root can mount it top-level, alongside (not under)
/// the cookie-surface CSRF layer — a resolver carries no `Origin` and no session.
pub(crate) fn wellknown_router() -> Router<AppState> {
    Router::new().route("/.well-known/atproto-did", get(atproto_did))
}

/// Parse a request `Host` into the account [`Handle`] it addresses, or `None` if
/// the host is not ours to resolve.
///
/// The host must be a **subdomain** of `handle_domain` (it ends with
/// `.{handle_domain}`) — the apex itself, or any other authority, yields `None`, so
/// we only ever answer for handles in the Zurfur-issued namespace. Any optional
/// `:port` is dropped and the whole host is normalized/validated through the shared
/// [`Handle`] gate, so a punycode or otherwise malformed host resolves to `None`
/// (and is answered `404`) rather than reaching the store.
fn handle_from_host(host: &str, handle_domain: &str) -> Option<Handle> {
    // Drop an optional `:port`. Handles are domain names (no colon), so the first
    // segment is the authority.
    let host = host.split(':').next().unwrap_or(host);
    // Drop a single FQDN-root trailing dot so `alice.zurfur.app.` resolves the same
    // as `alice.zurfur.app` — consistent with `Handle::try_new`'s normalization.
    let host = host.strip_suffix('.').unwrap_or(host);
    // Only answer for a subdomain of our handle namespace — never the apex, never a
    // foreign authority (a BYO-domain handle resolves at its own domain, not here).
    let suffix = format!(".{handle_domain}");
    if !host
        .to_ascii_lowercase()
        .ends_with(&suffix.to_ascii_lowercase())
    {
        return None;
    }
    // Normalize + validate the whole host as a handle; a bad one (punycode,
    // reserved label, malformed) is not a resolvable handle.
    Handle::try_new(host).ok()
}

/// `GET /.well-known/atproto-did` — resolve a Zurfur-issued handle (carried in the
/// `Host` header) to its account's `did:plc`, returned as a bare `text/plain` body
/// (HTTP 200). A `Host` that is not a subdomain of
/// [`handle_domain`](crate::Config::handle_domain), or that no live account holds,
/// is `404`. No auth; a single private-store read (no PDS touch).
///
/// ```text
/// GET /.well-known/atproto-did   Host: alice.zurfur.app
/// → 200 text/plain  did:plc:abc123        (alice's account DID)
/// → 404                                   (unknown handle, or a foreign Host)
/// ```
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
            did.as_str().to_owned(),
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        // A store failure is a 500 (the request was fine); the resolver may retry.
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_subdomain_of_the_handle_domain() {
        let h = handle_from_host("alice.zurfur.app", "zurfur.app").expect("a valid subdomain");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn drops_an_optional_port() {
        let h = handle_from_host("alice.zurfur.app:443", "zurfur.app").expect("port is dropped");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn normalizes_mixed_case_host() {
        let h = handle_from_host("Alice.Zurfur.App", "zurfur.app").expect("normalized");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn strips_a_trailing_fqdn_dot() {
        let h = handle_from_host("alice.zurfur.app.", "zurfur.app").expect("trailing dot dropped");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn refuses_the_apex_itself() {
        assert!(handle_from_host("zurfur.app", "zurfur.app").is_none());
    }

    #[test]
    fn refuses_a_foreign_authority() {
        assert!(handle_from_host("alice.example.com", "zurfur.app").is_none());
        // A look-alike that only contains the domain mid-string is still refused.
        assert!(handle_from_host("zurfur.app.evil.com", "zurfur.app").is_none());
    }

    #[test]
    fn refuses_a_punycode_host() {
        assert!(handle_from_host("xn--80ak6aa92e.zurfur.app", "zurfur.app").is_none());
    }
}
