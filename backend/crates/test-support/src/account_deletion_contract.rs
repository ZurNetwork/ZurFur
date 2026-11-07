//! The shared account-deletion conformance suite, run by both adapters: `adapter-mem`'s
//! `tests/account_deletion.rs` and `adapter-pg`'s `tests/it/account_deletion.rs`.
//!
//! Each case takes a fresh [`Runtime`] over one store and an adapter's [`CommissionSeeder`],
//! drives the account use cases as a driver would, and checks what the delete decides: soft
//! for an Account holding a fact, hard for an empty one, never hard over a deactivated one.
//! [`account_deletion_contract_tests!`](crate::account_deletion_contract_tests) writes one
//! test per case.

use chrono::Utc;
use domain::elements::{
    account::{AccountFactKind, AccountId},
    did::Did,
    level::Level,
    role::Role,
    user::UserId,
    workflow::{ColumnName, LexOrdering, WorkflowName},
};
use domain::ports::UnitOfWork;

use application::account::{self, delete::Output, invitation};

pub use application::account::{AccountEntity, AccountError, delete::DeleteOutcome};
pub use composition::Runtime;

use crate::seeder::{CommissionSeeder, seed_commission};

/// Writes one `#[tokio::test]` per suite case. `$setup` is an async fn returning
/// `(Runtime, Arc<dyn CommissionSeeder>, impl Sized)` over one fresh store; the third item is
/// kept alive for the test.
#[macro_export]
macro_rules! account_deletion_contract_tests {
    ($setup:path) => {
        $crate::account_deletion_contract_tests!(@cases $setup;
            an_account_with_a_member_deletes_soft,
            an_account_whose_member_left_deletes_soft,
            an_account_handed_over_deletes_soft,
            an_empty_account_deletes_hard_and_frees_its_handle,
            a_board_card_is_not_a_fact,
            a_declined_invitation_is_not_a_fact,
            accept_records_the_member_fact_once,
            a_deactivated_account_never_deletes_hard,
        );
    };
    (@cases $setup:path; $($case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                let (runtime, seeder, _kept) = $setup().await;
                $crate::account_deletion_contract::$case(&runtime, &*seeder).await;
            }
        )*
    };
}

/// Recognizes `did` as a User in a unit of its own.
pub async fn recognized(runtime: &Runtime, did: &str) -> UserId {
    let did = Did::from(did.to_owned());
    let user =
        application::transaction(&*runtime.database, async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&did).await
        })
        .await
        .expect("provision the user");
    user.id
}

/// Founds an Account holding `handle` with `owner` as its Owner.
pub async fn founded(runtime: &Runtime, owner: &UserId, handle: &str) -> AccountId {
    let handle_domain = "zurfur.app".parse().expect("a valid handle domain");
    let founding = account::create::Command {
        actor_id: owner.clone(),
        name: "Acme Studio".parse().expect("a valid name"),
        handle: handle.parse().expect("a valid handle"),
    };
    let founded = runtime
        .app()
        .accounts()
        .create(founding, &handle_domain, Utc::now())
        .await
        .expect("found the account");
    founded.account_id
}

/// `inviter` offers `invitee` a seat in `account` at `role`.
pub async fn offer(
    runtime: &Runtime,
    account: &AccountId,
    inviter: &UserId,
    invitee: &UserId,
    role: Role,
) {
    let issuing = invitation::issue::Command {
        account_id: account.clone(),
        target_id: invitee.clone(),
        actor_id: inviter.clone(),
        role,
    };
    runtime
        .app()
        .accounts()
        .invitations()
        .issue(issuing, Utc::now())
        .await
        .expect("issue the invitation");
}

/// `invitee` accepts their pending offer into `account`, answering what the use case answered.
pub async fn try_accept(
    runtime: &Runtime,
    account: &AccountId,
    invitee: &UserId,
) -> Result<(), AccountError> {
    let accepting = invitation::accept::Command {
        target_id: invitee.clone(),
        account_id: account.clone(),
        listed_on_profile: true,
    };
    runtime
        .app()
        .accounts()
        .invitations()
        .accept(accepting)
        .await
        .map(|_| ())
}

/// `invitee` accepts their pending offer into `account`, which must succeed.
pub async fn accept(runtime: &Runtime, account: &AccountId, invitee: &UserId) {
    try_accept(runtime, account, invitee)
        .await
        .expect("accept the invitation");
}

/// Seats `member` in `account` as a Member: `owner` offers, `member` accepts.
pub async fn seat(runtime: &Runtime, account: &AccountId, owner: &UserId, member: &UserId) {
    offer(runtime, account, owner, member, Role::Member).await;
    accept(runtime, account, member).await;
}

/// `actor` deletes `account`, answering what the use case answered.
pub async fn delete(
    runtime: &Runtime,
    account: &AccountId,
    actor: &UserId,
) -> Result<Output, AccountError> {
    let deleting = account::delete::Command {
        actor_id: actor.clone(),
        account_id: account.clone(),
    };
    runtime.app().accounts().delete(deleting).await
}

/// `actor` deletes `account`, which must succeed; answers its outcome.
pub async fn deleted(runtime: &Runtime, account: &AccountId, actor: &UserId) -> DeleteOutcome {
    delete(runtime, account, actor)
        .await
        .expect("delete the account")
        .outcome
}

/// Whether `account` is among the deactivated Accounts `owner` owns.
pub async fn is_deactivated_for(runtime: &Runtime, account: &AccountId, owner: &UserId) -> bool {
    runtime
        .accounts
        .list_deactivated_for_owner(owner)
        .await
        .expect("list the deactivated accounts")
        .iter()
        .any(|deactivated| &deactivated.id == account)
}

/// Whether the pool still finds `account` live.
pub async fn is_live(runtime: &Runtime, account: &AccountId) -> bool {
    runtime
        .accounts
        .find(account)
        .await
        .expect("find the account")
        .is_some()
}

/// Asserts `account` was deleted soft: absent from the pool's reads, kept as deactivated.
async fn assert_kept_deactivated(runtime: &Runtime, account: &AccountId, owner: &UserId) {
    assert!(!is_live(runtime, account).await, "reads treat it as absent");
    assert!(
        is_deactivated_for(runtime, account, owner).await,
        "its Owner still holds it, deactivated"
    );
}

// An Account with a member besides its founder holds a fact: its delete is soft.
pub async fn an_account_with_a_member_deletes_soft(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let owner = recognized(runtime, "did:plc:delsoftowner").await;
    let member = recognized(runtime, "did:plc:delsoftmember").await;
    let account = founded(runtime, &owner, "delsoft.zurfur.app").await;
    seat(runtime, &account, &owner, &member).await;

    let outcome = deleted(runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Soft), "{outcome}");
    assert_kept_deactivated(runtime, &account, &owner).await;
}

// The fact outlives the membership: a member who joined and left still keeps the delete soft.
pub async fn an_account_whose_member_left_deletes_soft(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let owner = recognized(runtime, "did:plc:delleftowner").await;
    let member = recognized(runtime, "did:plc:delleftmember").await;
    let account = founded(runtime, &owner, "delleft.zurfur.app").await;
    seat(runtime, &account, &owner, &member).await;
    let leaving = account::leave::Command {
        leaving_user_id: member,
        account_id: account.clone(),
    };
    runtime
        .app()
        .accounts()
        .leave(leaving)
        .await
        .expect("the member leaves");

    let outcome = deleted(runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Soft), "{outcome}");
    assert_kept_deactivated(runtime, &account, &owner).await;
}

// An Account handed over to a member, its founder since gone, holds one member and a fact:
// the new Owner's delete is soft.
pub async fn an_account_handed_over_deletes_soft(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let founder = recognized(runtime, "did:plc:delhandfounder").await;
    let heir = recognized(runtime, "did:plc:delhandheir").await;
    let account = founded(runtime, &founder, "delhand.zurfur.app").await;
    seat(runtime, &account, &founder, &heir).await;
    let handing_over = account::transfer_ownership::Command {
        account_id: account.clone(),
        actor_id: founder.clone(),
        target_id: heir.clone(),
    };
    runtime
        .app()
        .accounts()
        .transfer_ownership(handing_over)
        .await
        .expect("hand the account over");
    let leaving = account::leave::Command {
        leaving_user_id: founder,
        account_id: account.clone(),
    };
    runtime
        .app()
        .accounts()
        .leave(leaving)
        .await
        .expect("the founder leaves");

    let outcome = deleted(runtime, &account, &heir).await;

    assert!(matches!(outcome, DeleteOutcome::Soft), "{outcome}");
    assert_kept_deactivated(runtime, &account, &heir).await;
}

// An Account its founder alone ever held deletes hard: it is gone, and its handle founds again.
pub async fn an_empty_account_deletes_hard_and_frees_its_handle(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let owner = recognized(runtime, "did:plc:delhardowner").await;
    let account = founded(runtime, &owner, "delhard.zurfur.app").await;

    let outcome = deleted(runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Hard), "{outcome}");
    assert!(!is_live(runtime, &account).await, "the Account is gone");
    assert!(
        !is_deactivated_for(runtime, &account, &owner).await,
        "nothing of it is kept"
    );
    let refounded = founded(runtime, &owner, "delhard.zurfur.app").await;
    assert_ne!(refounded, account, "the freed handle founds a new Account");
}

// A board holding a commission's card is no fact: the Account still deletes hard.
pub async fn a_board_card_is_not_a_fact(runtime: &Runtime, seeder: &dyn CommissionSeeder) {
    let owner = recognized(runtime, "did:plc:delboardowner").await;
    let account = founded(runtime, &owner, "delboard.zurfur.app").await;
    let commission = seed_commission(
        &*runtime.database,
        seeder,
        &owner,
        Level::Private,
        "A made-up commission",
    )
    .await;
    let mut unit = runtime.database.begin().await.expect("begin");
    let board_name = "Queue".parse::<WorkflowName>().expect("a board name");
    let mut board = unit
        .workflows()
        .create(&board_name, &account)
        .await
        .expect("create the board");
    let column_name = "Open".parse::<ColumnName>().expect("a column name");
    let mut column = board.new_column(column_name, board.visibility);
    column.push(commission).expect("the column takes the card");
    board.insert(0, column.clone()).expect("the board is empty");
    unit.workflows()
        .set_indexes(&board)
        .await
        .expect("persist the column");
    unit.columns()
        .set_commissions(&column)
        .await
        .expect("place the card");
    unit.commit().await.expect("commit the board");

    let outcome = deleted(runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Hard), "{outcome}");
    assert!(!is_live(runtime, &account).await, "the Account is gone");
}

// An offer declined, and one revoked, seat no one and record no fact: the delete is hard.
pub async fn a_declined_invitation_is_not_a_fact(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let owner = recognized(runtime, "did:plc:deldeclineowner").await;
    let decliner = recognized(runtime, "did:plc:deldecliner").await;
    let revoked = recognized(runtime, "did:plc:delrevoked").await;
    let account = founded(runtime, &owner, "deldecline.zurfur.app").await;
    offer(runtime, &account, &owner, &decliner, Role::Member).await;
    offer(runtime, &account, &owner, &revoked, Role::Member).await;
    let declining = invitation::decline::Command {
        account_id: account.clone(),
        actor_id: decliner,
    };
    runtime
        .app()
        .accounts()
        .invitations()
        .decline(declining, Utc::now())
        .await
        .expect("decline the offer");
    let revoking = invitation::revoke::Command {
        account_id: account.clone(),
        actor_id: owner.clone(),
        target_id: revoked,
    };
    runtime
        .app()
        .accounts()
        .invitations()
        .revoke(revoking)
        .await
        .expect("revoke the offer");

    let outcome = deleted(runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Hard), "{outcome}");
}

// Every accept records the member fact; a second accept keeps the one fact, and fails nothing.
pub async fn accept_records_the_member_fact_once(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let owner = recognized(runtime, "did:plc:delonceowner").await;
    let first = recognized(runtime, "did:plc:deloncefirst").await;
    let second = recognized(runtime, "did:plc:deloncesecond").await;
    let account = founded(runtime, &owner, "delonce.zurfur.app").await;
    let before = facts_of(runtime, &account).await;
    seat(runtime, &account, &owner, &first).await;
    seat(runtime, &account, &owner, &second).await;

    let after = facts_of(runtime, &account).await;

    assert!(
        before.is_empty(),
        "a founded Account holds none: {before:?}"
    );
    assert_eq!(after, [AccountFactKind::Member]);
}

/// The kinds of fact `account` holds, read on a unit of their own.
async fn facts_of(runtime: &Runtime, account: &AccountId) -> Vec<AccountFactKind> {
    let mut unit = runtime.database.begin().await.expect("begin");
    let facts = unit
        .accounts()
        .facts(account)
        .await
        .expect("read the facts");
    unit.rollback().await.expect("end the read");
    facts.iter().collect()
}

// A deactivated Account never deletes hard: a second delete answers not found, and the store's
// hard delete refuses a deactivated row that holds no fact, keeping it.
pub async fn a_deactivated_account_never_deletes_hard(
    runtime: &Runtime,
    _seeder: &dyn CommissionSeeder,
) {
    let owner = recognized(runtime, "did:plc:delneverowner").await;
    let member = recognized(runtime, "did:plc:delnevermember").await;
    let with_fact = founded(runtime, &owner, "delneverfact.zurfur.app").await;
    seat(runtime, &with_fact, &owner, &member).await;
    let empty = founded(runtime, &owner, "delneverempty.zurfur.app").await;
    let first = deleted(runtime, &with_fact, &owner).await;
    let mut unit = runtime.database.begin().await.expect("begin");
    unit.accounts()
        .soft_delete(&empty)
        .await
        .expect("deactivate the empty Account");
    unit.commit().await.expect("commit the deactivation");

    let second = delete(runtime, &with_fact, &owner).await;
    let mut unit = runtime.database.begin().await.expect("begin");
    let hard = unit.accounts().hard_delete(&empty).await;
    unit.rollback().await.expect("end the refused unit");

    assert!(matches!(first, DeleteOutcome::Soft), "{first}");
    assert!(
        matches!(second, Err(AccountError::NotFound(AccountEntity::Account))),
        "a second delete answers not found"
    );
    assert!(hard.is_err(), "the store refuses a deactivated row");
    assert!(
        is_deactivated_for(runtime, &empty, &owner).await,
        "the deactivated row is kept"
    );
    assert_kept_deactivated(runtime, &with_fact, &owner).await;
}
