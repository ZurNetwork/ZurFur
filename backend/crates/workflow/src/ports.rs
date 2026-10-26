//! Workflow ports: traits named by the role they play for the domain,
//! implemented by the adapter crates. Per-area ports live in submodules
//! ([`commission`], [`changelog`], …) and are re-exported flat here, so a new
//! area adds a file rather than growing this one.

pub mod changelog;
pub mod commission;
pub mod file;
pub mod workflow;

mod errors;

pub use changelog::{ChangelogStore, ChangelogWrites};
pub use commission::{
    CommissionReads, CommissionRepo, CommissionRows, CommissionStore, CommissionWrites,
    ElementNotFound, UnknownSurface, UnknownTab,
};
pub use file::FileStore;
pub use workflow::{ColumnStore, ColumnWrites, WorkflowStore, WorkflowWrites};
