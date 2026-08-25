//! Process-level harness (ZMVP-201): spawn the real `zurfur` binary and pin
//! the conventions — stdout is data only, stderr is diagnostics with one JSON
//! problem as its last line, exit codes are the four classes. Nothing here
//! needs a database: every case stops before the runtime boots.

use assert_cmd::Command;

fn zurfur() -> Command {
    let mut cmd = Command::cargo_bin("zurfur").expect("the zurfur binary is built");
    // The binary loads no `.env` itself (ZMVP-203 F1), so clearing the
    // inherited variables is enough to keep the developer's stack out of a
    // harness run.
    cmd.env_remove("DATABASE_URL")
        .env_remove("ZURFUR_DID_KEY_ROOT_KEY")
        .env_remove("ZURFUR_CLI_HOME")
        .env("RUST_LOG", "off");
    cmd
}

/// The stderr contract: diagnostics first, the problem JSON as the LAST line.
fn problem(stderr: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(stderr);
    let last = text.lines().last().expect("stderr has a problem line");
    serde_json::from_str(last).expect("the last stderr line is one JSON problem")
}

#[test]
fn help_exits_zero() {
    zurfur().arg("--help").assert().success();
}

#[test]
fn version_exits_zero_and_names_the_tool() {
    let output = zurfur().arg("--version").assert().success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.starts_with("zurfur "), "{stdout:?}");
}

// Malformed arguments are clap's: exit 2, nothing on stdout.
#[test]
fn an_unknown_subcommand_is_a_usage_error() {
    let output = zurfur().arg("frobnicate").assert().code(2);
    assert!(output.get_output().stdout.is_empty());
    assert!(!output.get_output().stderr.is_empty());
}

// `completions` never boots the runtime: it works with no config at all.
#[test]
fn completions_print_a_zsh_script_without_a_backend() {
    let output = zurfur()
        .args(["completions", "zsh"])
        .env("ZURFUR_CONFIG_DIR", "/nonexistent")
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("_zurfur"), "{stdout:?}");
    assert!(stdout.contains("completions"), "{stdout:?}");
}

// A backend-needing command with an unloadable config is an INFRA failure:
// exit 3, stdout empty, stderr ends in one JSON problem with class + code.
#[test]
fn a_missing_config_is_an_infra_problem_on_stderr() {
    let output = zurfur()
        .args(["session", "whoami", "--config-dir", "/nonexistent"])
        .env("ZURFUR_ENV", "dev")
        .assert()
        .code(3);
    assert!(output.get_output().stdout.is_empty());
    let problem = problem(&output.get_output().stderr);
    assert_eq!(problem["class"], "infra");
    assert_eq!(problem["code"], "config");
    // `env` itself is satisfied by ZURFUR_ENV=dev (the lowercase alias); the
    // first key the profile file would have supplied is the one reported.
    assert_eq!(problem["detail"], "missing configuration key `public_url`");
}

// Diagnostics on stderr never displace the problem: it is the last line.
#[test]
fn the_problem_is_the_last_stderr_line_even_under_verbose_logging() {
    let output = zurfur()
        .args(["session", "whoami", "--config-dir", "/nonexistent"])
        .env("ZURFUR_ENV", "dev")
        .env("RUST_LOG", "trace")
        .assert()
        .code(3);
    assert_eq!(problem(&output.get_output().stderr)["code"], "config");
}

// A type-mismatched secret is reported by KEY only — figment's own message
// would echo the parsed value (ZMVP-203 F4). The all-digit value is what
// makes this a CONFIG error rather than a boot error: figment's `Env`
// provider parses it as an integer, which then fails to deserialize into
// the `String` field — before any database connection is attempted.
#[test]
fn a_malformed_secret_is_never_echoed() {
    let output = zurfur()
        .args(["health", "--config-dir", "/nonexistent"])
        .env("ZURFUR_ENV", "dev")
        .env("ZURFUR_PUBLIC_URL", "http://x")
        .env("ZURFUR_LOG_LEVEL", "info")
        .env("DATABASE_URL", "postgres://x")
        .env("ZURFUR_DID_KEY_ROOT_KEY", "987654321012345")
        .assert()
        .code(3);
    let stderr = String::from_utf8(output.get_output().stderr.clone()).unwrap();
    assert!(!stderr.contains("987654321012345"), "{stderr}");
    let problem = problem(&output.get_output().stderr);
    assert_eq!(problem["code"], "config");
    assert!(
        problem["detail"]
            .as_str()
            .unwrap()
            .contains("did_key_root_key"),
        "{stderr}"
    );
}

// `session logout` needs neither config nor a database (ZMVP-203 F3).
#[test]
fn logout_works_with_no_backend_at_all() {
    let home = tempfile::tempdir().unwrap();
    let output = zurfur()
        .args([
            "--json",
            "session",
            "logout",
            "--config-dir",
            "/nonexistent",
        ])
        .env(cli::identity::HOME_ENV, home.path())
        .assert()
        .success();
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout, "{\"hadIdentity\":false,\"loggedOut\":true}\n");
}

// Global flags are accepted before or after the subcommand.
#[test]
fn global_flags_work_on_either_side_of_the_subcommand() {
    zurfur()
        .args(["--json", "completions", "zsh"])
        .assert()
        .success();
    zurfur()
        .args(["completions", "zsh", "--json"])
        .assert()
        .success();
}
