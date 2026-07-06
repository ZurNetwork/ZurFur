//! ZMVP-70 — the owner places a commission in an account's position and manages
//! its view grants, end to end over HTTP (Ownership Separation DD `29130754`).
//!
//! Pins the acceptance criteria at the API surface:
//!
//! - **AC1/AC2** — the owner places the commission; each (re)placement appends a
//!   log row, the log is never rewritten, current = latest, origin = first.
//! - **AC3** — the cached current-placement pointer equals the latest log row
//!   after every (re)placement.
//! - **AC4** — the owner grants an account a view grant and revokes it; a revoked
//!   key no longer lifts (its row is gone), effective immediately.
//! - **AC5** — placement and view grants confer **no** in-commission authority: a
//!   member of a granted account is still not a Participant and is turned away
//!   from every commission door with the closed-door 404.
//! - **AC6** — a commission with no placement and no grants is valid.
//! - **Closed door** — a non-owner gets the byte-identical 404 a missing
//!   commission gets (never a 403 oracle); an unauthenticated caller gets 401.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
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

    let backend = MemBackend::new();
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url: "postgres://unused".to_string(),
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".to_string(),
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
        },
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "artist.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        database: backend.database(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
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

/// Seeds a committed account with a distinct handle, returning its id (as a raw
/// `Uuid`, the URL/body boundary form). `member`, when given, is seated as a
/// plain Member of the account.
async fn seed_account(backend: &MemBackend, handle: &str, member: Option<UserId>) -> uuid::Uuid {
    let owner = backend
        .provision(&Did::new(format!("did:plc:acctowner-{handle}")))
        .await
        .expect("provision account owner");
    let (account, owner_membership) = Account::open(
        owner.id,
        Did::new(format!("did:plc:acct-{handle}")),
        Handle::try_new(handle).expect("handle"),
        AccountName::try_new("Acme Studio".to_string()).expect("account name"),
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
                account_id: account.id,
                role: Role::Member(None),
            })
            .await
            .expect("seat the member");
    }
    *account.id
}

/// Seeds a committed commission owned by a directly-provisioned foreign user.
async fn seed_foreign_commission(backend: &MemBackend) -> (uuid::Uuid, UserId) {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = CommissionTitle::try_new("Not yours").expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
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

// AC1 + AC2 + AC3 — the owner places, re-places, and re-places again: each is a
// 204, the log grows (never rewritten), current = latest, origin = first, and the
// cached current-placement pointer equals the latest log row every time. No
// placement changelog entry is appended (the placement log IS the record).
#[tokio::test]
async fn placement_appends_and_the_current_pointer_tracks_the_latest_row() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let cid = CommissionId::new(id);
    let store = backend.commission_store();

    // AC6 — before any placement the commission is valid with no current placement.
    assert!(
        store.current_placement(cid).await.unwrap().is_none(),
        "an unplaced commission has no current placement (still valid)"
    );

    let account_a = seed_account(&backend, "a.zurfur.app", None).await;
    let account_b = seed_account(&backend, "b.zurfur.app", None).await;

    for (n, account) in [account_a, account_b, account_a].into_iter().enumerate() {
        let res = client
            .post(format!("{base}/commissions/{id}/placements"))
            .json(&json!({ "account_id": account.to_string() }))
            .send()
            .await
            .expect("POST placement");
        assert_eq!(res.status(), 204, "the owner places the commission");

        let log = store.placement_log(cid).await.unwrap();
        assert_eq!(log.len(), n + 1, "each placement appends exactly one row");
        let current = store
            .current_placement(cid)
            .await
            .unwrap()
            .expect("a placed commission has a current placement");
        let latest = log.last().unwrap();
        assert_eq!(
            (current.seq, current.account_id),
            (latest.seq, latest.account_id),
            "the cached current pointer equals the latest log row (AC3)",
        );
        assert_eq!(
            current.account_id,
            AccountId::new(account),
            "current = the just-placed account"
        );
    }

    let log = store.placement_log(cid).await.unwrap();
    assert_eq!(
        log.first().unwrap().account_id,
        AccountId::new(account_a),
        "origin = first row"
    );
    assert_eq!(
        log.last().unwrap().account_id,
        AccountId::new(account_a),
        "current = latest row"
    );
    assert!(
        log[0].seq < log[1].seq && log[1].seq < log[2].seq,
        "seq orders the log"
    );

    // No placement changelog entry — only the creation entry exists.
    assert_eq!(
        read_changelog_kinds(&client, &base, id).await,
        ["created"],
        "placement appends no changelog entry (the placement log is the record)",
    );
}

// AC4 — the owner grants an account a view grant (a 204, key stored, changelog
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
    let account = seed_account(&backend, "studio.zurfur.app", None).await;
    let store = backend.commission_store();

    // Grant Presentation, then re-grant Total — the key replaces, not stacks.
    for level in ["presentation", "total"] {
        let res = client
            .post(format!("{base}/commissions/{id}/grants"))
            .json(&json!({ "account_id": account.to_string(), "level": level }))
            .send()
            .await
            .expect("POST grant");
        assert_eq!(res.status(), 204, "the owner issues a view grant");
    }
    assert_eq!(
        store
            .view_grant(cid, AccountId::new(account))
            .await
            .unwrap(),
        Some(GrantLevel::Total),
        "re-granting replaces the level (issuing anew)",
    );

    // Revoke — the key is gone immediately (revocation effective by construction).
    let res = client
        .delete(format!("{base}/commissions/{id}/grants/{account}"))
        .send()
        .await
        .expect("DELETE grant");
    assert_eq!(res.status(), 204, "the owner revokes the grant");
    assert!(
        store
            .view_grant(cid, AccountId::new(account))
            .await
            .unwrap()
            .is_none(),
        "a revoked key no longer lifts — its row is gone immediately (AC4)",
    );

    // A repeat revoke is an idempotent no-op — no duplicate changelog entry.
    let res = client
        .delete(format!("{base}/commissions/{id}/grants/{account}"))
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

// AC5 — placement and view grants confer NO in-commission authority. A member of
// an account that holds a Total view grant on (and is the placement of) the
// commission is still not a Participant: they get the closed-door 404 from every
// commission door (owner-gated place/grant AND participant-gated changelog),
// exactly as a total stranger would. The read-side VIEW lift is a separate,
// later serializer (ZMVP-75); authority never follows a key.
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
    let (id, owner_id) = seed_foreign_commission(&backend).await;
    let cid = CommissionId::new(id);
    let account_uuid = seed_account(&backend, "granted.zurfur.app", Some(member.id)).await;
    let account = AccountId::new(account_uuid);
    {
        let db = backend.database();
        let mut uow = db.begin().await.unwrap();
        uow.commissions()
            .grant_view(cid, account, GrantLevel::Total)
            .await
            .unwrap();
        uow.commissions()
            .place(cid, account, owner_id, Utc::now())
            .await
            .unwrap();
        uow.commit().await.unwrap();
    }

    // The member is a member of the granted account...
    assert_eq!(
        backend.role_of(member.id, account).await.unwrap(),
        Some(Role::Member(None)),
        "the actor really is a member of the granted account",
    );
    // ...yet the grant/placement made them no Participant of the commission.
    assert!(
        !backend
            .commission_store()
            .is_participant(cid, member.id)
            .await
            .unwrap(),
        "a view grant / placement never makes an account member a Participant (D8)",
    );

    // Every commission door is closed to them, byte-identical to a stranger's 404.
    let changelog = client
        .get(format!("{base}/commissions/{id}/changelog"))
        .send()
        .await
        .expect("GET changelog");
    common::assert_problem(changelog, 404, "commission_not_found").await;

    let place = client
        .post(format!("{base}/commissions/{id}/placements"))
        .json(&json!({ "account_id": account_uuid.to_string() }))
        .send()
        .await
        .expect("POST placement");
    common::assert_problem(place, 404, "commission_not_found").await;

    let grant = client
        .post(format!("{base}/commissions/{id}/grants"))
        .json(&json!({ "account_id": account_uuid.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant");
    common::assert_problem(grant, 404, "commission_not_found").await;
}

// Closed door — a non-owner placing/granting/revoking gets the byte-identical
// problem+json a missing commission gets: a 404, never a 403 oracle.
#[tokio::test]
async fn a_non_owner_gets_the_same_404_as_a_missing_commission() {
    let (base, backend) = spawn_app("did:plc:outsider").await;
    let client = client();
    sign_in(&client, &base).await;
    let (foreign, _owner) = seed_foreign_commission(&backend).await;
    let account = seed_account(&backend, "x.zurfur.app", None).await;

    let hidden = client
        .post(format!("{base}/commissions/{foreign}/placements"))
        .json(&json!({ "account_id": account.to_string() }))
        .send()
        .await
        .expect("POST placement on a hidden commission");
    assert_eq!(hidden.status(), 404);
    let hidden_body = hidden.text().await.expect("body");

    let missing_id = uuid::Uuid::now_v7();
    let missing = client
        .post(format!("{base}/commissions/{missing_id}/placements"))
        .json(&json!({ "account_id": account.to_string() }))
        .send()
        .await
        .expect("POST placement on a missing commission");
    assert_eq!(missing.status(), 404);
    let missing_body = missing.text().await.expect("body");
    assert_eq!(
        hidden_body, missing_body,
        "hidden and missing are indistinguishable (no existence oracle)",
    );

    // Grant + revoke on a hidden commission are the same closed door.
    let grant = client
        .post(format!("{base}/commissions/{foreign}/grants"))
        .json(&json!({ "account_id": account.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant hidden");
    common::assert_problem(grant, 404, "commission_not_found").await;

    let revoke = client
        .delete(format!("{base}/commissions/{foreign}/grants/{account}"))
        .send()
        .await
        .expect("DELETE grant hidden");
    common::assert_problem(revoke, 404, "commission_not_found").await;

    // Nothing was written to the foreign commission.
    let store = backend.commission_store();
    assert!(
        store
            .current_placement(CommissionId::new(foreign))
            .await
            .unwrap()
            .is_none(),
        "the outsider placed nothing",
    );
}

// Granting/placing into a non-existent account is a clean 404 account_not_found —
// the owner (who passed the closed door) is told the *account* is unknown, not a
// leaked FK 500.
#[tokio::test]
async fn placing_or_granting_into_an_unknown_account_is_account_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let ghost = uuid::Uuid::now_v7();

    let place = client
        .post(format!("{base}/commissions/{id}/placements"))
        .json(&json!({ "account_id": ghost.to_string() }))
        .send()
        .await
        .expect("POST placement");
    common::assert_problem(place, 404, "account_not_found").await;

    let grant = client
        .post(format!("{base}/commissions/{id}/grants"))
        .json(&json!({ "account_id": ghost.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant");
    common::assert_problem(grant, 404, "account_not_found").await;
}

// A malformed grant level is a 422 invalid_request (the grant vocabulary is the
// raw modes, never the Private/Listed/Public aliases).
#[tokio::test]
async fn an_unknown_grant_level_is_422() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let account = seed_account(&backend, "y.zurfur.app", None).await;

    for bad in ["private", "everything", ""] {
        let res = client
            .post(format!("{base}/commissions/{id}/grants"))
            .json(&json!({ "account_id": account.to_string(), "level": bad }))
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

    let anon = client();
    let place = anon
        .post(format!("{base}/commissions/{id}/placements"))
        .json(&json!({ "account_id": account.to_string() }))
        .send()
        .await
        .expect("POST placement unauth");
    common::assert_problem(place, 401, "not_authenticated").await;

    let grant = anon
        .post(format!("{base}/commissions/{id}/grants"))
        .json(&json!({ "account_id": account.to_string(), "level": "total" }))
        .send()
        .await
        .expect("POST grant unauth");
    common::assert_problem(grant, 401, "not_authenticated").await;

    let revoke = anon
        .delete(format!("{base}/commissions/{id}/grants/{account}"))
        .send()
        .await
        .expect("DELETE grant unauth");
    common::assert_problem(revoke, 401, "not_authenticated").await;
}
