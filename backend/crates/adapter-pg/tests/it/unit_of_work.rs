//! The PostgreSQL unit of work's commit, against a throwaway container: a
//! statement failure that aborted the transaction makes `commit` fail, even
//! when the caller swallowed the statement's error. Requires a container
//! runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{Commission, CommissionTitle},
        did::Did,
        user::{User, UserId},
    },
    ports::{CommissionStore, Database},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work.
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::from(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// A rename lands, then a create for an owner nobody provisioned fails and its
/// error is dropped. The commit must refuse, and the rename must not be stored.
#[tokio::test]
async fn a_swallowed_statement_failure_fails_the_commit_and_keeps_nothing() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:swallow-owner").await;
    let original_title = "Before".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(original_title, owner.id, Utc::now(), None);
    let id = commission.id;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin seeding unit");
    uow.commissions().create(&commission).await.expect("create");
    uow.commit().await.expect("commit seeding unit");

    let renamed_title = "After".parse::<CommissionTitle>().expect("valid title");
    let stranger = UserId::from(Did::from("did:plc:never-provisioned".to_string()));
    let orphan_title = "Orphan".parse::<CommissionTitle>().expect("valid title");
    let orphan = Commission::create(orphan_title, stranger, Utc::now(), None);

    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        let changed = commissions
            .set_title(&id, &renamed_title)
            .await
            .expect("the rename itself succeeds");
        assert!(changed, "the rename changed the stored title in the unit");
        let orphan_created = commissions.create(&orphan).await;
        assert!(
            orphan_created.is_err(),
            "the owner's foreign key refuses the orphan"
        );
    }
    let committed = uow.commit().await;
    assert!(
        committed.is_err(),
        "a unit whose transaction PostgreSQL aborted must not commit as success"
    );

    let stored = PgCommissionStore::new(pool.clone())
        .find(&id)
        .await
        .expect("read back")
        .expect("the seeded commission exists");
    assert_eq!(
        stored.title.as_ref(),
        "Before",
        "the rename in the refused unit was not kept"
    );
}
