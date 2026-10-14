//! Identity's reads behind den's access check, on the in-memory fixture world and on the
//! PostgreSQL den world: `GET /accounts` keeps only the Accounts the check opens, in order and
//! unchanged; `GET /me` asks the check about the viewer's own root first, so a missing user row
//! is the `401` and a store failure in the check is the house `500`. PostgreSQL needs a
//! container runtime socket.

use std::sync::{Arc, OnceLock};

use adapter_mem::fixture::{alice, bob, seed_identity};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountMembership, AccountName, ListingScope},
        did::Did,
        handle::Handle,
        profile::Profile,
        role::Role,
        user_account::UserAccount,
    },
    ports::{AccountStore, UnitOfWork},
};
use serde_json::{Value, json};
use test_support::{
    den::{Answer, get},
    http::{client, serve, sign_in},
    pg_app::{self, DenWorld},
    store_doubles::{
        AccountRead, AccountsScript, ScriptedAccounts, ScriptedUsers, UserAnswer, UserRead,
        UsersScript,
    },
};

mod common;
use common::assert_problem;

/// The DID the in-memory stack seeds "Closed Studio" under; PostgreSQL mints its own.
const CLOSED_STUDIO_DID: &str = "did:plc:c1osedstudiofixtureaaaaa";

/// One stack served for alice, its identity reads behind scripted doubles.
struct Stack {
    /// The served app's base URL.
    base: String,
    /// A client signed in as alice.
    alice: reqwest::Client,
    /// Scripts the served app's Account store.
    accounts: AccountsScript,
    /// Scripts the served app's user store.
    users: UsersScript,
    /// The served app's own Account store, under the double.
    real_accounts: Arc<dyn AccountStore>,
    /// A deactivated Account alice is a Member of and doesn't own: no real listing holds it
    /// and den's check never opens it.
    refused: Arc<Account>,
    /// The PostgreSQL den world, kept alive with its database.
    _pg: Option<DenWorld>,
}

/// `runtime` with its Account and user stores behind doubles scripted by `accounts` and
/// `users`; the real Account store is kept in `real_accounts`.
fn scripted(
    mut runtime: composition::Runtime,
    accounts: &AccountsScript,
    users: &UsersScript,
    real_accounts: &OnceLock<Arc<dyn AccountStore>>,
) -> composition::Runtime {
    let _ = real_accounts.set(Arc::clone(&runtime.accounts));
    let scripted_accounts = ScriptedAccounts::new(Arc::clone(&runtime.accounts), accounts.clone());
    let scripted_users = ScriptedUsers::new(Arc::clone(&runtime.users), users.clone());
    runtime.accounts = Arc::new(scripted_accounts);
    runtime.users = Arc::new(scripted_users);
    runtime
}

/// `POST /accounts` as `owner`: founds `name` under `<label>.zurfur.app`, returning its id.
async fn found_account(owner: &reqwest::Client, base: &str, name: &str, label: &str) -> AccountId {
    let founding = json!({ "name": name, "handle": format!("{label}.zurfur.app") });
    let response = owner
        .post(format!("{base}/accounts"))
        .json(&founding)
        .send()
        .await
        .expect("POST /accounts is answered");
    assert_eq!(response.status(), 201, "founding {name}");
    let body: Value = response.json().await.expect("a JSON body");
    let did = body["did"].as_str().expect("the founded Account's DID");
    AccountId::from(Did::from(did.to_owned()))
}

/// alice's Member role on `account`.
fn alice_member_of(account: &AccountId) -> UserAccount {
    UserAccount {
        user_id: alice(),
        account_id: account.clone(),
        role: Role::Member,
        alias: None,
    }
}

/// A copy of `account`; an Account is not `Clone`.
fn copy_of(account: &Account) -> Account {
    Account {
        id: account.id.clone(),
        handle: account.handle.clone(),
        name: account.name.clone(),
        created_at: account.created_at,
        updated_at: account.updated_at,
        deleted_at: account.deleted_at,
    }
}

/// The membership a stale listing would hold for alice on `account`.
fn membership_of_alice(account: &Account) -> AccountMembership {
    AccountMembership {
        account: copy_of(account),
        role: Role::Member,
        alias: None,
    }
}

/// The fixture world served on the in-memory stores, alice signed in, with "Supreme Arts"
/// and a second Account of hers, and "Closed Studio" deactivated with alice a Member.
async fn mem_stack() -> Stack {
    let accounts = AccountsScript::default();
    let users = UsersScript::default();
    let real_accounts = Arc::new(OnceLock::new());
    let alice_profile =
        Profile::new(alice().did().clone(), "alice.test").with_display_name("Alice");
    let builder = test_support::runtime::mem(alice().did()).profile(alice_profile);
    let (script_accounts, script_users, slot) =
        (accounts.clone(), users.clone(), Arc::clone(&real_accounts));
    let served = serve(builder, move |runtime| {
        api::app(scripted(runtime, &script_accounts, &script_users, &slot))
    })
    .await;
    seed_identity(&served.backend)
        .await
        .expect("seeds identity's fixtures");
    let alice_client = client();
    sign_in(&alice_client, &served.base_url, "alice.test").await;
    found_account(&alice_client, &served.base_url, "Night Owls", "night-owls").await;
    let closed_studio = seed_closed_studio(&served.backend).await;
    Stack {
        base: served.base_url,
        alice: alice_client,
        accounts,
        users,
        real_accounts: Arc::clone(real_accounts.get().expect("the app was built")),
        refused: Arc::new(closed_studio),
        _pg: None,
    }
}

/// Seeds "Closed Studio", bob its Owner and alice a Member, and deactivates it.
async fn seed_closed_studio(backend: &adapter_mem::MemBackend) -> Account {
    let closed_studio = AccountId::from(Did::from(CLOSED_STUDIO_DID.to_owned()));
    let handle = "closed-studio.zurfur.app"
        .parse::<Handle>()
        .expect("a valid handle");
    let name = "Closed Studio"
        .parse::<AccountName>()
        .expect("a valid name");
    let (account, ownership) =
        Account::open(bob(), closed_studio.did().clone(), handle, name, Utc::now());
    let member = alice_member_of(&closed_studio);
    let seeded = copy_of(&account);
    let database = backend.database();
    application::transaction(&*database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create(&account, &ownership).await?;
        uow.accounts().grant_role(&member).await?;
        uow.accounts().soft_delete(&closed_studio).await
    })
    .await
    .expect("seeds a deactivated Account");
    seeded
}

/// The den world served on PostgreSQL, with a second Account of alice's, and bob's "Closed
/// Studio" deactivated with alice a Member.
async fn pg_stack() -> Stack {
    let accounts = AccountsScript::default();
    let users = UsersScript::default();
    let real_accounts = Arc::new(OnceLock::new());
    let (script_accounts, script_users, slot) =
        (accounts.clone(), users.clone(), Arc::clone(&real_accounts));
    let world = pg_app::den_world(move |runtime| {
        api::app(scripted(runtime, &script_accounts, &script_users, &slot))
    })
    .await;
    let base = world.base_url().to_owned();
    found_account(&world.alice, &base, "Night Owls", "night-owls").await;
    let closed_studio = found_account(&world.bob, &base, "Closed Studio", "closed-studio").await;
    let refused = deactivate_with_alice_a_member(&world, closed_studio).await;
    Stack {
        base,
        alice: world.alice.clone(),
        accounts,
        users,
        real_accounts: Arc::clone(real_accounts.get().expect("the app was built")),
        refused: Arc::new(refused),
        _pg: Some(world),
    }
}

/// Makes alice a Member of `account` and deactivates it, on the world's own stores; the
/// Account as it stood before.
async fn deactivate_with_alice_a_member(world: &DenWorld, account: AccountId) -> Account {
    let runtime = &world.served.runtime;
    let live = runtime
        .accounts
        .find(&account)
        .await
        .expect("the Account store answers")
        .expect("the founded Account is live");
    let member = alice_member_of(&account);
    runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.accounts().grant_role(&member).await?;
            uow.accounts().soft_delete(&account).await
        })
        .await
        .expect("alice joins, then the Account is deactivated");
    live
}

/// `GET {path}` as alice, read whole.
async fn alice_reads(stack: &Stack, path: &str) -> Answer {
    let response = get(&stack.alice, &stack.base, path).await;
    Answer::read(response).await
}

/// alice's `GET /accounts` body.
async fn alice_s_accounts(stack: &Stack) -> Value {
    let response = get(&stack.alice, &stack.base, "/accounts").await;
    assert_eq!(response.status(), 200, "GET /accounts");
    response.json().await.expect("a JSON body")
}

/// The handles a `GET /accounts` body lists, in order.
fn listed_handles(listing: &Value) -> Vec<String> {
    let accounts = listing["accounts"].as_array().expect("an accounts array");
    accounts
        .iter()
        .map(|account| account["handle"].as_str().expect("a handle").to_owned())
        .collect()
}

async fn assert_an_unopened_account_is_left_out(stack: Stack) {
    let before = alice_s_accounts(&stack).await;
    let real_listing = stack
        .real_accounts
        .list_for_user(&alice(), ListingScope::SelfView)
        .await
        .expect("the real store lists");
    let refused = Arc::clone(&stack.refused);
    stack
        .accounts
        .also_list(move || membership_of_alice(&refused));
    let stale_listing =
        ScriptedAccounts::new(Arc::clone(&stack.real_accounts), stack.accounts.clone());
    let listed = stale_listing
        .list_for_user(&alice(), ListingScope::SelfView)
        .await
        .expect("the double lists");

    let after = alice_s_accounts(&stack).await;

    let real_order: Vec<String> = real_listing
        .iter()
        .map(|membership| membership.account.handle.to_string())
        .collect();
    let mut live_handles = real_order.clone();
    live_handles.sort();
    let expected_live = ["night-owls.zurfur.app", "supreme-arts.zurfur.app"];
    assert_eq!(live_handles, expected_live, "alice's two live Accounts");
    let listed_ids: Vec<AccountId> = listed
        .iter()
        .map(|membership| membership.account.id.clone())
        .collect();
    assert_eq!(
        listed_ids.len(),
        3,
        "the store double lists the refused Account too"
    );
    assert_eq!(listed_ids[1], stack.refused.id, "between the two live ones");
    assert_eq!(
        listed_handles(&after),
        real_order,
        "the opened Accounts keep the real store's order"
    );
    assert_eq!(
        after, before,
        "the refused Account leaves no trace; the rest are unchanged"
    );
}

#[tokio::test]
async fn an_account_the_check_does_not_open_is_left_out_of_the_list_in_memory() {
    assert_an_unopened_account_is_left_out(mem_stack().await).await;
}

#[tokio::test]
async fn an_account_the_check_does_not_open_is_left_out_of_the_list_on_postgres() {
    assert_an_unopened_account_is_left_out(pg_stack().await).await;
}

async fn assert_me_answers_as_before(stack: Stack) {
    let response = get(&stack.alice, &stack.base, "/me").await;

    assert_eq!(response.status(), 200, "GET /me");
    let body: Value = response.json().await.expect("a JSON body");
    let expected = json!({
        "did": alice().to_string(),
        "handle": "alice.test",
        "displayName": "Alice",
    });
    assert_eq!(body, expected);
}

#[tokio::test]
async fn me_answers_as_before_for_a_live_user_in_memory() {
    assert_me_answers_as_before(mem_stack().await).await;
}

#[tokio::test]
async fn me_answers_as_before_for_a_live_user_on_postgres() {
    assert_me_answers_as_before(pg_stack().await).await;
}

async fn assert_a_gone_root_is_401(stack: Stack) {
    stack.users.answer(UserRead::Find, UserAnswer::Gone);

    let response = get(&stack.alice, &stack.base, "/me").await;

    assert_problem(response, 401, "not_authenticated").await;
}

#[tokio::test]
async fn me_is_401_through_the_check_when_the_root_s_user_row_is_gone_in_memory() {
    assert_a_gone_root_is_401(mem_stack().await).await;
}

#[tokio::test]
async fn me_is_401_through_the_check_when_the_root_s_user_row_is_gone_on_postgres() {
    assert_a_gone_root_is_401(pg_stack().await).await;
}

#[tokio::test]
async fn me_for_a_session_whose_user_row_is_gone_is_401_in_memory() {
    let stack = mem_stack().await;
    stack.users.answer(UserRead::Find, UserAnswer::Gone);
    stack.users.answer(UserRead::FindByDid, UserAnswer::Gone);

    let response = get(&stack.alice, &stack.base, "/me").await;

    assert_problem(response, 401, "not_authenticated").await;
}

#[tokio::test]
async fn me_for_a_session_whose_user_row_is_deleted_is_401_on_postgres() {
    let world = pg_app::den_world(api::app).await;
    let me = get(&world.stranger, world.base_url(), "/me").await;
    let me: Value = me.json().await.expect("the stranger's /me");
    let stranger = me["did"].as_str().expect("the stranger's DID").to_owned();
    let deleted = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(&stranger)
        .execute(&world.served.runtime.pool)
        .await
        .expect("the stranger's user row is deleted");
    assert_eq!(deleted.rows_affected(), 1, "one user row is deleted");

    let response = get(&world.stranger, world.base_url(), "/me").await;

    assert_problem(response, 401, "not_authenticated").await;
}

/// Asserts `/me` asks the check before its own user read. The script is a state no real
/// store reaches (the check's read finds no row, the read by DID fails), used only to tell the
/// two orders apart: check first answers `401`, the read first `500`.
async fn assert_me_asks_the_check_first(stack: Stack) {
    stack.users.answer(UserRead::Find, UserAnswer::Gone);
    stack.users.answer(UserRead::FindByDid, UserAnswer::Fails);

    let response = get(&stack.alice, &stack.base, "/me").await;

    assert_problem(response, 401, "not_authenticated").await;
}

#[tokio::test]
async fn me_asks_the_check_before_reading_the_user_in_memory() {
    assert_me_asks_the_check_first(mem_stack().await).await;
}

#[tokio::test]
async fn me_asks_the_check_before_reading_the_user_on_postgres() {
    assert_me_asks_the_check_first(pg_stack().await).await;
}

async fn assert_a_failing_check_fails_the_list_as_today(stack: Stack) {
    stack.accounts.fail(AccountRead::ListForUser);
    let today = alice_reads(&stack, "/accounts").await;
    stack.accounts.read_through(AccountRead::ListForUser);
    stack.accounts.fail(AccountRead::Find);

    let through_the_check = alice_reads(&stack, "/accounts").await;

    assert_eq!(today.status, 500, "a failing listing is the house 500");
    assert_eq!(through_the_check, today);
}

#[tokio::test]
async fn a_store_failure_in_the_check_fails_the_accounts_list_as_today_in_memory() {
    assert_a_failing_check_fails_the_list_as_today(mem_stack().await).await;
}

#[tokio::test]
async fn a_store_failure_in_the_check_fails_the_accounts_list_as_today_on_postgres() {
    assert_a_failing_check_fails_the_list_as_today(pg_stack().await).await;
}

async fn assert_a_failing_check_fails_me_as_today(stack: Stack) {
    stack.users.answer(UserRead::FindByDid, UserAnswer::Fails);
    let today = alice_reads(&stack, "/me").await;
    stack.users.read_through(UserRead::FindByDid);
    stack.users.answer(UserRead::Find, UserAnswer::Fails);

    let through_the_check = alice_reads(&stack, "/me").await;

    assert_eq!(today.status, 500, "a failing user read is the house 500");
    assert_eq!(through_the_check, today);
}

#[tokio::test]
async fn a_store_failure_in_the_check_fails_me_as_today_in_memory() {
    assert_a_failing_check_fails_me_as_today(mem_stack().await).await;
}

#[tokio::test]
async fn a_store_failure_in_the_check_fails_me_as_today_on_postgres() {
    assert_a_failing_check_fails_me_as_today(pg_stack().await).await;
}
