//! Identity ports: traits named by the role they play for the domain,
//! implemented by the adapter crates. Per-area ports live in submodules
//! ([`actor_identity`], [`character`], …) and are re-exported flat here, so a
//! new area adds a file rather than growing this one.

pub mod actor_identity;
pub mod character;

mod account;
mod authenticator;
mod errors;
mod identity;
mod identity_resolver;
mod profile;
mod public_records;
mod user;

pub use account::{AccountReads, AccountRepo, AccountStore, AccountWrites};
pub use actor_identity::{ActorIdentityStore, ActorIdentityWrites};
pub use authenticator::{Authenticator, BrowserBinding, SigninStarted};
pub use character::{CharacterStore, CharacterWrites};
pub use errors::{
    AccountMismatch, BrowserMismatch, DidBelongsToAnotherActor, HandleTaken, PublicRecordsError,
    ResolveError,
};
pub use identity::{DidMinter, DidOperations, KeyStore, PlcOperationLog};
pub use identity_resolver::IdentityResolver;
pub use profile::{ProfileCache, ProfileSource};
pub use public_records::PublicRecords;
pub use user::{UserStore, UserWrites};
