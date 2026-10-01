//! An Owner transfers Account ownership to another member
//! (`POST /accounts/{id}/transfer`). Covers the four acceptance criteria — the
//! transfer is immediate and effective, the named member becomes the sole Owner, the
//! prior Owner becomes Admin, and only the current Owner may transfer and only to an
//! existing member — plus the follow-on enablement (a former Owner, now Admin, can
//! leave). Authority and the "another member" rule are the handler's, so they're
//! exercised here against the in-process fakes; the `parent` re-homing (rule 5) is the
//! store's job and is proven against PostgreSQL in `adapter-pg`'s own tests (the mem
//! fake doesn't model `parent`). DESIGN/Roles rule 8.
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountName},
    did::Did,
    handle::Handle,
    profile::Profile,
    role::Role,
    user_account::UserAccount,
};
use serde_json::{Value, json};
use test_support::http::{client, serve, sign_in};
use uuid::Uuid;

mod common;

/// Founds an account for the signed-in Owner and returns its id.
async fn found_account(client: &reqwest::Client, base: &str, name: &str, handle: &str) -> String {
    let res = client
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": name, "handle": handle }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding returns 201");
    let body: Value = res.json().await.expect("json");
    body["id"].as_str().expect("account id").to_string()
}

#[tokio::test]
async fn owner_transfers_ownership_and_the_roles_swap() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:xferowner".to_string())).profile(
            Profile::new(
                Did::from("did:plc:xferowner".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let account_id = found_account(&client, &base, "Hand-Off Studio", "handoff.zurfur.app").await;

    // Seat a second, existing member — the transfer target.
    let heir = backend
        .provision(&Did::from("did:plc:heir".to_string()))
        .await
        .expect("provision heir");
    let owner = backend
        .find_by_did(&Did::from("did:plc:xferowner".to_string()))
        .await
        .expect("find owner")
        .expect("sign-in provisioned owner");
    backend
        .grant_role(&UserAccount {
            user_id: heir.id.clone(),
            account_id: domain::elements::account::AccountId::from(Did::from(account_id.clone())),
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("seat the heir as a member");

    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:heir" }))
        .send()
        .await
        .expect("POST /transfer");
    assert_eq!(res.status(), 200, "the transfer settles immediately");
    let body: Value = res.json().await.expect("json");
    assert_eq!(body["account"].as_str(), Some(account_id.as_str()));
    assert_eq!(body["owner"].as_str(), Some("did:plc:heir"));
    assert_eq!(body["previous_owner"].as_str(), Some("did:plc:xferowner"));

    let account = domain::elements::account::AccountId::from(Did::from(account_id));
    // AC: the named member is now the sole Owner; the prior Owner is now Admin.
    assert_eq!(
        backend
            .role_of(&heir.id, &account)
            .await
            .expect("role_of heir"),
        Some(Role::Owner),
        "the heir is the new Owner",
    );
    assert_eq!(
        backend
            .role_of(&owner.id, &account)
            .await
            .expect("role_of owner"),
        Some(Role::Admin),
        "the prior Owner is demoted to Admin",
    );
}

#[tokio::test]
async fn only_the_owner_may_transfer() {
    // The signed-in user is a mere Member of an account someone else owns.
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:notowner".to_string())).profile(
            Profile::new(
                Did::from("did:plc:notowner".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let me = backend
        .find_by_did(&Did::from("did:plc:notowner".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");
    let host = backend
        .provision(&Did::from("did:plc:host".to_string()))
        .await
        .expect("provision host");
    let (account, owner_membership) = Account::open(
        host.id,
        Did::from("did:plc:hostacct".to_string()),
        "host.zurfur.app".parse::<Handle>().unwrap(),
        "Host Studio".parse::<AccountName>().unwrap(),
        Utc::now(),
    );
    backend
        .create(&account, &owner_membership)
        .await
        .expect("found host account");
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: account.id.clone(),
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("seat me as a member");

    let res = client
        .post(format!("{base}/accounts/{}/transfer", account.id))
        .json(&json!({ "new_owner": "did:plc:host" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 403, "forbidden").await;
}

#[tokio::test]
async fn cannot_transfer_to_a_non_member() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:lonelyowner".to_string())).profile(
            Profile::new(
                Did::from("did:plc:lonelyowner".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let account_id = found_account(&client, &base, "Solo Studio", "solo.zurfur.app").await;

    // A user who exists but holds no membership in this account is not a valid target.
    backend
        .provision(&Did::from("did:plc:stranger".to_string()))
        .await
        .expect("provision stranger");

    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:stranger" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 404, "member_not_found").await;
}

#[tokio::test]
async fn cannot_transfer_to_an_unknown_did() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:owner2".to_string())).profile(Profile::new(
            Did::from("did:plc:owner2".to_string()),
            "owner.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let account_id = found_account(&client, &base, "Studio Two", "two.zurfur.app").await;

    // A DID we have never recognized is, by definition, not a member.
    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:neverseen" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 404, "member_not_found").await;
}

#[tokio::test]
async fn cannot_transfer_to_yourself() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:selfxfer".to_string())).profile(
            Profile::new(
                Did::from("did:plc:selfxfer".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let account_id = found_account(&client, &base, "Mine Studio", "mine.zurfur.app").await;

    // Ownership moves to *another* member (Roles rule 8) — self-transfer is refused.
    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:selfxfer" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 422, "invalid_request").await;
}

#[tokio::test]
async fn transferring_on_a_missing_account_is_404() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:ghostowner".to_string())).profile(
            Profile::new(
                Did::from("did:plc:ghostowner".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let res = client
        .post(format!("{base}/accounts/{}/transfer", Uuid::now_v7()))
        .json(&json!({ "new_owner": "did:plc:whoever" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 404, "account_not_found").await;
}

#[tokio::test]
async fn after_transfer_the_former_owner_can_leave() {
    // The ZMVP-21 enablement: a sole Owner can't leave, but after transferring they
    // are an Admin and may walk out.
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:exitowner".to_string())).profile(
            Profile::new(
                Did::from("did:plc:exitowner".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    let account_id = found_account(&client, &base, "Exit Studio", "exit.zurfur.app").await;
    let account = domain::elements::account::AccountId::from(Did::from(account_id.clone()));

    let heir = backend
        .provision(&Did::from("did:plc:successor".to_string()))
        .await
        .expect("provision successor");
    backend
        .grant_role(&UserAccount {
            user_id: heir.id,
            account_id: account,
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("seat the successor");

    // Before transfer, the Owner cannot leave.
    let res = client
        .delete(format!("{base}/accounts/{account_id}/members/me"))
        .send()
        .await
        .expect("DELETE members/me");
    common::assert_problem(res, 409, "owner_cannot_leave").await;

    // Transfer, then the former Owner (now Admin) may leave.
    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:successor" }))
        .send()
        .await
        .expect("POST /transfer");
    assert_eq!(res.status(), 200, "transfer settles");

    let res = client
        .delete(format!("{base}/accounts/{account_id}/members/me"))
        .send()
        .await
        .expect("DELETE members/me");
    assert_eq!(res.status(), 204, "a former Owner, now Admin, may leave");
}
