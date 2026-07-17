//! The commission changelog over PostgreSQL (ZMVP-87), against a throwaway
//! container: append is a [`UnitOfWork`] view (entries commit **atomically with
//! domain writes** — Changelog DD `30408741` D4, never a dual write), the ordered
//! read is the pool-backed [`ChangelogStore`], the table is append-only at the
//! database (a `BEFORE UPDATE` trigger refuses edits; `DELETE` stays ungoverned so
//! the commission hard-delete cascade works), and the owner-arm
//! [`CommissionStore::is_participant`] predicate is born here. Requires a
//! container runtime socket (`DOCKER_HOST` honored).

use adapter_pg::{PgChangelogStore, PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            ChangelogEntryKind, ChannelPointer, Commission, CommissionId, CommissionTitle,
            NewChangelogEntry,
        },
        did::Did,
        user::User,
    },
    ports::{ChangelogStore, CommissionStore, Database},
};
use serde_json::json;

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work (`commission.owner_id`
/// references `users(id)`).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create and commit a commission owned by `owner_did`, returning it.
async fn seed_commission(pool: &PgPool, owner_did: &str) -> Commission {
    let owner = provision(pool, owner_did).await;
    let title = "A ref sheet"
        .parse::<CommissionTitle>()
        .expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .create(&commission)
        .await
        .expect("create commission");
    uow.commit().await.expect("commit");
    commission
}

// DD D4 — an appended entry commits atomically with the unit of work: visible
// after commit, and a rolled-back unit leaves no entry behind.
#[tokio::test]
async fn append_commits_and_rolls_back_with_the_unit() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:log-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgChangelogStore::new(pool.clone());

    // Committed append is visible.
    let mut uow = db.begin().await.expect("begin");
    uow.changelog()
        .append(&NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Created,
            commission.owner_id,
            json!({ "title": "A ref sheet" }),
            Utc::now(),
        ))
        .await
        .expect("append");
    uow.commit().await.expect("commit");
    assert_eq!(
        store.entries(commission.id).await.expect("read").len(),
        1,
        "the committed entry is visible",
    );

    // A rolled-back append never lands.
    let mut uow = db.begin().await.expect("begin");
    uow.changelog()
        .append(&NewChangelogEntry::note(
            commission.id,
            commission.owner_id,
            "never happened".to_string(),
            Utc::now(),
        ))
        .await
        .expect("append (to be rolled back)");
    uow.rollback().await.expect("rollback");
    assert_eq!(
        store.entries(commission.id).await.expect("read").len(),
        1,
        "a rolled-back entry is invisible — atomic with the unit, no dual write",
    );
}

// AC5 — the read is ordered by seq ascending and round-trips every envelope
// field: kind token, actor (including the NULL system arm), payload, note,
// created_at.
#[tokio::test]
async fn entries_read_back_in_seq_order_with_the_full_envelope() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:ordered-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgChangelogStore::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    {
        let mut log = uow.changelog();
        log.append(&NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Created,
            commission.owner_id,
            json!({ "title": "A ref sheet" }),
            Utc::now(),
        ))
        .await
        .expect("append created");
        log.append(&NewChangelogEntry::note(
            commission.id,
            commission.owner_id,
            "traveling next week".to_string(),
            Utc::now(),
        ))
        .await
        .expect("append note");
        // A system entry (the shape ZMVP-86's Late emitter will use): no actor.
        log.append(&NewChangelogEntry::system(
            commission.id,
            ChangelogEntryKind::Late,
            json!({ "deadline": "2026-07-01T00:00:00Z" }),
            Utc::now(),
        ))
        .await
        .expect("append system entry");
    }
    uow.commit().await.expect("commit");

    let entries = store.entries(commission.id).await.expect("read");
    assert_eq!(entries.len(), 3);
    assert!(
        entries.windows(2).all(|w| w[0].seq < w[1].seq),
        "seq strictly increases in read order",
    );
    assert!(matches!(entries[0].kind, ChangelogEntryKind::Created));
    assert_eq!(entries[0].actor_id, Some(commission.owner_id));
    assert_eq!(entries[0].payload["title"], "A ref sheet");
    assert!(matches!(entries[1].kind, ChangelogEntryKind::Note));
    assert_eq!(entries[1].note.as_deref(), Some("traveling next week"));
    assert!(matches!(entries[2].kind, ChangelogEntryKind::Late));
    assert_eq!(entries[2].actor_id, None, "a system entry carries no actor");

    // Another commission's stream stays separate.
    let other = seed_commission(&pool, "did:plc:other-owner").await;
    assert!(
        store
            .entries(other.id)
            .await
            .expect("read other")
            .is_empty(),
        "streams are per-commission",
    );
}

// AC4 at the database — an UPDATE against commission_changelog is refused by
// the append-only trigger, no matter who issues it.
#[tokio::test]
async fn updates_are_refused_at_the_database() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:immutable-owner").await;
    let db = PgDatabase::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    uow.changelog()
        .append(&NewChangelogEntry::note(
            commission.id,
            commission.owner_id,
            "the record".to_string(),
            Utc::now(),
        ))
        .await
        .expect("append");
    uow.commit().await.expect("commit");

    let refused = sqlx::query("UPDATE commission_changelog SET note = 'rewritten'")
        .execute(&pool)
        .await;
    let err = refused.expect_err("the append-only trigger refuses UPDATE");
    assert!(
        err.to_string().contains("append-only"),
        "the refusal names the invariant, got: {err}",
    );
}

// The E35 retention convention — changelog rows are commission-owned bookkeeping:
// hard-deleting the commission cascades them away (DD retention: hard-delete only
// with commission hard-delete), which is exactly why DELETE stays ungoverned at
// the database while UPDATE is refused.
#[tokio::test]
async fn entries_cascade_away_with_the_commission() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:gone-owner").await;
    let db = PgDatabase::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    uow.changelog()
        .append(&NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Created,
            commission.owner_id,
            json!({}),
            Utc::now(),
        ))
        .await
        .expect("append");
    uow.commit().await.expect("commit");

    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(*commission.id)
        .execute(&pool)
        .await
        .expect("hard-delete the commission");

    let store = PgChangelogStore::new(pool.clone());
    assert!(
        store.entries(commission.id).await.expect("read").is_empty(),
        "the commission's entries cascade away with it",
    );
}

// The owner-arm participant predicate born here (ZMVP-79 adds the seated arm):
// the owner IS a Participant without holding a Seat; everyone else — and every
// unknown commission — answers false.
#[tokio::test]
async fn is_participant_answers_the_owner_arm_only() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:participant-owner").await;
    let stranger = provision(&pool, "did:plc:stranger").await;
    let store = PgCommissionStore::new(pool.clone());

    assert!(
        store
            .is_participant(commission.id, commission.owner_id)
            .await
            .expect("ask owner"),
        "the owner is a Participant without a Seat",
    );
    assert!(
        !store
            .is_participant(commission.id, stranger.id)
            .await
            .expect("ask stranger"),
        "a non-owner is not (yet) a Participant",
    );
    assert!(
        !store
            .is_participant(CommissionId::new(uuid::Uuid::now_v7()), commission.owner_id)
            .await
            .expect("ask unknown commission"),
        "an unknown commission has no participants",
    );
}

// CommissionStore::find round-trips the aggregate — including the nullable
// linked-channel pointer set and cleared through the write view.
#[tokio::test]
async fn find_roundtrips_the_linked_channel() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:channel-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    let found = store
        .find(commission.id)
        .await
        .expect("find")
        .expect("the commission exists");
    assert_eq!(found.title.as_str(), "A ref sheet");
    assert_eq!(found.owner_id, commission.owner_id);
    assert!(found.linked_channel.is_none(), "born with no channel");

    let pointer = "https://t.me/refsheet-chat"
        .parse::<ChannelPointer>()
        .expect("valid pointer");
    let mut uow = db.begin().await.expect("begin");
    assert!(
        uow.commissions()
            .set_linked_channel(commission.id, Some(&pointer))
            .await
            .expect("set channel"),
        "the first link is a real change"
    );
    assert!(
        !uow.commissions()
            .set_linked_channel(commission.id, Some(&pointer))
            .await
            .expect("re-set channel"),
        "re-linking the identical pointer answers false"
    );
    uow.commit().await.expect("commit");
    let found = store
        .find(commission.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(
        found.linked_channel.as_ref().map(|c| c.as_str()),
        Some("https://t.me/refsheet-chat"),
    );

    let mut uow = db.begin().await.expect("begin");
    assert!(
        uow.commissions()
            .set_linked_channel(commission.id, None)
            .await
            .expect("clear channel"),
        "the clear is a real change"
    );
    assert!(
        !uow.commissions()
            .set_linked_channel(commission.id, None)
            .await
            .expect("re-clear channel"),
        "clearing an already-clear channel answers false"
    );
    uow.commit().await.expect("commit");
    let found = store
        .find(commission.id)
        .await
        .expect("find")
        .expect("exists");
    assert!(found.linked_channel.is_none(), "the pointer clears to NULL");

    // An unknown commission finds nothing.
    assert!(
        store
            .find(CommissionId::new(uuid::Uuid::now_v7()))
            .await
            .expect("find unknown")
            .is_none(),
    );
}

// ZMVP-85 — CommissionStore::find round-trips the nullable direction-axis
// Status set, replaced, and cleared through the write view: one column, so a
// set REPLACES by construction (ruling E29) and NULL is the cleared state.
#[tokio::test]
async fn find_roundtrips_the_direction_status() {
    use domain::elements::commission::DirectionStatus;

    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:status-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    let found = store
        .find(commission.id)
        .await
        .expect("find")
        .expect("the commission exists");
    assert!(
        found.direction_status.is_none(),
        "born with no direction status"
    );

    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_direction_status(commission.id, Some(DirectionStatus::WaitingForInput))
        .await
        .expect("set status");
    uow.commit().await.expect("commit");
    assert_eq!(
        store
            .find(commission.id)
            .await
            .expect("find")
            .expect("exists")
            .direction_status,
        Some(DirectionStatus::WaitingForInput),
    );

    // A second set replaces the value whole — never accumulates.
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_direction_status(commission.id, Some(DirectionStatus::ChangesRequested))
        .await
        .expect("replace status");
    uow.commit().await.expect("commit");
    assert_eq!(
        store
            .find(commission.id)
            .await
            .expect("find")
            .expect("exists")
            .direction_status,
        Some(DirectionStatus::ChangesRequested),
    );

    // Clearing writes NULL; the deadline envelope is a separate axis and is
    // untouched throughout (it was None at seed and stays None).
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_direction_status(commission.id, None)
        .await
        .expect("clear status");
    uow.commit().await.expect("commit");
    let found = store
        .find(commission.id)
        .await
        .expect("find")
        .expect("exists");
    assert!(found.direction_status.is_none(), "cleared to NULL");

    // An absent commission is a no-op write, not an error.
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_direction_status(
            CommissionId::new(uuid::Uuid::now_v7()),
            Some(DirectionStatus::WaitingForApproval),
        )
        .await
        .expect("no-op on an unknown commission");
    uow.commit().await.expect("commit");
}
