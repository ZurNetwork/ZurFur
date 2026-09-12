//! The atproto well-known route group: `GET /.well-known/atproto-did`,
//! handle→DID resolution for the `*.zurfur.app` namespace (DD 26607618).
//! Reads `Host`, validates it's a subdomain of `handle_domain`, and returns
//! the bare DID as `text/plain`, or `404`. No auth, no cookie; a single
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
/// `handle_domain`, or malformed).
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
    handle.is_in_namespace(handle_domain).then_some(handle)
}

/// `GET /.well-known/atproto-did` — resolves the `Host` header's handle to its
/// account's `did:plc`, `text/plain` (`200`).
///
/// - `404` — `Host` not a subdomain of `handle_domain`, or no live account holds it
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

    /// The configured namespace, the way `Config::handle_domain` now arrives:
    /// already parsed and normalized.
    fn domain(raw: &str) -> HandleDomain {
        raw.parse().expect("a valid handle domain")
    }

    #[test]
    fn resolves_a_subdomain_of_the_handle_domain() {
        let h =
            handle_from_host("alice.zurfur.app", &domain("zurfur.app")).expect("a valid subdomain");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn drops_an_optional_port() {
        let h = handle_from_host("alice.zurfur.app:443", &domain("zurfur.app"))
            .expect("port is dropped");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn normalizes_mixed_case_host() {
        let h = handle_from_host("Alice.Zurfur.App", &domain("zurfur.app")).expect("normalized");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn strips_a_trailing_fqdn_dot() {
        let h = handle_from_host("alice.zurfur.app.", &domain("zurfur.app"))
            .expect("trailing dot dropped");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn refuses_the_apex_itself() {
        assert!(handle_from_host("zurfur.app", &domain("zurfur.app")).is_none());
    }

    #[test]
    fn a_trailing_dot_or_cased_handle_domain_still_resolves() {
        // The same normalizer the claim checks use: a config value like
        // `Zurfur.App.` must not silently kill resolution platform-wide.
        let h = handle_from_host("alice.zurfur.app", &domain("Zurfur.App."))
            .expect("normalized domain");
        assert_eq!(h.as_str(), "alice.zurfur.app");
    }

    #[test]
    fn refuses_a_foreign_authority() {
        assert!(handle_from_host("alice.example.com", &domain("zurfur.app")).is_none());
        // A look-alike that only contains the domain mid-string is still refused.
        assert!(handle_from_host("zurfur.app.evil.com", &domain("zurfur.app")).is_none());
    }

    #[test]
    fn refuses_a_dot_boundary_near_miss() {
        // The suffix gate requires a real label boundary (a leading dot): a host that
        // ends in `-zurfur.app` or `xzurfur.app` is NOT a subdomain of `zurfur.app`.
        assert!(handle_from_host("evil-zurfur.app", &domain("zurfur.app")).is_none());
        assert!(handle_from_host("notzurfur.app", &domain("zurfur.app")).is_none());
    }

    #[test]
    fn refuses_a_multi_colon_authority() {
        // `host:port:garbage` is malformed — fail closed rather than take a prefix.
        assert!(handle_from_host("alice.zurfur.app:443:garbage", &domain("zurfur.app")).is_none());
    }

    #[test]
    fn refuses_an_ipv6_authority() {
        // An IPv6 literal is not a handle authority.
        assert!(handle_from_host("[::1]:443", &domain("zurfur.app")).is_none());
    }

    #[test]
    fn refuses_a_punycode_host() {
        assert!(handle_from_host("xn--80ak6aa92e.zurfur.app", &domain("zurfur.app")).is_none());
    }
}
