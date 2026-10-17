//! Use cases about bug reports: a signed-in User files one, and an operator
//! lists them.

pub mod file;
pub mod list;

use crate::ports::WithPorts;

/// Bug-report use cases, with the ports already bound.
#[derive(Clone, Copy, WithPorts)]
pub struct BugReports<'a> {
    #[ports(is_root = true)]
    ports: &'a crate::Ports,
}

impl<'a> From<&'a crate::Ports> for BugReports<'a> {
    /// Binds the namespace to the bag.
    fn from(ports: &'a crate::Ports) -> Self {
        Self { ports }
    }
}

impl<'a> From<&'a crate::App> for BugReports<'a> {
    /// Binds the namespace to the app's ports.
    fn from(app: &'a crate::App) -> Self {
        Self::from(app.ports())
    }
}

/// Why a bug-report use case could not answer. `Display` stays terse and never
/// carries the report; a store failure's cause rides `source()`.
#[derive(Debug, thiserror::Error)]
pub enum BugReportError {
    /// The reporter has filed as many reports as the window allows.
    #[error("too many bug reports recently")]
    RateLimited,
    /// A port failed; nothing was stored, and the caller may retry.
    #[error("the bug-report store failed")]
    Infrastructure(#[from] anyhow::Error),
}

/// What a bug-report use case answers.
pub type BugReportResult<T> = Result<T, BugReportError>;
