//! Actor tables as shared-PK projections of `actor_identity` (ZMVP-123, DD
//! `34013187` decisions 1-2), against a throwaway container: the migration
//! backfills one identity row per pre-existing users/accounts row and stops
//! loudly on an ambiguous DID; the composite FK makes a wrong-kind or orphaned
//! projection row unrepresentable; and actor creation is a two-step write
//! (intern + projection) that commits — or rolls back — as one unit. Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgAccountStore, PgDatabase, PgPool, PgUserStore};
use chrono::{DateTime, Utc};
use domain::{
    elements::{
        account::AccountId,
        account::{Account, AccountName},
        did::Did,
        handle::Handle,
    },
    ports::{AccountStore, Database, DidBelongsToAnotherActor, HandleTaken, UserStore},
};

/// The backfill migration (slice 1: backfill + dedupe assertion), as sqlx numbers
/// it. The backfill tests run everything *before* this version, seed old-shape
/// rows, then let the full migrator catch up (the ZMVP-71/76 backfill pattern).
const BACKFILL_MIGRATION: i64 = 20260718193956;

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container alive.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// A fresh, empty private database with NO migrations applied (the backfill tests
/// drive the migrator themselves).
async fn bare_pool() -> (PgPool, impl Sized) {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    (pool, db)
}

/// Run every embedded migration strictly BEFORE `version` — the pre-backfill
/// world, where users/accounts still carry their own `did` columns.
async fn run_migrations_before(pool: &PgPool, version: i64) {
    let mut migrator = adapter_pg::migrator();
    let subset: Vec<_> = migrator
        .migrations
        .iter()
        .filter(|migration| migration.version < version)
        .cloned()
        .collect();
    assert!(
        !subset.is_empty() && subset.len() < migrator.migrations.len(),
        "the version constant matches an embedded migration"
    );
    migrator.migrations = subset.into();
    migrator
        .run(pool)
        .await
        .expect("pre-backfill migrations run");
}

/// Does `table` have `column`? Reads the live catalog so the schema shape itself
/// is under test (the DID moved, the handle stayed).
async fn has_column(pool: &PgPool, table: &str, column: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
         WHERE table_name = $1 AND column_name = $2)",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await
    .expect("column existence query")
}

// Slice 1 — the migration backfills one identity row per pre-existing users/accounts
// row: id carried across (shared PK), DID moved into the band, state born 'active',
// first_seen seeded from the row's own creation instant, handle cache left NULL.
#[tokio::test]
async fn migration_backfills_one_identity_per_projection_row() {
    let (pool, _container) = bare_pool().await;
    run_migrations_before(&pool, BACKFILL_MIGRATION).await;

    // Whole-second timestamps so the round-trip through Postgres (microsecond) is exact.
    let user_id = uuid::Uuid::now_v7();
    let user_did = "did:plc:early-user";
    let user_created = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
    sqlx::query("INSERT INTO users (id, did, created_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(user_did)
        .bind(user_created)
        .execute(&pool)
        .await
        .expect("seed pre-backfill user");

    let account_id = uuid::Uuid::now_v7();
    let account_did = "did:plc:early-account";
    let account_created = DateTime::from_timestamp(1_700_000_500, 0).unwrap();
    sqlx::query(
        "INSERT INTO accounts (id, did, handle, name, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $5)",
    )
    .bind(account_id)
    .bind(account_did)
    .bind("early.zurfur.app")
    .bind("Early Studio")
    .bind(account_created)
    .execute(&pool)
    .await
    .expect("seed pre-backfill account");

    // Catch up: backfill + composite FK + drop the projection did columns.
    adapter_pg::migrate(&pool).await.expect("catch-up migrates");

    let (kind, did, state, handle, first_seen): (
        String,
        String,
        String,
        Option<String>,
        DateTime<Utc>,
    ) = sqlx::query_as(
        "SELECT kind, did, state, handle, first_seen FROM actor_identity WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("the user's identity row was backfilled");
    assert_eq!(kind, "user");
    assert_eq!(did, user_did);
    assert_eq!(state, "active", "backfilled rows are born active");
    assert_eq!(
        handle, None,
        "the display-handle cache is born NULL, not seeded"
    );
    assert_eq!(
        first_seen, user_created,
        "first_seen is the row's own created_at"
    );

    let (kind, did, first_seen): (String, String, DateTime<Utc>) =
        sqlx::query_as("SELECT kind, did, first_seen FROM actor_identity WHERE id = $1")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("the account's identity row was backfilled");
    assert_eq!(kind, "account");
    assert_eq!(did, account_did);
    assert_eq!(first_seen, account_created);

    // The DID left the projections (single home = actor_identity); the account handle
    // stayed (the authoritative, globally-unique resolution claim, not the cache).
    assert!(
        !has_column(&pool, "users", "did").await,
        "users.did was dropped"
    );
    assert!(
        !has_column(&pool, "accounts", "did").await,
        "accounts.did was dropped"
    );
    assert!(
        has_column(&pool, "accounts", "handle").await,
        "accounts.handle stays — it is authoritative, not the actor_identity cache"
    );

    // The reads join the DID back through the super-table: the backfilled account
    // resolves by id and by handle, DID intact.
    let store = PgAccountStore::new(pool.clone());
    let found = store
        .find(AccountId::new(account_id))
        .await
        .expect("find")
        .expect("the backfilled account is readable");
    assert_eq!(
        found.did.as_str(),
        account_did,
        "the DID joins back from the band"
    );
    let resolved = store
        .find_did_by_handle(&Handle::try_new("early.zurfur.app").unwrap())
        .await
        .expect("resolve");
    assert_eq!(
        resolved.map(|d| d.as_str().to_string()),
        Some(account_did.to_string())
    );
}

// Slice 1 — the dedupe assertion: a DID present in BOTH a users row and an accounts
// row would map to two identity rows, which the UNIQUE `did` cannot hold. The
// migration must FAIL LOUDLY rather than silently merge or drop an identity.
#[tokio::test]
async fn backfill_aborts_loudly_when_a_did_is_shared_across_tables() {
    let (pool, _container) = bare_pool().await;
    run_migrations_before(&pool, BACKFILL_MIGRATION).await;

    let shared_did = "did:plc:double-interned";
    sqlx::query("INSERT INTO users (id, did, created_at) VALUES ($1, $2, now())")
        .bind(uuid::Uuid::now_v7())
        .bind(shared_did)
        .execute(&pool)
        .await
        .expect("seed user");
    sqlx::query(
        "INSERT INTO accounts (id, did, handle, name, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, now(), now())",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(shared_did)
    .bind("double.zurfur.app")
    .bind("Double")
    .execute(&pool)
    .await
    .expect("seed account sharing the did");

    let result = adapter_pg::migrate(&pool).await;
    let err = result.expect_err("a DID shared across users+accounts must abort the backfill");
    let message = format!("{err:#}");
    assert!(
        message.contains("shared between users and accounts"),
        "the abort carries the shared-DID diagnostic, got: {message}"
    );
}

// Slice 2 — the composite FK makes a wrong-kind or orphaned projection row
// unrepresentable, not merely checked: a users row whose id points at an ACCOUNT's
// identity (kind mismatch), or at no identity at all, is rejected by the database.
#[tokio::test]
async fn composite_fk_rejects_wrong_kind_and_orphan_projection_rows() {
    let (pool, _container) = fresh_pool().await;

    // An account-kind identity, inserted directly.
    let account_identity = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO actor_identity (id, kind, did, state, first_seen) \
         VALUES ($1, 'account', $2, 'active', now())",
    )
    .bind(account_identity)
    .bind("did:plc:acct-only")
    .execute(&pool)
    .await
    .expect("seed an account identity");

    // A users row keyed at that id is (id, 'user') — absent from actor_identity, which
    // holds (id, 'account'). The composite FK rejects it.
    let wrong_kind = sqlx::query("INSERT INTO users (id, created_at) VALUES ($1, now())")
        .bind(account_identity)
        .execute(&pool)
        .await;
    assert!(
        wrong_kind.is_err(),
        "a users row keyed at an ACCOUNT identity must be rejected (kind mismatch)"
    );

    // A users row with no identity parent at all is likewise rejected.
    let orphan = sqlx::query("INSERT INTO users (id, created_at) VALUES ($1, now())")
        .bind(uuid::Uuid::now_v7())
        .execute(&pool)
        .await;
    assert!(
        orphan.is_err(),
        "a users row with no actor_identity parent must be rejected"
    );
}

// The per-kind DID guarantee: a user/account identity with a NULL DID is rejected by
// the CHECK (their dropped `did NOT NULL` columns became this invariant), while a
// DID-less character identity is allowed (actor-ness ≠ DID-ness, DD amendment).
#[tokio::test]
async fn actor_identity_requires_a_did_for_user_and_account_kinds() {
    let (pool, _container) = fresh_pool().await;

    for kind in ["user", "account"] {
        let null_did = sqlx::query(
            "INSERT INTO actor_identity (id, kind, state, first_seen) VALUES ($1, $2, 'active', now())",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(kind)
        .execute(&pool)
        .await;
        assert!(
            null_did.is_err(),
            "a {kind} identity with a NULL DID must be rejected by the per-kind CHECK"
        );
    }

    // A DID-less character identity is allowed.
    let character = sqlx::query(
        "INSERT INTO actor_identity (id, kind, state, first_seen) VALUES ($1, 'character', 'active', now())",
    )
    .bind(uuid::Uuid::now_v7())
    .execute(&pool)
    .await;
    assert!(
        character.is_ok(),
        "a DID-less character identity is allowed"
    );
}

// Slice 3 — the two-step create commits as one unit: provisioning a visitor lands
// BOTH the users projection and its actor_identity parent, and is idempotent by DID.
#[tokio::test]
async fn provision_commits_both_rows_and_is_idempotent() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());
    let did = Did::new("did:plc:provision-me".to_string());

    let mut uow = db.begin().await.unwrap();
    let first = uow.users().provision(&did).await.expect("provision");
    uow.commit().await.unwrap();

    // The identity parent exists and shares the user's id.
    let identity_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM actor_identity WHERE did = $1 AND kind = 'user'")
            .bind(did.as_str())
            .fetch_one(&pool)
            .await
            .expect("the user's identity was interned");
    assert_eq!(
        identity_id, *first.id,
        "the users row shares its identity's id"
    );

    // The read joins the DID back.
    let found = PgUserStore::new(pool.clone())
        .find(first.id)
        .await
        .expect("find")
        .expect("the provisioned user is readable");
    assert_eq!(found.did, did);

    // Idempotent: a repeat sign-in returns the same User and mints no second identity.
    let mut uow = db.begin().await.unwrap();
    let second = uow.users().provision(&did).await.expect("re-provision");
    uow.commit().await.unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(second.created_at, first.created_at);
    let identity_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM actor_identity WHERE did = $1")
            .bind(did.as_str())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(identity_count, 1, "re-provision interns no second identity");
}

// Slice 3 — atomicity: dropping the unit before commit discards BOTH rows of the
// two-step create (the projection AND its interned identity), the mem/pg rollback
// contract applied to the shared-PK write.
#[tokio::test]
async fn dropping_the_unit_discards_both_the_user_and_its_identity() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());
    let did = Did::new("did:plc:rolled-back".to_string());

    {
        let mut uow = db.begin().await.unwrap();
        uow.users().provision(&did).await.expect("provision");
        // Dropped without commit → rollback.
    }

    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 0, "no users row survives a dropped unit");
    let identities: i64 = sqlx::query_scalar("SELECT count(*) FROM actor_identity WHERE did = $1")
        .bind(did.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        identities, 0,
        "no interned identity survives either — both rows roll back together"
    );
}

// Slice 3 — the account create is atomic with its identity too: a dropped unit leaves
// neither the accounts projection nor its interned identity, even though the owner
// (committed earlier) keeps its own user + identity.
#[tokio::test]
async fn account_create_is_atomic_with_its_identity() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());

    // The owner exists (committed) — the account membership FKs into users(id).
    let mut uow = db.begin().await.unwrap();
    let owner = uow
        .users()
        .provision(&Did::new("did:plc:acct-owner".to_string()))
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let account_did = "did:plc:atomic-account";
    let (account, membership) = Account::open(
        owner.id,
        Did::new(account_did.to_string()),
        Handle::try_new("atomic.zurfur.app").unwrap(),
        "Atomic Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );

    {
        let mut uow = db.begin().await.unwrap();
        uow.accounts()
            .create(&account, &membership)
            .await
            .expect("create");
        // Dropped without commit → rollback.
    }

    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(accounts, 0, "no accounts row survives a dropped unit");
    let account_identities: i64 =
        sqlx::query_scalar("SELECT count(*) FROM actor_identity WHERE did = $1")
            .bind(account_did)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        account_identities, 0,
        "the account's interned identity rolled back too"
    );
    // The owner's own identity is untouched (it committed earlier).
    let owner_identities: i64 =
        sqlx::query_scalar("SELECT count(*) FROM actor_identity WHERE did = 'did:plc:acct-owner'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        owner_identities, 1,
        "the owner's committed identity is untouched"
    );
}

// Security-review nit N1 — the HandleTaken collision path, made self-evident:
// founding B on A's claimed handle fails, and B's freshly interned identity
// rolls back with the rejected projection — no orphan identity survives a
// handle collision (single txn; the state the review proved by construction).
#[tokio::test]
async fn handle_collision_discards_the_interned_identity() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());

    let mut uow = db.begin().await.unwrap();
    let owner = uow
        .users()
        .provision(&Did::new("did:plc:collide-owner".to_string()))
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let contested = Handle::try_new("contested.zurfur.app").unwrap();
    let (first, first_membership) = Account::open(
        owner.id,
        Did::new("did:plc:first-claimant".to_string()),
        contested.clone(),
        "First Claimant".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let mut uow = db.begin().await.unwrap();
    uow.accounts()
        .create(&first, &first_membership)
        .await
        .expect("the first claim succeeds");
    uow.commit().await.unwrap();

    let loser_did = "did:plc:second-claimant";
    let (second, second_membership) = Account::open(
        owner.id,
        Did::new(loser_did.to_string()),
        contested,
        "Second Claimant".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    {
        let mut uow = db.begin().await.unwrap();
        let handle_taken = uow
            .accounts()
            .create(&second, &second_membership)
            .await
            .expect_err("the second claim on the handle is rejected");
        assert!(
            handle_taken.downcast_ref::<HandleTaken>().is_some(),
            "the rejection is the HandleTaken contract error"
        );
        // Dropped (aborted) unit — rollback is the only legal continuation.
    }

    let loser_identities: i64 =
        sqlx::query_scalar("SELECT count(*) FROM actor_identity WHERE did = $1")
            .bind(loser_did)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        loser_identities, 0,
        "the loser's interned identity rolled back with its rejected projection"
    );
}

// Security-review nit N2 — the concurrent-provision race, pinned directly:
// two simultaneous first sign-ins with one DID converge on a single identity
// and a single users row — the loser's projection binds to the WINNER's
// identity id (intern's ON CONFLICT … RETURNING id is the arbiter).
#[tokio::test]
async fn concurrent_provisions_of_one_did_converge_on_a_single_identity() {
    let (pool, _container) = fresh_pool().await;
    let did = Did::new("did:plc:raced-signin".to_string());

    let provision = |db: PgDatabase| {
        let did = did.clone();
        async move {
            let mut uow = db.begin().await.unwrap();
            let user = uow.users().provision(&did).await.expect("provision");
            uow.commit().await.unwrap();
            user
        }
    };
    let (first, second) = tokio::join!(
        provision(PgDatabase::new(pool.clone())),
        provision(PgDatabase::new(pool.clone()))
    );

    assert_eq!(first.id, second.id, "both racers resolve to one User");
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let identities: i64 = sqlx::query_scalar("SELECT count(*) FROM actor_identity WHERE did = $1")
        .bind(did.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    let one_of_each = (users, identities);
    assert_eq!(
        one_of_each,
        (1, 1),
        "one users row, one identity — the race has a single winner"
    );
}

// Ultrareview round (2026-07-18) — the cross-kind conflict is TYPED at the
// store: provisioning a DID that already belongs to an account fails with the
// downcastable DidBelongsToAnotherActor marker (routes map it to a 409),
// never an opaque error and never a silent reuse of the other actor's id.
#[tokio::test]
async fn provisioning_an_accounts_did_is_a_typed_conflict() {
    let (pool, _container) = fresh_pool().await;
    let db = PgDatabase::new(pool.clone());

    let mut uow = db.begin().await.unwrap();
    let owner = uow
        .users()
        .provision(&Did::new("did:plc:typed-conflict-owner".to_string()))
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let account_did = "did:plc:typed-conflict-account";
    let (account, membership) = Account::open(
        owner.id,
        Did::new(account_did.to_string()),
        Handle::try_new("typedconflict.zurfur.app").unwrap(),
        "Typed Conflict".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    let mut uow = db.begin().await.unwrap();
    uow.accounts().create(&account, &membership).await.unwrap();
    uow.commit().await.unwrap();

    let mut uow = db.begin().await.unwrap();
    let conflict = uow
        .users()
        .provision(&Did::new(account_did.to_string()))
        .await
        .expect_err("an account's DID cannot become a User");
    assert!(
        conflict
            .downcast_ref::<DidBelongsToAnotherActor>()
            .is_some(),
        "the conflict is the typed marker, not an opaque error"
    );
}
