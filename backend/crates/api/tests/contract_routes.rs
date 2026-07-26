//! The router-vs-contract weld (DD 40992770 decision 12).
//!
//! `google.api.http` annotations in `contract/zurfur/api/v1/*.proto` *declare*
//! each endpoint's path and verb; nothing else connects that declaration to
//! axum's real route table, so a renamed path would compile while the contract
//! lied. This test holds the weld: every route the contract declares must be
//! served, at the declared verb, under the declared path-major.
//!
//! **Text-level by design.** The `.proto` files are parsed textually for the
//! HttpRule options. ZMVP-160's prost adoption deliberately did NOT upgrade
//! this: the generated code carries no service machinery at all (`NoServices`,
//! DD 40992770 decision 3), so there is no compiled descriptor at test time —
//! and compiling one here (protox) would add machinery for route metadata
//! only. The assertion's shape (every declared route served, under the
//! path-major) is the load-bearing part; the parse is the lightest tool that
//! feeds it.
//!
//! The `/api/v1` prefix is STRIPPED by the proxy layer (Caddy in dev, mirrored
//! by SvelteKit's `handleFetch`) — axum's own table is unprefixed. The strip
//! constant lives here, asserted against the contract's package version, so
//! the path-major ⇄ proto-package weld (`package zurfur.api.v1` ⇔ `/api/v1`)
//! is checked even though axum never sees the prefix.

use std::collections::BTreeSet;

/// The one strip constant: what the proxy removes before axum routes. Bound to
/// the contract's proto package (`zurfur.api.v1` ⇒ `/api/v1`) — the assertion
/// below fails if either side moves alone.
const STRIP_PREFIX: &str = "/api/v1";

/// Where the corpus lives, relative to this crate's manifest.
const CONTRACT_DIR: &str = "../../../contract/zurfur/api/v1";

/// Parse every `(google.api.http)` option in the corpus into `(verb, path)`.
fn declared_routes() -> BTreeSet<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTRACT_DIR);
    let mut routes = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("contract dir readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("proto") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("proto readable");
        for line in source.lines() {
            let line = line.trim();
            for verb in ["get", "post", "patch", "delete", "put"] {
                let Some(rest) = line.strip_prefix(&format!("{verb}: \"")) else {
                    continue;
                };
                let Some(route) = rest.strip_suffix('"') else {
                    continue;
                };
                routes.insert((verb.to_uppercase(), route.to_string()));
            }
        }
    }
    assert!(
        !routes.is_empty(),
        "no HttpRule routes parsed from {dir:?} — the contract moved or the \
         option format changed; fix the parser, do not delete the weld"
    );
    routes
}

/// The package declared by the corpus — the other half of the path-major weld.
fn declared_package() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(CONTRACT_DIR);
    for entry in std::fs::read_dir(&dir).expect("contract dir readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("proto") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("proto readable");
        for line in source.lines() {
            if let Some(package) = line.trim().strip_prefix("package ") {
                return package.trim_end_matches(';').to_string();
            }
        }
    }
    panic!("no package declaration found in the corpus");
}

/// The path-major ⇄ proto-package weld: `zurfur.api.v1` ⇔ `/api/v1`. Cutting
/// v2 must change both in one move; this is what makes forgetting one a test
/// failure instead of a lie.
#[test]
fn the_path_major_matches_the_proto_package() {
    let package = declared_package();
    let major = package
        .rsplit('.')
        .next()
        .expect("package has a version segment");
    assert_eq!(
        STRIP_PREFIX,
        format!("/api/{major}"),
        "the proxy strip prefix and the proto package version must agree \
         (DD 40992770 decision 9: one number in two checkable places)"
    );
}

/// Every declared route is under the path-major, and every declared route is
/// one axum actually serves (after the proxy strip). The served set is pinned
/// literally: axum's `Router` exposes no route iterator, so the pin is the
/// route table transcribed — updating it is part of adding an endpoint, which
/// is exactly the contract-first discipline (contract change → this test →
/// route). A route served but not declared does NOT fail here (the pre-GA
/// remainder of the surface is not yet in the corpus); a route declared but
/// not served does.
#[test]
fn every_declared_route_is_served() {
    let declared = declared_routes();

    // The cookie-surface routes axum serves today, post-strip. Transcribed
    // from `routes/session.rs`, `routes/accounts.rs`, `routes/commissions/mod.rs`.
    let served: BTreeSet<(String, String)> = [
        ("GET", "/me"),
        ("POST", "/signin"),
        ("POST", "/logout"),
        ("GET", "/accounts"),
        ("POST", "/accounts"),
        ("DELETE", "/accounts/{id}"),
        ("PATCH", "/accounts/{id}/handle"),
        ("GET", "/commissions"),
        ("POST", "/commissions"),
    ]
    .into_iter()
    .map(|(verb, path)| (verb.to_string(), path.to_string()))
    .collect();

    for (verb, declared_path) in &declared {
        let stripped = declared_path.strip_prefix(STRIP_PREFIX).unwrap_or_else(|| {
            panic!(
                "{verb} {declared_path}: every contract route must live under \
                     the path-major {STRIP_PREFIX} (DD 40992770 decision 9)"
            )
        });
        assert!(
            served.contains(&(verb.clone(), stripped.to_string())),
            "{verb} {declared_path}: declared by the contract but not served by \
             axum (post-strip: {stripped}) — the contract is lying about a route; \
             either serve it or remove the declaration"
        );
    }

    // And the declared set covers the whole v1 surface — nine endpoints.
    assert_eq!(
        declared.len(),
        9,
        "the v1 corpus declares nine endpoints (Engineer scope ruling \
         2026-07-25); got {declared:?}"
    );
}
