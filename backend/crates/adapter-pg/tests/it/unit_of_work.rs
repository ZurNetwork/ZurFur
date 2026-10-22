//! The PostgreSQL unit of work's commit, against a throwaway container: a
//! statement failure that aborted the transaction makes `commit` fail, even
//! when the caller swallowed the statement's error. Requires a container
//! runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{CommissionBirth, CommissionId, NewCommission},
        did::Did,
        level::Level,
        text::StoredText,
        user::{User, UserId},
    },
    ports::{CommissionStore, Database},
};
use test_support::seeder::commission_node;

use crate::seed;

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
    let id = seed::seed_titled(&pool, &owner.id, "Before").await;
    let db = PgDatabase::new(pool.clone());

    let renamed_title = StoredText::from("After".to_owned());
    let stranger = UserId::from(Did::from("did:plc:never-provisioned".to_string()));
    let orphan_id = CommissionId::mint();
    let orphan_node = commission_node(orphan_id, Level::Private, "Orphan");
    let orphan_birth = CommissionBirth {
        node: orphan_node.id,
        owner_id: stranger,
        created_at: Utc::now(),
    };
    let unrated = NewCommission {
        maturity: None,
        deadline: None,
    };

    let mut uow = db.begin().await.expect("begin");
    uow.nodes()
        .create(&orphan_node)
        .await
        .expect("store the orphan's node");
    let locked = uow
        .commissions()
        .standing_for_update(&id, &owner.id)
        .await
        .expect("the locked read")
        .expect("the seeded commission stands");
    let renamed = uow
        .nodes()
        .set_name(&locked.node, &renamed_title)
        .await
        .expect("the rename itself succeeds");
    assert!(
        renamed,
        "the rename found the commission's node in the unit"
    );
    let orphan_created = seed::create(&mut *uow, orphan_id, &orphan_birth, &unrated).await;
    assert!(
        orphan_created.is_err(),
        "the owner's foreign key refuses the orphan"
    );
    let committed = uow.commit().await;
    assert!(
        committed.is_err(),
        "a unit whose transaction PostgreSQL aborted must not commit as success"
    );

    let stored = PgCommissionStore::new(pool.clone())
        .find_summary(&id)
        .await
        .expect("read back")
        .expect("the seeded commission exists");
    assert_eq!(
        stored.title.as_ref(),
        "Before",
        "the rename in the refused unit was not kept"
    );
}
