//! The dependency rule (ZMVP-205 AC1): `domain` never depends on
//! `application`, and `application` links no adapter, no composition root,
//! and no HTTP stack. `cargo tree` over the normal dependency graph is the
//! witness.
//!
//! The **dev** graph is deliberately cyclic — `application` dev-depends on
//! `test-support`, which links `composition`, which links `application` (for
//! `Runtime::transaction`). Cargo allows it; the guard checks `--edges normal`
//! on purpose, so the rule is about what ships, not what the tests boot.

use std::process::Command;

/// What `application` may never pull in.
const FORBIDDEN_IN_APPLICATION: &[&str] = &[
    "adapter-pg",
    "adapter-atproto",
    "adapter-mem",
    "composition",
    "axum",
    "axum-core",
    "tower-sessions",
];

#[test]
fn domain_does_not_depend_on_application() {
    let tree = normal_tree("domain");
    assert!(
        !tree
            .lines()
            .any(|line| line.split_whitespace().next() == Some("application")),
        "domain must not depend on application\n{tree}"
    );
}

#[test]
fn application_links_only_the_domain() {
    let tree = normal_tree("application");
    let offenders: Vec<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| FORBIDDEN_IN_APPLICATION.contains(name))
        .collect();
    assert!(
        offenders.is_empty(),
        "application must stay adapter- and HTTP-free but links {offenders:?}\n{tree}"
    );
}

fn normal_tree(package: &str) -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--package",
            package,
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
    tree
}
