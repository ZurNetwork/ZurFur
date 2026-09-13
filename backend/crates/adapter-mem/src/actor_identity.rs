//! In-memory fakes for the actor super-table, mirroring the pg
//! `actor_identity` table one field per column.
//!
//! No removal path exists anywhere in this module: identity rows are immortal.

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind, ActorState};
use domain::elements::did::Did;
use domain::ports::{ActorIdentityStore, ActorIdentityWrites};

use crate::MemBackend;

/// The stored parts of one actor identity, keyed by its id in the backend map.
/// `PartialEq` lets `crate::merge_map` tell an untouched row from one this
/// unit wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredActorIdentity {
    /// What kind of actor the row is.
    pub kind: ActorKind,
    /// The actor's DID, when it has one; uniqueness binds only present DIDs.
    pub did: Option<Did>,
    /// Liveness; rows are born Active.
    pub state: ActorState,
    /// The refreshable display-handle cache; foreign data, born `None`.
    pub handle: Option<String>,
    /// When the Index first saw the actor; immutable across a re-intern.
    pub first_seen: DateTimeUtc,
}

/// Rebuild the domain row from its stored parts.
fn rebuild(id: ActorIdentityId, stored: &StoredActorIdentity) -> ActorIdentity {
    ActorIdentity {
        id,
        kind: stored.kind,
        did: stored.did.clone(),
        state: stored.state,
        handle: stored.handle.clone(),
        first_seen: stored.first_seen,
    }
}

/// In-memory [`ActorIdentityStore`] read surface over the shared [`MemBackend`].
pub struct MemActorIdentityStore(pub(crate) MemBackend);

#[async_trait]
impl ActorIdentityStore for MemActorIdentityStore {
    async fn find(&self, id: &ActorIdentityId) -> anyhow::Result<Option<ActorIdentity>> {
        let identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        Ok(identities.get(id).map(|stored| rebuild(*id, stored)))
    }

    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<ActorIdentity>> {
        let identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        Ok(identities
            .iter()
            .find(|(_, stored)| stored.did.as_ref() == Some(did))
            .map(|(id, stored)| rebuild(*id, stored)))
    }
}

/// In-memory [`ActorIdentityWrites`] view, vended only by the mem
/// [`UnitOfWork`](domain::ports::UnitOfWork) over its staged snapshot.
pub struct MemActorIdentityWrites(pub(crate) MemBackend);

#[async_trait]
impl ActorIdentityWrites for MemActorIdentityWrites {
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()> {
        // The DID-less path by contract: intern owns DID-bearing rows.
        anyhow::ensure!(
            identity.did.is_none(),
            "create is the DID-less path; intern DID-bearing actors instead"
        );
        // Born active by invariant: transitions never pass through creation.
        anyhow::ensure!(
            identity.state == ActorState::Active,
            "create only persists born-active identities"
        );
        // Born uncached: the handle is filled via cache_handle only.
        anyhow::ensure!(
            identity.handle.is_none(),
            "create only persists born-uncached identities; fill via cache_handle"
        );
        let mut identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        // Check-then-insert, so the error path never clobbers the stored row.
        if identities.contains_key(&identity.id) {
            // Mirror the pg PK: creating the same id twice is a caller bug.
            anyhow::bail!("actor identity already exists: {}", *identity.id);
        }
        identities.insert(
            identity.id,
            StoredActorIdentity {
                kind: identity.kind,
                did: None,
                state: identity.state,
                handle: None,
                first_seen: identity.first_seen,
            },
        );
        Ok(())
    }

    async fn intern(
        &mut self,
        did: &Did,
        kind: ActorKind,
        now: DateTimeUtc,
    ) -> anyhow::Result<ActorIdentity> {
        let mut identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        // The mem mirror of ON CONFLICT (did): an existing DID wins as-is.
        if let Some((id, stored)) = identities
            .iter()
            .find(|(_, stored)| stored.did.as_ref() == Some(did))
        {
            return Ok(rebuild(*id, stored));
        }
        let minted = ActorIdentity {
            id: ActorIdentityId::new(uuid::Uuid::now_v7()),
            kind,
            did: Some(did.clone()),
            state: ActorState::Active,
            handle: None,
            first_seen: now,
        };
        identities.insert(
            minted.id,
            StoredActorIdentity {
                kind,
                did: Some(did.clone()),
                state: ActorState::Active,
                handle: None,
                first_seen: now,
            },
        );
        Ok(minted)
    }

    async fn cache_handle(
        &mut self,
        id: &ActorIdentityId,
        handle: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        let stored = identities
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("actor identity not found: {}", **id))?;
        stored.handle = handle.map(str::to_string);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
