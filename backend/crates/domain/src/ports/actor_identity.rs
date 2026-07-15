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

use crate::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind};
use crate::elements::did::Did;

/// The read surface of the actor super-table. Pool-backed, non-transactional.
#[async_trait]
pub trait ActorIdentityStore: Send + Sync {
    /// Resolve an [`ActorIdentityId`] back to its row, or `None` if no such
    /// identity exists. (There is no "gone" — rows are immortal — so `None`
    /// always means *never seen*.)
    async fn find(&self, id: ActorIdentityId) -> anyhow::Result<Option<ActorIdentity>>;

    /// Resolve a [`Did`] to the one actor that holds it, or `None` if the DID
    /// has never been seen. One DID maps to at most one actor, ever — the
    /// schema's `UNIQUE` enforces it, this read just follows the index.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<ActorIdentity>>;
}

/// The write surface of the actor super-table — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork), so no identity write can skip a
/// transaction.
#[async_trait]
pub trait ActorIdentityWrites: Send {
    /// Persist a freshly minted **DID-less** [`ActorIdentity`]. The contract
    /// is `did == None` (adapter-enforced), not any particular kind — in the
    /// domain, Characters are the actors born DID-less (DD `34013187`).
    /// Creating the same id twice is an error (PK) — ids are always freshly
    /// minted, so a collision is a caller bug, not a race to absorb.
    /// DID-bearing actors go through [`intern`](ActorIdentityWrites::intern).
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()>;

    /// Intern a DID-bearing actor: the **race-safe, idempotent** upsert of DD
    /// `34013187` decision 6, and the only write path for DID-keyed identities.
    /// The first call for a DID mints its row; every later call returns that
    /// same row — two concurrent interns of one DID converge on one row at the
    /// `did UNIQUE` index, never a duplicate and never an error.
    ///
    /// `kind` is the caller's classification for a *new* row. An existing row's
    /// stored kind is returned as-is and **not** rewritten — kind refinement is
    /// intake's business (ZMVP-126), not a side effect of re-seeing a DID.
    async fn intern(&mut self, did: &Did, kind: ActorKind) -> anyhow::Result<ActorIdentity>;
}
