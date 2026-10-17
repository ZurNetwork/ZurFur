//! The [`Account`] — a platform-custodied entity that is its own sovereign
//! identity.
//!
//! An account holds a minted `did:plc` of its own, a validated human name, a
//! handle, and soft-delete timestamps. It is founded together with its founder's
//! Owner membership in a single act, [`Account::open`].

mod entity;
mod errors;
mod id;
mod rows;
mod value;

pub use entity::Account;
pub use errors::AccountNameError;
pub use id::AccountId;
pub use rows::{AccountMembership, AccountProfile, ListingScope};
pub use value::{ACCOUNT_NAME_MAX_LEN, AccountName};
