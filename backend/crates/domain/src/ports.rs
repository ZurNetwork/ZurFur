//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates. Per-area ports live in submodules ([`commission`],
//! [`changelog`], …) and are re-exported flat here, so a new area adds a file
//! rather than growing this one.

pub mod changelog;
pub mod commission;
pub mod file;
pub mod workflow;

mod errors;
mod unit_of_work;

pub use ::identity::ports::*;
pub use changelog::{ChangelogStore, ChangelogWrites};
pub use commission::{
    CommissionReads, CommissionRepo, CommissionStore, CommissionWrites, ElementNotFound,
    UnknownSurface, UnknownTab,
};
pub use file::FileStore;
pub use unit_of_work::{Database, Unit, UnitOfWork, UnitOfWorkFn};
pub use workflow::{ColumnStore, ColumnWrites, WorkflowStore, WorkflowWrites};
