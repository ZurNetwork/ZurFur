//! The `session` namespace: who the CLI acts as. Ops are declared here so the
//! command tree is complete; `login` (ZMVP-204) and `logout`/`whoami`
//! (ZMVP-203) fill them in. Until then each answers an honest
//! `not_implemented` infrastructure problem — never a fake success.

use clap::Subcommand;
use composition::Runtime;

use crate::CliError;

/// The session operations.
#[derive(Debug, Subcommand)]
pub enum SessionOp {
    /// Sign in through the browser and record the acting identity.
    Login,
    /// Forget the acting identity.
    Logout,
    /// Show the acting identity.
    Whoami,
}

/// Run one session op over the runtime.
pub async fn run(_runtime: &Runtime, op: SessionOp) -> Result<serde_json::Value, CliError> {
    let name = match op {
        SessionOp::Login => "session login",
        SessionOp::Logout => "session logout",
        SessionOp::Whoami => "session whoami",
    };
    Err(CliError::infra(
        "not_implemented",
        format!("`{name}` is not built yet (ZMVP-203 / ZMVP-204)"),
    ))
}
