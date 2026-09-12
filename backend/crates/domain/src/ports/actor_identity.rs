//! Ports for the actor super-table: [`ActorIdentityStore`] pool-backed,
//! [`ActorIdentityWrites`] reachable only on an open
//! [`UnitOfWork`](crate::ports::UnitOfWork). Neither carries a delete — identity
//! rows are immortal, and liveness is a state on the row.

use async_trait::async_trait;

use crate::datetime::DateTimeUtc;
use crate::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind};
use crate::elements::did::Did;

/// The read surface of the actor super-table. Pool-backed, non-transactional.
#[async_trait]
pub trait ActorIdentityStore: Send + Sync {
    /// Resolve an [`ActorIdentityId`] back to its row, or `None`. Rows are
    /// immortal, so `None` always means never seen.
    async fn find(&self, id: &ActorIdentityId) -> anyhow::Result<Option<ActorIdentity>>;

    /// Resolve a [`Did`] to the one actor that holds it, or `None` if never seen.
    /// One DID maps to at most one actor, ever.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<ActorIdentity>>;
}

/// The write surface of the actor super-table — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork), so no identity write can skip a
/// transaction.
#[async_trait]
pub trait ActorIdentityWrites: Send {
    /// Persist a freshly minted DID-less [`ActorIdentity`]. Adapter-enforced: the
    /// row must arrive with `did == None`, born-active and handle-uncached, and a
    /// `user`/`account` kind is rejected here since those must carry a DID.
    /// Creating the same id twice is an error, not a race to absorb. DID-bearing
    /// actors go through [`intern`](ActorIdentityWrites::intern).
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()>;

    /// Intern a DID-bearing actor — the race-safe, idempotent upsert, and the only
    /// write path for DID-keyed identities: concurrent interns of one DID converge
    /// on one row at the `did UNIQUE` index. `kind` and `now` apply to a new row
    /// only; an existing row comes back as-is, keeping its kind and `first_seen`.
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
        id: &ActorIdentityId,
        handle: Option<&str>,
    ) -> anyhow::Result<()>;
}
