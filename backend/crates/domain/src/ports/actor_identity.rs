//! Ports for the actor super-table (ZMVP-122, DD `34013187`).
//!
//! Reads/writes split per the house idiom: [`ActorIdentityStore`] is pool-backed
//! and non-transactional; [`ActorIdentityWrites`] is reachable only on an open
//! [`UnitOfWork`](crate::ports::UnitOfWork) (DD `24150017`).
//!
//! **Deliberately no delete on either trait** — identity rows are immortal
//! (DD `34013187` decision 3); liveness arrives in a later slice as a *state*
//! whose transition machinery is ticket ZMVP-125's scope (a Jira key, not a
//! slice number of this PR series) — never as a removal.

use async_trait::async_trait;

use crate::elements::actor_identity::{ActorIdentity, ActorIdentityId};

/// The read surface of the actor super-table. Pool-backed, non-transactional.
#[async_trait]
pub trait ActorIdentityStore: Send + Sync {
    /// Resolve an [`ActorIdentityId`] back to its row, or `None` if no such
    /// identity exists. (There is no "gone" — rows are immortal — so `None`
    /// always means *never seen*.)
    async fn find(&self, id: ActorIdentityId) -> anyhow::Result<Option<ActorIdentity>>;
}

/// The write surface of the actor super-table — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork), so no identity write can skip a
/// transaction.
#[async_trait]
pub trait ActorIdentityWrites: Send {
    /// Persist a freshly minted [`ActorIdentity`]. Creating the same id twice is
    /// an error (PK) — in this slice ids are always freshly minted, so a
    /// collision is a caller bug, not a race to absorb.
    ///
    /// This becomes the race-safe, DID-keyed `intern` upsert in the slice that
    /// adds the (nullable) `did` column — the one enforced write path of DD
    /// `34013187` decision 6.
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()>;
}
