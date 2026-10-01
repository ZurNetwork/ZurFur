use chrono::Utc;
use domain::elements::actor_identity::{ActorIdentity, ActorKind, ActorState};
use domain::elements::did::Did;

use crate::MemBackend;

/// Slice-1 round-trip: created through the unit of work, committed, found.
#[tokio::test]
async fn create_commit_find_round_trips() {
    let backend = MemBackend::new();
    let identity = ActorIdentity::mint(ActorKind::User, Utc::now());

    let mut uow = backend.database().begin().await.expect("begin");
    uow.actor_identities()
        .create(&identity)
        .await
        .expect("create");
    uow.commit().await.expect("commit");

    let found = backend
        .actor_identity_store()
        .find(&identity.id)
        .await
        .expect("find");
    assert_eq!(found, Some(identity));
}

/// The mem mirror of pg's rollback-on-drop: an uncommitted create is invisible.
#[tokio::test]
async fn uncommitted_create_rolls_back() {
    let backend = MemBackend::new();
    let identity = ActorIdentity::mint(ActorKind::User, Utc::now());

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
        .find(&identity.id)
        .await
        .expect("find");
    assert_eq!(found, None);
}

/// The PK mirror: the same id cannot be created twice.
#[tokio::test]
async fn duplicate_create_errors() {
    let backend = MemBackend::new();
    let identity = ActorIdentity::mint(ActorKind::User, Utc::now());

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
    let did = Did::from("did:plc:intern-me".to_string());

    let first_sighting = Utc::now();
    let mut uow = backend.database().begin().await.expect("begin");
    let first = uow
        .actor_identities()
        .intern(&did, ActorKind::User, first_sighting)
        .await
        .expect("first intern");
    uow.commit().await.expect("commit");

    // Different kind AND different clock on re-sight: row untouched.
    let mut uow = backend.database().begin().await.expect("begin");
    let again = uow
        .actor_identities()
        .intern(
            &did,
            ActorKind::Account,
            first_sighting + chrono::Duration::days(1),
        )
        .await
        .expect("re-intern");
    uow.commit().await.expect("commit");

    assert_eq!(again, first, "re-intern returns the existing row as-is");

    let other = Did::from("did:plc:someone-else".to_string());
    let mut uow = backend.database().begin().await.expect("begin");
    let second = uow
        .actor_identities()
        .intern(&other, ActorKind::User, Utc::now())
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

/// Slice 5: the handle cache fills, refreshes, clears — and errors for a
/// never-seen actor.
#[tokio::test]
async fn handle_cache_fills_refreshes_and_clears() {
    let backend = MemBackend::new();
    let did = Did::from("did:plc:cache-me".to_string());

    let mut uow = backend.database().begin().await.expect("begin");
    let interned = uow
        .actor_identities()
        .intern(&did, ActorKind::User, Utc::now())
        .await
        .expect("intern");
    uow.actor_identities()
        .cache_handle(&interned.id, Some("alice.bsky.social"))
        .await
        .expect("cache");
    uow.commit().await.expect("commit");

    let found = backend
        .actor_identity_store()
        .find(&interned.id)
        .await
        .expect("find")
        .expect("row exists");
    assert_eq!(found.handle.as_deref(), Some("alice.bsky.social"));

    let mut uow = backend.database().begin().await.expect("begin");
    uow.actor_identities()
        .cache_handle(&interned.id, None)
        .await
        .expect("clear");
    let missing = uow
        .actor_identities()
        .cache_handle(
            &ActorIdentity::mint(ActorKind::User, Utc::now()).id,
            Some("ghost"),
        )
        .await;
    assert!(
        missing.is_err(),
        "caching for a never-seen actor must error"
    );
    uow.commit().await.expect("commit");

    let cleared = backend
        .actor_identity_store()
        .find(&interned.id)
        .await
        .expect("find")
        .expect("row exists");
    assert_eq!(cleared.handle, None);
}

/// Slice 3: create refuses a DID-bearing identity — intern owns that path.
#[tokio::test]
async fn create_refuses_did_bearing_rows() {
    let backend = MemBackend::new();
    let mut identity = ActorIdentity::mint(ActorKind::User, Utc::now());
    identity.did = Some(Did::from("did:plc:sneaky".to_string()));

    let mut uow = backend.database().begin().await.expect("begin");
    let result = uow.actor_identities().create(&identity).await;
    assert!(result.is_err(), "create is the DID-less path only");
}

/// Slice 4: create refuses a non-active identity — rows are born active,
/// and liveness transitions never pass through creation.
#[tokio::test]
async fn create_refuses_non_active_rows() {
    let backend = MemBackend::new();
    let mut identity = ActorIdentity::mint(ActorKind::User, Utc::now());
    identity.state = ActorState::Tombstoned;

    let mut uow = backend.database().begin().await.expect("begin");
    let result = uow.actor_identities().create(&identity).await;
    assert!(result.is_err(), "create persists born-active rows only");
}

/// Create refuses a pre-cached handle — rows are born uncached, and the
/// cache fills only via `cache_handle` (a silent drop would hide the
/// caller bug).
#[tokio::test]
async fn create_refuses_pre_cached_handles() {
    let backend = MemBackend::new();
    let mut identity = ActorIdentity::mint(ActorKind::User, Utc::now());
    identity.handle = Some("sneaky.example.com".to_string());

    let mut uow = backend.database().begin().await.expect("begin");
    let result = uow.actor_identities().create(&identity).await;
    assert!(result.is_err(), "create persists born-uncached rows only");
}
