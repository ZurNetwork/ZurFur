//! In-memory fakes for the actor super-table (ZMVP-122, DD `34013187`).
//!
//! The map mirrors the pg `actor_identity` table — a [`StoredActorIdentity`]
//! parts struct per id, growing a field per slice (kind landed in slice 2;
//! optional did, handle, state follow) exactly as the table grows columns.
//! No removal path exists anywhere in this module: identity rows are immortal.

use async_trait::async_trait;
use domain::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind};
use domain::ports::{ActorIdentityStore, ActorIdentityWrites};

use crate::MemBackend;

/// The stored parts of one actor identity, keyed by its id in the backend map —
/// growing a field per slice exactly as the pg table grows columns.
#[derive(Debug, Clone)]
pub struct StoredActorIdentity {
    /// What kind of actor the row is (slice 2).
    pub kind: ActorKind,
}

/// In-memory [`ActorIdentityStore`] read surface over the shared [`MemBackend`].
pub struct MemActorIdentityStore(pub(crate) MemBackend);

#[async_trait]
impl ActorIdentityStore for MemActorIdentityStore {
    async fn find(&self, id: ActorIdentityId) -> anyhow::Result<Option<ActorIdentity>> {
        let identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        Ok(identities.get(&id).map(|stored| ActorIdentity {
            id,
            kind: stored.kind,
        }))
    }
}

/// In-memory [`ActorIdentityWrites`] view — vended only by the mem
/// [`UnitOfWork`](domain::ports::UnitOfWork) over its staged snapshot, so an
/// uncommitted create is discarded exactly as pg rolls back.
pub struct MemActorIdentityWrites(pub(crate) MemBackend);

#[async_trait]
impl ActorIdentityWrites for MemActorIdentityWrites {
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()> {
        let mut identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        // Check-then-insert, NOT insert-then-check: the pg PK rejects a
        // duplicate without touching the existing row, and the mem mirror must
        // not clobber the stored value on the error path either.
        if identities.contains_key(&identity.id) {
            // Mirror the pg PK: creating the same id twice is a caller bug.
            anyhow::bail!("actor identity already exists: {}", *identity.id);
        }
        identities.insert(
            identity.id,
            StoredActorIdentity {
                kind: identity.kind,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use domain::elements::actor_identity::{ActorIdentity, ActorKind};

    use crate::MemBackend;

    /// Slice-1 round-trip: created through the unit of work, committed, found.
    #[tokio::test]
    async fn create_commit_find_round_trips() {
        let backend = MemBackend::new();
        let identity = ActorIdentity::mint(ActorKind::User);

        let mut uow = backend.database().begin().await.expect("begin");
        uow.actor_identities()
            .create(&identity)
            .await
            .expect("create");
        uow.commit().await.expect("commit");

        let found = backend
            .actor_identity_store()
            .find(identity.id)
            .await
            .expect("find");
        assert_eq!(found, Some(identity));
    }

    /// The mem mirror of pg's rollback-on-drop: an uncommitted create is invisible.
    #[tokio::test]
    async fn uncommitted_create_rolls_back() {
        let backend = MemBackend::new();
        let identity = ActorIdentity::mint(ActorKind::User);

        {
            let mut uow = backend.database().begin().await.expect("begin");
            uow.actor_identities()
                .create(&identity)
                .await
                .expect("create");
            // Dropped without commit.
        }

        let found = backend
            .actor_identity_store()
            .find(identity.id)
            .await
            .expect("find");
        assert_eq!(found, None);
    }

    /// The PK mirror: the same id cannot be created twice.
    #[tokio::test]
    async fn duplicate_create_errors() {
        let backend = MemBackend::new();
        let identity = ActorIdentity::mint(ActorKind::User);

        let mut uow = backend.database().begin().await.expect("begin");
        uow.actor_identities()
            .create(&identity)
            .await
            .expect("first create");
        uow.commit().await.expect("commit");

        let mut uow = backend.database().begin().await.expect("begin");
        let second = uow.actor_identities().create(&identity).await;
        assert!(second.is_err(), "duplicate create must error");
    }
}
