//! The domain elements — Zurfur's nouns.
//!
//! Each submodule holds one entity (or closely-related cluster): its identity
//! type, its value objects, and the pure construction/invariant logic that
//! belongs with it. Every page here mirrors an entry in the DESIGN glossary
//! (<https://zurnetwork.atlassian.net/wiki/spaces/DESIGN>), which is the single
//! source of truth for what each element *means*.
//!
//! The live elements drive the current MVP tickets:
//! - [`account`] — an [`account::Account`], its own sovereign identity, founded
//!   with an Owner (ZMVP-14, DESIGN/Account).
//! - [`user`] — a recognized visitor (ZMVP-9, DESIGN/User).
//! - [`role`] — a member's rank inside an account and the grant rule
//!   (ZMVP-15/16, DESIGN/Roles).
//! - [`user_account`] — the membership tuple binding a user, an account, and a role.
//! - [`invitation`] — a pending offer of membership, issued then accepted/revoked
//!   (ZMVP-32/ZMVP-20, DESIGN/Roles).
//! - [`did`] — a decentralized identifier, the AT Protocol identity primitive.
//! - [`handle`] — a validated, normalized atproto-style Account handle, the one
//!   shared claim-validation gate (ZMVP-48/45, DESIGN/24870914 §6, DD/26050561).
//! - [`profile`] — a visitor's public, PDS-owned profile (ZMVP-10).
//!
//! The rest are stubs — identity types and shapes sketched ahead of the work
//! that fills them in, documented honestly per the glossary: [`achievement`],
//! [`blob`], [`character`], [`commission`], [`golem`], [`markdown`], [`workflow`].

pub mod account;
pub mod achievement;
pub mod blob;
pub mod character;
pub mod commission;
pub mod did;
pub mod golem;
pub mod handle;
pub mod invitation;
pub mod markdown;
pub mod profile;
pub mod role;
pub mod user;
pub mod user_account;
pub mod workflow;
