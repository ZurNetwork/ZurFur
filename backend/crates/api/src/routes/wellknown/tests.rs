use super::*;

/// The configured namespace, the way `Config::handle_domain` now arrives:
/// already parsed and normalized.
fn domain(raw: &str) -> HandleDomain {
    raw.parse().expect("a valid handle domain")
}

#[test]
fn resolves_a_subdomain_of_the_handle_domain() {
    let h = handle_from_host("alice.zurfur.app", &domain("zurfur.app")).expect("a valid subdomain");
    assert_eq!(h.as_str(), "alice.zurfur.app");
}

#[test]
fn drops_an_optional_port() {
    let h =
        handle_from_host("alice.zurfur.app:443", &domain("zurfur.app")).expect("port is dropped");
    assert_eq!(h.as_str(), "alice.zurfur.app");
}

#[test]
fn normalizes_mixed_case_host() {
    let h = handle_from_host("Alice.Zurfur.App", &domain("zurfur.app")).expect("normalized");
    assert_eq!(h.as_str(), "alice.zurfur.app");
}

#[test]
fn strips_a_trailing_fqdn_dot() {
    let h =
        handle_from_host("alice.zurfur.app.", &domain("zurfur.app")).expect("trailing dot dropped");
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
    let h =
        handle_from_host("alice.zurfur.app", &domain("Zurfur.App.")).expect("normalized domain");
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
