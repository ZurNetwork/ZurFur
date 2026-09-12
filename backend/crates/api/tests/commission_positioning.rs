//! The owner places a commission in an account's position and manages
//! its view grants, end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface:
//!
//! - **AC1/AC2** — the owner places the commission; each (re)placement appends a
//!   log row, the log is never rewritten, current = latest, origin = first.
//! - **AC3** — the cached current-placement pointer equals the latest log row
//!   after every (re)placement.
//! - **AC4** — the owner grants a User a view grant and revokes it; a revoked
//!   key no longer lifts (its row is gone), effective immediately.
//! - **AC5** — placement and view grants confer **no** in-commission authority: a
//!   User holding a Total key on (and whose account is the placement of) the
//!   commission is still not a Participant and is turned away from every
//!   commission door with the closed-door 404.
//!
//! ⚠️ View grants are issued to **Users**, never Accounts,
//! so AC4/AC5 name a grantee User. Placement is a card on
//! an account's board: the account-level placement rails were
//! deleted, so AC1/AC2 exercise the board and AC3's
//! current-placement pointer has no referent.
//! - **AC6** — a commission with no placement and no grants is valid.
//! - **Closed door** — a non-owner gets the byte-identical 404 a missing
//!   commission gets (never a 403 oracle); an unauthenticated caller gets 401.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use adapter_mem::MemBackend;
use api::AppState;
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    commission::{Commission, CommissionId, CommissionTitle, GrantLevel},
    did::Did,
    handle::Handle,
    profile::Profile,
    role::Role,
    user::{User, UserId},
    user_account::UserAccount,
    workflow::{ColumnId, ColumnName, LexOrdering, WorkflowId, WorkflowName},
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

async fn spawn_app(did: &str) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let test_support::runtime::MemRuntime { runtime, backend } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .profile(Profile::new(
                Did::new(did.to_string()),
                "artist.bsky.social",
            ))
            .public_url(format!("http://{addr}"))
            .build();
    let state: AppState = runtime;
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=artist.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303);
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
}

/// Creates a commission over HTTP as the signed-in caller and returns its id.
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201);
    let all = backend.all_commissions().await.expect("list commissions");
    *all.last().expect("a commission was persisted").id
}

/// Seeds a committed account with a distinct handle, returning its
/// [`AccountId`] — which *is* its DID, with no separate surrogate id.
/// `member`, when given, is seated as a plain Member of the account.
async fn seed_account(backend: &MemBackend, handle: &str, member: Option<UserId>) -> AccountId {
    let owner = backend
        .provision(&Did::new(format!("did:plc:acctowner-{handle}")))
        .await
        .expect("provision account owner");
    let (account, owner_membership) = Account::open(
        owner.id,
        Did::new(format!("did:plc:acct-{handle}")),
        handle.parse::<Handle>().expect("handle"),
        "Acme Studio".parse::<AccountName>().expect("account name"),
        Utc::now(),
    );
    backend
        .create(&account, &owner_membership)
        .await
        .expect("found the account");
    if let Some(user) = member {
        backend
            .grant_role(&UserAccount {
                user_id: user,
                account_id: account.id.clone(),
                role: Role::Member,
                alias: None,
            })
            .await
            .expect("seat the member");
    }
    account.id
}

/// Seeds a committed board for `account` with a single column, returning both
/// ids. A board is where placement lives, so every positioning test needs one.
async fn seed_board(backend: &MemBackend, account: &AccountId) -> (WorkflowId, ColumnId) {
    let database = backend.database();
    let name = "Queue".parse::<WorkflowName>().expect("board name");

    let mut uow = database.begin().await.expect("begin");
    let mut workflow = uow
        .workflows()
        .create(&name, account)
        .await
        .expect("create the board");
    let column_name = "Open".parse::<ColumnName>().expect("column name");
    let column = workflow.new_column(column_name, workflow.visibility.clone());
    let column_id = column.id.clone();
    workflow.insert(0, column).expect("the board is empty");
    uow.workflows()
        .set_indexes(&workflow)
        .await
        .expect("persist the column");
    uow.commit().await.expect("commit the board");

    (workflow.id, column_id)
}

/// Seeds a committed commission owned by a directly-provisioned foreign user.
async fn seed_foreign_commission(backend: &MemBackend) -> (uuid::Uuid, UserId) {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = "Not yours".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(title, owner.id.clone(), Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    (id, owner.id)
}

async fn read_changelog_kinds(client: &reqwest::Client, base: &str, id: uuid::Uuid) -> Vec<String> {
    let res = client
        .get(format!("{base}/commissions/{id}/changelog"))
        .send()
        .await
        .expect("GET changelog");
    assert_eq!(res.status(), 200);
    let body: Vec<serde_json::Value> = res.json().await.expect("array");
    body.iter()
        .map(|e| e["kind"].as_str().unwrap().to_string())
        .collect()
}

// AC1 + AC2 — **placement is a card on a board** (Ownership Separation DD
// `29130754` Decision 6: "placement = workflow membership rows, account-side").
// The owner places the commission onto an account's board over HTTP: a 204, and
// the board now holds the card. Placing it onto a SECOND account's board leaves
// it on both — one commission, N boards, no conflict, because no account ever
// claimed it (D1: users own commissions).
//
// AC3's cached current-placement pointer has no referent any more: it belonged
// to the 1:1-current placement log, which reconstructed the very managing-account
// model this DD superseded. There is no per-commission "current" account to
// cache — by design.
//
// No changelog entry is appended: positioning is account-side view state the
// commission never learns about, and the Changelog DD taxonomy has no placement
// variant.
#[tokio::test]
async fn placing_puts_the_card_on_a_board_and_one_commission_sits_on_many() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let cid = CommissionId::new(id);
    let store = backend.commission_store();

    // The caller must be a member of the board's account: a card goes on a board
    // you belong to (DESIGN/Workflow — the account owns its positioning).
    let artist = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find")
        .expect("sign-in provisioned the artist")
        .id;
    let account_a = seed_account(&backend, "a.zurfur.app", Some(artist.clone())).await;
    let account_b = seed_account(&backend, "b.zurfur.app", Some(artist)).await;
    let (board_a, column_a) = seed_board(&backend, &account_a).await;
    let (board_b, column_b) = seed_board(&backend, &account_b).await;

    // AC6 — before any placement the commission is valid, on nobody's board.
    assert!(
        store
            .current_column_of_workflow(&cid, &board_a)
            .await
            .unwrap()
            .is_none(),
        "an unplaced commission sits on no board (still valid)"
    );

    for (column, board) in [(&column_a, &board_a), (&column_b, &board_b)] {
        let res = client
            .post(format!("{base}/commissions/{id}/placements"))
            .json(&json!({ "column_id": column.to_string(), "index": 0 }))
            .send()
            .await
            .expect("POST placement");
        assert_eq!(res.status(), 204, "the owner places the commission");

        assert_eq!(
            store
                .current_column_of_workflow(&cid, board)
                .await
                .unwrap()
                .map(|found| found.id),
            Some(column.clone()),
            "the board that positioned it holds the card",
        );
        assert_eq!(
            store
                .current_position_in_column(&cid, column)
                .await
                .unwrap(),
            Some(0),
            "at the index the caller asked for",
        );
    }

    // Both boards still hold it — the NxM the DD makes native.
    assert!(
        store
            .current_column_of_workflow(&cid, &board_a)
            .await
            .unwrap()
            .is_some(),
        "the first board did not lose the card to the second",
    );

    // No placement changelog entry — only the creation entry exists.
    assert_eq!(
        read_changelog_kinds(&client, &base, id).await,
        ["created"],
        "positioning appends no changelog entry",
    );
}

// AC4 — the owner grants a User a view grant (a 204, key stored, changelog
// records the issuance), re-grants at a different level (replaces), then revokes
// (key gone immediately, changelog records the revoke). A repeat revoke is an
// idempotent no-op that appends no duplicate entry.
#[tokio::test]
async fn grant_then_revoke_takes_effect_immediately_and_is_recorded() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let cid = CommissionId::new(id);
    let grantee = UserId::new(Did::new("did:plc:grantee".to_string()));
    let store = backend.commission_store();

    // Grant Presentation, then re-grant Total — the key replaces, not stacks.
    for level in ["presentation", "total"] {
        let res = client
            .post(format!("{base}/commissions/{id}/grants"))
            .json(&json!({ "target_user_id": grantee.to_string(), "level": level }))
            .send()
            .await
            .expect("POST grant");
        assert_eq!(res.status(), 204, "the owner issues a view grant");
    }
    assert_eq!(
        store.view_grant(&cid, &grantee.clone()).await.unwrap(),
        Some(GrantLevel::Total),
        "re-granting replaces the level (issuing anew)",
    );

    // Revoke — the key is gone immediately (revocation effective by construction).
    let revoke_body = json!({ "target_user_id": grantee.to_string() });
    let res = client
        .delete(format!("{base}/commissions/{id}/grants/{}", *grantee))
        .json(&revoke_body)
        .send()
        .await
        .expect("DELETE grant");
    assert_eq!(res.status(), 204, "the owner revokes the grant");
    assert!(
        store
            .view_grant(&cid, &grantee.clone())
            .await
            .unwrap()
            .is_none(),
        "a revoked key no longer lifts — its row is gone immediately (AC4)",
    );

    // A repeat revoke is an idempotent no-op — no duplicate changelog entry.
    let res = client
        .delete(format!("{base}/commissions/{id}/grants/{}", *grantee))
        .json(&revoke_body)
        .send()
        .await
        .expect("DELETE grant repeat");
    assert_eq!(res.status(), 204, "revoking a non-existent key is a no-op");

    assert_eq!(
        read_changelog_kinds(&client, &base, id).await,
        [
            "created",
            "view_grant_issued",
            "view_grant_issued",
            "view_grant_revoked"
        ],
        "each issue records; only the real revoke records; no-op revoke is silent",
    );
}

// AC5 — placement and view grants confer NO in-commission authority. A User who
// holds a Total view grant on the commission — and whose account is its
// placement — is still not a Participant: they get the closed-door 404 from
// every commission door (owner-gated place/grant AND participant-gated
// changelog), exactly as a total stranger would. The read-side VIEW lift is a
// separate, later serializer (ZMVP-75); authority never follows a key.
#[tokio::test]
async fn a_granted_accounts_member_gains_no_in_commission_authority() {
    // Sign in AS the account member.
    let (base, backend) = spawn_app("did:plc:member").await;
    let client = client();
    sign_in(&client, &base).await;
    let member = backend
        .find_by_did(&Did::new("did:plc:member".to_string()))
        .await
        .expect("find")
        .expect("sign-in provisioned the member");

    // A foreign owner's commission, an account the member belongs to, and a
    // Total grant + placement of the commission into that account.
    let (id, _owner_id) = seed_foreign_commission(&backend).await;
    let cid = CommissionId::new(id);
    let account = seed_account(&backend, "granted.zurfur.app", Some(member.id.clone())).await;
    let (_board, column) = seed_board(&backend, &account).await;
    {
        let db = backend.database();
        let mut uow = db.begin().await.unwrap();
        uow.commissions()
            .grant_view(&cid, &member.id, GrantLevel::Total)
            .await
            .unwrap();
        uow.commit().await.unwrap();
    }

    // The actor really holds the key, and is a member of the placed account...
    assert_eq!(
        backend
            .commission_store()
            .view_grant(&cid, &member.id)
            .await
            .unwrap(),
        Some(GrantLevel::Total),
        "the actor really holds a Total view grant",
    );
    assert_eq!(
        backend.role_of(&member.id, &account).await.unwrap(),
        Some(Role::Member),
        "the actor really is a member of the placed account",
    );
    // ...yet the grant/placement made them no Participant of the commission.
    assert!(
        !backend
            .commission_store()
            .is_participant(&cid, &member.id)
            .await
            .unwrap(),
        "a view grant / placement never makes its holder a Participant (D8)",
    );

    // Every commission door is closed to them, byte-identical to a stranger's 404.
    let changelog = client
        .get(format!("{base}/commissions/{id}/changelog"))
        .send()
        .await
        .expect("GET changelog");
    common::assert_problem(changelog, 404, "commission_not_found").await;

    let grant = client
        .post(format!("{base}/commissions/{id}/grants"))
        .json(&json!({ "target_user_id": member.id.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant");
    common::assert_problem(grant, 404, "commission_not_found").await;

    // ...but POSITIONING is not an in-commission act, and their OWN key is one
    // of the two rails a commission may reach a board by (DD `29130754` D3;
    // DESIGN/Workflow: "a commission a member can already see through their own
    // standing or their own view grant"). So this one succeeds — and it is the
    // line the DD draws: they may file it on their board, and still not read it.
    let place = client
        .post(format!("{base}/commissions/{id}/placements"))
        .json(&json!({ "column_id": column.to_string(), "index": 0 }))
        .send()
        .await
        .expect("POST placement");
    assert_eq!(
        place.status(),
        204,
        "their own key carries them onto their own board (the push rail)",
    );
}

// Closed door — a non-owner placing/granting/revoking gets the byte-identical
// problem+json a missing commission gets: a 404, never a 403 oracle.
#[tokio::test]
async fn a_non_owner_gets_the_same_404_as_a_missing_commission() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let (foreign, _owner) = seed_foreign_commission(&backend).await;
    let outsider = backend
        .find_by_did(&Did::new("did:plc:outsider".to_string()))
        .await
        .expect("find")
        .expect("sign-in provisioned the outsider")
        .id;
    // The outsider owns a board of their own, so the refusal below is about the
    // COMMISSION, not about the board — otherwise the membership check would
    // answer first and the test would prove nothing.
    let account = seed_account(&backend, "x.zurfur.app", Some(outsider)).await;
    let (_board, column) = seed_board(&backend, &account).await;

    let hidden = client
        .post(format!("{base}/commissions/{foreign}/placements"))
        .json(&json!({ "column_id": column.to_string(), "index": 0 }))
        .send()
        .await
        .expect("POST placement on a hidden commission");
    assert_eq!(hidden.status(), 404);
    let hidden_body = hidden.text().await.expect("body");

    let missing_id = uuid::Uuid::now_v7();
    let missing = client
        .post(format!("{base}/commissions/{missing_id}/placements"))
        .json(&json!({ "column_id": column.to_string(), "index": 0 }))
        .send()
        .await
        .expect("POST placement on a missing commission");
    assert_eq!(missing.status(), 404);
    let missing_body = missing.text().await.expect("body");
    assert_eq!(
        hidden_body, missing_body,
        "hidden and missing are indistinguishable (no existence oracle)",
    );

    // Grant + revoke on a hidden commission are the same closed door. Their
    // bodies are kept: the actor-class check below compares against them.
    let grantee = UserId::new(Did::new("did:plc:would-be-grantee".to_string()));
    let grant = client
        .post(format!("{base}/commissions/{foreign}/grants"))
        .json(&json!({ "target_user_id": grantee.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant hidden");
    assert_eq!(grant.status(), 404);
    let grant_body = grant.text().await.expect("body");
    let grant_problem: serde_json::Value = serde_json::from_str(&grant_body).expect("problem+json");
    assert_eq!(grant_problem["code"], "commission_not_found");

    let revoke = client
        .delete(format!("{base}/commissions/{foreign}/grants/{}", *grantee))
        .json(&json!({ "target_user_id": grantee.to_string() }))
        .send()
        .await
        .expect("DELETE grant hidden");
    assert_eq!(revoke.status(), 404);
    let revoke_body = revoke.text().await.expect("body");
    let revoke_problem: serde_json::Value =
        serde_json::from_str(&revoke_body).expect("problem+json");
    assert_eq!(revoke_problem["code"], "commission_not_found");

    // Nothing was written to the foreign commission.
    let store = backend.commission_store();
    let (outsider_board, _outsider_column) = seed_board(&backend, &account).await;
    assert!(
        store
            .current_column_of_workflow(&CommissionId::new(foreign), &outsider_board)
            .await
            .unwrap()
            .is_none(),
        "the outsider placed nothing",
    );

    // The closed door must not leak the ACTOR CLASS of the DID the caller
    // names. `provision` is a write keyed to that DID and it refuses a DID
    // already interned as another kind of actor: while it ran before the
    // ownership check, naming an Account's DID answered `409
    // did_belongs_to_another_actor` where naming a free DID reached this `404`
    // — telling a stranger apart "this DID is some other actor" from "it is
    // not", on a commission id they invented. Both must be the same door.
    let interned_elsewhere = account.to_string(); // an Account's DID, not a User's
    let grant_interned = client
        .post(format!("{base}/commissions/{foreign}/grants"))
        .json(&json!({ "target_user_id": interned_elsewhere, "level": "total" }))
        .send()
        .await
        .expect("POST grant naming a non-User actor");
    assert_eq!(grant_interned.status(), 404);
    let grant_interned_body = grant_interned.text().await.expect("body");

    let revoke_interned = client
        .delete(format!("{base}/commissions/{foreign}/grants/{}", *grantee))
        .json(&json!({ "target_user_id": interned_elsewhere }))
        .send()
        .await
        .expect("DELETE grant naming a non-User actor");
    assert_eq!(revoke_interned.status(), 404);
    let revoke_interned_body = revoke_interned.text().await.expect("body");

    assert_eq!(
        grant_interned_body, grant_body,
        "an already-interned DID and a free one get the byte-identical closed door",
    );
    assert_eq!(
        revoke_interned_body, revoke_body,
        "an already-interned DID and a free one get the byte-identical closed door",
    );

    // And the refused grant/revoke left no User behind for the grantee — the
    // commission-side twin of the account assertion in `account_scope_gate.rs`.
    assert!(
        backend
            .find_by_did(&Did::new("did:plc:would-be-grantee".to_string()))
            .await
            .expect("find grantee")
            .is_none(),
        "a forbidden grant/revoke provisions no User for the grantee",
    );
}

// Placing into a non-existent account is a clean 404 account_not_found — the
// owner (who passed the closed door) is told the *account* is unknown, not a
// leaked FK 500.
//
// ⚠️ The grant half of this AC dissolved with the 2026-09-04 amendment to DD
// `29130754`: a grant no longer names an account, and the grantee User is
// provisioned on first grant rather than looked up — so there is no "unknown
// account" for a grant to answer. Whether an unknown *grantee* should 404 or
// provision silently is an open contract question for the Engineer.
#[tokio::test]
async fn placing_into_an_unknown_column_is_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let ghost = uuid::Uuid::now_v7();

    let place = client
        .post(format!("{base}/commissions/{id}/placements"))
        .json(&json!({ "column_id": ghost.to_string(), "index": 0 }))
        .send()
        .await
        .expect("POST placement");
    common::assert_problem(place, 404, "column_not_found").await;
}

// A malformed grant level is a 422 invalid_request (the grant vocabulary is the
// raw modes, never the Private/Listed/Public aliases).
#[tokio::test]
async fn an_unknown_grant_level_is_422() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let grantee = UserId::new(Did::new("did:plc:level-probe".to_string()));

    for bad in ["private", "everything", ""] {
        let res = client
            .post(format!("{base}/commissions/{id}/grants"))
            .json(&json!({ "target_user_id": grantee.to_string(), "level": bad }))
            .send()
            .await
            .expect("POST grant with a bad level");
        common::assert_problem(res, 422, "invalid_request").await;
    }
}

// The positioning surface requires a session: an unauthenticated caller gets 401
// in every direction, before any existence answer.
#[tokio::test]
async fn unauthenticated_positioning_is_401() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let account = seed_account(&backend, "z.zurfur.app", None).await;
    let (_board, column) = seed_board(&backend, &account).await;

    let anon = client();
    let place = anon
        .post(format!("{base}/commissions/{id}/placements"))
        .json(&json!({ "column_id": column.to_string(), "index": 0 }))
        .send()
        .await
        .expect("POST placement unauth");
    common::assert_problem(place, 401, "not_authenticated").await;

    let grantee = UserId::new(Did::new("did:plc:anon-probe".to_string()));
    let grant = anon
        .post(format!("{base}/commissions/{id}/grants"))
        .json(&json!({ "target_user_id": grantee.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant unauth");
    common::assert_problem(grant, 401, "not_authenticated").await;

    let revoke = anon
        .delete(format!("{base}/commissions/{id}/grants/{}", *grantee))
        .json(&json!({ "target_user_id": grantee.to_string() }))
        .send()
        .await
        .expect("DELETE grant unauth");
    common::assert_problem(revoke, 401, "not_authenticated").await;
}
