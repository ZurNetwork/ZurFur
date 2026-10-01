//! Ports: traits named by the role they play for the domain, implemented by the
//! adapter crates. Per-area ports live in submodules ([`commission`],
//! [`changelog`], …) and are re-exported flat here, so a new area adds a file
//! rather than growing this one.

pub mod actor_identity;
pub mod changelog;
pub mod character;
pub mod commission;
pub mod file;
pub mod workflow;

mod account;
mod authenticator;
mod errors;
mod identity;
mod profile;
mod public_records;
mod unit_of_work;
mod user;

pub use account::{AccountReads, AccountRepo, AccountStore, AccountWrites};
pub use actor_identity::{ActorIdentityStore, ActorIdentityWrites};
pub use authenticator::Authenticator;
pub use changelog::{ChangelogStore, ChangelogWrites};
pub use character::{CharacterStore, CharacterWrites};
pub use commission::{
    CommissionReads, CommissionRepo, CommissionStore, CommissionWrites, ElementNotFound,
    UnknownSurface, UnknownTab,
};
pub use errors::{DidBelongsToAnotherActor, HandleTaken, PublicRecordsError};
pub use file::FileStore;
pub use identity::{DidMinter, DidOperations, KeyStore, PlcOperationLog};
pub use profile::{ProfileCache, ProfileSource};
pub use public_records::PublicRecords;
pub use unit_of_work::{Database, Unit, UnitOfWork, UnitOfWorkFn};
pub use user::{UserStore, UserWrites};
pub use workflow::{ColumnStore, ColumnWrites, WorkflowStore, WorkflowWrites};
