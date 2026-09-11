//! The domain elements — Zurfur's nouns.
//!
//! Each submodule holds one entity or closely-related cluster: its identity
//! type, its value objects, and the pure construction and invariant logic that
//! belongs with it. Every element mirrors an entry in the DESIGN glossary, which
//! is the single source of truth for what it means; [`achievement`], [`blob`],
//! [`character`] and [`markdown`] are still stubs.

pub mod account;
pub mod account_keys;
pub mod achievement;
pub mod actor_identity;
pub mod blob;
pub mod character;
pub mod commission;
pub mod did;
pub mod handle;
pub mod id;
pub mod invitation;
pub mod markdown;
pub mod maturity;
pub mod plc_operation;
pub mod profile;
pub mod public_record;
pub mod role;
pub mod user;
pub mod user_account;
pub mod workflow;
