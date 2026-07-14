//! The actor super-table over PostgreSQL (ZMVP-122 slices 1–4, DD 34013187),
//! against a throwaway container: round-trips through the Unit of Work,
//! rollback-on-drop, the duplicate-id PK, the kind and state CHECKs, the
//! race-safe idempotent intern by DID, and NULL-did coexistence next to
//! DB-enforced DID uniqueness. The cached handle lands in a later slice.
//! Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgActorIdentityStore, PgDatabase, PgPool};
use domain::{
    elements::{
        actor_identity::{ActorIdentity, ActorKind, ActorState},
        did::Did,
    },
    ports::{ActorIdentityStore, Database},
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// Boots a fresh database and runs all migrations. The container is returned so
/// the caller keeps it alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");
    (pool, container)
}

/// Slice-1 round-trip: created through the unit of work, committed, found.
#[tokio::test]
async fn create_commit_find_round_trips() {
    let (pool, _container) = fresh_pool().await;
    let identity = ActorIdentity::mint(ActorKind::User);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.actor_identities()
        .create(&identity)
        .await
        .expect("create");
    uow.commit().await.expect("commit");

    let store = PgActorIdentityStore::new(pool.clone());
    let found = store.find(identity.id).await.expect("find");
    assert_eq!(found, Some(identity));
}

/// Dropping the unit uncommitted rolls the create back (DD 24150017).
#[tokio::test]
async fn uncommitted_create_rolls_back() {
    let (pool, _container) = fresh_pool().await;
    let identity = ActorIdentity::mint(ActorKind::User);

    let db = PgDatabase::new(pool.clone());
    {
        let mut uow = db.begin().await.expect("begin");
        uow.actor_identities()
            .create(&identity)
            .await
            .expect("create");
        // Dropped without commit.
    }

    let store = PgActorIdentityStore::new(pool.clone());
    let found = store.find(identity.id).await.expect("find");
    assert_eq!(found, None, "an uncommitted create must not persist");
}

/// Slice 2: every kind round-trips, and the schema's CHECK holds the closed
/// vocabulary — a spelling outside it (e.g. a hypothetical `golem`) is rejected
/// by the database itself, not just app-side parsing.
#[tokio::test]
async fn kind_round_trips_and_check_rejects_unknown() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());
    let store = PgActorIdentityStore::new(pool.clone());

    for kind in [ActorKind::User, ActorKind::Account, ActorKind::Character] {
        let identity = ActorIdentity::mint(kind);
        let mut uow = db.begin().await.expect("begin");
        uow.actor_identities()
            .create(&identity)
            .await
            .expect("create");
        uow.commit().await.expect("commit");

        let found = store.find(identity.id).await.expect("find");
        assert_eq!(found, Some(identity), "{kind:?} must round-trip");
    }

    let rejected =
        sqlx::query("INSERT INTO actor_identity (id, kind, state) VALUES ($1, 'golem', 'active')")
            .bind(uuid::Uuid::now_v7())
            .execute(&pool)
            .await;
    assert!(
        rejected.is_err(),
        "the kind CHECK must reject a spelling outside the closed vocabulary"
    );
}

/// Slice 3: intern is race-safe-idempotent by DID — a re-intern (even claiming
/// a different kind) returns the first row untouched; a distinct DID mints its
/// own row; `find_by_did` follows the unique index.
#[tokio::test]
async fn intern_is_idempotent_by_did() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());
    let store = PgActorIdentityStore::new(pool.clone());
    let did = Did::new("did:plc:intern-me".to_string());

    let mut uow = db.begin().await.expect("begin");
    let first = uow
        .actor_identities()
        .intern(&did, ActorKind::User)
        .await
        .expect("first intern");
    uow.commit().await.expect("commit");

    let mut uow = db.begin().await.expect("begin");
    let again = uow
        .actor_identities()
        .intern(&did, ActorKind::Account)
        .await
        .expect("re-intern");
    uow.commit().await.expect("commit");
    assert_eq!(again, first, "re-intern returns the existing row as-is");

    let by_did = store.find_by_did(&did).await.expect("find_by_did");
    assert_eq!(by_did, Some(first.clone()));

    let other = Did::new("did:plc:someone-else".to_string());
    let mut uow = db.begin().await.expect("begin");
    let second = uow
        .actor_identities()
        .intern(&other, ActorKind::User)
        .await
        .expect("intern other");
    uow.commit().await.expect("commit");
    assert_ne!(second.id, first.id, "a distinct DID mints its own row");
}

/// Slice 3: DID-lessness is a designed state — many rows may carry NULL
/// (Characters), while a present DID is unique at the DB even against a raw
/// insert that skips the intern path.
#[tokio::test]
async fn null_dids_coexist_and_present_dids_are_unique() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    uow.actor_identities()
        .create(&ActorIdentity::mint(ActorKind::Character))
        .await
        .expect("first DID-less create");
    uow.actor_identities()
        .create(&ActorIdentity::mint(ActorKind::Character))
        .await
        .expect("second DID-less create — NULLs never collide");
    let interned = uow
        .actor_identities()
        .intern(&Did::new("did:plc:unique-me".to_string()), ActorKind::User)
        .await
        .expect("intern");
    uow.commit().await.expect("commit");

    let raw_duplicate = sqlx::query(
        "INSERT INTO actor_identity (id, kind, did, state) VALUES ($1, 'user', $2, 'active')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind("did:plc:unique-me")
    .execute(&pool)
    .await;
    assert!(
        raw_duplicate.is_err(),
        "did UNIQUE must hold even against a raw insert"
    );
    assert_eq!(
        PgActorIdentityStore::new(pool.clone())
            .find(interned.id)
            .await
            .expect("find"),
        Some(interned)
    );
}

/// Slice 4: rows are born active through both write paths, and the state
/// CHECK holds the closed vocabulary at the DB.
#[tokio::test]
async fn rows_are_born_active_and_state_check_holds() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());
    let store = PgActorIdentityStore::new(pool.clone());

    let created = ActorIdentity::mint(ActorKind::Character);
    let mut uow = db.begin().await.expect("begin");
    uow.actor_identities()
        .create(&created)
        .await
        .expect("create");
    let interned = uow
        .actor_identities()
        .intern(
            &Did::new("did:plc:born-active".to_string()),
            ActorKind::User,
        )
        .await
        .expect("intern");
    uow.commit().await.expect("commit");

    for identity in [&created, &interned] {
        let found = store
            .find(identity.id)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(found.state, ActorState::Active);
    }

    let rejected =
        sqlx::query("INSERT INTO actor_identity (id, kind, state) VALUES ($1, 'user', 'deleted')")
            .bind(uuid::Uuid::now_v7())
            .execute(&pool)
            .await;
    assert!(
        rejected.is_err(),
        "the state CHECK must reject a spelling outside the closed vocabulary"
    );
}

/// Slice 5: the handle cache is born empty, refreshable, clearable — and
/// caching for a never-seen actor is a loud error, not a silent no-op.
#[tokio::test]
async fn handle_cache_fills_refreshes_and_clears() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());
    let store = PgActorIdentityStore::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    let interned = uow
        .actor_identities()
        .intern(&Did::new("did:plc:cache-me".to_string()), ActorKind::User)
        .await
        .expect("intern");
    uow.commit().await.expect("commit");
    assert_eq!(interned.handle, None, "born uncached");

    for (set_to, expect) in [
        (Some("alice.bsky.social"), Some("alice.bsky.social")),
        (Some("alice.zurfur.app"), Some("alice.zurfur.app")),
        (None, None),
    ] {
        let mut uow = db.begin().await.expect("begin");
        uow.actor_identities()
            .cache_handle(interned.id, set_to)
            .await
            .expect("cache_handle");
        uow.commit().await.expect("commit");

        let found = store
            .find(interned.id)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(found.handle.as_deref(), expect);
    }

    let mut uow = db.begin().await.expect("begin");
    let missing = uow
        .actor_identities()
        .cache_handle(
            domain::elements::actor_identity::ActorIdentity::mint(ActorKind::User).id,
            Some("ghost.example.com"),
        )
        .await;
    assert!(
        missing.is_err(),
        "caching for a never-seen actor must error"
    );
}

/// The primary key holds: creating the same id twice across two units errors.
#[tokio::test]
async fn duplicate_create_errors() {
    let (pool, _container) = fresh_pool().await;
    let identity = ActorIdentity::mint(ActorKind::User);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.actor_identities()
        .create(&identity)
        .await
        .expect("first create");
    uow.commit().await.expect("commit");

    let mut uow = db.begin().await.expect("begin");
    let second = uow.actor_identities().create(&identity).await;
    assert!(second.is_err(), "duplicate create must violate the PK");
}
