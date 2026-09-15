//! `GET /accounts` — every live account the signed-in visitor holds
//! a role in, each row carrying the caller's own role:
//!
//! - role-based listing includes a **non-Owner** membership, not just accounts
//!   the caller founded;
//! - soft-deleted accounts are excluded;
//! - ordering is deterministic (by id — UUIDv7 sorts as creation order);
//! - an anonymous caller gets a `401` problem+json, same as `GET /me`.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountName},
    did::Did,
    handle::Handle,
    profile::Profile,
    role::{Role, RoleAlias},
    user_account::UserAccount,
};

mod common;
use common::assert_problem;
use test_support::http::{client, serve, sign_in};

/// Found an account for `owner_did` directly on the backend (test seed of
/// [`domain::ports::AccountWrites::create`]), skipping the DID-minting HTTP
/// round trip. Returns the founded [`Account`].
async fn seed_account(backend: &MemBackend, owner_did: &str, handle: &str) -> Account {
    let owner = backend
        .provision(&Did::from(owner_did.to_string()))
        .await
        .expect("provision owner");
    let (account, membership) = Account::open(
        owner.id,
        Did::from(format!("{owner_did}:acct")),
        handle.parse::<Handle>().expect("valid handle"),
        "Seed Studio".parse::<AccountName>().expect("valid name"),
        Utc::now(),
    );
    backend
        .create(&account, &membership)
        .await
        .expect("seed account");
    account
}

// Role-based listing: GET /accounts returns every live account the caller
// holds a role in — an account they FOUNDED (Owner) and one they only hold a
// non-Owner role on via a grant (the accepted-invitation surface, ZMVP-20).
#[tokio::test]
async fn lists_every_live_account_the_caller_holds_a_role_in_with_that_role() {
    let did = "did:plc:lister";
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())).profile(Profile::new(
            Did::from(did.to_string()),
            "lister.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "lister.bsky.social").await;
    let me = backend
        .find_by_did(&Did::from(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // Found an account through the real HTTP surface — the caller is its Owner.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Owned Studio", "handle": "owned.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    let founded: serde_json::Value = res.json().await.expect("json body");
    let owned_id = founded["id"].as_str().expect("id").to_string();

    // Seed a SECOND account owned by someone else, then grant the signed-in
    // caller a non-Owner role on it — the accepted-invitation shape (ZMVP-20).
    let granted = seed_account(&backend, "did:plc:someone-else", "granted.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: granted.id.clone(),
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("grant member role");

    // Seed a THIRD account the caller holds NO role in. This is the containment
    // case: without it, an implementation that listed every live account —
    // joined to whatever role the caller happened to hold — would return the
    // same two rows and pass. It is also the property the cross-persona
    // invariant (ZMVP-17) actually rests on, so it is not optional coverage.
    let strangers = seed_account(&backend, "did:plc:stranger", "stranger.zurfur.app").await;

    let res = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    // Wrapped at the /api/v1 mint (contract R7): rows ride under "accounts" so
    // pagination can later land additively as a sibling key.
    let rows = body["accounts"].as_array().expect("accounts array");
    assert_eq!(rows.len(), 2, "both memberships are listed: {body}");

    let listed_ids: Vec<&str> = rows
        .iter()
        .map(|row| row["id"].as_str().expect("id"))
        .collect();
    assert!(
        !listed_ids.contains(&strangers.id.to_string().as_str()),
        "an account the caller holds no role in is never listed: {body}"
    );

    let owned_row = rows
        .iter()
        .find(|row| row["id"] == owned_id)
        .expect("the founded account is listed");
    assert_eq!(owned_row["role"], "owner", "the founder is Owner");
    assert_eq!(owned_row["handle"], "owned.zurfur.app");

    let granted_row = rows
        .iter()
        .find(|row| row["id"] == granted.id.to_string())
        .expect("the granted-only account is listed too — not owned-only");
    assert_eq!(
        granted_row["role"], "member",
        "the caller's OWN role rides along, not just presence"
    );
    assert_eq!(granted_row["handle"], "granted.zurfur.app");

    // Ordering is deterministic: ascending by account id (UUIDv7 sorts as
    // creation order) — independent of insertion order into the response.
    let mut expected_order: Vec<String> = vec![owned_id.clone(), granted.id.to_string()];
    expected_order.sort();
    let actual_order: Vec<String> = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        actual_order, expected_order,
        "rows are ordered ascending by id"
    );
}

// Soft-deleted accounts are excluded — mirrors `AccountStore::find`'s liveness
// semantics (DD 23003138): a tombstoned account the caller once held a role on
// must not appear, even though the membership row itself is left intact.
#[tokio::test]
async fn excludes_a_soft_deleted_account_the_caller_holds_a_role_in() {
    let did = "did:plc:lister2";
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())).profile(Profile::new(
            Did::from(did.to_string()),
            "lister.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "lister.bsky.social").await;
    let me = backend
        .find_by_did(&Did::from(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // A live account the caller owns — must still show up.
    let live = seed_account(&backend, did, "live.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id.clone(),
            account_id: live.id.clone(),
            role: Role::Owner,
            alias: None,
        })
        .await
        .expect("seed live membership");

    // A second account the caller also holds a role on, then soft-deleted
    // through the real write path (never a bare-pool/backdoor mutation).
    let tombstoned = seed_account(&backend, "did:plc:tombstone-owner", "gone.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: tombstoned.id.clone(),
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("seed tombstoned membership");
    let mut uow = backend.database().begin().await.expect("begin");
    uow.accounts()
        .soft_delete(&tombstoned.id)
        .await
        .expect("soft delete");
    uow.commit().await.expect("commit soft delete");

    let res = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    // Wrapped at the /api/v1 mint (contract R7): rows ride under "accounts" so
    // pagination can later land additively as a sibling key.
    let rows = body["accounts"].as_array().expect("accounts array");
    assert_eq!(
        rows.len(),
        1,
        "the soft-deleted account is excluded: {body}"
    );
    assert_eq!(rows[0]["id"], live.id.to_string());
}

// A member's own role alias (a free-form label, e.g. an Owner aliased "Studio
// Head") rides along on the listing row when they set one, and is absent —
// never a literal `""` or `null`-shaped-but-present key — when they haven't.
#[tokio::test]
async fn a_members_role_alias_rides_along_when_set_and_is_absent_when_not() {
    let did = "did:plc:aliaslister";
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())).profile(Profile::new(
            Did::from(did.to_string()),
            "lister.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "lister.bsky.social").await;
    let me = backend
        .find_by_did(&Did::from(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // One account where the caller's role carries an alias, one where it doesn't.
    let aliased = seed_account(&backend, "did:plc:aliased-owner", "aliased.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id.clone(),
            account_id: aliased.id.clone(),
            role: Role::Manager,
            alias: None,
        })
        .await
        .expect("seat me as a manager");
    backend.seed_role_alias(
        me.id.clone(),
        aliased.id.clone(),
        RoleAlias::new("Studio Head").expect("non-empty alias"),
    );

    let unaliased = seed_account(&backend, "did:plc:unaliased-owner", "unaliased.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: unaliased.id.clone(),
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("seat me as a member");

    let res = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    let rows = body["accounts"].as_array().expect("accounts array");

    let aliased_row = rows
        .iter()
        .find(|row| row["id"] == aliased.id.to_string())
        .expect("the aliased membership is listed");
    assert_eq!(
        aliased_row["alias"], "Studio Head",
        "the caller's own alias for their role rides along: {body}"
    );

    let unaliased_row = rows
        .iter()
        .find(|row| row["id"] == unaliased.id.to_string())
        .expect("the unaliased membership is listed");
    assert!(
        unaliased_row
            .get("alias")
            .is_none_or(|alias| alias.is_null()),
        "no alias was set, so the field is absent (or null), never an empty string: {body}"
    );
}

// An anonymous caller gets a 401 problem+json, exactly like `GET /me` — never a
// redirect, since the frontend calls this endpoint.
#[tokio::test]
async fn anonymous_caller_is_turned_away_with_401() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:lister3".to_string())).profile(
            Profile::new(
                Did::from("did:plc:lister3".to_string()),
                "lister.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);

    let res = client()
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_problem(res, 401, "not_authenticated").await;
}
