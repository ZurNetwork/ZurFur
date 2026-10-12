//! CI guard: the known doors around the guarded web client stay shut.
//!
//! Every fetch to a host a visitor, a handle or a DID document names must go
//! through `GuardedHttp`. This scan fails the build if the non-test source of
//! a runtime crate names a client that skips it: reqwest outside the three
//! files allowed it, jacquard's own resolver, `ResolverOptions` named anywhere
//! but the sign-in bridge (and built there only as a full literal), one of
//! jacquard's convenience clients and DNS switches, or a call that weakens the
//! transport. It also reads each runtime crate's
//! manifest (no renamed reqwest, reqwest only in adapter-atproto) and checks
//! that unit-test files are only ever compiled under `#[cfg(test)]`. It sees
//! only the doors it names: hyper, raw sockets or an unnamed jacquard path
//! would pass it, and the review reads for those.

use std::path::{Path, PathBuf};

/// Crates that never ship in the server binary, so are not scanned: the test
/// rig (it drives a throwaway PDS over its own reqwest) and the two codegen
/// runners.
const UNSCANNED_CRATES: &[&str] = &["test-support", "query-codegen", "contract-gen"];

/// The one runtime crate whose manifest may list reqwest as a dependency.
const REQWEST_CRATE: &str = "adapter-atproto";

/// The files that may name reqwest, as `<crate>/src/<file…>`, each with why.
const REQWEST_ALLOWED: &[(&str, &str)] = &[
    (
        "adapter-atproto/src/guarded_http/client.rs",
        "the guarded client itself",
    ),
    (
        "adapter-atproto/src/plc_directory.rs",
        "talks only to the PLC directory the operator configures",
    ),
    (
        "adapter-atproto/src/public_records.rs",
        "holds a Bearer token for Zurfur's own PDS, whose endpoint must come only \
         from operator config; no production caller, and the guard blocks one \
         until its transport is settled with the hosted-repos design decision",
    ),
];

/// The files that may name `AtprotoPublicRecords`: its own and the crate
/// façade's re-export. Anywhere else would be a production caller.
const PUBLIC_RECORDS_ALLOWED: &[&str] = &[
    "adapter-atproto/src/public_records.rs",
    "adapter-atproto/src/lib.rs",
];

/// The one file that may name jacquard's `ResolverOptions` at all, and may build
/// it only as a struct literal naming every field: the sign-in bridge's
/// no-fallback options. A line scan cannot follow a type across lines, so
/// outside the bridge the name alone is refused, and inside it the bridge's own
/// test checks the options it serves.
const RESOLVER_OPTIONS_FILE: &str = "adapter-atproto/src/authenticator/jacquard_bridge.rs";

/// Tokens forbidden in every scanned file, the guarded client's own included:
/// jacquard's resolver, clients and sessions (each hard-wired to a bare reqwest
/// or able to reach Bluesky), its convenience constructors that pick their own
/// transport or Google's DNS, the Bluesky fallback host, and the named ways to
/// weaken TLS or to reopen a proxy, a redirect or a pinned address past the DNS filter.
const FORBIDDEN_EVERYWHERE: &[&str] = &[
    "JacquardResolver",
    "BasicClient",
    "PublicResolver",
    "MemoryCredentialSession",
    "UnauthenticatedSession",
    "OAuthClient::new(",
    "with_default_config",
    "with_memory_store",
    "new_dns",
    "with_system_dns",
    "slingshot_resolver_default",
    "public.api.bsky.app",
    "danger_accept_invalid",
    "add_root_certificate",
    "tls_built_in_root_certs",
    "use_preconfigured_tls",
    ".resolve(",
    ".resolve_to_addrs(",
    ".proxy(",
    "Policy::limited",
    "Policy::custom",
    "Policy::default",
];

/// The `backend/crates` directory.
fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("adapter-atproto sits inside backend/crates")
        .to_path_buf()
}

/// The repository root, where the workspace manifest lives.
fn workspace_root() -> PathBuf {
    crates_dir()
        .parent()
        .and_then(Path::parent)
        .expect("backend/crates sits two levels below the root")
        .to_path_buf()
}

fn crate_name(crate_dir: &Path) -> String {
    crate_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Every runtime crate's directory.
fn scanned_crates() -> Vec<PathBuf> {
    let entries = std::fs::read_dir(crates_dir()).expect("guard reads backend/crates");
    let mut crates = Vec::new();
    for entry in entries {
        let crate_dir = entry.expect("dir entry").path();
        let name = crate_name(&crate_dir);
        if UNSCANNED_CRATES.contains(&name.as_str()) || !crate_dir.join("src").is_dir() {
            continue;
        }
        crates.push(crate_dir);
    }
    crates
}

/// The non-test source files of every runtime crate.
fn scanned_files() -> Vec<PathBuf> {
    scanned_crates()
        .iter()
        .flat_map(|crate_dir| rust_files(&crate_dir.join("src")))
        .collect()
}

/// Every `*.rs` under `dir`, recursively, minus unit-test files (`tests.rs`,
/// `proptests.rs`), which compile only under `#[cfg(test)]` (checked below).
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs")
            && file_name != "tests.rs"
            && file_name != "proptests.rs"
        {
            out.push(path);
        }
    }
    out
}

/// The `<crate>/src/<file…>` suffix of a scanned path.
fn crate_relative(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    match text.rsplit_once("/crates/") {
        Some((_, relative)) => relative.to_string(),
        None => text,
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("guard cannot read {}: {e}", path.display()))
}

/// The forbidden tokens on one line of `relative`, comment lines skipped.
fn forbidden_on_line(relative: &str, line: &str) -> Vec<&'static str> {
    if line.trim_start().starts_with("//") {
        return Vec::new();
    }
    let reqwest_allowed = REQWEST_ALLOWED
        .iter()
        .any(|(allowed, _)| *allowed == relative);
    let mut found: Vec<&'static str> = FORBIDDEN_EVERYWHERE
        .iter()
        .copied()
        .filter(|token| line.contains(token))
        .collect();
    if !reqwest_allowed && line.contains("reqwest") {
        found.push("reqwest");
    }
    // Any redirect policy but `Policy::none()`, however it is spelled.
    if line.contains(".redirect(") && !line.contains("Policy::none()") {
        found.push(".redirect(");
    }
    found.extend(resolver_options_on_line(relative, line));
    let public_records_allowed = PUBLIC_RECORDS_ALLOWED.contains(&relative);
    if !public_records_allowed && line.contains("AtprotoPublicRecords") {
        found.push("AtprotoPublicRecords");
    }
    found
}

/// How `line` reaches jacquard's `ResolverOptions` other than through the
/// bridge's full literal: any associated constructor or default (whose
/// fallbacks are on), the name anywhere outside the bridge, or, in the bridge,
/// a `..` update from a default.
fn resolver_options_on_line(relative: &str, line: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let in_bridge = relative == RESOLVER_OPTIONS_FILE;
    if line.contains("ResolverOptions::") || line.contains("ResolverOptions>::") {
        found.push("ResolverOptions::");
    }
    if line.contains("ResolverOptions") && line.to_ascii_lowercase().contains("default") {
        found.push("ResolverOptions default");
    }
    if line.contains("ResolverOptions") && !in_bridge {
        found.push("ResolverOptions outside the bridge");
    }
    let update_from_default =
        line.trim_start().starts_with("..") && line.to_ascii_lowercase().contains("default");
    if in_bridge && update_from_default {
        found.push("ResolverOptions ..update");
    }
    found
}

/// Every forbidden token in `files`, as `file:line: token`.
fn offenders(files: &[PathBuf]) -> Vec<String> {
    let mut hits = Vec::new();
    for file in files {
        let relative = crate_relative(file);
        for (index, line) in read(file).lines().enumerate() {
            for token in forbidden_on_line(&relative, line) {
                hits.push(format!("{relative}:{}: {token}", index + 1));
            }
        }
    }
    hits
}

/// Whether `line` declares the `tests` or `proptests` file module.
fn declares_test_module(line: &str) -> bool {
    let declaration = line.trim();
    let item = ["pub(crate) ", "pub(super) ", "pub "]
        .iter()
        .find_map(|visibility| declaration.strip_prefix(visibility))
        .unwrap_or(declaration);
    item == "mod tests;" || item == "mod proptests;"
}

/// The 1-based lines of `source` that would compile a unit-test file outside
/// `#[cfg(test)]`: a `mod tests;`/`mod proptests;` whose attributes lack it,
/// or a `#[path]` pointing at one.
fn test_module_leaks(source: &str) -> Vec<usize> {
    let lines: Vec<&str> = source.lines().collect();
    let mut leaks = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let names_test_file = line.contains("tests.rs");
        if line.trim_start().starts_with("#[path") && names_test_file {
            leaks.push(index + 1);
            continue;
        }
        if !declares_test_module(line) {
            continue;
        }
        let gated = lines[..index]
            .iter()
            .rev()
            .map(|above| above.trim())
            .take_while(|above| above.starts_with("#[") || above.starts_with("///"))
            .any(|above| above == "#[cfg(test)]");
        if !gated {
            leaks.push(index + 1);
        }
    }
    leaks
}

/// Every unit-test module compiled outside `#[cfg(test)]`, as `file:line`.
fn test_module_offenders(files: &[PathBuf]) -> Vec<String> {
    let mut hits = Vec::new();
    for file in files {
        let relative = crate_relative(file);
        for line in test_module_leaks(&read(file)) {
            hits.push(format!("{relative}:{line}"));
        }
    }
    hits
}

/// What `manifest` (of crate `crate_name`, or the workspace root when `None`)
/// gets wrong: a dependency renamed from reqwest anywhere, or reqwest as a
/// non-dev dependency of any crate but [`REQWEST_CRATE`].
fn manifest_violations(crate_name: Option<&str>, manifest: &str) -> Vec<String> {
    let reqwest_allowed = crate_name.is_none_or(|name| name == REQWEST_CRATE);
    let mut section = String::new();
    let mut violations = Vec::new();
    for (index, raw) in manifest.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.contains("package=\"reqwest\"") || compact.contains("package='reqwest'") {
            violations.push(format!("line {}: reqwest renamed", index + 1));
        }
        if line.starts_with('[') {
            section = compact;
            let is_reqwest_table =
                section.ends_with(".reqwest]") && !section.contains("dev-dependencies");
            if is_reqwest_table && !reqwest_allowed {
                violations.push(format!("line {}: reqwest dependency", index + 1));
            }
            continue;
        }
        let in_dependencies =
            section.ends_with("dependencies]") && !section.contains("dev-dependencies");
        let key = line.split(['=', '.']).next().unwrap_or_default().trim();
        if in_dependencies && key == "reqwest" && !reqwest_allowed {
            violations.push(format!("line {}: reqwest dependency", index + 1));
        }
    }
    violations
}

/// Every manifest violation across the runtime crates and the workspace root.
fn manifest_offenders() -> Vec<String> {
    let mut hits = Vec::new();
    let root_manifest = workspace_root().join("Cargo.toml");
    for violation in manifest_violations(None, &read(&root_manifest)) {
        hits.push(format!("Cargo.toml {violation}"));
    }
    for crate_dir in scanned_crates() {
        let name = crate_name(&crate_dir);
        let manifest = read(&crate_dir.join("Cargo.toml"));
        for violation in manifest_violations(Some(&name), &manifest) {
            hits.push(format!("{name}/Cargo.toml {violation}"));
        }
    }
    hits
}

#[test]
fn scan_covers_the_runtime_crates_and_skips_the_rig() {
    let relatives: Vec<String> = scanned_files().iter().map(|f| crate_relative(f)).collect();

    let scans = |path: &str| relatives.iter().any(|relative| relative == path);
    assert!(scans("adapter-atproto/src/guarded_http/client.rs"));
    assert!(scans("adapter-atproto/src/profile.rs"));
    assert!(scans("api/src/main.rs"));
    assert!(scans("composition/src/runtime.rs"));
    let rig_scanned = relatives
        .iter()
        .any(|relative| relative.starts_with("test-support/"));
    assert!(!rig_scanned, "test-support must not be scanned");
    let unit_tests_scanned = relatives
        .iter()
        .any(|relative| relative.ends_with("/tests.rs"));
    assert!(!unit_tests_scanned, "unit-test files must not be scanned");
}

#[test]
fn the_known_doors_stay_shut() {
    let offenders = offenders(&scanned_files());

    let none: Vec<String> = Vec::new();
    assert_eq!(
        offenders, none,
        "a fetch to a user-chosen host must go through GuardedHttp; these name a \
         client that skips it"
    );
}

#[test]
fn unit_test_files_compile_only_under_cfg_test() {
    let offenders = test_module_offenders(&scanned_files());

    let none: Vec<String> = Vec::new();
    assert_eq!(
        offenders, none,
        "a tests.rs or proptests.rs compiled outside #[cfg(test)] is never scanned"
    );
}

#[test]
fn no_manifest_renames_reqwest_or_adds_it_outside_adapter_atproto() {
    let offenders = manifest_offenders();

    let none: Vec<String> = Vec::new();
    assert_eq!(offenders, none);
}

#[test]
fn the_reqwest_allowlist_is_exactly_three_files() {
    let allowed: Vec<&str> = REQWEST_ALLOWED.iter().map(|(path, _)| *path).collect();

    let expected = [
        "adapter-atproto/src/guarded_http/client.rs",
        "adapter-atproto/src/plc_directory.rs",
        "adapter-atproto/src/public_records.rs",
    ];
    assert_eq!(allowed, expected);
}

#[test]
fn detector_flags_each_door() {
    let elsewhere = "adapter-atproto/src/profile.rs";

    let flagged = [
        "let client = reqwest::Client::new();",
        "use jacquard::client::BasicClient;",
        "let r = jacquard::identity::PublicResolver::new(http, opts);",
        "let s = jacquard::client::MemoryCredentialSession::unauthenticated();",
        "let x: jacquard::client::MemoryCredentialSession = Default::default();",
        "let u = jacquard::client::UnauthenticatedSession::new_public();",
        "let r = jacquard::identity::JacquardResolver::default();",
        "let r: JacquardResolver<_> = Default::default();",
        "let r = <JacquardResolver<_>>::default();",
        "let r = JacquardResolver::<GuardedHttp>::default();",
        "let resolver = JacquardResolver::new(http, options);",
        "type Oauth = OAuthClient<JacquardResolver<GuardedHttp>, Store>;",
        "let options = ResolverOptions::default();",
        "let options = ResolverOptions::new().build();",
        "let options = <ResolverOptions>::default();",
        "let options: ResolverOptions = Default::default();",
        "let options = ResolverOptions { ..Default::default() };",
        "Arc::new(OAuthClient::new(store, data, http))",
        "OAuthClient::with_default_config(store)",
        "OAuthClient::with_memory_store()",
        "JacquardResolver::new_dns(http, opts)",
        "resolver.with_system_dns()",
        "slingshot_resolver_default()",
        "let url = \"https://public.api.bsky.app\";",
        "builder.danger_accept_invalid_certs(true)",
        "builder.add_root_certificate(cert)",
        "builder.tls_built_in_root_certs(false)",
        "builder.use_preconfigured_tls(config)",
        "builder.resolve(\"pds.example.com\", address)",
        "builder.resolve_to_addrs(\"pds.example.com\", &addresses)",
        "builder.proxy(proxy)",
        ".redirect(reqwest::redirect::Policy::limited(10))",
        ".redirect(Policy::custom(|attempt| attempt.follow()))",
        ".redirect(Policy::default())",
        ".redirect(policy)",
        ".redirect(Default::default())",
    ];

    let missed: Vec<&str> = flagged
        .into_iter()
        .filter(|line| forbidden_on_line(elsewhere, line).is_empty())
        .collect();

    assert_eq!(missed, Vec::<&str>::new());
}

#[test]
fn detector_allows_reqwest_only_in_the_allowed_files_and_ignores_comments() {
    let line = "let client = reqwest::Client::new();";
    let client = "adapter-atproto/src/guarded_http/client.rs";

    let in_client = forbidden_on_line(client, line);
    let in_directory = forbidden_on_line("adapter-atproto/src/plc_directory.rs", line);
    let in_comment = forbidden_on_line("api/src/main.rs", "// reqwest is fine in a comment");
    let new_from_resolver = forbidden_on_line(
        "adapter-atproto/src/lib.rs",
        "OAuthClient::new_from_resolver(store, resolver, data)",
    );
    // The guarded client's own configuration must not trip the transport tokens.
    let guarded_lines: Vec<&str> = [
        "        .no_proxy()",
        "        .redirect(reqwest::redirect::Policy::none())",
        "    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {",
    ]
    .into_iter()
    .flat_map(|line| forbidden_on_line(client, line))
    .collect();

    let nothing: Vec<&str> = Vec::new();
    assert_eq!(in_client, nothing);
    assert_eq!(in_directory, nothing);
    assert_eq!(in_comment, nothing);
    assert_eq!(new_from_resolver, nothing);
    assert_eq!(guarded_lines, nothing);
}

#[test]
fn detector_flags_a_test_module_outside_cfg_test() {
    let ungated = "mod helpers;\nmod tests;\n";
    let gated = "#[cfg(test)]\nmod tests;\n";
    let gated_with_more_attributes =
        "#[cfg(test)]\n#[allow(dead_code)]\npub(crate) mod proptests;\n";
    let pathed = "#[cfg(test)]\n#[path = \"other/tests.rs\"]\nmod sneaky;\n";
    let ungated_proptests = "#[allow(dead_code)]\nmod proptests;\n";

    let line_two = vec![2];
    let none: Vec<usize> = Vec::new();
    assert_eq!(test_module_leaks(ungated), line_two);
    assert_eq!(test_module_leaks(gated), none);
    assert_eq!(test_module_leaks(gated_with_more_attributes), none);
    assert_eq!(test_module_leaks(pathed), line_two);
    assert_eq!(test_module_leaks(ungated_proptests), line_two);
}

#[test]
fn detector_flags_a_renamed_or_misplaced_reqwest_dependency() {
    let renamed = "[dependencies]\nweb = { package = \"reqwest\", version = \"0.12\" }\n";
    let renamed_in_root = "[workspace.dependencies]\nweb = { package=\"reqwest\" }\n";
    let plain = "[dependencies]\nreqwest = \"0.12\"\n";
    let table = "[dependencies.reqwest]\nversion = \"0.12\"\n";
    let target = "[target.'cfg(unix)'.dependencies]\nreqwest = { version = \"0.12\" }\n";
    let dev_only = "[dev-dependencies]\nreqwest = \"0.13\"\n";

    let renamed_on_line_two = vec!["line 2: reqwest renamed".to_string()];
    let dependency_on_line_two = vec!["line 2: reqwest dependency".to_string()];
    let dependency_on_line_one = vec!["line 1: reqwest dependency".to_string()];
    let none: Vec<String> = Vec::new();
    assert_eq!(
        manifest_violations(Some("api"), renamed),
        renamed_on_line_two
    );
    assert_eq!(
        manifest_violations(None, renamed_in_root),
        renamed_on_line_two
    );
    assert_eq!(
        manifest_violations(Some("api"), plain),
        dependency_on_line_two
    );
    assert_eq!(
        manifest_violations(Some("api"), table),
        dependency_on_line_one
    );
    assert_eq!(
        manifest_violations(Some("cli"), target),
        dependency_on_line_two
    );
    assert_eq!(manifest_violations(Some("api"), dev_only), none);
    assert_eq!(manifest_violations(Some(REQWEST_CRATE), plain), none);
}

#[test]
fn detector_flags_a_public_records_caller_outside_its_own_files() {
    let caller = "let records = adapter_atproto::AtprotoPublicRecords::new(&endpoint, jwt)?;";
    let re_export = "pub use public_records::AtprotoPublicRecords;";

    let in_composition = forbidden_on_line("composition/src/runtime.rs", caller);
    let in_profile = forbidden_on_line("adapter-atproto/src/profile.rs", caller);
    let in_facade = forbidden_on_line("adapter-atproto/src/lib.rs", re_export);
    let in_own_file = forbidden_on_line(
        "adapter-atproto/src/public_records.rs",
        "impl AtprotoPublicRecords {",
    );

    let flagged = vec!["AtprotoPublicRecords"];
    let nothing: Vec<&str> = Vec::new();
    assert_eq!(in_composition, flagged);
    assert_eq!(in_profile, flagged);
    assert_eq!(in_facade, nothing);
    assert_eq!(in_own_file, nothing);
}

#[test]
fn the_public_records_allowlist_is_exactly_two_files() {
    let expected = [
        "adapter-atproto/src/public_records.rs",
        "adapter-atproto/src/lib.rs",
    ];

    assert_eq!(PUBLIC_RECORDS_ALLOWED, expected);
}

#[test]
fn the_bridge_may_build_resolver_options_only_as_a_full_literal() {
    let literal = [
        "use jacquard::identity::resolver::{PlcSource, ResolverOptions};",
        "    options: ResolverOptions,",
        "fn no_fallbacks() -> ResolverOptions {",
        "    ResolverOptions {",
        "        public_fallback_for_handle: false,",
        "    fn options(&self) -> &ResolverOptions {",
    ];
    let elsewhere = "adapter-atproto/src/authenticator.rs";

    let in_bridge: Vec<&str> = literal
        .iter()
        .flat_map(|line| forbidden_on_line(RESOLVER_OPTIONS_FILE, line))
        .collect();
    let literal_elsewhere = forbidden_on_line(elsewhere, "    ResolverOptions {");
    // Defaults whose type is inferred across lines or through a generic: no
    // one line names both the type and a default, and none is a literal.
    let default_by_inference_elsewhere: Vec<&str> = [
        "    let fallback_enabled_resolver_options: ResolverOptions =",
        "        Default::default();",
        "    let options = fallback_options::<ResolverOptions>();",
    ]
    .iter()
    .flat_map(|line| forbidden_on_line(elsewhere, line))
    .collect();
    let update_in_bridge = forbidden_on_line(RESOLVER_OPTIONS_FILE, "        ..Default::default()");
    let update_from_self = forbidden_on_line(RESOLVER_OPTIONS_FILE, "            ..self");
    let default_in_bridge = forbidden_on_line(
        RESOLVER_OPTIONS_FILE,
        "    let options = ResolverOptions::default();",
    );

    let nothing: Vec<&str> = Vec::new();
    assert_eq!(in_bridge, nothing);
    assert_eq!(
        literal_elsewhere,
        vec!["ResolverOptions outside the bridge"]
    );
    assert_eq!(
        default_by_inference_elsewhere,
        vec![
            "ResolverOptions outside the bridge",
            "ResolverOptions outside the bridge"
        ]
    );
    assert_eq!(update_in_bridge, vec!["ResolverOptions ..update"]);
    assert_eq!(update_from_self, nothing);
    assert!(!default_in_bridge.is_empty());
    assert_eq!(
        RESOLVER_OPTIONS_FILE,
        "adapter-atproto/src/authenticator/jacquard_bridge.rs"
    );
}
