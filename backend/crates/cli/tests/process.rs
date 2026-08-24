//! Process-level harness (ZMVP-201): spawn the real `zurfur` binary and pin
//! the conventions — stdout is data only, stderr is diagnostics + one JSON
//! problem, exit codes are the four classes. Nothing here needs a database:
//! every case stops before the runtime boots.

use assert_cmd::Command;

fn zurfur() -> Command {
    let mut cmd = Command::cargo_bin("zurfur").expect("the zurfur binary is built");
    // Never let the developer's .env leak a real database into a harness run.
    cmd.env_remove("DATABASE_URL");
    cmd
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
// exit 3, stdout empty, stderr = one JSON problem with class + code.
#[test]
fn a_missing_config_is_an_infra_problem_on_stderr() {
    let output = zurfur()
        .args(["session", "whoami", "--config-dir", "/nonexistent"])
        .env("ZURFUR_ENV", "dev")
        .assert()
        .code(3);
    assert!(output.get_output().stdout.is_empty());
    let stderr = String::from_utf8(output.get_output().stderr.clone()).unwrap();
    let problem: serde_json::Value =
        serde_json::from_str(stderr.trim()).expect("stderr is one JSON problem");
    assert_eq!(problem["class"], "infra");
    assert_eq!(problem["code"], "config");
    assert!(problem["detail"].as_str().is_some());
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
