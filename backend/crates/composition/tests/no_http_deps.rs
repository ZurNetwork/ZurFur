//! The dependency guard (ZMVP-200, board finding): `composition` is the
//! composition root for EVERY driving adapter, including the non-HTTP `cli`, so
//! it must never link a web framework. Runs `cargo tree` over this crate's
//! normal (non-dev) dependency graph and refuses any HTTP-stack crate in it.
//! A trip here means a driven adapter grew an HTTP dependency it must not have
//! — fix the adapter, never this list.

use std::process::Command;

/// Crates that belong to an HTTP *server* driver, never to the composition
/// root or the adapters beneath it. Client-side HTTP (`reqwest`, and the
/// `tower-http` it pulls) is legitimately an adapter's — `adapter-atproto`
/// talks to the PDS — so it is not on this list.
const FORBIDDEN: &[&str] = &["axum", "axum-core", "tower-sessions"];

#[test]
fn composition_links_no_http_stack() {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--package",
            "composition",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--locked",
        ])
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("utf-8 tree");
    assert!(!tree.trim().is_empty(), "cargo tree printed nothing");

    let offenders: Vec<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| FORBIDDEN.contains(name))
        .collect();
    assert!(
        offenders.is_empty(),
        "composition must stay HTTP-free but links {offenders:?}\n{tree}"
    );
}
