//! The orchestrator's factory: one [`Ports`] bag assembled by the composition
//! root, one [`App`] over it, and the per-entity namespaces vended from it.
//! Drivers hold an [`App`] and pass `now`; they never see a port.

use std::sync::Arc;

use domain::ports::{
    AccountStore, ChangelogStore, ColumnStore, CommissionStore, Database, DidMinter, FileStore,
    ProfileCache, ProfileSource, UserStore, WorkflowStore,
};

use crate::{account::Accounts, commission::Commissions, user::Users};

/// Every port the orchestrator may use, built once by the composition root.
/// Reads are pool-side; writes and in-unit reads come from [`Database::begin`].
/// Every entry is required, so no namespace can find one missing at first use.
pub struct Ports {
    pub database: Arc<dyn Database>,
    pub users: Arc<dyn UserStore>,
    pub accounts: Arc<dyn AccountStore>,
    pub commissions: Arc<dyn CommissionStore>,
    pub changelog: Arc<dyn ChangelogStore>,
    pub profile_source: Arc<dyn ProfileSource>,
    pub profile_cache: Arc<dyn ProfileCache>,
    pub did_minter: Arc<dyn DidMinter>,
    pub files: Arc<dyn FileStore>,
    pub workflows: Arc<dyn WorkflowStore>,
    pub columns: Arc<dyn ColumnStore>,
}

/// The orchestrator, built once at composition: a namespace over [`Ports`]
/// whose accessors vend each entity's use cases with the ports already bound.
pub struct App {
    ports: Ports,
}

impl App {
    /// Wrap the assembled ports.
    pub fn new(ports: Ports) -> Self {
        Self { ports }
    }

    /// The bag itself, for call sites that still build a per-module `*Ports`
    /// view.
    pub fn ports(&self) -> &Ports {
        &self.ports
    }

    /// Commission use cases, with the bag's ports already bound.
    pub fn commissions(&self) -> Commissions<'_> {
        Commissions::from(self)
    }

    /// Account use cases, with the bag's ports already bound.
    pub fn accounts(&self) -> Accounts<'_> {
        Accounts::from(self)
    }

    /// User use cases.
    pub fn users(&self) -> Users<'_> {
        Users::from(self)
    }
}

/// A namespace needs a port the composition root did not supply. Unreachable
/// while every [`Ports`] entry is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPort {
    Files,
    DidMinter,
}

impl std::fmt::Display for MissingPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            MissingPort::Files => "files",
            MissingPort::DidMinter => "did_minter",
        };
        write!(f, "missing port: {name}")
    }
}

impl std::error::Error for MissingPort {}
