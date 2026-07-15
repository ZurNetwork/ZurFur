//! In-memory fakes for the actor super-table (ZMVP-122, DD `34013187`).
//!
//! The map mirrors the pg `actor_identity` table — a [`StoredActorIdentity`]
//! parts struct per id, growing a field per slice (kind in slice 2, the
//! optional did in slice 3; handle, state follow) exactly as the table grows
//! columns. No removal path exists anywhere in this module: identity rows are
//! immortal.

use async_trait::async_trait;
use domain::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind};
use domain::elements::did::Did;
use domain::ports::{ActorIdentityStore, ActorIdentityWrites};

use crate::MemBackend;

/// The stored parts of one actor identity, keyed by its id in the backend map —
/// growing a field per slice exactly as the pg table grows columns.
#[derive(Debug, Clone)]
pub struct StoredActorIdentity {
    /// What kind of actor the row is (slice 2).
    pub kind: ActorKind,
    /// The actor's DID, when it has one (slice 3) — `None` is the designed
    /// DID-less state (Characters), and uniqueness binds only present DIDs.
    pub did: Option<Did>,
}

/// Rebuild the domain row from its stored parts.
fn rebuild(id: ActorIdentityId, stored: &StoredActorIdentity) -> ActorIdentity {
    ActorIdentity {
        id,
        kind: stored.kind,
        did: stored.did.clone(),
    }
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
        Ok(identities.get(&id).map(|stored| rebuild(id, stored)))
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

/// In-memory [`ActorIdentityWrites`] view — vended only by the mem
/// [`UnitOfWork`](domain::ports::UnitOfWork) over its staged snapshot, so an
/// uncommitted create is discarded exactly as pg rolls back.
pub struct MemActorIdentityWrites(pub(crate) MemBackend);

#[async_trait]
impl ActorIdentityWrites for MemActorIdentityWrites {
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()> {
        // The DID-less path by contract: intern owns DID-bearing rows.
        anyhow::ensure!(
            identity.did.is_none(),
            "create is the DID-less path; intern DID-bearing actors instead"
        );
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
                did: None,
            },
        );
        Ok(())
    }

    async fn intern(&mut self, did: &Did, kind: ActorKind) -> anyhow::Result<ActorIdentity> {
        let mut identities = self
            .0
            .actor_identities
            .lock()
            .expect("MemBackend actor_identities mutex poisoned");
        // The mem mirror of ON CONFLICT (did): an existing DID wins as-is —
        // its stored kind is deliberately not rewritten (ZMVP-126 refines).
        if let Some((id, stored)) = identities
            .iter()
            .find(|(_, stored)| stored.did.as_deref() == Some(&**did))
        {
            return Ok(rebuild(*id, stored));
        }
        let minted = ActorIdentity {
            id: ActorIdentityId::new(uuid::Uuid::now_v7()),
            kind,
            did: Some(did.clone()),
        };
        identities.insert(
            minted.id,
            StoredActorIdentity {
                kind,
                did: Some(did.clone()),
            },
        );
        Ok(minted)
    }
}

#[cfg(test)]
mod tests {
    use domain::elements::actor_identity::{ActorIdentity, ActorKind};
    use domain::elements::did::Did;

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

    /// Slice 3: intern is idempotent by DID — the second call returns the
    /// first call's row, kind un-rewritten; a distinct DID gets its own row.
    #[tokio::test]
    async fn intern_is_idempotent_by_did() {
        let backend = MemBackend::new();
        let did = Did::new("did:plc:intern-me".to_string());

        let mut uow = backend.database().begin().await.expect("begin");
        let first = uow
            .actor_identities()
            .intern(&did, ActorKind::User)
            .await
            .expect("first intern");
        uow.commit().await.expect("commit");

        let mut uow = backend.database().begin().await.expect("begin");
        let again = uow
            .actor_identities()
            .intern(&did, ActorKind::Account)
            .await
            .expect("re-intern");
        uow.commit().await.expect("commit");

        assert_eq!(again, first, "re-intern returns the existing row as-is");

        let other = Did::new("did:plc:someone-else".to_string());
        let mut uow = backend.database().begin().await.expect("begin");
        let second = uow
            .actor_identities()
            .intern(&other, ActorKind::User)
            .await
            .expect("intern other");
        uow.commit().await.expect("commit");
        assert_ne!(second.id, first.id);

        let by_did = backend
            .actor_identity_store()
            .find_by_did(&did)
            .await
            .expect("find_by_did");
        assert_eq!(by_did, Some(first));
    }

    /// Slice 3: create refuses a DID-bearing identity — intern owns that path.
    #[tokio::test]
    async fn create_refuses_did_bearing_rows() {
        let backend = MemBackend::new();
        let mut identity = ActorIdentity::mint(ActorKind::User);
        identity.did = Some(Did::new("did:plc:sneaky".to_string()));

        let mut uow = backend.database().begin().await.expect("begin");
        let result = uow.actor_identities().create(&identity).await;
        assert!(result.is_err(), "create is the DID-less path only");
    }
}
