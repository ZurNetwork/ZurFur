//! Guard: the domain crates point one way.
//!
//! Each domain crate declares only the workspace crates below it, of every
//! dependency kind (dev included, so no test or doctest leans on a higher
//! crate), and no other workspace crate names a domain crate but the `domain`
//! façade. `cargo metadata` is the witness: it reads the manifests, so an edge
//! is caught even before any code uses it.

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

/// Each domain crate and the workspace crates it may declare, of any kind.
/// Every other workspace crate is forbidden to it, itself and the façade
/// included. Gallery and moderation widen deliberately with their first types.
const ALLOWED: &[(&str, &[&str])] = &[
    ("kernel", &["shared"]),
    ("identity", &["kernel", "shared"]),
    ("workflow", &["kernel", "identity", "shared"]),
    ("gallery", &["kernel", "shared"]),
    ("moderation", &["kernel", "shared"]),
    ("domain", &["kernel", "identity", "workflow", "shared"]),
];

/// The domain crates behind the façade: no crate outside the domain crates may
/// declare one.
const BEHIND_THE_FACADE: &[&str] = &["kernel", "identity", "workflow", "gallery", "moderation"];

/// Each workspace member's name, with the workspace members it declares as
/// dependencies of any kind.
type Declared = BTreeMap<String, BTreeSet<String>>;

/// Every workspace member and the workspace members it declares, read from
/// `cargo metadata` without resolving the dependency graph.
fn workspace_declarations() -> Declared {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
            "--manifest-path",
            manifest,
        ])
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata prints JSON");
    let packages = metadata["packages"].as_array().expect("a package list");
    let name = |value: &serde_json::Value| value["name"].as_str().expect("a name").to_owned();
    let members: BTreeSet<String> = packages.iter().map(name).collect();
    packages
        .iter()
        .map(|package| {
            let dependencies = package["dependencies"]
                .as_array()
                .expect("a dependency list");
            let inside = dependencies
                .iter()
                .map(name)
                .filter(|dependency| members.contains(dependency))
                .collect();
            (name(package), inside)
        })
        .collect()
}

/// Every declared edge from a domain crate to a workspace crate outside its
/// allowed set, as `from -> to`.
fn edges_outside_the_allowed_sets(declared: &Declared) -> Vec<String> {
    let mut offenders = Vec::new();
    for (name, allowed) in ALLOWED {
        let Some(dependencies) = declared.get(*name) else {
            continue;
        };
        let forbidden = dependencies
            .iter()
            .filter(|dependency| !allowed.contains(&dependency.as_str()));
        offenders.extend(forbidden.map(|dependency| format!("{name} -> {dependency}")));
    }
    offenders
}

/// Every declared edge from a crate outside the domain crates to a domain crate
/// behind the façade, as `from -> to`.
fn consumers_naming_a_crate_behind_the_facade(declared: &Declared) -> Vec<String> {
    let domain_crates: BTreeSet<&str> = ALLOWED.iter().map(|(name, _)| *name).collect();
    let mut offenders = Vec::new();
    for (name, dependencies) in declared {
        if domain_crates.contains(name.as_str()) {
            continue;
        }
        let behind = dependencies
            .iter()
            .filter(|dependency| BEHIND_THE_FACADE.contains(&dependency.as_str()));
        offenders.extend(behind.map(|dependency| format!("{name} -> {dependency}")));
    }
    offenders
}

/// A declaration table built from `(crate, [dependencies])` pairs.
fn declarations(edges: &[(&str, &[&str])]) -> Declared {
    edges
        .iter()
        .map(|(name, dependencies)| {
            let dependencies = dependencies.iter().map(|dependency| dependency.to_string());
            (name.to_string(), dependencies.collect())
        })
        .collect()
}

#[test]
fn each_domain_crate_declares_only_the_crates_below_it() {
    let declared = workspace_declarations();
    let missing: Vec<&str> = ALLOWED
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !declared.contains_key(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "domain crates missing from the workspace: {missing:?}"
    );
    let offenders = edges_outside_the_allowed_sets(&declared);
    assert!(
        offenders.is_empty(),
        "domain crates declare workspace crates outside their allowed sets — \
         dependencies point down, never up or sideways:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn consumers_name_only_the_facade() {
    let declared = workspace_declarations();
    assert!(
        declared.contains_key("application") && declared.contains_key("api"),
        "the workspace members were not read: {:?}",
        declared.keys()
    );
    let offenders = consumers_naming_a_crate_behind_the_facade(&declared);
    assert!(
        offenders.is_empty(),
        "crates outside the domain name a domain crate other than `domain`:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn the_checks_flag_planted_edges() {
    let declared = declarations(&[
        ("kernel", &["shared", "identity"]),
        ("identity", &["kernel", "workflow", "atta"]),
        ("workflow", &["kernel", "identity", "domain"]),
        ("gallery", &["kernel", "identity"]),
        ("moderation", &["shared"]),
        ("domain", &["kernel", "identity", "workflow", "application"]),
        ("application", &["domain", "shared", "identity"]),
        ("api", &["application", "domain"]),
    ]);

    let upward_edges = edges_outside_the_allowed_sets(&declared);
    let expected_upward_edges = [
        "kernel -> identity",
        "identity -> atta",
        "identity -> workflow",
        "workflow -> domain",
        "gallery -> identity",
        "domain -> application",
    ];
    assert_eq!(upward_edges, expected_upward_edges);

    let consumer_edges = consumers_naming_a_crate_behind_the_facade(&declared);
    let expected_consumer_edges = ["application -> identity"];
    assert_eq!(consumer_edges, expected_consumer_edges);
}
