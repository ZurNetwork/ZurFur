//! The account board use cases (`account::workflow::**`) over the in-memory
//! fakes — the module the drivers call, exercised below the HTTP layer.
//!
//! Every test here pins a refusal the module shipped without: the delete
//! role gate, the liveness gate `account/NODE.md` makes mandatory, the
//! caller-named-account-vs-board reconciliation, and the order that keeps a
//! non-member from reading board state off an error.

use application::account::{self, AccountEntity, AccountError, workflow};
use application::transaction;
use chrono::Utc;
use composition::Runtime;
use domain::elements::{
    account::AccountId,
    commission::{Commission, CommissionId, CommissionTitle},
    did::Did,
    handle::HandleDomain,
    role::Role,
    user::UserId,
    user_account::UserAccount,
    workflow::{ColumnId, LexOrdering, WorkflowId},
};
use domain::ports::UnitOfWork;
use test_support::runtime::MemRuntime;

/// The configured Zurfur handle namespace, the way `Config::handle_domain`
/// hands it to the use case: already parsed at load.
fn handle_domain() -> HandleDomain {
    "zurfur.app".parse().expect("a valid handle domain")
}

/// A runtime whose acting DID is `did` — the boards below are founded on it.
fn fixture(did: &str) -> MemRuntime {
    let acting = Did::new(did.to_string());
    test_support::runtime::mem(&acting).build()
}

/// Recognize a DID as a User (the `provision` seed), answering its id.
async fn recognized(fixture: &MemRuntime, did: &str) -> UserId {
    let did = Did::new(did.to_string());
    fixture
        .backend
        .provision(&did)
        .await
        .expect("provisions the user")
        .id
}

/// Found an account under `handle`, seating `owner` as its Owner.
async fn found(runtime: &Runtime, owner: &UserId, handle: &str) -> AccountId {
    let command = account::create::Command {
        actor_id: owner.clone(),
        name: "Board Studio".parse().expect("a valid account name"),
        handle: handle.parse().expect("a valid handle"),
    };
    let app = runtime.app();
    app.accounts()
        .create(command, &handle_domain(), Utc::now())
        .await
        .expect("founds the account")
        .account_id
}

/// Seat `user` in `account_id` at `role` (test seed, no use case involved).
async fn seat(fixture: &MemRuntime, user: &UserId, account_id: &AccountId, role: Role) {
    let member = UserAccount {
        user_id: user.clone(),
        account_id: account_id.clone(),
        role,
        alias: None,
    };
    fixture
        .backend
        .grant_role(&member)
        .await
        .expect("seats the member");
}

/// Create one board on `account_id` as `actor`.
async fn board(
    runtime: &Runtime,
    actor: &UserId,
    account_id: &AccountId,
    name: &str,
) -> WorkflowId {
    let command = workflow::create::Command {
        actor_id: actor.clone(),
        account_id: account_id.clone(),
        workflow_name: name.parse().expect("a valid board name"),
    };
    let app = runtime.app();
    let accounts = app.accounts();
    accounts
        .workflows()
        .create(command)
        .await
        .expect("creates the board")
        .workflow
        .id
}

/// Add a column to `workflow_id` as `actor`, handing back the refusal so a
/// test can pin which door closed.
async fn add_column(
    runtime: &Runtime,
    actor: &UserId,
    workflow_id: &WorkflowId,
    name: &str,
    position: u8,
) -> Result<workflow::column::add::Output, AccountError> {
    let command = workflow::column::add::Command {
        actor_id: actor.clone(),
        workflow_id: workflow_id.clone(),
        column_name: name.parse().expect("a valid column name"),
        position,
    };
    let app = runtime.app();
    let accounts = app.accounts();
    let workflows = accounts.workflows();
    workflows.columns().add(command).await
}

/// Add `name` and answer the id it landed under.
async fn column_id(
    runtime: &Runtime,
    actor: &UserId,
    workflow_id: &WorkflowId,
    name: &str,
    position: u8,
) -> ColumnId {
    let added = add_column(runtime, actor, workflow_id, name, position)
        .await
        .expect("adds the column");
    added
        .columns
        .into_iter()
        .find(|column| column.name.as_str() == name)
        .expect("the added column is on the board")
        .id
}

/// Delete `workflow_id` as `actor`.
async fn delete_board(
    runtime: &Runtime,
    actor: &UserId,
    workflow_id: &WorkflowId,
) -> Result<workflow::delete::Output, AccountError> {
    let command = workflow::delete::Command {
        actor_id: actor.clone(),
        workflow_id: workflow_id.clone(),
    };
    let app = runtime.app();
    let accounts = app.accounts();
    accounts.workflows().delete(command).await
}

/// Rename a column; the board names its own account.
async fn rename_column(
    runtime: &Runtime,
    actor: &UserId,
    workflow_id: &WorkflowId,
    column_id: &ColumnId,
    name: &str,
) -> Result<workflow::column::rename::Output, AccountError> {
    let command = workflow::column::rename::Command {
        column_id: column_id.clone(),
        actor_id: actor.clone(),
        workflow_id: workflow_id.clone(),
        name: name.parse().expect("a valid column name"),
    };
    let app = runtime.app();
    let accounts = app.accounts();
    let workflows = accounts.workflows();
    workflows.columns().rename(command).await
}

/// Move a column; the board names its own account.
async fn reposition_column(
    runtime: &Runtime,
    actor: &UserId,
    workflow_id: &WorkflowId,
    column_id: &ColumnId,
    to_index: usize,
) -> Result<workflow::column::reposition::Output, AccountError> {
    let command = workflow::column::reposition::Command {
        actor_id: actor.clone(),
        column_id: column_id.clone(),
        workflow_id: workflow_id.clone(),
        to_index,
    };
    let app = runtime.app();
    let accounts = app.accounts();
    let workflows = accounts.workflows();
    workflows.columns().reposition(command).await
}

/// The board's column names, in stored board order.
async fn column_names(runtime: &Runtime, workflow_id: &WorkflowId) -> Vec<String> {
    runtime
        .workflows
        .find(workflow_id)
        .await
        .expect("reads the board")
        .expect("the board is stored")
        .iter()
        .map(|column| column.name.as_str().to_owned())
        .collect()
}

/// Tombstone the account, leaving its memberships in place — the state
/// `role_of` cannot see, which is why the liveness gate exists. (DD 23003138)
async fn tombstone(runtime: &Runtime, account_id: &AccountId) {
    let target = account_id.clone();
    application::transaction(&*runtime.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().soft_delete(&target).await
    })
    .await
    .expect("soft-deletes the account");
}

/// Remove `column_id` as `actor`.
async fn remove_column(
    runtime: &Runtime,
    actor: &UserId,
    column_id: &ColumnId,
) -> Result<workflow::column::remove::Output, AccountError> {
    let command = workflow::column::remove::Command {
        column_id: column_id.clone(),
        actor_id: actor.clone(),
    };
    let app = runtime.app();
    let accounts = app.accounts();
    accounts.workflows().columns().remove(command).await
}

/// Seed a commission owned by `owner`, directly (test seed, no use case
/// involved) — owning it is enough to make it immediately visible to
/// `insert_in_column`'s PUSH rail (DD 29130754), without standing up a
/// separate view grant.
async fn seed_commission(runtime: &Runtime, owner: &UserId, title: &str) -> CommissionId {
    let title: CommissionTitle = title.parse().expect("a valid commission title");
    let owner = owner.clone();
    transaction(&*runtime.database, async move |uow: &mut dyn UnitOfWork| {
        let commission = Commission::create(title, owner, Utc::now(), None);
        let id = commission.id;
        uow.commissions().create(&commission).await?;
        Ok(id)
    })
    .await
    .expect("seeds a commission")
}

// --- delete: the role gate (F1 — the predicate was inverted, so the two
// administrative roles were the only ones refused) ---

#[tokio::test]
async fn an_owner_may_delete_a_board() {
    let fixture = fixture("did:plc:board-owner-delete");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-owner-delete").await;
    let account_id = found(runtime, &owner, "owner-delete.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;

    delete_board(runtime, &owner, &workflow_id)
        .await
        .expect("an Owner deletes their account's board");

    let gone = runtime.workflows.find(&workflow_id).await.expect("reads");
    assert!(gone.is_none(), "the board must be gone after a delete");
}

#[tokio::test]
async fn an_admin_may_delete_a_board() {
    let fixture = fixture("did:plc:board-admin-delete");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-admin-delete").await;
    let admin = recognized(&fixture, "did:plc:board-admin-member").await;
    let account_id = found(runtime, &owner, "admin-delete.zurfur.app").await;
    seat(&fixture, &admin, &account_id, Role::Admin).await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;

    delete_board(runtime, &admin, &workflow_id)
        .await
        .expect("an Admin deletes their account's board");

    let gone = runtime.workflows.find(&workflow_id).await.expect("reads");
    assert!(gone.is_none(), "the board must be gone after a delete");
}

#[tokio::test]
async fn a_member_may_not_delete_a_board() {
    let fixture = fixture("did:plc:board-member-delete");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-member-delete").await;
    let member = recognized(&fixture, "did:plc:board-plain-member").await;
    let account_id = found(runtime, &owner, "member-delete.zurfur.app").await;
    seat(&fixture, &member, &account_id, Role::Member).await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;

    let Err(error) = delete_board(runtime, &member, &workflow_id).await else {
        panic!("a Member must not delete their account's board");
    };

    assert!(matches!(error, AccountError::IncorrectRole));
    let still_there = runtime.workflows.find(&workflow_id).await.expect("reads");
    assert!(
        still_there.is_some(),
        "a refused delete must leave the board standing"
    );
}

#[tokio::test]
async fn a_manager_may_not_delete_a_board() {
    let fixture = fixture("did:plc:board-manager-delete");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-manager-delete").await;
    let manager = recognized(&fixture, "did:plc:board-manager").await;
    let account_id = found(runtime, &owner, "manager-delete.zurfur.app").await;
    seat(&fixture, &manager, &account_id, Role::Manager).await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;

    let Err(error) = delete_board(runtime, &manager, &workflow_id).await else {
        panic!("a Manager must not delete their account's board");
    };

    assert!(matches!(error, AccountError::IncorrectRole));
}

// --- reposition: the success path (F2 — it ended in `todo!()`, so every
// successful move panicked AFTER the transaction had already committed) ---

#[tokio::test]
async fn repositioning_a_column_succeeds_and_the_new_order_persists() {
    let fixture = fixture("did:plc:board-reposition");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-reposition").await;
    let account_id = found(runtime, &owner, "reposition.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;
    let first = column_id(runtime, &owner, &workflow_id, "Sketching", 0).await;
    column_id(runtime, &owner, &workflow_id, "Lining", 1).await;
    column_id(runtime, &owner, &workflow_id, "Colouring", 2).await;
    assert_eq!(
        column_names(runtime, &workflow_id).await,
        ["Sketching", "Lining", "Colouring"]
    );

    let moved = reposition_column(runtime, &owner, &workflow_id, &first, 2)
        .await
        .expect("an Owner reorders their own board");

    assert_eq!(moved, workflow::column::reposition::Output);
    assert_eq!(
        column_names(runtime, &workflow_id).await,
        ["Lining", "Sketching", "Colouring"],
        "the moved column must land in front of the one it was sent to"
    );
}

// --- rename / reposition: standing is settled against the account the BOARD
// names, never one the caller supplies (F4 — the Command carried an
// `account_id` that was authorized against and then never reconciled with the
// board, so an Owner of A could reshape B's board; the field is gone) ---

/// Two accounts with a board each, owned by separate Users: `(owner of A,
/// owner of B, B's board, B's first column)`.
async fn two_accounts(fixture: &MemRuntime, slug: &str) -> (UserId, UserId, WorkflowId, ColumnId) {
    let runtime = &fixture.runtime;
    let mine = recognized(fixture, &format!("did:plc:{slug}-mine")).await;
    let theirs = recognized(fixture, &format!("did:plc:{slug}-theirs")).await;
    let _my_account = found(runtime, &mine, &format!("{slug}-mine.zurfur.app")).await;
    let their_account = found(runtime, &theirs, &format!("{slug}-theirs.zurfur.app")).await;
    let their_board = board(runtime, &theirs, &their_account, "Their Queue").await;
    let their_column = column_id(runtime, &theirs, &their_board, "Sketching", 0).await;
    (mine, theirs, their_board, their_column)
}

#[tokio::test]
async fn an_owner_of_one_account_may_not_rename_another_accounts_column() {
    let fixture = fixture("did:plc:cross-rename");
    let runtime = &fixture.runtime;
    let (mine, _theirs, their_board, their_column) = two_accounts(&fixture, "cross-rename").await;

    let Err(error) = rename_column(runtime, &mine, &their_board, &their_column, "Mine Now").await
    else {
        panic!("an Owner of one account must not rename another account's column");
    };

    // Standing is settled against the board's OWN account, where this Owner
    // holds nothing.
    assert!(matches!(error, AccountError::IncorrectRole), "{error:?}");
    assert_eq!(
        column_names(runtime, &their_board).await,
        ["Sketching"],
        "a refused rename must leave the other account's board untouched"
    );
}

#[tokio::test]
async fn an_owner_of_one_account_may_not_reposition_another_accounts_column() {
    let fixture = fixture("did:plc:cross-move");
    let runtime = &fixture.runtime;
    let (mine, theirs, their_board, their_column) = two_accounts(&fixture, "cross-move").await;
    column_id(runtime, &theirs, &their_board, "Lining", 1).await;

    let Err(error) = reposition_column(runtime, &mine, &their_board, &their_column, 1).await else {
        panic!("an Owner of one account must not reorder another account's board");
    };

    assert!(matches!(error, AccountError::IncorrectRole), "{error:?}");
    assert_eq!(
        column_names(runtime, &their_board).await,
        ["Sketching", "Lining"],
        "a refused reposition must leave the other account's order untouched"
    );
}

// --- the liveness gate (F3 — `role_of` carries no tombstone predicate, so a
// soft-deleted account's surviving memberships still authorized writes) ---

#[tokio::test]
async fn a_soft_deleted_accounts_board_refuses_a_write_as_account_not_found() {
    let fixture = fixture("did:plc:board-tombstoned");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-tombstoned").await;
    let account_id = found(runtime, &owner, "tombstoned.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;
    tombstone(runtime, &account_id).await;
    // The membership survives the tombstone — the whole point of the gate.
    let standing = fixture
        .backend
        .role_of(&owner, &account_id)
        .await
        .expect("reads the membership");
    assert!(matches!(standing, Some(Role::Owner)));

    let Err(error) = delete_board(runtime, &owner, &workflow_id).await else {
        panic!("a soft-deleted account must authorize nothing");
    };

    assert!(matches!(
        error,
        AccountError::NotFound(AccountEntity::Account)
    ));
    let still_there = runtime.workflows.find(&workflow_id).await.expect("reads");
    assert!(
        still_there.is_some(),
        "the refused delete must not have run"
    );
}

#[tokio::test]
async fn a_soft_deleted_account_refuses_a_new_column_as_account_not_found() {
    let fixture = fixture("did:plc:column-tombstoned");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:column-tombstoned").await;
    let account_id = found(runtime, &owner, "tombstoned-column.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;
    tombstone(runtime, &account_id).await;

    let Err(error) = add_column(runtime, &owner, &workflow_id, "Sketching", 0).await else {
        panic!("a soft-deleted account must authorize nothing");
    };

    assert!(matches!(
        error,
        AccountError::NotFound(AccountEntity::Account)
    ));
    assert!(
        column_names(runtime, &workflow_id).await.is_empty(),
        "the refused add must not have run"
    );
}

// --- add: standing before board state (F5 — the duplicate-name and capacity
// checks ran first, so a non-member could tell an existing board and the names
// on it from an absent one) ---

#[tokio::test]
async fn a_non_member_adding_a_duplicate_column_name_is_refused_on_their_role() {
    let fixture = fixture("did:plc:board-outsider");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-outsider").await;
    let outsider = recognized(&fixture, "did:plc:board-stranger").await;
    let account_id = found(runtime, &owner, "outsider.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;
    column_id(runtime, &owner, &workflow_id, "Sketching", 0).await;

    let Err(error) = add_column(runtime, &outsider, &workflow_id, "Sketching", 0).await else {
        panic!("a non-member must not add a column");
    };

    // `DuplicateName` here would answer "is there already a 'Sketching'
    // column on this board?" for anyone who guessed the board's id.
    assert!(
        matches!(error, AccountError::IncorrectRole),
        "a non-member must be refused on standing, never on board state: {error:?}"
    );
}

#[tokio::test]
async fn an_owner_adding_a_duplicate_column_name_still_gets_the_duplicate_refusal() {
    let fixture = fixture("did:plc:board-duplicate");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-duplicate").await;
    let account_id = found(runtime, &owner, "duplicate.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;
    column_id(runtime, &owner, &workflow_id, "Sketching", 0).await;

    let Err(error) = add_column(runtime, &owner, &workflow_id, "Sketching", 1).await else {
        panic!("a board holds one column per name");
    };

    // The reorder must not have cost an authorized caller their real answer.
    assert!(matches!(error, AccountError::DuplicateName));
}

// --- R2: `owning_account_of` answers absence with `None`, never `Err` — a
// bogus id a client invented is a `NotFound`, not an infrastructure failure ---

#[tokio::test]
async fn deleting_a_board_that_does_not_exist_is_refused_as_not_found() {
    let fixture = fixture("did:plc:board-delete-missing");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-delete-missing").await;
    let workflow_id = WorkflowId::from(uuid::Uuid::now_v7());

    let Err(error) = delete_board(runtime, &owner, &workflow_id).await else {
        panic!("a board that was never created must not be deletable");
    };

    assert!(matches!(
        error,
        AccountError::NotFound(AccountEntity::Workflow)
    ));
}

#[tokio::test]
async fn removing_a_column_that_does_not_exist_is_refused_as_not_found() {
    let fixture = fixture("did:plc:column-remove-missing");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:column-remove-missing").await;
    let column_id = ColumnId::from(uuid::Uuid::now_v7());

    let Err(error) = remove_column(runtime, &owner, &column_id).await else {
        panic!("a column that was never created must not be removable");
    };

    assert!(matches!(
        error,
        AccountError::NotFound(AccountEntity::Column)
    ));
}

// --- R1: `WorkflowError::IndexOutOfRange` surfaces as a typed client error,
// never `Infrastructure` (the account-side catch-alls used to swallow it) ---

#[tokio::test]
async fn adding_a_column_past_the_end_of_the_board_is_refused_as_index_out_of_range() {
    let fixture = fixture("did:plc:board-add-out-of-range");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:board-add-out-of-range").await;
    let account_id = found(runtime, &owner, "add-out-of-range.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;

    // The board is empty: index 1 is past its only valid insertion point, 0.
    let Err(error) = add_column(runtime, &owner, &workflow_id, "Sketching", 1).await else {
        panic!("an empty board has no first column to insert in front of");
    };

    assert!(
        matches!(error, AccountError::IndexOutOfRange(_)),
        "{error:?}"
    );
}

#[tokio::test]
async fn inserting_a_commission_past_the_end_of_a_column_is_refused_as_index_out_of_range() {
    let fixture = fixture("did:plc:card-add-out-of-range");
    let runtime = &fixture.runtime;
    let owner = recognized(&fixture, "did:plc:card-add-out-of-range").await;
    let account_id = found(runtime, &owner, "card-out-of-range.zurfur.app").await;
    let workflow_id = board(runtime, &owner, &account_id, "Queue").await;
    let sketching_column_id = column_id(runtime, &owner, &workflow_id, "Sketching", 0).await;
    // Owning the commission is enough to make it immediately visible to the
    // PUSH rail (DD 29130754), without standing up a separate view grant.
    let commission_id = seed_commission(runtime, &owner, "A Wolf in Moonlight").await;

    let command = workflow::column::commission::set_in_column::Command {
        commission_id,
        column_id: sketching_column_id,
        index: 1,
        actor_id: owner.clone(),
    };
    let Err(error) = runtime.app().commissions().insert_in_column(command).await else {
        panic!("an empty column has no first card to insert in front of");
    };

    assert!(
        matches!(error, AccountError::IndexOutOfRange(_)),
        "{error:?}"
    );
}
