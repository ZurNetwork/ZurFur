//! Ports for the actor super-table (ZMVP-122, DD `34013187`).
//!
//! Reads/writes split per the house idiom: [`ActorIdentityStore`] is pool-backed
//! and non-transactional; [`ActorIdentityWrites`] is reachable only on an open
//! [`UnitOfWork`](crate::ports::UnitOfWork) (DD `24150017`).
//!
//! **Deliberately no delete on either trait** — identity rows are immortal
//! (DD `34013187` decision 3); liveness is a *state* on the row, never a
//! removal. Its transition machinery is ticket ZMVP-125's scope.

use async_trait::async_trait;

use crate::datetime::DateTimeUtc;
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
    /// Persist a freshly minted **DID-less** [`ActorIdentity`]. The path contract
    /// is `did == None` (adapter-enforced), and the row must arrive as minted:
    /// born-active, handle uncached (also enforced). In the domain the DID-less
    /// actors are Characters (DD `34013187`); since ZMVP-123 the store's per-kind
    /// DID CHECK makes that concrete — a `user`/`account` identity MUST carry a DID
    /// (their projections' former `did NOT NULL` invariant) and is rejected here, so
    /// this path persists only DID-less *kinds* (Characters, and any future DID-less
    /// kind). Creating the same id twice is an error (PK) — ids are always freshly
    /// minted, so a collision is a caller bug, not a race to absorb. DID-bearing
    /// actors go through [`intern`](ActorIdentityWrites::intern).
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()>;

    /// Intern a DID-bearing actor: the **race-safe, idempotent** upsert of DD
    /// `34013187` decision 6, and the only write path for DID-keyed identities.
    /// The first call for a DID mints its row; every later call returns that
    /// same row — two concurrent interns of one DID converge on one row at the
    /// `did UNIQUE` index, never a duplicate and never an error.
    ///
    /// `kind` is the caller's classification for a *new* row; `now` stamps a
    /// *new* row's `first_seen`. An existing row is returned **as-is** — its
    /// stored kind is not rewritten (refinement is intake's business,
    /// ZMVP-126) and its `first_seen` keeps the original sighting.
    async fn intern(
        &mut self,
        did: &Did,
        kind: ActorKind,
        now: DateTimeUtc,
    ) -> anyhow::Result<ActorIdentity>;

    /// Refresh (or clear, with `None`) the actor's cached display handle — a
    /// cache fill, not a claim: the value is foreign network data and is never
    /// validated against Zurfur's handle rules. Errors if no such identity
    /// exists (caching for a never-seen actor is a caller bug).
    async fn cache_handle(
        &mut self,
        id: ActorIdentityId,
        handle: Option<&str>,
    ) -> anyhow::Result<()>;
}
