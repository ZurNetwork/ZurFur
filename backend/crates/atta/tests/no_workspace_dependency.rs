//! Guard: Atta depends on no other crate of this workspace, of any kind, so it can be split out
//! unchanged.

use std::process::Command;

/// The names of every workspace member, and the dependency names atta declares, of every kind.
fn workspace_members_and_atta_dependencies() -> (Vec<String>, Vec<String>) {
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
    let name =
        |package: &serde_json::Value| package["name"].as_str().expect("a package name").to_owned();
    let members: Vec<String> = packages.iter().map(name).collect();
    let atta = packages
        .iter()
        .find(|package| package["name"] == "atta")
        .expect("atta is a member");
    let dependencies = atta["dependencies"].as_array().expect("a dependency list");
    let dependency_names = dependencies.iter().map(name).collect();
    (members, dependency_names)
}

#[test]
fn atta_depends_on_no_workspace_crate() {
    let (members, dependencies) = workspace_members_and_atta_dependencies();
    assert!(
        members.len() > 1,
        "the workspace members were not read: {members:?}"
    );
    let inside: Vec<_> = dependencies
        .iter()
        .filter(|dependency| members.contains(dependency))
        .collect();
    assert!(
        inside.is_empty(),
        "atta depends on workspace crates: {inside:?}"
    );
}
