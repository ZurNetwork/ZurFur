//! Failure as a value: [`CliError`] carries an [`ExitClass`] and renders as one
//! compact JSON [`Problem`] on stderr — never plain text, so `jq` pipelines on
//! stderr stay parseable (board finding, ZMVP-201).

use std::io::Write as _;
use std::process::ExitCode;

use serde::Serialize;

/// The exit-code classes every command maps to. The numbers are the contract
/// scripts rely on; clap owns `2` (usage) and exits before [`CliError`] is
/// ever built.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitClass {
    /// The domain refused: not found, not permitted, invalid, conflict — the
    /// backend's own error, rendered. Exit `1`.
    Domain,
    /// Malformed arguments. Exit `2` (clap's default; listed so the mapping
    /// is total and testable).
    Usage,
    /// Config, database, network, or the runtime failed to boot. Exit `3`.
    Infra,
    /// Ctrl-C. Exit `130` (128 + SIGINT), the shell convention.
    Interrupted,
}

impl ExitClass {
    /// The process exit code for this class.
    pub fn exit_code(self) -> ExitCode {
        ExitCode::from(self.code())
    }

    /// The numeric code (`0` is never a class — success has no problem).
    pub fn code(self) -> u8 {
        match self {
            ExitClass::Domain => 1,
            ExitClass::Usage => 2,
            ExitClass::Infra => 3,
            ExitClass::Interrupted => 130,
        }
    }
}

/// The one JSON object a failed run prints on stderr.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Problem {
    /// The [`ExitClass`], spelled out so a reader needs no code table.
    pub class: ExitClass,
    /// A stable machine-readable code (`snake_case`): what went wrong.
    pub code: String,
    /// Human-readable detail. Never parse it; never put a secret in it.
    pub detail: String,
}

impl Problem {
    /// Write the compact JSON + newline to stderr (best-effort, like stdout).
    pub fn write_stderr(&self) {
        let mut rendered = serde_json::to_vec(self).expect("Problem serializes");
        rendered.push(b'\n');
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(&rendered);
        let _ = stderr.flush();
    }
}

/// A failed command: the class that becomes the exit code, plus the
/// code/detail that become the stderr [`Problem`].
#[derive(Debug)]
pub struct CliError {
    class: ExitClass,
    code: String,
    detail: String,
}

impl CliError {
    /// A domain refusal (exit `1`) with a stable `code`.
    pub fn domain(code: impl Into<String>, detail: impl std::fmt::Display) -> Self {
        CliError {
            class: ExitClass::Domain,
            code: code.into(),
            detail: detail.to_string(),
        }
    }

    /// An infrastructure failure (exit `3`) with a stable `code`.
    pub fn infra(code: impl Into<String>, detail: impl std::fmt::Display) -> Self {
        CliError {
            class: ExitClass::Infra,
            code: code.into(),
            detail: detail.to_string(),
        }
    }

    /// Ctrl-C.
    pub fn interrupted() -> Self {
        CliError {
            class: ExitClass::Interrupted,
            code: "interrupted".into(),
            detail: "interrupted by signal".into(),
        }
    }

    pub fn class(&self) -> ExitClass {
        self.class
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    /// The stderr rendering.
    pub fn problem(&self) -> Problem {
        Problem {
            class: self.class,
            code: self.code.clone(),
            detail: self.detail.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The mapping is total and the numbers are the contract.
    #[test]
    fn every_class_has_its_documented_code() {
        assert_eq!(ExitClass::Domain.code(), 1);
        assert_eq!(ExitClass::Usage.code(), 2);
        assert_eq!(ExitClass::Infra.code(), 3);
        assert_eq!(ExitClass::Interrupted.code(), 130);
    }

    #[test]
    fn a_problem_renders_class_code_detail() {
        let error = CliError::domain("not_authenticated", "run `zurfur session login` first");
        let rendered = serde_json::to_string(&error.problem()).unwrap();
        assert_eq!(
            rendered,
            r#"{"class":"domain","code":"not_authenticated","detail":"run `zurfur session login` first"}"#
        );
    }

    #[test]
    fn constructors_pick_the_class() {
        assert_eq!(CliError::domain("x", "y").class(), ExitClass::Domain);
        assert_eq!(CliError::infra("x", "y").class(), ExitClass::Infra);
        assert_eq!(CliError::interrupted().class(), ExitClass::Interrupted);
    }
}
