//! A deleted Account through the served stack: an accept into a soft-deleted Account answers
//! `404` and seats no one, on the in-memory stores and on PostgreSQL; a soft-deleted Account
//! shows in the Den only to its Owner, and only with `includeDeleted`; an empty Account
//! deletes hard and its DID is tombstoned once, after the delete commits. The PostgreSQL cases
//! require a container runtime socket.

use std::sync::Arc;

use chrono::Utc;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    did::Did,
    handle::Handle,
    invitation::Invitation,
    profile::Profile,
    role::Role,
    user_account::UserAccount,
};
use serde_json::{Value, json};
use test_support::{
    account_deletion_contract::{DeleteOutcome, deleted, founded, offer, recognized, seat},
    den::{entry_names, get, ok_json},
    http::{client, serve, sign_in},
    pg_app::{self, stranger},
    recording::{CallLog, RecordingDatabase, RecordingDidMinter},
};

mod common;

/// The Den's listing of the viewer's Accounts, soft-deleted ones included.
const EVERY_ACCOUNT: &str = "/den/accounts?includeDeleted=true";

/// `POST /accounts/{account}/invitations/accept` as `client`.
async fn post_accept(
    client: &reqwest::Client,
    base: &str,
    account: &AccountId,
) -> reqwest::Response {
    let acceptance = json!({ "listed_on_profile": false });
    client
        .post(format!("{base}/accounts/{account}/invitations/accept"))
        .json(&acceptance)
        .send()
        .await
        .expect("POST accept")
}

/// How many rows of `table` name `account`, read on `pool`.
async fn rows_naming(pool: &sqlx::PgPool, table: &str, account: &AccountId) -> i64 {
    let counting = format!("SELECT count(*) FROM {table} WHERE account_id = $1");
    sqlx::query_scalar(sqlx::AssertSqlSafe(counting))
        .bind(AsRef::<str>::as_ref(account))
        .fetch_one(pool)
        .await
        .expect("count the rows")
}

// An invitee whose offer was left pending by a soft delete is answered `404` on the
// in-memory stores, and seated nowhere.
#[tokio::test]
async fn accepting_into_a_soft_deleted_account_answers_404() {
    let invitee_did = Did::from("did:plc:softinvitee".to_owned());
    let profile = Profile::new(invitee_did.clone(), "invitee.bsky.social");
    let served = serve(
        test_support::runtime::mem(&invitee_did).profile(profile),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let invitee_client = client();
    sign_in(&invitee_client, &base, "invitee.bsky.social").await;
    let invitee = backend.provision(&invitee_did).await.expect("the invitee");
    let owner = backend
        .provision(&Did::from("did:plc:softowner".to_owned()))
        .await
        .expect("the owner");
    let member = backend
        .provision(&Did::from("did:plc:softmember".to_owned()))
        .await
        .expect("the member");
    let (account, founding) = Account::open(
        owner.id.clone(),
        Did::from("did:plc:softaccount".to_owned()),
        "softgone.zurfur.app"
            .parse::<Handle>()
            .expect("a valid handle"),
        "Gone Studio".parse::<AccountName>().expect("a valid name"),
        Utc::now(),
    );
    backend
        .create(&account, &founding)
        .await
        .expect("found the account");
    let seated = UserAccount {
        user_id: member.id,
        account_id: account.id.clone(),
        role: Role::Member,
        alias: None,
    };
    backend.seed_member(&seated);
    let offered = Invitation::issue(
        account.id.clone(),
        invitee.id.clone(),
        Role::Member,
        owner.id,
        Utc::now(),
    );
    let mut unit = backend.database().begin().await.expect("begin");
    unit.accounts()
        .create_invitation(&offered)
        .await
        .expect("offer the invitee a seat");
    unit.accounts()
        .soft_delete(&account.id)
        .await
        .expect("soft-delete the account");
    unit.commit().await.expect("commit");

    let answer = post_accept(&invitee_client, &base, &account.id).await;

    common::assert_problem(answer, 404, "account_not_found").await;
    let role = backend
        .role_of(&invitee.id, &account.id)
        .await
        .expect("read the invitee's role");
    assert!(role.is_none(), "the invitee is seated nowhere: {role:?}");
}

// On PostgreSQL, an accept into an Account soft-deleted by its Owner answers `404`, seats no
// one and records no further fact.
#[tokio::test]
async fn accepting_into_a_soft_deleted_account_answers_404_on_postgres() {
    let world = pg_app::den_world(api::app).await;
    let runtime = &world.served.runtime;
    let alice = recognized(runtime, adapter_mem::fixture::alice().as_ref()).await;
    let bob = recognized(runtime, adapter_mem::fixture::bob().as_ref()).await;
    let account = founded(runtime, &alice, "gone-studio.zurfur.app").await;
    seat(runtime, &account, &alice, &bob).await;
    offer(runtime, &account, &alice, &stranger(), Role::Member).await;
    let outcome = deleted(runtime, &account, &alice).await;

    let answer = post_accept(&world.stranger, world.base_url(), &account).await;

    assert!(matches!(outcome, DeleteOutcome::Soft), "{outcome}");
    common::assert_problem(answer, 404, "account_not_found").await;
    let pool = &runtime.pool;
    assert_eq!(
        rows_naming(pool, "account_members", &account).await,
        2,
        "only the Owner and the member are seated"
    );
    assert_eq!(
        rows_naming(pool, "account_fact", &account).await,
        1,
        "the member's fact alone is recorded"
    );
}

// A soft-deleted Account shows in the Den to its Owner with `includeDeleted`, and to no one
// else: neither its former member, who left it once it was deleted, nor a stranger.
#[tokio::test]
async fn a_soft_deleted_account_shows_only_to_its_owner_with_include_deleted() {
    let world = pg_app::den_world(api::app).await;
    let runtime = &world.served.runtime;
    let alice = recognized(runtime, adapter_mem::fixture::alice().as_ref()).await;
    let bob = recognized(runtime, adapter_mem::fixture::bob().as_ref()).await;
    let account = founded(runtime, &alice, "gone-studio.zurfur.app").await;
    seat(runtime, &account, &alice, &bob).await;
    let outcome = deleted(runtime, &account, &alice).await;
    let leaving = application::account::leave::Command {
        leaving_user_id: bob,
        account_id: account.clone(),
    };
    runtime
        .app()
        .accounts()
        .leave(leaving)
        .await
        .expect("bob leaves the deleted Account");
    let base = world.base_url();

    let alice_s_all = ok_json(get(&world.alice, base, EVERY_ACCOUNT).await).await;
    let alice_s_live = ok_json(get(&world.alice, base, "/den/accounts").await).await;
    let bob_s_all = ok_json(get(&world.bob, base, EVERY_ACCOUNT).await).await;
    let stranger_s_all = get(&world.stranger, base, EVERY_ACCOUNT)
        .await
        .text()
        .await
        .expect("the stranger's answer");

    assert!(matches!(outcome, DeleteOutcome::Soft), "{outcome}");
    let mut alice_s_names = entry_names(&alice_s_all["listing"]);
    alice_s_names.sort();
    assert_eq!(alice_s_names, ["Acme Studio", "Supreme Arts"]);
    assert_eq!(entry_names(&alice_s_live["listing"]), ["Supreme Arts"]);
    assert_eq!(entry_names(&bob_s_all["listing"]), ["Night Shift"]);
    assert!(
        !stranger_s_all.contains("Acme Studio"),
        "a stranger never sees it: {stranger_s_all}"
    );
}

// An empty Account deletes hard, and its DID is tombstoned once, after the delete's unit
// commits.
#[tokio::test]
async fn an_empty_account_deletes_hard_and_is_tombstoned() {
    let did = Did::from("did:plc:hardowner".to_owned());
    let profile = Profile::new(did.clone(), "owner.bsky.social");
    let log = CallLog::default();
    let recording = log.clone();
    let served = serve(
        test_support::runtime::mem(&did).profile(profile),
        |runtime| {
            let did_minter = Arc::new(RecordingDidMinter::new(
                Arc::clone(&runtime.did_minter),
                recording.clone(),
            ));
            let database = Arc::new(RecordingDatabase::new(
                Arc::clone(&runtime.database),
                recording,
            ));
            let recorded = composition::Runtime {
                did_minter,
                database,
                ..runtime
            };
            api::app(recorded)
        },
    )
    .await;
    let base = served.base_url;
    let owner = client();
    sign_in(&owner, &base, "owner.bsky.social").await;
    let founding = json!({ "name": "Empty Studio", "handle": "empty.zurfur.app" });
    let founded = owner
        .post(format!("{base}/accounts"))
        .json(&founding)
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(founded.status(), 201, "the Account is founded");
    let body: Value = founded.json().await.expect("a JSON body");
    let id = body["id"].as_str().expect("the Account's id").to_owned();
    let before = log.events().len();

    let deleting = owner
        .delete(format!("{base}/accounts/{id}"))
        .send()
        .await
        .expect("DELETE /accounts/{id}");

    assert_eq!(deleting.status(), 200, "the delete answers 200");
    let outcome: Value = deleting.json().await.expect("a JSON body");
    assert_eq!(outcome["outcome"], "hard", "{outcome}");
    let events: Vec<&str> = log.events().split_off(before);
    let tombstones = events.iter().filter(|event| **event == "tombstone").count();
    assert_eq!(tombstones, 1, "tombstoned once: {events:?}");
    let tombstoned_at = events.iter().position(|event| *event == "tombstone");
    let committed_at = events.iter().position(|event| *event == "commit");
    assert!(
        committed_at < tombstoned_at && committed_at.is_some(),
        "the tombstone follows the commit: {events:?}"
    );
}
