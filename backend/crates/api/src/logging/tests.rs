use std::collections::{BTreeSet, HashMap};
use std::process::Command;

use test_support::log_capture::CapturedLog;

use super::*;

#[test]
fn a_dns_client_error_naming_a_handle_never_reaches_the_server_log() {
    let log = CapturedLog::default();
    let wide_open =
        EnvFilter::new("trace,hickory_resolver=trace,hickory_resolver::name_server_pool=trace");
    let subscriber = subscriber(&Environment::DEV, wide_open, log.clone());

    tracing::subscriber::with_default(subscriber, || {
        tracing::error!(target: "hickory_resolver::name_server_pool", "malformed answer for alice.example.com");
        tracing::error!(target: "hickory_proto::op", "bad record for bob.example.com");
        tracing::info!(target: "api", "server started");
    });

    let text = log.text();
    assert!(
        text.contains("server started"),
        "the rest of the log still flows: {text}"
    );
    assert!(!text.contains("alice.example.com"), "{text}");
    assert!(!text.contains("bob.example.com"), "{text}");
}

#[test]
fn outside_dev_a_debug_event_is_dropped_whatever_the_filter_says() {
    for env in [Environment::STG, Environment::PROD] {
        let log = CapturedLog::default();
        let subscriber = subscriber(&env, EnvFilter::new("trace"), log.clone());

        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: "api", did = "did:plc:debugonly", "a debug detail");
            tracing::trace!(target: "api", "a trace detail");
            tracing::info!(target: "api", "an info line");
        });

        let text = log.text();
        assert!(text.contains("an info line"), "{env:?}: {text}");
        assert!(!text.contains("a debug detail"), "{env:?}: {text}");
        assert!(!text.contains("did:plc:debugonly"), "{env:?}: {text}");
        assert!(!text.contains("a trace detail"), "{env:?}: {text}");
    }
}

#[test]
fn in_dev_a_debug_event_is_kept() {
    let log = CapturedLog::default();
    let subscriber = subscriber(&Environment::DEV, EnvFilter::new("trace"), log.clone());

    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "api", "a dev debug detail");
    });

    let text = log.text();
    assert!(text.contains("a dev debug detail"), "{text}");
}

#[test]
fn in_dev_a_span_prints_its_close_with_its_timing() {
    let log = CapturedLog::default();
    let subscriber = subscriber(&Environment::DEV, EnvFilter::new("debug"), log.clone());

    tracing::subscriber::with_default(subscriber, || {
        let span = tracing::debug_span!(target: "api", "dev_probe");
        span.in_scope(|| tracing::debug!(target: "api", "inside the probe"));
    });

    let text = log.text();
    let close = text
        .lines()
        .find(|line| line.contains("dev_probe") && line.contains("close"));
    let close = close.unwrap_or_else(|| panic!("no close line: {text}"));
    assert!(close.contains("time.busy"), "{close}");
}

#[test]
fn outside_dev_no_span_event_is_printed() {
    for env in [Environment::STG, Environment::PROD] {
        let log = CapturedLog::default();
        let subscriber = subscriber(&env, EnvFilter::new("trace"), log.clone());

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(target: "api", "staging_probe");
            span.in_scope(|| tracing::info!(target: "api", "inside the staging probe"));
        });

        let text = log.text();
        assert!(text.contains("inside the staging probe"), "{env:?}: {text}");
        assert!(!text.contains("close"), "{env:?}: {text}");
        assert!(!text.contains("time.busy"), "{env:?}: {text}");
    }
}

#[test]
fn rust_log_wins_when_it_is_set_and_parses() {
    let chosen = ChosenFilter::new(Some("warn"), "info");

    assert_eq!(chosen.source, FilterSource::RustLog);
    assert_eq!(chosen.filter.to_string(), "warn");
}

#[test]
fn the_profile_s_level_applies_when_rust_log_is_unset_or_unreadable() {
    let unset = ChosenFilter::new(None, "info,api=debug");
    let unreadable = ChosenFilter::new(Some("api=loudest"), "info,api=debug");

    for chosen in [unset, unreadable] {
        assert_eq!(chosen.source, FilterSource::Profile);
        let directives: BTreeSet<String> = chosen
            .filter
            .to_string()
            .split(',')
            .map(str::to_owned)
            .collect();
        let expected = BTreeSet::from(["info".to_owned(), "api=debug".to_owned()]);
        assert_eq!(directives, expected);
    }
}

#[test]
fn the_filter_sources_read_as_their_names() {
    let names = [FilterSource::RustLog, FilterSource::Profile].map(|source| source.to_string());

    assert_eq!(names, ["RUST_LOG", "the profile's log_level"]);
}

/// The dev profile's `log_level`, as `backend/config/dev.toml` sets it.
fn dev_log_level() -> String {
    let dev_toml = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config/dev.toml"
    ));
    let assignment = dev_toml
        .lines()
        .find_map(|line| line.trim().strip_prefix("log_level"))
        .expect("dev.toml sets log_level");
    let value = assignment.trim().trim_start_matches('=').trim();
    value.trim_matches('"').to_owned()
}

/// The library crates the server is built from: `api` and every workspace crate in its normal
/// dependency closure (optional ones included), proc macros aside, by crate name.
fn server_crates() -> BTreeSet<String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
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
    let by_name: HashMap<&str, &serde_json::Value> = packages
        .iter()
        .map(|package| (package["name"].as_str().expect("a package name"), package))
        .collect();
    let proc_macro_kind = serde_json::json!(["proc-macro"]);
    let is_proc_macro = |package: &serde_json::Value| {
        package["targets"].as_array().is_some_and(|targets| {
            targets
                .iter()
                .any(|target| target["kind"] == proc_macro_kind)
        })
    };

    let mut seen = BTreeSet::new();
    let mut next = vec!["api"];
    while let Some(name) = next.pop() {
        if !seen.insert(name) {
            continue;
        }
        let dependencies = by_name[name]["dependencies"]
            .as_array()
            .expect("a dependency list");
        let workspace_dependencies = dependencies
            .iter()
            .filter(|dependency| dependency["kind"].is_null())
            .filter_map(|dependency| dependency["name"].as_str())
            .filter(|dependency| by_name.contains_key(dependency));
        next.extend(workspace_dependencies);
    }
    seen.into_iter()
        .filter(|name| !is_proc_macro(by_name[name]))
        .map(|name| name.replace('-', "_"))
        .collect()
}

#[test]
fn the_dev_profile_logs_every_server_crate_at_debug_and_dependencies_at_info() {
    let log_level = dev_log_level();
    let directives: BTreeSet<&str> = log_level.split(',').map(str::trim).collect();
    let crates = server_crates();

    assert!(
        crates.contains("adapter_pg") && crates.contains("http_stack"),
        "the closure was read: {crates:?}"
    );
    assert_eq!(log_level.split(',').next(), Some("info"), "{log_level}");
    let missing: Vec<&String> = crates
        .iter()
        .filter(|name| !directives.contains(format!("{name}=debug").as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "dev.toml's log_level lacks {missing:?} at debug: {log_level}"
    );
    let parsed = EnvFilter::try_new(&log_level);
    assert!(parsed.is_ok(), "dev.toml's log_level parses: {log_level}");
}

/// `cargo tree` of the server's normal dependencies with their enabled features, both
/// profiles' builds included.
fn server_features() -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--package",
            "api",
            "--edges",
            "normal,features",
            "--all-features",
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
    String::from_utf8(output.stdout).expect("a UTF-8 tree")
}

#[test]
fn no_jacquard_crate_builds_with_its_tracing_feature() {
    let tree = server_features();

    assert!(
        tree.lines()
            .any(|line| line.starts_with("jacquard-oauth v")),
        "cargo tree printed no jacquard-oauth:\n{tree}"
    );
    let traced: BTreeSet<&str> = tree
        .lines()
        .filter(|line| line.starts_with("jacquard"))
        .filter(|line| line.contains(" feature \"tracing\""))
        .collect();
    assert!(
        traced.is_empty(),
        "a jacquard crate builds with tracing, which spans the OAuth callback with its state: {traced:?}"
    );
}
