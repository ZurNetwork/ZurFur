//! The commission changelog over PostgreSQL, against a throwaway
//! container: append is a [`UnitOfWork`] view (entries commit **atomically with
//! domain writes**, never a dual write), the ordered
//! read is the pool-backed [`ChangelogStore`], the table is append-only at the
//! database (a `BEFORE UPDATE` trigger refuses edits; `DELETE` stays ungoverned so
//! the commission hard-delete cascade works), and the unit's owner-arm
//! [`CommissionReads::is_participant`] predicate is born here. Requires a
//! container runtime socket (`DOCKER_HOST` honored).

use crate::seed;
use adapter_pg::{PgChangelogStore, PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            ChangelogEntryKind, ChannelPointer, Commission, CommissionId, CommissionState,
            DeadlineStatus, DirectionChange, DirectionStatus, LateFacts, LifecycleStep,
            NewChangelogEntry, clear_deadline, clear_delayed, clear_direction, move_lifecycle,
            owes_late, rate_maturity, schedule_deadline, set_delayed, set_direction,
        },
        did::Did,
        maturity::{Maturity, MaturityRating},
        user::User,
    },
    ports::{ChangelogStore, CommissionRows, CommissionStore, Database},
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
        .provision(&Did::from(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create and commit a commission owned by `owner_did`, returning it.
async fn seed_commission(pool: &PgPool, owner_did: &str) -> Commission {
    let owner = provision(pool, owner_did).await;
    seed::seed_stored(pool, &owner.id, "A ref sheet").await
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
        .append(&NewChangelogEntry::created(
            commission.id,
            commission.owner_id.clone(),
            Utc::now(),
        ))
        .await
        .expect("append");
    uow.commit().await.expect("commit");
    assert_eq!(
        store.entries(&commission.id).await.expect("read").len(),
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
        store.entries(&commission.id).await.expect("read").len(),
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
    let mut commission = seed_commission(&pool, "did:plc:ordered-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgChangelogStore::new(pool.clone());

    let mut uow = db.begin().await.expect("begin");
    {
        let mut log = uow.changelog();
        log.append(&NewChangelogEntry::created(
            commission.id,
            commission.owner_id.clone(),
            Utc::now(),
        ))
        .await
        .expect("append created");
        log.append(&NewChangelogEntry::note(
            commission.id,
            commission.owner_id.clone(),
            "traveling next week".to_string(),
            Utc::now(),
        ))
        .await
        .expect("append note");
        // The system's Late mark: no actor. The deadline is set on the loaded value only,
        // for the Late rule to read.
        commission.deadline = Some("2026-07-01T00:00:00Z".parse().expect("an RFC 3339 instant"));
        let late_facts = LateFacts {
            state: CommissionState::from(&commission),
            owed: true,
        };
        let mark = owes_late(&late_facts, Utc::now()).expect("a late entry is owed");
        log.append(&NewChangelogEntry::late(commission.id, &mark, Utc::now()))
            .await
            .expect("append system entry");
    }
    uow.commit().await.expect("commit");

    let entries = store.entries(&commission.id).await.expect("read");
    assert_eq!(entries.len(), 3);
    assert!(
        entries.windows(2).all(|w| w[0].seq < w[1].seq),
        "seq strictly increases in read order",
    );
    assert!(matches!(entries[0].kind, ChangelogEntryKind::Created));
    assert_eq!(entries[0].actor_id, Some(commission.owner_id));
    assert_eq!(entries[0].payload, json!({}));
    assert!(matches!(entries[1].kind, ChangelogEntryKind::Note));
    assert_eq!(entries[1].note.as_deref(), Some("traveling next week"));
    assert!(matches!(entries[2].kind, ChangelogEntryKind::Late));
    assert_eq!(entries[2].actor_id, None, "a system entry carries no actor");

    // Another commission's stream stays separate.
    let other = seed_commission(&pool, "did:plc:other-owner").await;
    assert!(
        store
            .entries(&other.id)
            .await
            .expect("read other")
            .is_empty(),
        "streams are per-commission",
    );
}

/// Every gated entry built from a change its rule passed on `commission`, by its owner, beside
/// the literal JSON it stores.
fn typed_entries(commission: &mut Commission) -> Vec<(NewChangelogEntry, serde_json::Value)> {
    let at = Utc::now();
    let march: chrono::DateTime<Utc> = "2026-03-01T12:00:00Z".parse().expect("an RFC 3339 instant");
    let april: chrono::DateTime<Utc> = "2026-04-01T12:00:00Z".parse().expect("an RFC 3339 instant");
    let owner = commission.owner_id.clone();
    let id = commission.id;

    commission.lifecycle_step = LifecycleStep::Active;
    commission.deadline = None;
    let undated = CommissionState::from(&*commission);
    let moved = move_lifecycle(&undated, LifecycleStep::Batched)
        .expect("passes")
        .expect("a real move");
    let lifecycle_moved = NewChangelogEntry::lifecycle_moved(id, owner.clone(), &moved, at);
    let direction = set_direction(&undated, DirectionStatus::WaitingForInput).expect("a real set");
    let direction_set = NewChangelogEntry::direction_set(id, owner.clone(), &direction, at);
    let first = schedule_deadline(&undated, march.into())
        .expect("passes")
        .expect("a real set");
    let deadline_set = NewChangelogEntry::deadline_set(id, owner.clone(), &first, at);

    commission.deadline = Some(march);
    commission.direction_status = Some(DirectionStatus::ChangesRequested);
    let dated = CommissionState::from(&*commission);
    let extended = schedule_deadline(&dated, april.into())
        .expect("passes")
        .expect("a real set");
    let deadline_extended = NewChangelogEntry::deadline_set(id, owner.clone(), &extended, at);
    let gone = clear_deadline(&dated)
        .expect("passes")
        .expect("a real clear");
    let deadline_cleared = NewChangelogEntry::deadline_cleared(id, owner.clone(), &gone, at);
    let cleared = clear_direction(&dated).expect("a real clear");
    let direction_cleared = NewChangelogEntry::direction_cleared(id, owner.clone(), &cleared, at);
    let before_march = "2026-02-01T12:00:00Z".parse().expect("an RFC 3339 instant");
    let raised = set_delayed(&dated, before_march)
        .expect("passes")
        .expect("a real set");
    let delayed_set = NewChangelogEntry::delayed_set(id, owner.clone(), &raised, at);

    commission.delayed_flag = Some(DeadlineStatus::Delayed);
    let flagged = CommissionState::from(&*commission);
    let lowered = clear_delayed(&flagged, before_march)
        .expect("passes")
        .expect("a real clear");
    let delayed_cleared = NewChangelogEntry::delayed_cleared(id, owner.clone(), &lowered, at);
    let late_facts = LateFacts {
        state: flagged,
        owed: true,
    };
    let mark = owes_late(&late_facts, at).expect("a late entry is owed");
    let late = NewChangelogEntry::late(id, &mark, at);
    let suggestive = Maturity {
        rating: MaturityRating::Suggestive,
        graphic: true,
    };
    let unrated = CommissionState::from(&*commission);
    let rating = rate_maturity(&unrated, suggestive).expect("a real rating");
    let maturity_rated = NewChangelogEntry::maturity_rated(id, owner, &rating, at);

    vec![
        (
            lifecycle_moved,
            json!({ "from": "active", "to": "batched" }),
        ),
        (
            direction_set,
            json!({ "from": null, "to": "waiting_for_input" }),
        ),
        (
            direction_cleared,
            json!({ "from": "changes_requested", "to": null }),
        ),
        (
            deadline_set,
            json!({ "from": null, "to": "2026-03-01T12:00:00Z" }),
        ),
        (
            deadline_extended,
            json!({ "from": "2026-03-01T12:00:00Z", "to": "2026-04-01T12:00:00Z" }),
        ),
        (
            deadline_cleared,
            json!({ "from": "2026-03-01T12:00:00Z", "to": null }),
        ),
        (
            delayed_set,
            json!({ "from": null, "to": "delayed", "deadline": "2026-03-01T12:00:00Z" }),
        ),
        (
            delayed_cleared,
            json!({ "from": "delayed", "to": null, "deadline": "2026-03-01T12:00:00Z" }),
        ),
        (
            late,
            json!({ "deadline": "2026-03-01T12:00:00Z", "from": "delayed" }),
        ),
        (
            maturity_rated,
            json!({ "rating": "suggestive", "graphic": true }),
        ),
    ]
}

// Each typed entry, the new `lifecycle_moved` and `maturity_rated` among them, reads back with
// its kind and the literal JSON its constructor wrote, and no note.
#[tokio::test]
async fn each_new_kind_s_stored_shape_is_its_literal_json() {
    let (pool, _container) = fresh_pool().await;
    let mut commission = seed_commission(&pool, "did:plc:shape-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgChangelogStore::new(pool.clone());
    let typed = typed_entries(&mut commission);

    let mut uow = db.begin().await.expect("begin");
    for (entry, _) in &typed {
        uow.changelog().append(entry).await.expect("append");
    }
    uow.commit().await.expect("commit");

    let stored = store.entries(&commission.id).await.expect("read");
    assert_eq!(stored.len(), typed.len());
    for (read, (written, shape)) in stored.iter().zip(&typed) {
        assert_eq!(read.kind, written.kind());
        assert_eq!(read.payload, *shape, "{}", read.kind);
        assert_eq!(read.note, None, "{}", read.kind);
    }
    assert_eq!(stored[0].kind, ChangelogEntryKind::LifecycleMoved);
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
        .append(&NewChangelogEntry::created(
            commission.id,
            commission.owner_id,
            Utc::now(),
        ))
        .await
        .expect("append");
    uow.commit().await.expect("commit");

    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(uuid::Uuid::from(commission.id))
        .execute(&pool)
        .await
        .expect("hard-delete the commission");

    let store = PgChangelogStore::new(pool.clone());
    assert!(
        store
            .entries(&commission.id)
            .await
            .expect("read")
            .is_empty(),
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
    let unknown = CommissionId::from(uuid::Uuid::now_v7());

    assert!(
        seed::participates(&pool, &commission.id, &commission.owner_id).await,
        "the owner is a Participant without a Seat",
    );
    assert!(
        !seed::participates(&pool, &commission.id, &stranger.id).await,
        "a non-owner is not (yet) a Participant",
    );
    assert!(
        !seed::participates(&pool, &unknown, &commission.owner_id).await,
        "an unknown commission has no participants",
    );
}

// CommissionRows::find round-trips the aggregate — including the nullable
// linked-channel pointer set and cleared through the write view.
#[tokio::test]
async fn find_roundtrips_the_linked_channel() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:channel-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    let found = store
        .find(&commission.id)
        .await
        .expect("find")
        .expect("the commission exists");
    let viewed = store
        .find_summary(&commission.id, &commission.owner_id)
        .await
        .expect("find_summary")
        .expect("the commission exists");
    assert_eq!(viewed.summary.title.as_str(), "A ref sheet");
    assert_eq!(found.owner_id, commission.owner_id);
    assert!(found.linked_channel.is_none(), "born with no channel");

    let pointer = "https://t.me/refsheet-chat"
        .parse::<ChannelPointer>()
        .expect("valid pointer");
    let mut uow = db.begin().await.expect("begin");
    assert!(
        uow.commissions()
            .set_linked_channel(&commission.id, Some(&pointer))
            .await
            .expect("set channel"),
        "the first link is a real change"
    );
    assert!(
        !uow.commissions()
            .set_linked_channel(&commission.id, Some(&pointer))
            .await
            .expect("re-set channel"),
        "re-linking the identical pointer answers false"
    );
    uow.commit().await.expect("commit");
    let found = store
        .find(&commission.id)
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
            .set_linked_channel(&commission.id, None)
            .await
            .expect("clear channel"),
        "the clear is a real change"
    );
    assert!(
        !uow.commissions()
            .set_linked_channel(&commission.id, None)
            .await
            .expect("re-clear channel"),
        "clearing an already-clear channel answers false"
    );
    uow.commit().await.expect("commit");
    let found = store
        .find(&commission.id)
        .await
        .expect("find")
        .expect("exists");
    assert!(found.linked_channel.is_none(), "the pointer clears to NULL");

    // An unknown commission finds nothing.
    assert!(
        store
            .find(&CommissionId::from(uuid::Uuid::now_v7()))
            .await
            .expect("find unknown")
            .is_none(),
    );
}

/// The direction status `store` holds for `id`.
async fn direction_of(store: &PgCommissionStore, id: &CommissionId) -> Option<DirectionStatus> {
    store
        .find(id)
        .await
        .expect("find")
        .expect("exists")
        .direction_status
}

/// The direction change `to` on what `store` holds for `id`, as its rule checks it; `None`
/// clears.
async fn direction_change(
    store: &PgCommissionStore,
    id: &CommissionId,
    to: Option<DirectionStatus>,
) -> DirectionChange {
    let stored = store.find(id).await.expect("find").expect("exists");
    let state = CommissionState::from(&stored);
    match to {
        Some(to) => DirectionChange::from(set_direction(&state, to).expect("a real set")),
        None => DirectionChange::from(clear_direction(&state).expect("a real clear")),
    }
}

// CommissionRows::find round-trips the nullable direction status set, replaced and cleared
// through the write view by compare-and-set: one column, so a set replaces by construction
// and NULL is the cleared state; a change checked from a status the row no longer holds, or
// on no commission, matches no row and answers `false`.
#[tokio::test]
async fn find_roundtrips_the_direction_status_by_compare_and_set() {
    let (pool, _container) = fresh_pool().await;
    let commission = seed_commission(&pool, "did:plc:status-owner").await;
    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());
    let capability = seed::participant_on(commission.id);
    assert_eq!(
        direction_of(&store, &commission.id).await,
        None,
        "born with no direction status"
    );

    let first = direction_change(
        &store,
        &commission.id,
        Some(DirectionStatus::WaitingForInput),
    )
    .await;
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_direction_status(&capability, &first)
        .await
        .expect("set status");
    uow.commit().await.expect("commit");
    assert!(landed);
    assert_eq!(
        direction_of(&store, &commission.id).await,
        Some(DirectionStatus::WaitingForInput),
    );

    // A second set replaces the value whole — never accumulates.
    let replaced = direction_change(
        &store,
        &commission.id,
        Some(DirectionStatus::ChangesRequested),
    )
    .await;
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_direction_status(&capability, &replaced)
        .await
        .expect("replace status");
    let stale = uow
        .commissions()
        .set_direction_status(&capability, &first)
        .await
        .expect("a stale set runs");
    uow.commit().await.expect("commit");
    assert!(landed);
    assert!(
        !stale,
        "a set from a status the row no longer holds matches no row"
    );
    assert_eq!(
        direction_of(&store, &commission.id).await,
        Some(DirectionStatus::ChangesRequested),
    );

    // Clearing writes NULL; the deadline envelope is a separate axis and is
    // untouched throughout (it was None at seed and stays None).
    let cleared = direction_change(&store, &commission.id, None).await;
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_direction_status(&capability, &cleared)
        .await
        .expect("clear status");
    let elsewhere = uow
        .commissions()
        .set_direction_status(
            &seed::participant_on(CommissionId::from(uuid::Uuid::now_v7())),
            &first,
        )
        .await
        .expect("an unknown commission runs");
    uow.commit().await.expect("commit");
    assert!(landed);
    assert!(!elsewhere, "an absent commission matches no row");
    let found = store
        .find(&commission.id)
        .await
        .expect("find")
        .expect("exists");
    assert!(found.direction_status.is_none(), "cleared to NULL");
    assert!(found.deadline.is_none());
}
