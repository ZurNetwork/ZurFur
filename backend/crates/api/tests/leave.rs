//! A member leaves their own account (`DELETE /accounts/{id}/members/me`).
//! Covers the handler-side preconditions (Owner can't leave → 409, a non-member →
//! 404) and the happy path (a member leaves → 204, and is no longer a member). The
//! role-tree re-homing and invitation revocation are the store's job and are proven
//! against PostgreSQL in `adapter-pg`'s own tests (the mem fake doesn't model
//! `parent`). Same in-process fakes as the other account e2e tests.
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

#[tokio::test]
async fn the_owner_cannot_leave_their_own_account() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:leaveowner".to_string())).profile(
            Profile::new(
                Did::from("did:plc:leaveowner".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let (base, _backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    // Founding makes the signed-in user the Owner.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": "Solo Studio", "handle": "solo.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding returns 201");
    let body: Value = res.json().await.expect("json");
    let account_id = body["id"].as_str().expect("account id");

    let res = client
        .delete(format!("{base}/accounts/{account_id}/members/me"))
        .send()
        .await
        .expect("DELETE members/me");
    common::assert_problem(res, 409, "owner_cannot_leave").await;
}

#[tokio::test]
async fn leaving_an_account_you_are_not_a_member_of_is_404() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:leavestranger".to_string())).profile(
            Profile::new(
                Did::from("did:plc:leavestranger".to_string()),
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
        .delete(format!("{base}/accounts/{}/members/me", Uuid::now_v7()))
        .send()
        .await
        .expect("DELETE members/me");
    common::assert_problem(res, 404, "member_not_found").await;
}

#[tokio::test]
async fn a_member_leaves_and_is_no_longer_a_member() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:leaver".to_string())).profile(Profile::new(
            Did::from("did:plc:leaver".to_string()),
            "owner.bsky.social",
        )),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "owner.bsky.social").await;

    // The signed-in user is provisioned by sign-in; seat them as a *Member* of an
    // account someone else owns, so leaving isn't blocked by the Owner rule.
    let me = backend
        .find_by_did(&Did::from("did:plc:leaver".to_string()))
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
            user_id: me.id.clone(),
            account_id: account.id.clone(),
            role: Role::Member,
            alias: None,
        })
        .await
        .expect("seat me as a member");

    let res = client
        .delete(format!("{base}/accounts/{}/members/me", account.id))
        .send()
        .await
        .expect("DELETE members/me");
    assert_eq!(
        res.status(),
        204,
        "a member leaves on their own action, no approval"
    );

    let role = backend.role_of(&me.id, &account.id).await.expect("role_of");
    assert_eq!(role, None, "after leaving, the user holds no role");
}
