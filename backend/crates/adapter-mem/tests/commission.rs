//! The commission fact predicate over the in-memory fake (ZMVP-67):
//! `commission_has_facts` is reachable only on an open [`UnitOfWork`]'s
//! commissions view — the same transaction a future delete/archive gate
//! (ZMVP-66/68) runs in — and, with no fact-minter wired anywhere, every
//! commission answers `false`.

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
    let title = CommissionTitle::try_new("A ref sheet").expect("valid title");
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
