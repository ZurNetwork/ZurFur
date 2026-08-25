//! The process-level harness shared by every `zurfur` test that boots a
//! runtime: one place for the environment a backend command needs and for
//! the stderr contract.

#![allow(dead_code)]

use assert_cmd::Command;

/// A 32-byte base64 root key — the custody guard only refuses the shipped
/// example key, and only when submitting.
pub const TEST_ROOT_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// The `zurfur` binary pointed at `database_url`, with the dev profile, the
/// test root key, logging off, and the developer's own config dir kept out.
pub fn zurfur(database_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("zurfur").expect("the zurfur binary is built");
    cmd.env(composition::PROFILE_ENV, "dev")
        .env(composition::DATABASE_URL_ENV, database_url)
        .env(composition::ROOT_KEY_ENV, TEST_ROOT_KEY)
        .env("RUST_LOG", "off")
        .env_remove(composition::CONFIG_DIR_ENV);
    cmd
}

/// The stderr contract: diagnostics first, the problem JSON as the LAST line.
pub fn problem(stderr: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(stderr);
    let last = text.lines().last().expect("stderr has a problem line");
    serde_json::from_str(last).expect("the last stderr line is one JSON problem")
}
