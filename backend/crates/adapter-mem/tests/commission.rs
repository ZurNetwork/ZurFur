//! The commission fact predicate over the in-memory fake (ZMVP-67):
//! `commission_has_facts` is reachable only on an open [`UnitOfWork`]'s
//! commissions view — the same transaction a future delete/archive gate
//! (ZMVP-66/68) runs in — and, with no fact-minter wired anywhere, every
//! commission answers `false`. The archive flag (ZMVP-68) rides the same
//! seam: `set_archived` is a transactional write reporting whether the state
//! actually flipped.

use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle},
    did::Did,
};

/// AC2+AC3 (mem): a freshly created commission holds no facts — asked in the
/// **same unit of work** that created it (the transactional read the delete gate
/// needs), and again from a later unit after the commit.
#[tokio::test]
async fn every_commission_answers_false_with_no_fact_minters_wired() {
    let backend = MemBackend::new();
    let owner = backend
        .provision(&Did::new("did:plc:factless-owner".to_string()))
        .await
        .expect("provision owner");
    let title = "A ref sheet"
        .parse::<CommissionTitle>()
        .expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = commission.id;

    let db = backend.database();

    // Same-transaction read: create and ask inside one unit of work.
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.create(&commission).await.expect("create");
        let has_facts = commissions
            .commission_has_facts(id)
            .await
            .expect("has_facts in the creating unit");
        assert!(
            !has_facts,
            "no fact-minter exists, so no commission can bear facts"
        );
    }
    uow.commit().await.expect("commit");

    // A later unit of work sees the committed commission and the same answer.
    let mut uow = db.begin().await.expect("begin second unit");
    let has_facts = uow
        .commissions()
        .commission_has_facts(id)
        .await
        .expect("has_facts in a later unit");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// A commission id nobody ever created also answers `false`: absence of the
/// commission is absence of facts, not an error — the gate's own existence check
/// is a separate concern.
#[tokio::test]
async fn an_unknown_commission_answers_false() {
    let backend = MemBackend::new();
    let db = backend.database();

    let mut uow = db.begin().await.expect("begin");
    let has_facts = uow
        .commissions()
        .commission_has_facts(CommissionId::new(uuid::Uuid::now_v7()))
        .await
        .expect("has_facts for an unknown id");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// ZMVP-68 (mem store layer): `set_archived` flips the archive flag through the
/// unit of work and reports **whether the state actually transitioned** — the
/// bool the route keys its changelog append on, so a repeated archive (or
/// un-archive) can never mint a duplicate entry. The first stamp survives a
/// repeat, and both directions round-trip through `find`.
#[tokio::test]
async fn set_archived_round_trips_and_reports_transitions() {
    let backend = MemBackend::new();
    let owner = backend
        .provision(&Did::new("did:plc:archiving-owner".to_string()))
        .await
        .expect("provision owner");
    let commission = Commission::create(
        "A ref sheet"
            .parse::<CommissionTitle>()
            .expect("valid title"),
        owner.id,
        Utc::now(),
        None,
    );
    let id = commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed commission");
    assert!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "a fresh commission is active"
    );

    // Archive: a real transition.
    let stamp = Utc::now();
    let mut uow = backend.database().begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(id, Some(stamp))
        .await
        .expect("archive");
    assert!(changed, "active -> archived is a transition");
    uow.commit().await.expect("commit");
    let stored = backend
        .find_commission(id)
        .await
        .expect("find")
        .expect("the record survives");
    assert_eq!(stored.archived_at, Some(stamp), "the stamp round-trips");

    // A repeat archive is no transition and keeps the first stamp.
    let mut uow = backend.database().begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(id, Some(Utc::now()))
        .await
        .expect("repeat archive");
    assert!(!changed, "archived -> archived is not a transition");
    uow.commit().await.expect("commit");
    assert_eq!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at,
        Some(stamp),
        "a repeat archive never rewrites the original stamp",
    );

    // Un-archive: back to active.
    let mut uow = backend.database().begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(id, None)
        .await
        .expect("unarchive");
    assert!(changed, "archived -> active is a transition");
    uow.commit().await.expect("commit");
    assert!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "un-archiving clears the flag"
    );

    // A repeat un-archive is no transition; an unknown commission is a no-op.
    let mut uow = backend.database().begin().await.expect("begin");
    assert!(
        !uow.commissions()
            .set_archived(id, None)
            .await
            .expect("repeat unarchive"),
        "active -> active is not a transition",
    );
    assert!(
        !uow.commissions()
            .set_archived(CommissionId::new(uuid::Uuid::now_v7()), Some(Utc::now()))
            .await
            .expect("archive an unknown id"),
        "an absent commission matches nothing (existence is the caller's check)",
    );
    uow.rollback().await.expect("rollback");
}

/// ZMVP-68 (mem store layer): an archive staged in a dropped unit of work is
/// discarded — the flag write obeys the same commit-or-discard rule as every
/// other commission write (DD 24150017).
#[tokio::test]
async fn a_dropped_unit_of_work_discards_the_archive() {
    let backend = MemBackend::new();
    let owner = backend
        .provision(&Did::new("did:plc:rollback-owner".to_string()))
        .await
        .expect("provision owner");
    let commission = Commission::create(
        "Kept active"
            .parse::<CommissionTitle>()
            .expect("valid title"),
        owner.id,
        Utc::now(),
        None,
    );
    let id = commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed commission");

    {
        let mut uow = backend.database().begin().await.expect("begin");
        assert!(
            uow.commissions()
                .set_archived(id, Some(Utc::now()))
                .await
                .expect("stage archive"),
        );
        // `uow` drops here without `commit` → the staged flag is discarded.
    }

    assert!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "a dropped unit of work archives nothing"
    );
}
