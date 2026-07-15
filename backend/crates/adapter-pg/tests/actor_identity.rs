//! The actor super-table over PostgreSQL (ZMVP-122 slice 1, DD 34013187),
//! against a throwaway container: a minted identity created through the Unit of
//! Work round-trips (id + kind), an uncommitted create rolls back, the primary
//! key rejects a duplicate id, and the kind CHECK rejects a spelling outside
//! the closed vocabulary. did/handle/state land in later slices with their own
//! tests. Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgActorIdentityStore, PgDatabase, PgPool};
use domain::{
    elements::actor_identity::{ActorIdentity, ActorKind},
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

    let rejected = sqlx::query("INSERT INTO actor_identity (id, kind) VALUES ($1, 'golem')")
        .bind(uuid::Uuid::now_v7())
        .execute(&pool)
        .await;
    assert!(
        rejected.is_err(),
        "the kind CHECK must reject a spelling outside the closed vocabulary"
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
