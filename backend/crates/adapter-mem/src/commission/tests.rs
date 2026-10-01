use chrono::Utc;
use domain::elements::account::AccountId;
use domain::elements::commission::{NewSlot, SKELETON, SeatInvitation, SeatKind, SlotTitle};
use domain::elements::did::Did;
use domain::elements::workflow::{ColumnName, WorkflowName};
use domain::ports::{ElementNotFound, UnknownSurface, UnknownTab};
use serde_json::json;

use super::*;

/// A fresh, unique synthetic actor DID; the UUID is only a uniqueness source.
fn mint_did() -> Did {
    Did::from(format!("did:plc:mem{}", uuid::Uuid::now_v7().simple()))
}

fn user_id() -> UserId {
    UserId::from(mint_did())
}

fn commission(title: &str, owner: UserId) -> Commission {
    Commission::create(
        title.parse::<CommissionTitle>().unwrap(),
        owner,
        Utc::now(),
        None,
    )
}

// ZMVP-65 AC1/AC2/AC3 (store layer) — a commission written through the
// UnitOfWork's commission view (begin → commissions().create → commit) is read
// back with its fixed metadata intact: the creating User is the owner and the
// fresh commission is in `Draft`. The mem seam, end to end — proving the write
// view and the shared store share state, mirroring the account seam test.
#[tokio::test]
async fn uow_create_commission_is_visible_after_commit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();

    let created = commission("A ref sheet", owner.clone());
    let id = created.id;

    let mut uow = database.begin().await.unwrap();
    uow.commissions().create(&created).await.unwrap();
    uow.commit().await.unwrap();

    let found = backend
        .find_commission(id)
        .await
        .unwrap()
        .expect("commission present");
    assert_eq!(found.id, id);
    assert_eq!(found.title.as_str(), "A ref sheet");
    assert_eq!(found.owner_id, owner, "the creating User owns it");
    assert!(
        matches!(found.lifecycle_step, LifecycleStep::Draft),
        "a fresh commission is in Draft"
    );
    assert!(
        matches!(found.visibility, Visibility::Private),
        "a fresh commission is Private (the closed-door default)"
    );
    assert!(
        found.linked_channel.is_none(),
        "a fresh commission declares no channel"
    );
}

// Dropping a unit of work before `commit()` discards the commission — the mem
// mirror of pg's drop = rollback (DD 24150017), the commission analogue of
// `a_dropped_unit_of_work_rolls_back_every_write`.
#[tokio::test]
async fn a_dropped_unit_of_work_rolls_back_the_commission() {
    let backend = MemBackend::new();
    let database = backend.database();

    let created = commission("Uncommitted", user_id());
    let id = created.id;

    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        // `uow` drops here without `commit` → the staged write is discarded.
    }

    assert!(
        backend.find_commission(id).await.unwrap().is_none(),
        "a dropped unit of work persists no commission row"
    );
}

// An uncommitted unit's commission is invisible to a read off the shared store
// *before* the unit commits — matching pg, where a pool read can't see another
// connection's open transaction.
#[tokio::test]
async fn uncommitted_commission_is_invisible_until_commit() {
    let backend = MemBackend::new();
    let database = backend.database();

    let created = commission("Isolated", user_id());
    let id = created.id;

    let mut uow = database.begin().await.unwrap();
    uow.commissions().create(&created).await.unwrap();
    assert!(
        backend.find_commission(id).await.unwrap().is_none(),
        "an open unit's staged commission is invisible to a shared read"
    );

    uow.commit().await.unwrap();
    assert!(
        backend.find_commission(id).await.unwrap().is_some(),
        "the commission becomes visible once the unit commits"
    );
}

// ZMVP-87 (store layer) — an appended entry commits with its unit and rolls
// back with it (the mem mirror of the DD's atomic-with-domain-writes rule),
// and the stream reads back in seq order, per commission.
#[tokio::test]
async fn changelog_appends_commit_and_roll_back_with_the_unit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Logged", owner.clone());
    let id = created.id;

    let mut uow = database.begin().await.unwrap();
    uow.commissions().create(&created).await.unwrap();
    uow.changelog()
        .append(&NewChangelogEntry::event(
            id,
            ChangelogEntryKind::Created,
            owner.clone(),
            json!({ "title": "Logged" }),
            Utc::now(),
        ))
        .await
        .unwrap();
    uow.commit().await.unwrap();

    // A rolled-back (dropped) unit's append is discarded.
    {
        let mut uow = database.begin().await.unwrap();
        uow.changelog()
            .append(&NewChangelogEntry::note(
                id,
                owner.clone(),
                "never happened".to_string(),
                Utc::now(),
            ))
            .await
            .unwrap();
    }

    let entries = backend.changelog_entries(id).await.unwrap();
    assert_eq!(entries.len(), 1, "only the committed entry survives");
    assert!(matches!(entries[0].kind, ChangelogEntryKind::Created));
    assert_eq!(entries[0].actor_id, Some(owner.clone()));

    // A second committed entry lands after the first, and other commissions'
    // streams stay separate.
    let mut uow = database.begin().await.unwrap();
    uow.changelog()
        .append(&NewChangelogEntry::note(
            id,
            owner,
            "traveling next week".to_string(),
            Utc::now(),
        ))
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let entries = backend.changelog_entries(id).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].seq < entries[1].seq, "seq orders the stream");
    assert_eq!(entries[1].note.as_deref(), Some("traveling next week"));
    assert!(
        backend
            .changelog_entries(CommissionId::from(uuid::Uuid::now_v7()))
            .await
            .unwrap()
            .is_empty(),
        "an unknown commission has an empty stream"
    );
}

// ZMVP-66 AC1 (store layer) — `delete` removes the commission and its
// changelog entries together (the mem mirror of the pg ON DELETE CASCADE),
// leaving other commissions' streams untouched.
#[tokio::test]
async fn delete_removes_the_commission_and_cascades_its_changelog() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let doomed = commission("Doomed", owner.clone());
    let doomed_id = doomed.id;
    let survivor = commission("Survivor", owner.clone());
    let survivor_id = survivor.id;

    let mut uow = database.begin().await.unwrap();
    uow.commissions().create(&doomed).await.unwrap();
    uow.commissions().create(&survivor).await.unwrap();
    for (id, title) in [(doomed_id, "Doomed"), (survivor_id, "Survivor")] {
        uow.changelog()
            .append(&NewChangelogEntry::event(
                id,
                ChangelogEntryKind::Created,
                owner.clone(),
                json!({ "title": title }),
                Utc::now(),
            ))
            .await
            .unwrap();
    }
    uow.commit().await.unwrap();

    let mut uow = database.begin().await.unwrap();
    uow.commissions().delete(&doomed_id).await.unwrap();
    uow.commit().await.unwrap();

    assert!(
        backend.find_commission(doomed_id).await.unwrap().is_none(),
        "the deleted commission is gone"
    );
    assert!(
        backend
            .changelog_entries(doomed_id)
            .await
            .unwrap()
            .is_empty(),
        "its changelog cascaded away with it"
    );
    assert!(
        backend
            .find_commission(survivor_id)
            .await
            .unwrap()
            .is_some(),
        "other commissions survive"
    );
    assert_eq!(
        backend.changelog_entries(survivor_id).await.unwrap().len(),
        1,
        "other streams are untouched"
    );
}

// ZMVP-66 (store layer) — a delete staged in a dropped (uncommitted) unit of
// work is discarded: the commission and its changelog survive. The gate that
// precedes the delete runs in this same unit (ruling E17), so rollback must
// undo the delete too.
#[tokio::test]
async fn a_dropped_unit_of_work_rolls_back_the_delete() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Kept", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions().delete(&id).await.unwrap();
        // `uow` drops here without `commit` → the staged delete is discarded.
    }

    assert!(
        backend.find_commission(id).await.unwrap().is_some(),
        "a dropped unit of work deletes nothing"
    );
}

// ZMVP-66 (store layer) — deleting an absent commission is a no-op, not an
// error (existence is the caller's separate check, per the port contract).
#[tokio::test]
async fn deleting_an_absent_commission_is_a_no_op() {
    let backend = MemBackend::new();
    let database = backend.database();

    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .delete(&CommissionId::from(uuid::Uuid::now_v7()))
        .await
        .unwrap();
    uow.commit().await.unwrap();
}

fn account_id() -> AccountId {
    AccountId::from(mint_did())
}

/// A board owned by `account`, with one column on it, committed.
async fn board_with_a_column(backend: &MemBackend, account: &AccountId) -> (WorkflowId, ColumnId) {
    let database = backend.database();
    let name = "Queue".parse::<WorkflowName>().expect("a valid board name");

    let mut uow = database.begin().await.unwrap();
    let mut workflow = uow.workflows().create(&name, account).await.unwrap();
    let column_name = "Open".parse::<ColumnName>().expect("a valid column name");
    let column = workflow.new_column(column_name, workflow.visibility.clone());
    let column_id = column.id;
    workflow.insert(0, column).expect("the board is empty");
    uow.workflows().set_indexes(&workflow).await.unwrap();
    uow.commit().await.unwrap();

    (workflow.id, column_id)
}

// ZMVP-70 (mem store layer) — a view grant upserts and revoke hard-deletes;
// it stages with the unit (drop = rollback) and confers NO participant-hood
// (Ownership Separation DD Decision 8). A key is issued to a **User**, never
// an account (D3 as amended 2026-09-04), so it is read back by that User.
#[tokio::test]
async fn grants_stage_lift_nothing_and_roll_back() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Positioned", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let store = backend.commission_store();
    let grantee = user_id();

    // Grant Total, then revoke — the key is gone immediately.
    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .grant_view(&id, &grantee, GrantLevel::Total)
        .await
        .unwrap();
    uow.commit().await.unwrap();
    assert_eq!(
        store.view_grant(&id, &grantee).await.unwrap(),
        Some(GrantLevel::Total)
    );

    // A key is only a view: it makes its holder no Participant (D8).
    assert!(
        !store.is_participant(&id, &grantee).await.unwrap(),
        "a key confers no in-commission authority",
    );
    assert!(
        store.is_participant(&id, &owner).await.unwrap(),
        "the owner still is"
    );

    let mut uow = database.begin().await.unwrap();
    assert!(
        uow.commissions().revoke_view(&id, &grantee).await.unwrap(),
        "revoking an existing key reports a transition",
    );
    uow.commit().await.unwrap();
    assert!(
        store.view_grant(&id, &grantee).await.unwrap().is_none(),
        "a revoked key is gone immediately",
    );

    // A dropped unit rolls the grant back.
    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .grant_view(&id, &grantee, GrantLevel::Description)
            .await
            .unwrap();
        // drop without commit
    }
    assert!(
        store.view_grant(&id, &grantee).await.unwrap().is_none(),
        "the dropped grant never landed",
    );
}

// ZMVP-70 (mem store layer) — **placement is a card on a board** (Ownership
// Separation DD `29130754` Decision 6): positioning a commission means
// putting it in a column, it stages with the unit (drop = rollback), and it
// confers NO participant-hood (Decision 8). The same commission may sit on
// two accounts' boards at once — the NxM the DD makes native.
#[tokio::test]
async fn placement_is_a_card_stages_lifts_nothing_and_rolls_back() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Positioned", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let store = backend.commission_store();
    let columns = backend.column_store();

    let account = account_id();
    let (workflow_id, column_id) = board_with_a_column(&backend, &account).await;
    let other_account = account_id();
    let (other_workflow, other_column) = board_with_a_column(&backend, &other_account).await;

    // Place the card on the first board.
    let mut uow = database.begin().await.unwrap();
    let mut column = columns.find(&column_id).await.unwrap().expect("the column");
    column.push(id).expect("an empty column takes a card");
    uow.columns().set_commissions(&column).await.unwrap();
    uow.commit().await.unwrap();

    assert_eq!(
        store
            .current_column_of_workflow(&id, &workflow_id)
            .await
            .unwrap()
            .map(|found| found.id),
        Some(column_id),
        "the board that positioned it now holds the card",
    );
    assert_eq!(
        store
            .current_position_in_column(&id, &column_id)
            .await
            .unwrap(),
        Some(0),
        "at the index the domain put it",
    );

    // The SAME commission on a SECOND account's board — no conflict, because
    // no account ever claimed it (DD D1: users own commissions).
    let mut uow = database.begin().await.unwrap();
    let mut column = columns
        .find(&other_column)
        .await
        .unwrap()
        .expect("the other column");
    column.push(id).expect("an empty column takes a card");
    uow.columns().set_commissions(&column).await.unwrap();
    uow.commit().await.unwrap();

    assert!(
        store
            .current_column_of_workflow(&id, &other_workflow)
            .await
            .unwrap()
            .is_some(),
        "one commission sits on N boards at once",
    );
    assert!(
        store
            .current_column_of_workflow(&id, &workflow_id)
            .await
            .unwrap()
            .is_some(),
        "and the first board still holds it",
    );

    // Positioning makes nobody a Participant (D8).
    let member = user_id();
    assert!(
        !store.is_participant(&id, &member).await.unwrap(),
        "positioning confers no in-commission authority",
    );

    // A dropped unit rolls a card back off the board.
    let third = commission("Not placed", owner.clone());
    let third_id = third.id;
    backend.create_commission(&third).await.unwrap();
    {
        let mut uow = database.begin().await.unwrap();
        let mut column = columns.find(&column_id).await.unwrap().expect("the column");
        column
            .push(third_id)
            .expect("the column takes a second card");
        uow.columns().set_commissions(&column).await.unwrap();
        // drop without commit
    }
    assert!(
        store
            .current_column_of_workflow(&third_id, &workflow_id)
            .await
            .unwrap()
            .is_none(),
        "the dropped card never landed",
    );
}

// ZMVP-57 AC1 (mem parity) — hard-deleting an account **severs** its
// positioning while the positioned commission **survives untouched**. Since
// placement is a card on a board (DD `29130754` D6), the rail severed is the
// board itself: this mirrors pg's `workflow.account_id … ON DELETE CASCADE`
// and the column/card cascades below it. Only the account-side positioning
// goes; the User-owned commission stays.
//
// The **view grant** used to be asserted here as a second rail. It no longer
// is: a grant is issued to a User (Engineer ruling 2026-09-04), so the pg
// table holds no reference to an account to cascade from and the mem fake
// mirrors that.
#[tokio::test]
async fn hard_deleting_an_account_severs_its_positioning_but_keeps_the_commission() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Placed then orphaned", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let store = backend.commission_store();
    let columns = backend.column_store();
    let workflows = backend.workflow_store();
    let account = account_id();

    // Position the commission on the account's board.
    let (workflow_id, column_id) = board_with_a_column(&backend, &account).await;
    let mut uow = database.begin().await.unwrap();
    let mut column = columns.find(&column_id).await.unwrap().expect("the column");
    column.push(id).expect("an empty column takes a card");
    uow.columns().set_commissions(&column).await.unwrap();
    uow.commit().await.unwrap();
    assert!(
        store
            .current_column_of_workflow(&id, &workflow_id)
            .await
            .unwrap()
            .is_some(),
        "positioned before the delete"
    );

    // Hard-delete the account.
    let mut uow = database.begin().await.unwrap();
    uow.accounts().hard_delete(&account).await.unwrap();
    uow.commit().await.unwrap();

    // The positioning rail is severed — board, column and card together...
    assert!(
        workflows.find(&workflow_id).await.unwrap().is_none(),
        "the board is severed with the account",
    );
    assert!(
        columns.find(&column_id).await.unwrap().is_none(),
        "and its columns with it",
    );
    assert!(
        store
            .current_column_of_workflow(&id, &workflow_id)
            .await
            .unwrap()
            .is_none(),
        "so the card is gone too",
    );
    // ...but the commission itself survives untouched.
    assert!(
        backend.find_commission(id).await.unwrap().is_some(),
        "the User-owned commission survives account deletion",
    );
}

// ZMVP-166 (store layer) — a commission is born with its SKELETON TABS in
// the same unit of work: after create+commit the loaded composition holds
// exactly the declared tabs, every one born Total (the closed door), and no
// elements. Tab state exists explicitly from the first instant — the
// withheld-at-birth discipline — so absence never has to mean anything.
#[tokio::test]
async fn creating_a_commission_mints_its_skeleton_tabs() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Composed", owner);
    let id = created.id;

    let mut uow = database.begin().await.unwrap();
    uow.commissions().create(&created).await.unwrap();
    uow.commit().await.unwrap();

    let composition = backend
        .commission_store()
        .load_composition(&id)
        .await
        .unwrap()
        .expect("a created commission always has its tabs");
    let names: Vec<&str> = composition
        .tabs
        .iter()
        .map(|tab| tab.tab.as_ref())
        .collect();
    let declared: Vec<String> = declared_tabs()
        .iter()
        .map(|tab| tab.as_ref().to_owned())
        .collect();
    assert_eq!(
        names, declared,
        "exactly the code-declared skeleton, nothing more"
    );
    assert!(
        composition
            .tabs
            .iter()
            .all(|tab| tab.mode == VisibilityMode::Total),
        "every tab is born Total — the commission's own visibility is NOT copied in"
    );
    assert!(
        composition.elements.is_empty(),
        "a fresh commission is composed of nothing"
    );
    assert!(
        composition.surface_modes.is_empty(),
        "no surface has been widened, so no override row exists"
    );
}

// ZMVP-166 — elements append within their (tab, surface, band) group: two
// contributions keep append order, and every element is born Total.
#[tokio::test]
async fn add_element_appends_within_its_group() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Growing", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let address = only_address(&backend, id).await;

    let first = element_at(id, address.clone(), owner.clone());
    let second = element_at(id, address.clone(), owner);
    let (first_id, second_id) = (first.id, second.id);
    let mut uow = database.begin().await.unwrap();
    uow.commissions().add_element(&first).await.unwrap();
    uow.commissions().add_element(&second).await.unwrap();
    uow.commit().await.unwrap();

    let elements = backend.elements_of(id).await.unwrap();
    assert_eq!(elements.len(), 2);
    assert_eq!(elements[0].id, first_id, "append order holds");
    assert_eq!(elements[0].position, 0);
    assert_eq!(elements[1].id, second_id);
    assert_eq!(elements[1].position, 1);
    assert!(
        elements
            .iter()
            .all(|element| element.mode == VisibilityMode::Total),
        "every element is born Total — over-claiming has to be an explicit act"
    );
    assert!(
        elements
            .iter()
            .all(|element| element.band == Band::default()),
        "everything lands in the placeholder band (ZMVP-171 owns the vocabulary)"
    );
}

// ZMVP-166 — the tab must exist in THIS commission: a fabricated tab id and
// one belonging to another commission both fail with UnknownTab (one
// indistinguishable answer — no probing other commissions), and neither
// write lands. In pg the cross-commission case is additionally
// unrepresentable (the composite foreign key); the fake has only this gate,
// which is why the case is pinned here explicitly.
#[tokio::test]
async fn add_element_refuses_absent_and_foreign_tabs() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let mine = commission("Mine", owner.clone());
    let theirs = commission("Theirs", user_id());
    let mine_id = mine.id;
    let theirs_id = theirs.id;
    backend.create_commission(&mine).await.unwrap();
    backend.create_commission(&theirs).await.unwrap();
    let their_address = only_address(&backend, theirs_id).await;

    // A tab id that exists nowhere.
    let fabricated_address = SurfaceAddress::new(TabId::mint(), only_surface());
    let fabricated = element_at(mine_id, fabricated_address, owner.clone());
    let mut uow = database.begin().await.unwrap();
    let err = uow
        .commissions()
        .add_element(&fabricated)
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "absent tab surfaces as UnknownTab, got: {err:?}"
    );
    drop(uow);

    // A real tab — belonging to someone else's commission.
    let cross = element_at(mine_id, their_address, owner);
    let mut uow = database.begin().await.unwrap();
    let err = uow.commissions().add_element(&cross).await.unwrap_err();
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "a foreign tab is indistinguishable from an absent one, got: {err:?}"
    );
    drop(uow);

    assert!(
        backend.elements_of(mine_id).await.unwrap().is_empty(),
        "no refused write landed"
    );
    assert!(
        backend.elements_of(theirs_id).await.unwrap().is_empty(),
        "and nothing leaked into the other commission either"
    );
}

// ZMVP-166 — the surface must be one the CODE SKELETON declares. Surfaces
// have no rows, so the const is the only authority, and an unrecognized name
// is refused rather than created (fail-closed) — the same answer the pg
// adapter gives, from the same const.
#[tokio::test]
async fn add_element_refuses_an_undeclared_surface() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Fail-closed", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let tab = backend.tabs_of(id).await.unwrap()[0].id;

    let invented = SurfaceAddress::new(tab, "invented".parse::<SurfaceName>().unwrap());
    let element = element_at(id, invented, owner);
    let mut uow = database.begin().await.unwrap();
    let err = uow.commissions().add_element(&element).await.unwrap_err();
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "an undeclared surface surfaces as UnknownSurface, got: {err:?}"
    );
    drop(uow);

    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "nothing landed, and no surface was invented"
    );
}

// ZMVP-166 — the skeleton check is on the PAIR: a surface that is perfectly
// real under its own tab, addressed under a DIFFERENT tab of the same
// commission, is refused with UnknownSurface — the pair is not declared, and
// the tab itself is real, so this is not an UnknownTab. Same answer the pg
// adapter gives, from the same const.
#[tokio::test]
async fn add_element_refuses_a_real_surface_under_the_wrong_tab() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Wrongly addressed", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    // A real tab row of THIS commission whose name the skeleton does not
    // pair with the surface below. (The placeholder skeleton has one tab, so
    // the shape has to be seeded; ZMVP-171's real skeleton makes it
    // ordinary.)
    let other = backend.seed_tab(id, "other".parse::<TabName>().unwrap());
    let wrongly_addressed = SurfaceAddress::new(other, only_surface());
    let element = element_at(id, wrongly_addressed, owner);

    let mut uow = database.begin().await.unwrap();
    let err = uow.commissions().add_element(&element).await.unwrap_err();
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "the (tab, surface) pair is undeclared — UnknownSurface, not UnknownTab \
         (the tab is real), got: {err:?}"
    );
    drop(uow);

    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "nothing landed under a place the skeleton never described"
    );
}

// ZMVP-166 — the gate ORDER is part of the contract, because the pair check
// needs the tab's declared name and so cannot run first. An address that is
// wrong in BOTH ways answers UnknownTab; both adapters must agree, or one
// request would get two different answers depending on the store.
#[tokio::test]
async fn an_address_wrong_in_both_ways_answers_unknown_tab() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Doubly wrong", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    let doubly_wrong =
        SurfaceAddress::new(TabId::mint(), "invented".parse::<SurfaceName>().unwrap());
    let element = element_at(id, doubly_wrong, owner);
    let mut uow = database.begin().await.unwrap();
    let err = uow.commissions().add_element(&element).await.unwrap_err();
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "the tab is resolved FIRST, so a fabricated tab wins over an \
         undeclared surface, got: {err:?}"
    );
    drop(uow);
}

// ZMVP-166 — the opaque payload round-trips: whatever JSON went in reads back
// as an equal value, and the core never interprets it.
#[tokio::test]
async fn an_elements_payload_round_trips_opaquely() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Opaque", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let address = only_address(&backend, id).await;

    let body = json!({
        "kind": "text",
        "body": "Reference: 三毛猫 🐾",
        "nested": { "list": [1, 2, 3], "flag": true, "nothing": null },
    });
    let payload = ElementPayload::from(body.clone());
    let element = NewElement::contributed(
        id,
        address,
        "note".parse::<ElementType>().unwrap(),
        payload,
        owner,
        Utc::now(),
    );
    let element_id = element.id;
    let mut uow = database.begin().await.unwrap();
    uow.commissions().add_element(&element).await.unwrap();
    uow.commit().await.unwrap();

    let stored = backend.elements_of(id).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, element_id);
    assert_eq!(
        stored[0].payload.as_ref(),
        &body,
        "the payload is carried opaque"
    );
    assert_eq!(stored[0].element_type.as_ref(), "note");
}

// ZMVP-166 (transactionality) — a staged element is invisible until commit
// and discarded on drop, exactly like every other unit-of-work write.
#[tokio::test]
async fn add_element_commits_and_rolls_back_with_the_unit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Tx", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let address = only_address(&backend, id).await;

    {
        let element = element_at(id, address, owner);
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_element(&element).await.unwrap();
        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "an open unit's staged element is invisible to a shared read"
        );
        // `uow` drops here without `commit` -> the staged element is discarded.
    }

    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "a dropped unit of work persists no element"
    );
}

// ZMVP-166 — the three-term projection, read end to end off a real
// composition: an element that claims more than its surface allows is INERT.
#[tokio::test]
async fn effective_visibility_clamps_against_the_loaded_composition() {
    let backend = MemBackend::new();
    let owner = user_id();
    let created = commission("Clamped", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let address = only_address(&backend, id).await;

    let element = element_at(id, address.clone(), owner);
    let mut uow = backend.database().begin().await.unwrap();
    uow.commissions().add_element(&element).await.unwrap();
    uow.commit().await.unwrap();

    // Everything closed at birth: the element projects Total.
    let store = backend.commission_store();
    let composition = store
        .load_composition(&id)
        .await
        .unwrap()
        .expect("composed");
    let only = &composition.elements[0];
    assert_eq!(
        composition.effective_visibility_of(only),
        VisibilityMode::Total,
        "a commission nobody widened shows nothing"
    );

    // Widen the tab and the surface wide open (ZMVP-74 owns the real act);
    // the element's OWN mode is still Total, so it stays closed.
    backend.set_tab_mode(address.tab, VisibilityMode::Description);
    backend.set_surface_mode(id, address.surface.clone(), VisibilityMode::Description);
    let composition = store
        .load_composition(&id)
        .await
        .unwrap()
        .expect("composed");
    let only = &composition.elements[0];
    assert_eq!(
        composition.effective_visibility_of(only),
        VisibilityMode::Total,
        "the element is the narrowest term, and it can always close further"
    );

    // Narrow the surface back down and over-claim on the element: the
    // surface still wins, because the result is the MIN.
    backend.set_surface_mode(id, address.surface.clone(), VisibilityMode::Presentation);
    let mut composition = store
        .load_composition(&id)
        .await
        .unwrap()
        .expect("composed");
    composition.elements[0].mode = VisibilityMode::Description;
    let only = &composition.elements[0];
    assert_eq!(
        composition.effective_visibility_of(only),
        VisibilityMode::Presentation,
        "an over-claiming element is inert, never a leak"
    );
}

// load_composition for a commission nobody created is None, mirroring `find` —
// and distinct from a created-but-empty one, which is Some with tabs and no
// elements.
#[tokio::test]
async fn load_composition_answers_none_for_an_unknown_commission() {
    let backend = MemBackend::new();
    assert!(
        backend
            .commission_store()
            .load_composition(&CommissionId::from(uuid::Uuid::now_v7()))
            .await
            .unwrap()
            .is_none()
    );

    let created = commission("Empty", user_id());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let composition = backend
        .commission_store()
        .load_composition(&id)
        .await
        .unwrap()
        .expect("an existing commission composes to Some, however empty");
    assert!(composition.elements.is_empty(), "empty is not absent");
}

// ZMVP-85 (store layer) — the direction status sets, replaces, and clears
// through the unit of work; a dropped unit discards the staged change (the
// mem mirror of pg's drop = rollback).
#[tokio::test]
async fn direction_status_sets_replaces_and_clears_through_the_unit() {
    use domain::elements::commission::DirectionStatus;

    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Statused", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    let status_of = |backend: &MemBackend| {
        let backend = backend.clone();
        async move {
            backend
                .find_commission(id)
                .await
                .unwrap()
                .expect("exists")
                .direction_status
        }
    };
    assert_eq!(status_of(&backend).await, None, "born clear");

    // Set, then replace — one nullable cell, so the second set wins whole.
    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .set_direction_status(&id, Some(DirectionStatus::WaitingForInput))
        .await
        .unwrap();
    uow.commit().await.unwrap();
    assert_eq!(
        status_of(&backend).await,
        Some(DirectionStatus::WaitingForInput)
    );

    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .set_direction_status(&id, Some(DirectionStatus::ChangesRequested))
        .await
        .unwrap();
    uow.commit().await.unwrap();
    assert_eq!(
        status_of(&backend).await,
        Some(DirectionStatus::ChangesRequested),
        "a set replaces the current value"
    );

    // A dropped (uncommitted) unit discards its staged status write.
    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .set_direction_status(&id, None)
            .await
            .unwrap();
    }
    assert_eq!(
        status_of(&backend).await,
        Some(DirectionStatus::ChangesRequested),
        "a dropped unit rolls the clear back"
    );

    // Clear commits to NULL; an absent commission is a no-op, not an error.
    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .set_direction_status(&id, None)
        .await
        .unwrap();
    uow.commissions()
        .set_direction_status(
            &CommissionId::from(uuid::Uuid::now_v7()),
            Some(DirectionStatus::WaitingForApproval),
        )
        .await
        .unwrap();
    uow.commit().await.unwrap();
    assert_eq!(status_of(&backend).await, None, "cleared");
}

// ZMVP-86 (store layer) — the deadline and the MANUAL Delayed flag set and
// clear through the unit of work; a dropped unit discards the staged change
// (the mem mirror of pg's drop = rollback). Late is derived on lookup, never
// persisted, so it is exercised separately below.
#[tokio::test]
async fn deadline_and_status_set_and_clear_through_the_unit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Deadlined", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    // A FUTURE deadline, so the derived Late never masks the manual flag.
    let deadline = Utc::now() + chrono::Duration::days(30);
    let mut uow = database.begin().await.unwrap();
    {
        let mut commissions = uow.commissions();
        commissions.set_deadline(&id, Some(deadline)).await.unwrap();
        commissions
            .set_deadline_status(&id, Some(DeadlineStatus::Delayed))
            .await
            .unwrap();
    }
    uow.commit().await.unwrap();
    let found = backend.find_commission(id).await.unwrap().expect("exists");
    assert_eq!(found.deadline, Some(deadline));
    assert_eq!(
        found.deadline_status,
        Some(DeadlineStatus::Delayed),
        "the manual flag persists; a future deadline is not Late"
    );

    // A dropped (uncommitted) unit discards its staged writes.
    {
        let mut uow = database.begin().await.unwrap();
        let mut commissions = uow.commissions();
        commissions.set_deadline(&id, None).await.unwrap();
        commissions.set_deadline_status(&id, None).await.unwrap();
    }
    let found = backend.find_commission(id).await.unwrap().expect("exists");
    assert_eq!(found.deadline, Some(deadline), "the clear rolled back");
    assert_eq!(found.deadline_status, Some(DeadlineStatus::Delayed));

    // Clear commits; an absent commission is a no-op, not an error.
    let mut uow = database.begin().await.unwrap();
    {
        let mut commissions = uow.commissions();
        commissions.set_deadline(&id, None).await.unwrap();
        commissions.set_deadline_status(&id, None).await.unwrap();
        commissions
            .set_deadline(&CommissionId::from(uuid::Uuid::now_v7()), Some(deadline))
            .await
            .unwrap();
    }
    uow.commit().await.unwrap();
    let found = backend.find_commission(id).await.unwrap().expect("exists");
    assert_eq!(found.deadline, None);
    assert_eq!(
        found.deadline_status, None,
        "no deadline ⇒ no axis status (AC4)"
    );
}

// ZMVP-86 — Late is DERIVED on lookup from `deadline < now`, never persisted:
// a past deadline reads Late, and it supersedes a standing manual Delayed
// without overwriting it in storage.
#[tokio::test]
async fn late_is_derived_from_a_passed_deadline() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Slipping", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    let mut uow = database.begin().await.unwrap();
    {
        let mut commissions = uow.commissions();
        commissions
            .set_deadline(&id, Some(Utc::now() - chrono::Duration::days(1)))
            .await
            .unwrap();
        commissions
            .set_deadline_status(&id, Some(DeadlineStatus::Delayed))
            .await
            .unwrap();
    }
    uow.commit().await.unwrap();

    let found = backend.find_commission(id).await.unwrap().expect("exists");
    assert_eq!(
        found.deadline_status,
        Some(DeadlineStatus::Late),
        "a passed deadline derives Late, superseding the stored Delayed"
    );
}

// ZMVP-86 (store layer, ruling E12) — `lapsed_deadlines` returns exactly
// the sweepable set: past-deadline commissions that are not already Late
// and not in a terminal lifecycle, ordered by deadline; and it sees writes
// staged on the same open unit (the no-TOCTOU posture).
#[tokio::test]
async fn lapsed_deadlines_scans_exactly_the_sweepable_set() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let now = Utc::now();
    let past = |days: i64| now - chrono::Duration::days(days);

    let seed = |title: &str, deadline, step: Option<LifecycleStep>| {
        let mut c = Commission::create(
            title.parse::<CommissionTitle>().unwrap(),
            owner.clone(),
            now,
            deadline,
        );
        if let Some(step) = step {
            c.lifecycle_step = step;
        }
        c
    };
    let missed = seed("Missed", Some(past(30)), None);
    let slipping = seed("Slipping", Some(past(20)), None);
    let already_late = seed("Late", Some(past(10)), None);
    let future = seed("Future", Some(now + chrono::Duration::days(30)), None);
    let no_deadline = seed("No deadline", None, None);
    let completed = seed("Done", Some(past(30)), Some(LifecycleStep::Completed));
    let cancelled = seed("Dropped", Some(past(30)), Some(LifecycleStep::Cancelled));
    let disputed = seed("Contested", Some(past(5)), Some(LifecycleStep::Disputed));
    for c in [
        &missed,
        &slipping,
        &already_late,
        &future,
        &no_deadline,
        &completed,
        &cancelled,
        &disputed,
    ] {
        backend.create_commission(c).await.unwrap();
    }

    let mut uow = database.begin().await.unwrap();
    {
        uow.commissions()
            .set_deadline_status(&slipping.id, Some(DeadlineStatus::Delayed))
            .await
            .unwrap();
        // Late is deduped on the changelog (no persisted Late), staged on the
        // SAME unit: a commission already logged Late is skipped by the scan.
        uow.changelog()
            .append(&NewChangelogEntry::system(
                already_late.id,
                ChangelogEntryKind::Late,
                serde_json::json!({}),
                now,
            ))
            .await
            .unwrap();

        let lapsed = uow.commissions().lapsed_deadlines(now).await.unwrap();
        let ids: Vec<_> = lapsed.iter().map(|l| l.id).collect();
        assert_eq!(
            ids,
            vec![missed.id, slipping.id, disputed.id],
            "exactly the sweepable set, ordered by deadline"
        );
        assert_eq!(lapsed[0].status, None);
        assert_eq!(
            lapsed[1].status,
            Some(DeadlineStatus::Delayed),
            "the scan carries the standing flag"
        );
    }
}

// The owner-arm participant predicate and the linked-channel round-trip on
// the mem read store (ZMVP-87).
#[tokio::test]
async fn commission_store_answers_participant_and_channel_reads() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Mine", owner.clone());
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    let store = backend.commission_store();
    assert!(store.is_participant(&id, &owner).await.unwrap());
    assert!(!store.is_participant(&id, &user_id()).await.unwrap());
    assert!(
        !store
            .is_participant(&CommissionId::from(uuid::Uuid::now_v7()), &owner)
            .await
            .unwrap()
    );

    let pointer = "@artist on Telegram".parse::<ChannelPointer>().unwrap();
    let mut uow = database.begin().await.unwrap();
    assert!(
        uow.commissions()
            .set_linked_channel(&id, Some(&pointer))
            .await
            .unwrap(),
        "the first link is a real change"
    );
    assert!(
        !uow.commissions()
            .set_linked_channel(&id, Some(&pointer))
            .await
            .unwrap(),
        "re-linking the identical pointer answers false"
    );
    uow.commit().await.unwrap();
    assert_eq!(
        store
            .find(&id)
            .await
            .unwrap()
            .expect("exists")
            .linked_channel
            .map(|c| c.as_str().to_owned()),
        Some("@artist on Telegram".to_owned()),
    );

    let mut uow = database.begin().await.unwrap();
    assert!(
        uow.commissions()
            .set_linked_channel(&id, None)
            .await
            .unwrap(),
        "the clear is a real change"
    );
    assert!(
        !uow.commissions()
            .set_linked_channel(&id, None)
            .await
            .unwrap(),
        "clearing an already-clear channel answers false"
    );
    uow.commit().await.unwrap();
    assert!(
        store
            .find(&id)
            .await
            .unwrap()
            .expect("exists")
            .linked_channel
            .is_none(),
        "the pointer clears"
    );
}

// ZMVP-31 (store layer) — a fresh commission is unrated (the birth
// invariant); set_maturity round-trips every axis/graphic pairing and a
// later write REPLACES the posture (replace-only — no clear exists);
// the write is unit-of-work-scoped (a dropped unit rates nothing); an
// absent commission is a no-op, per the port contract.
#[tokio::test]
async fn set_maturity_round_trips_replaces_and_respects_the_unit() {
    use domain::elements::maturity::MaturityRating;
    use strum::VariantArray;

    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Rated", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    let unrated = backend.find_commission(id).await.unwrap().expect("exists");
    assert_eq!(unrated.maturity, None, "born unrated (the invariant)");

    for rating in MaturityRating::VARIANTS {
        for graphic in [true, false] {
            let posture = Maturity {
                rating: *rating,
                graphic,
            };
            let mut uow = database.begin().await.unwrap();
            uow.commissions().set_maturity(&id, posture).await.unwrap();
            uow.commit().await.unwrap();
            assert_eq!(
                backend
                    .find_commission(id)
                    .await
                    .unwrap()
                    .expect("exists")
                    .maturity,
                Some(posture),
                "each write replaces the whole posture",
            );
        }
    }

    // A dropped unit's write is discarded — the last committed posture holds.
    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .set_maturity(
                &id,
                Maturity {
                    rating: MaturityRating::Suggestive,
                    graphic: true,
                },
            )
            .await
            .unwrap();
        // `uow` drops here without `commit` → the staged write is discarded.
    }
    assert_eq!(
        backend
            .find_commission(id)
            .await
            .unwrap()
            .expect("exists")
            .maturity,
        Some(Maturity {
            rating: MaturityRating::Adult,
            graphic: false,
        }),
        "a dropped unit of work changes nothing — the loop's last committed posture holds",
    );

    // An absent commission is a no-op, not an error (existence is the
    // caller's check).
    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .set_maturity(
            &CommissionId::from(uuid::Uuid::now_v7()),
            Maturity {
                rating: MaturityRating::Adult,
                graphic: false,
            },
        )
        .await
        .unwrap();
    uow.commit().await.unwrap();
}

/// Seeds a committed commission and returns `(its id, its only address)` —
/// the placeholder skeleton declares exactly one tab holding exactly one
/// surface, so "the address" is unambiguous.
async fn composed_commission(
    backend: &MemBackend,
    owner: UserId,
) -> (CommissionId, SurfaceAddress) {
    let created = commission("Composed", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();
    let address = only_address(backend, id).await;
    (id, address)
}

/// The commission's one skeleton address (see [`composed_commission`]).
async fn only_address(backend: &MemBackend, commission: CommissionId) -> SurfaceAddress {
    let tab = backend.tabs_of(commission).await.unwrap()[0].id;
    SurfaceAddress::new(tab, only_surface())
}

/// The one surface the placeholder skeleton declares.
fn only_surface() -> SurfaceName {
    SKELETON[0].surfaces[0]
        .parse::<SurfaceName>()
        .expect("the skeleton declares valid labels")
}

/// An untyped element at `address` — the shape most of these tests only need
/// to exist, so its type tag and payload carry no meaning.
fn element_at(commission: CommissionId, address: SurfaceAddress, owner: UserId) -> NewElement {
    NewElement::contributed(
        commission,
        address,
        "note".parse::<ElementType>().unwrap(),
        ElementPayload::default(),
        owner,
        Utc::now(),
    )
}

/// The `(id, position)` pairs of one ordering group, in position order —
/// read straight off the shared element map, so a test can assert the
/// renumbering invariant (positions contiguous from 0).
fn group_positions(
    backend: &MemBackend,
    commission: CommissionId,
    address: &SurfaceAddress,
) -> Vec<(ElementId, i32)> {
    let elements = backend.elements.lock().expect("elements mutex");
    let mut pairs: Vec<(ElementId, i32)> = elements
        .iter()
        .filter(|(_, element)| element.commission_id == commission && element.address == *address)
        .map(|(id, element)| (*id, element.position))
        .collect();
    pairs.sort_by_key(|(_, position)| *position);
    pairs
}

/// Runs `remove_element` in its own committed unit of work.
async fn remove_element(
    database: &std::sync::Arc<dyn domain::ports::Database>,
    commission: CommissionId,
    element: ElementId,
) -> anyhow::Result<()> {
    let mut uow = database.begin().await?;
    uow.commissions()
        .remove_element(&commission, &element)
        .await?;
    uow.commit().await
}

// ZMVP-166 — removing an element takes exactly that element (there is no
// subtree to take: elements are leaves, always) and renumbers the remaining
// ordering group so positions stay contiguous from 0, in the same
// transaction.
#[tokio::test]
async fn remove_element_takes_only_it_and_renumbers_the_group() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let first = element_at(id, address.clone(), owner.clone());
    let doomed = element_at(id, address.clone(), owner.clone());
    let last = element_at(id, address.clone(), owner);
    let (first_id, doomed_id, last_id) = (first.id, doomed.id, last.id);
    let mut uow = database.begin().await.unwrap();
    for element in [&first, &doomed, &last] {
        uow.commissions().add_element(element).await.unwrap();
    }
    uow.commit().await.unwrap();
    assert_eq!(
        group_positions(&backend, id, &address),
        vec![(first_id, 0), (doomed_id, 1), (last_id, 2)],
    );

    remove_element(&database, id, doomed_id).await.unwrap();

    assert_eq!(
        group_positions(&backend, id, &address),
        vec![(first_id, 0), (last_id, 1)],
        "the survivors renumber contiguously from 0, order preserved"
    );
}

// ZMVP-166 — the target must exist in THIS commission: a fabricated element
// id and one belonging to another commission both fail with ElementNotFound
// (one indistinguishable answer — removal probes reveal nothing about other
// commissions), and neither removal lands.
#[tokio::test]
async fn remove_refuses_absent_and_foreign_elements() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (mine, my_address) = composed_commission(&backend, owner.clone()).await;
    let theirs = commission("Theirs", user_id());
    let theirs_id = theirs.id;
    backend.create_commission(&theirs).await.unwrap();
    let their_address = only_address(&backend, theirs_id).await;

    let ours = element_at(mine, my_address, owner);
    let theirs_element = element_at(theirs_id, their_address, user_id());
    let (our_id, their_id) = (ours.id, theirs_element.id);
    let mut uow = database.begin().await.unwrap();
    uow.commissions().add_element(&ours).await.unwrap();
    uow.commissions()
        .add_element(&theirs_element)
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let err = remove_element(&database, mine, ElementId::from(uuid::Uuid::now_v7()))
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<ElementNotFound>().is_some(),
        "an absent element surfaces as ElementNotFound, got: {err:?}"
    );

    let err = remove_element(&database, mine, their_id).await.unwrap_err();
    assert!(
        err.downcast_ref::<ElementNotFound>().is_some(),
        "a foreign element is indistinguishable from an absent one, got: {err:?}"
    );

    assert_eq!(
        backend.elements_of(mine).await.unwrap()[0].id,
        our_id,
        "our element survives"
    );
    assert_eq!(
        backend.elements_of(theirs_id).await.unwrap()[0].id,
        their_id,
        "and so, untouched, does theirs"
    );
}

// ZMVP-166 / ruling E35 — deleting the commission sweeps its WHOLE
// composition, so `load_composition` answers None afterwards exactly as pg
// does. Without this the fake would answer Some for a commission that is
// "gone entirely" — an adapter lie the api suites (which all run on the
// fake) would inherit.
#[tokio::test]
async fn deleting_the_commission_sweeps_its_whole_composition() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let element = element_at(id, address.clone(), owner.clone());
    let seat = NewSeat::contributed_at(
        id,
        address.clone(),
        "Creator".parse::<SeatKind>().unwrap(),
        None,
        None,
        owner.clone(),
        Utc::now(),
    );
    let seat_id = seat.id;
    let slot = NewSlot::contributed_at(
        id,
        address.clone(),
        "The knight".parse::<SlotTitle>().unwrap(),
        None,
        owner.clone(),
        Utc::now(),
    );
    let slot_id = slot.id;
    let invited = user_id();
    let invitation = SeatInvitation::issue(id, seat_id, invited.clone(), owner, Utc::now());
    let mut uow = database.begin().await.unwrap();
    uow.commissions().add_element(&element).await.unwrap();
    uow.commissions().declare_seat(&seat).await.unwrap();
    uow.commissions().declare_slots(&[slot]).await.unwrap();
    uow.commissions()
        .create_seat_invitation(&invitation)
        .await
        .unwrap();
    uow.commit().await.unwrap();
    backend.set_surface_mode(id, address.surface, VisibilityMode::Description);

    let mut uow = database.begin().await.unwrap();
    uow.commissions().delete(&id).await.unwrap();
    uow.commit().await.unwrap();

    let store = backend.commission_store();
    assert!(
        store.load_composition(&id).await.unwrap().is_none(),
        "a deleted commission composes to None, exactly as in pg"
    );
    assert!(store.seats(&id).await.unwrap().is_empty());
    assert!(backend.slots_of(id).await.unwrap().is_empty());
    assert!(backend.find_slot(slot_id).await.unwrap().is_none());
    assert!(
        store
            .find_pending_seat_invitation(&id, &seat_id, &invited)
            .await
            .unwrap()
            .is_none(),
        "a deleted commission's seat takes its pending offers with it"
    );
}

// ZMVP-166 (transactionality) — a staged removal is invisible until commit
// and discarded on drop, like every other unit-of-work write.
#[tokio::test]
async fn remove_commits_and_rolls_back_with_the_unit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let element = element_at(id, address, owner);
    let element_id = element.id;
    let mut uow = database.begin().await.unwrap();
    uow.commissions().add_element(&element).await.unwrap();
    uow.commit().await.unwrap();

    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .remove_element(&id, &element_id)
            .await
            .unwrap();
        assert_eq!(
            backend.elements_of(id).await.unwrap().len(),
            1,
            "an open unit's staged removal is invisible to a shared read"
        );
        // `uow` drops here without `commit` -> the removal is discarded.
    }

    assert_eq!(
        backend.elements_of(id).await.unwrap().len(),
        1,
        "a dropped unit of work removes nothing"
    );

    remove_element(&database, id, element_id).await.unwrap();
    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "a committed removal is visible"
    );
}

// ZMVP-76 (Engineer ruling B2, store layer) — creating a commission seats
// its owner as a PERSISTED participant in the same unit of work: the
// membership row exists (independent of the owner_id column), stamped with
// the commission's own creation instant, and is_participant reads it.
#[tokio::test]
async fn creating_a_commission_persists_its_owners_participant_row() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let created = commission("Membered", owner.clone());
    let id = created.id;
    let created_at = created.created_at;

    let mut uow = database.begin().await.unwrap();
    uow.commissions().create(&created).await.unwrap();
    uow.commit().await.unwrap();

    let participants = backend
        .participants
        .lock()
        .expect("participants mutex poisoned")
        .clone();
    assert_eq!(
        participants.get(&(id, owner.clone())),
        Some(&created_at),
        "the owner's membership row is born with the commission"
    );
    assert!(
        backend
            .commission_store()
            .is_participant(&id, &owner)
            .await
            .unwrap(),
        "the predicate reads the membership record"
    );
}

// ZMVP-76 — is_participant answers from the membership TABLE, not the
// owner_id column: a directly seeded membership row for a non-owner (the
// shape ZMVP-79's seated arm will write) already counts.
#[tokio::test]
async fn is_participant_reads_the_membership_record_not_the_owner_column() {
    let backend = MemBackend::new();
    let owner = user_id();
    let seated = user_id();
    let created = commission("Seated later", owner);
    let id = created.id;
    backend.create_commission(&created).await.unwrap();

    assert!(
        !backend
            .commission_store()
            .is_participant(&id, &seated)
            .await
            .unwrap(),
        "not a participant before any membership row exists"
    );
    backend
        .participants
        .lock()
        .expect("participants mutex poisoned")
        .insert((id, seated.clone()), Utc::now());
    assert!(
        backend
            .commission_store()
            .is_participant(&id, &seated)
            .await
            .unwrap(),
        "a membership row alone makes a participant (the ZMVP-79 seated arm's shape)"
    );
}

// ZMVP-76 AC1/AC2/AC3 (store layer) — declaring a seat contributes ONE
// element AND its interpreted satellite sharing the id, atomically in the
// unit: kind + requirements read back, the seat is born vacant, and kinds
// repeat freely across a commission's seats.
#[tokio::test]
async fn declare_seat_lands_an_element_and_its_satellite_together() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let first = NewSeat::contributed_at(
        id,
        address.clone(),
        "Creator".parse::<SeatKind>().unwrap(),
        Some("Two refs, please.".parse::<SeatPrompt>().unwrap()),
        Some("https://forms.example/apply".parse::<SeatLink>().unwrap()),
        owner.clone(),
        Utc::now(),
    );
    // A second seat of the SAME kind — kinds repeat freely (AC1).
    let second = NewSeat::contributed_at(
        id,
        address.clone(),
        "Creator".parse::<SeatKind>().unwrap(),
        None,
        None,
        owner,
        Utc::now(),
    );
    let (first_id, second_id) = (first.id, second.id);

    let mut uow = database.begin().await.unwrap();
    uow.commissions().declare_seat(&first).await.unwrap();
    uow.commissions().declare_seat(&second).await.unwrap();
    uow.commit().await.unwrap();

    // The composition half: two elements at the address, in append order.
    let elements = backend.elements_of(id).await.unwrap();
    assert_eq!(elements.len(), 2);
    assert_eq!(elements[0].id, first_id, "append order");
    assert_eq!(elements[1].id, second_id);
    assert!(
        elements
            .iter()
            .all(|element| element.element_type == ElementType::seat()),
        "a seat's element carries the seat type tag"
    );
    assert!(
        elements
            .iter()
            .all(|element| element.mode == VisibilityMode::Total),
        "a seat's element is born Total like any other"
    );

    // The interpreted half: the satellite rows, keyed by the same ids.
    let seats = backend.commission_store().seats(&id).await.unwrap();
    assert_eq!(seats.len(), 2);
    let first_seat = seats.iter().find(|s| s.id == first_id).expect("first");
    assert_eq!(first_seat.kind.as_str(), "Creator");
    assert_eq!(
        first_seat.prompt.as_ref().map(|p| p.as_str()),
        Some("Two refs, please.")
    );
    assert_eq!(
        first_seat.link.as_ref().map(|l| l.as_str()),
        Some("https://forms.example/apply")
    );
    assert!(first_seat.is_vacant(), "a seat is born vacant (AC3)");
    let second_seat = seats.iter().find(|s| s.id == second_id).expect("second");
    assert_eq!(
        second_seat.kind.as_str(),
        "Creator",
        "kinds repeat freely (AC1)"
    );
    assert!(second_seat.prompt.is_none());
    assert!(second_seat.link.is_none());
    assert!(second_seat.is_vacant());

    // An unknown commission simply has no seats.
    assert!(
        backend
            .commission_store()
            .seats(&CommissionId::from(uuid::Uuid::now_v7()))
            .await
            .unwrap()
            .is_empty()
    );
}

// ZMVP-76 (transactionality) — a dropped unit discards BOTH halves of a
// staged seat: neither the element nor the satellite survives, so a
// half-declared seat is unrepresentable.
#[tokio::test]
async fn a_dropped_unit_discards_both_halves_of_a_seat() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    {
        let seat = NewSeat::contributed_at(
            id,
            address,
            "Client".parse::<SeatKind>().unwrap(),
            None,
            None,
            owner,
            Utc::now(),
        );
        let mut uow = database.begin().await.unwrap();
        uow.commissions().declare_seat(&seat).await.unwrap();
        // drops without commit -> rollback
    }

    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "no element landed"
    );
    assert!(
        backend
            .commission_store()
            .seats(&id)
            .await
            .unwrap()
            .is_empty(),
        "no satellite landed"
    );
}

// ZMVP-76/166 — a seat walks the same address gate as every element write:
// an absent/foreign tab refuses with UnknownTab, an undeclared surface with
// UnknownSurface, and NEITHER half lands either time.
#[tokio::test]
async fn declare_seat_walks_the_shared_address_gate() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;
    let theirs = commission("Theirs", user_id());
    let theirs_id = theirs.id;
    backend.create_commission(&theirs).await.unwrap();
    let their_address = only_address(&backend, theirs_id).await;

    let seat_at = |address: SurfaceAddress| {
        NewSeat::contributed_at(
            id,
            address,
            "Creator".parse::<SeatKind>().unwrap(),
            None,
            None,
            owner.clone(),
            Utc::now(),
        )
    };

    // A tab id that exists nowhere.
    let fabricated = seat_at(SurfaceAddress::new(TabId::mint(), only_surface()));
    let mut uow = database.begin().await.unwrap();
    let err = uow
        .commissions()
        .declare_seat(&fabricated)
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "absent tab surfaces as UnknownTab, got: {err:?}"
    );
    drop(uow);

    // A real tab — belonging to someone else's commission.
    let cross = seat_at(their_address);
    let mut uow = database.begin().await.unwrap();
    let err = uow.commissions().declare_seat(&cross).await.unwrap_err();
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "a foreign tab is indistinguishable from an absent one, got: {err:?}"
    );
    drop(uow);

    // A surface the skeleton does not declare.
    let invented = seat_at(SurfaceAddress::new(
        address.tab,
        "invented".parse::<SurfaceName>().unwrap(),
    ));
    let mut uow = database.begin().await.unwrap();
    let err = uow.commissions().declare_seat(&invented).await.unwrap_err();
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "an undeclared surface surfaces as UnknownSurface, got: {err:?}"
    );
    drop(uow);

    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "no refused declaration left an element behind"
    );
    assert!(
        backend
            .commission_store()
            .seats(&id)
            .await
            .unwrap()
            .is_empty(),
        "nor a satellite"
    );
}

// ZMVP-166 — removing a seat's element sweeps its satellite AND its pending
// invitations, the mem mirror of the pg `ON DELETE CASCADE` chain off
// `commission_element (id)`. This is what keeps "one identity, two rows"
// honest through a removal.
#[tokio::test]
async fn removing_a_seats_element_sweeps_its_satellite_and_offers() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let seat = NewSeat::contributed_at(
        id,
        address,
        "Creator".parse::<SeatKind>().unwrap(),
        None,
        None,
        owner.clone(),
        Utc::now(),
    );
    let seat_id = seat.id;
    let invited = user_id();
    let invitation = SeatInvitation::issue(id, seat_id, invited.clone(), owner, Utc::now());
    let mut uow = database.begin().await.unwrap();
    uow.commissions().declare_seat(&seat).await.unwrap();
    uow.commissions()
        .create_seat_invitation(&invitation)
        .await
        .unwrap();
    uow.commit().await.unwrap();
    assert!(
        backend
            .commission_store()
            .find_pending_seat_invitation(&id, &seat_id, &invited)
            .await
            .unwrap()
            .is_some(),
        "the offer stands before the removal"
    );

    remove_element(&database, id, seat_id).await.unwrap();

    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "the element is gone"
    );
    assert!(
        backend
            .commission_store()
            .seats(&id)
            .await
            .unwrap()
            .is_empty(),
        "the satellite cascaded away with it"
    );
    assert!(
        backend
            .commission_store()
            .find_pending_seat_invitation(&id, &seat_id, &invited)
            .await
            .unwrap()
            .is_none(),
        "and so did the pending offer on that seat"
    );
}

// ZMVP-77 AC1 (store layer) — declaring Slots contributes one element per
// Slot AND its title/notes satellite sharing the id, all in one unit: the
// batch lands together, in request order, and nothing anywhere can name an
// occupant.
#[tokio::test]
async fn declare_slot_creates_an_element_with_its_satellite() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let knight = NewSlot::contributed_at(
        id,
        address.clone(),
        "The knight".parse::<SlotTitle>().unwrap(),
        Some("plate, not chain".to_string()),
        owner.clone(),
        Utc::now(),
    );
    let mage = NewSlot::contributed_at(
        id,
        address.clone(),
        "The mage".parse::<SlotTitle>().unwrap(),
        None,
        owner,
        Utc::now(),
    );
    let (knight_id, mage_id) = (knight.id, mage.id);

    let mut uow = database.begin().await.unwrap();
    uow.commissions()
        .declare_slots(&[knight, mage])
        .await
        .unwrap();
    uow.commit().await.unwrap();

    let elements = backend.elements_of(id).await.unwrap();
    assert_eq!(elements.len(), 2, "one element per Slot");
    assert_eq!(elements[0].id, knight_id, "request order is append order");
    assert_eq!(elements[1].id, mage_id);
    assert!(
        elements
            .iter()
            .all(|element| element.element_type == ElementType::slot()),
        "a Slot's element carries the slot type tag"
    );
    assert!(
        elements
            .iter()
            .all(|element| *element.payload.as_ref() == json!({})),
        "the carrying element's payload is empty — the substance is the satellite's"
    );

    let slots = backend.slots_of(id).await.unwrap();
    assert_eq!(slots.len(), 2, "zero or more Slots (AC2)");
    let knight = slots.iter().find(|s| s.element_id == knight_id).unwrap();
    assert_eq!(knight.title.as_str(), "The knight");
    assert_eq!(knight.notes.as_deref(), Some("plate, not chain"));
    let mage = slots.iter().find(|s| s.element_id == mage_id).unwrap();
    assert_eq!(mage.title.as_str(), "The mage");
    assert!(mage.notes.is_none(), "notes are optional");
}

// ZMVP-77/166 — Slots walk the same address gate, and the batch is
// ALL-OR-NOTHING: a refusal partway through leaves nothing behind, because
// the whole batch rides one unit of work.
#[tokio::test]
async fn declare_slot_refuses_bad_addresses_and_takes_the_batch_with_it() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let slot_at = |address: SurfaceAddress, title: &str| {
        NewSlot::contributed_at(
            id,
            address,
            title.parse::<SlotTitle>().unwrap(),
            None,
            owner.clone(),
            Utc::now(),
        )
    };

    // The first Slot is fine; the second names a tab that exists nowhere.
    let good = slot_at(address.clone(), "Fine");
    let bad = slot_at(SurfaceAddress::new(TabId::mint(), only_surface()), "Doomed");
    let mut uow = database.begin().await.unwrap();
    let err = uow
        .commissions()
        .declare_slots(&[good, bad])
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "absent tab surfaces as UnknownTab, got: {err:?}"
    );
    drop(uow);
    assert!(
        backend.elements_of(id).await.unwrap().is_empty(),
        "the EARLIER Slot of the aborted batch left nothing behind (all-or-nothing)"
    );
    assert!(backend.slots_of(id).await.unwrap().is_empty());

    // An undeclared surface refuses the same way.
    let invented = slot_at(
        SurfaceAddress::new(address.tab, "invented".parse::<SurfaceName>().unwrap()),
        "Invented",
    );
    let mut uow = database.begin().await.unwrap();
    let err = uow
        .commissions()
        .declare_slots(&[invented])
        .await
        .unwrap_err();
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "an undeclared surface surfaces as UnknownSurface, got: {err:?}"
    );
    drop(uow);
    assert!(backend.slots_of(id).await.unwrap().is_empty());
}

// ZMVP-77 (transactionality) — a dropped unit discards both halves of a
// staged Slot, and removing its element cascades the satellite away.
#[tokio::test]
async fn declare_slot_commits_and_rolls_back_with_the_unit() {
    let backend = MemBackend::new();
    let database = backend.database();
    let owner = user_id();
    let (id, address) = composed_commission(&backend, owner.clone()).await;

    let staged = NewSlot::contributed_at(
        id,
        address.clone(),
        "Uncommitted".parse::<SlotTitle>().unwrap(),
        None,
        owner.clone(),
        Utc::now(),
    );
    let staged_id = staged.id;
    {
        let mut uow = database.begin().await.unwrap();
        uow.commissions().declare_slots(&[staged]).await.unwrap();
        assert!(
            backend.find_slot(staged_id).await.unwrap().is_none(),
            "an open unit's staged Slot is invisible to a shared read"
        );
        // drops without commit -> rollback
    }
    assert!(backend.find_slot(staged_id).await.unwrap().is_none());
    assert!(backend.elements_of(id).await.unwrap().is_empty());

    // Committed, then removed: the satellite leaves with its element.
    let kept = NewSlot::contributed_at(
        id,
        address,
        "Kept".parse::<SlotTitle>().unwrap(),
        None,
        owner,
        Utc::now(),
    );
    let kept_id = kept.id;
    let mut uow = database.begin().await.unwrap();
    uow.commissions().declare_slots(&[kept]).await.unwrap();
    uow.commit().await.unwrap();
    assert!(backend.find_slot(kept_id).await.unwrap().is_some());

    remove_element(&database, id, kept_id).await.unwrap();
    assert!(
        backend.find_slot(kept_id).await.unwrap().is_none(),
        "the satellite cascaded away with its element"
    );
}
