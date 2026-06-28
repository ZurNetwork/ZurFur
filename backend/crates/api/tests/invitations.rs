//! End-to-end invitation flow: issuing and issuer-revocation (ZMVP-32), and the
//! invitee-side accept and decline (ZMVP-20). Same in-process fakes as the other
//! account e2e suites: no network, no database.
use std::sync::Arc;

use adapter_mem::{
    MemAccountRepo, MemAuthenticator, MemDidMinter, MemProfileCache, MemProfileSource, MemUserRepo,
};
use api::{AppState, Config, Environment};
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        invitation::{Invitation, InvitationState},
        profile::Profile,
        role::Role,
        user::UserId,
    },
    ports::{AccountRepo, UserRepo},
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process, returning the base URL plus
/// typed handles to the repos so a test can introspect them after the flow.
async fn spawn_app(did: &str) -> (String, Arc<MemUserRepo>, Arc<MemAccountRepo>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let user_repo = Arc::new(MemUserRepo::new());
    let account_repo = Arc::new(MemAccountRepo::new());
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url: "postgres://unused".to_string(),
            log_level: "info".to_string(),
        },
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        user_repo: user_repo.clone(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "owner.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: Arc::new(MemProfileCache::new()),
        account_repo: account_repo.clone(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), user_repo, account_repo)
}

/// A cookie-keeping client that does not auto-follow redirects.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Drives the two-step sign-in so the client's cookie jar carries a live session.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=owner.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin should redirect to the PDS");

    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback should redirect on success");
}

/// Founds an account and returns its id — the shared first step of every test.
async fn found_account(client: &reqwest::Client, base: &str, name: &str) -> String {
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding an account returns 201");
    let body: serde_json::Value = res.json().await.expect("json body");
    body["id"]
        .as_str()
        .expect("response carries the account id")
        .to_string()
}

// AC1/AC3 — an Owner invites a non-member; a pending invitation recording the
// invited User, the account, the offered role, and the inviter is created.
#[tokio::test]
async fn owner_invites_a_user_and_a_pending_invitation_is_recorded() {
    let did = "did:plc:e2einviter";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let invitee_did = "did:plc:e2einvitee";
    let res = client
        .post(format!("{base}/accounts/{account_id}/invitations"))
        .json(&serde_json::json!({ "user": invitee_did, "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations");
    assert_eq!(res.status(), 201, "an Owner's invitation is issued");
    let body: serde_json::Value = res.json().await.expect("json body");
    assert_eq!(body["user"], invitee_did, "the response echoes the invitee");
    assert_eq!(
        body["role"], "member",
        "the response echoes the offered role"
    );
    assert_eq!(body["state"], "pending", "the invitation is pending");
    assert!(
        body["id"].as_str().is_some(),
        "the response carries the new invitation's id"
    );

    // A pending invitation is recorded for the invitee, naming the Owner as inviter.
    let invitee = user_repo
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let owner = user_repo
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let pending = account_repo
        .find_pending_invitation(account, invitee.id)
        .await
        .expect("find_pending_invitation")
        .expect("a pending invitation exists");
    assert_eq!(pending.state, InvitationState::Pending);
    assert_eq!(
        pending.inviter, owner.id,
        "the inviter is recorded (Roles 4a)"
    );
}

// AC2 — the offered role must sit strictly below the inviter's rank. Owner is
// never offerable (it would be a transfer, not an invitation); refused, nothing stored.
#[tokio::test]
async fn inviting_at_owner_is_refused() {
    let did = "did:plc:e2einvowner";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let invitee_did = "did:plc:e2ewouldbeowner";
    let res = client
        .post(format!("{base}/accounts/{account_id}/invitations"))
        .json(&serde_json::json!({ "user": invitee_did, "role": "owner" }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations");
    assert_eq!(res.status(), 403, "Owner cannot be offered by invitation");

    let invitee = user_repo
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    assert!(
        account_repo
            .find_pending_invitation(account, invitee.id)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a refused invitation stores nothing"
    );
}

// AC5 — inviting a User who already has a pending invitation pings them rather than
// creating a second row: at most one pending per (account, user).
#[tokio::test]
async fn re_inviting_a_pending_user_is_idempotent() {
    let did = "did:plc:e2ereinviter";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let invitee_did = "did:plc:e2ereinvitee";
    let invite = |role: &'static str| {
        let client = &client;
        let base = &base;
        let account_id = &account_id;
        async move {
            client
                .post(format!("{base}/accounts/{account_id}/invitations"))
                .json(&serde_json::json!({ "user": invitee_did, "role": role }))
                .send()
                .await
                .expect("POST /accounts/{id}/invitations")
        }
    };

    let first = invite("member").await;
    assert_eq!(first.status(), 201, "the first invitation is issued");
    let first_id = first.json::<serde_json::Value>().await.expect("json")["id"]
        .as_str()
        .expect("first invitation id")
        .to_string();

    let second = invite("admin").await;
    assert!(
        second.status().is_success(),
        "re-inviting a pending user is not an error — it pings them"
    );

    // Still exactly one pending offer, and it's the original (no second row).
    let invitee = user_repo
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let pending = account_repo
        .find_pending_invitation(account, invitee.id)
        .await
        .expect("find_pending_invitation")
        .expect("a pending invitation exists");
    assert_eq!(
        pending.id.to_string(),
        first_id,
        "re-inviting keeps the original pending invitation, not a second"
    );
}

// AC4 — the issuing member revokes a pending invitation; afterward it can no longer
// be accepted (it is no longer the live pending offer, and reads back revoked).
#[tokio::test]
async fn issuer_revokes_a_pending_invitation() {
    let did = "did:plc:e2erevoker";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let invitee_did = "did:plc:e2erevinvitee";
    let res = client
        .post(format!("{base}/accounts/{account_id}/invitations"))
        .json(&serde_json::json!({ "user": invitee_did, "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations");
    assert_eq!(res.status(), 201);

    // Revoke is addressed by the invited user's DID in the body (mirrors revoke_role),
    // not the invitation id — there is at most one pending offer per (account, user).
    let res = client
        .delete(format!("{base}/accounts/{account_id}/invitations"))
        .json(&serde_json::json!({ "user": invitee_did }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/invitations");
    assert_eq!(
        res.status(),
        200,
        "the issuer revokes their pending invitation"
    );

    // The offer is no longer live, and reads back revoked — it can never be accepted.
    let invitee = user_repo
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    assert!(
        account_repo
            .find_pending_invitation(account, invitee.id)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a revoked invitation is no longer a live pending offer"
    );
}

// AC1 — an invitation is the path *to* membership, so inviting someone who already
// holds a role is a state conflict (409): nothing is minted, no row is written.
#[tokio::test]
async fn inviting_an_existing_member_is_a_conflict() {
    let did = "did:plc:e2ememberinviter";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    // Seat the invitee as a member first (a grant is how one joins, ZMVP-15).
    let invitee_did = "did:plc:e2ealreadymember";
    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": invitee_did, "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert!(
        res.status().is_success(),
        "the grant seats them as a member"
    );

    // Inviting that same member is refused as a conflict, minting nothing.
    let res = client
        .post(format!("{base}/accounts/{account_id}/invitations"))
        .json(&serde_json::json!({ "user": invitee_did, "role": "admin" }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations");
    common::assert_problem(res, 409, "already_member").await;

    let invitee = user_repo
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    assert!(
        account_repo
            .find_pending_invitation(account, invitee.id)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a refused invitation stores nothing"
    );
}

// An anonymous visitor cannot invite — turned away at 401 before any lookup.
#[tokio::test]
async fn anonymous_visitor_cannot_invite() {
    let (base, _user_repo, _account_repo) = spawn_app("did:plc:nobody").await;

    let res = client()
        .post(format!("{base}/accounts/{}/invitations", Uuid::now_v7()))
        .json(&serde_json::json!({ "user": "did:plc:whoever", "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations");
    common::assert_problem(res, 401, "not_authenticated").await;
}

/// Seeds an account (founded by a fresh owner) plus a pending invitation for
/// `invitee_did`, directly via the repos. The invitee-side actions (accept/decline)
/// run with the *invitee* as the session user, so the issuing — which needs the
/// owner's session — is set up out-of-band here. Returns (account, invitee, owner).
async fn seed_pending_invite(
    user_repo: &MemUserRepo,
    account_repo: &MemAccountRepo,
    invitee_did: &str,
) -> (AccountId, UserId, UserId) {
    let owner = user_repo
        .provision(&Did::new("did:plc:seedowner".to_string()))
        .await
        .expect("provision owner");
    let invitee = user_repo
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision invitee");
    let (account, owner_membership) = Account::open(
        owner.id,
        Did::new("did:plc:seedacct".to_string()),
        AccountName::try_new("Acme Studio".to_string()).expect("account name"),
        chrono::Utc::now(),
    );
    account_repo
        .create(&account, &owner_membership)
        .await
        .expect("found the account");
    let invitation = Invitation::issue(
        account.id,
        invitee.id,
        Role::Member(None),
        owner.id,
        chrono::Utc::now(),
    );
    account_repo
        .create_invitation(&invitation)
        .await
        .expect("issue the pending invitation");
    (account.id, invitee.id, owner.id)
}

// AC1/AC4 — the invitee actively declines their own pending offer: 200, the offer is
// no longer pending, and they hold no membership.
#[tokio::test]
async fn invitee_declines_a_pending_invitation() {
    let invitee_did = "did:plc:e2edecliner";
    let (base, user_repo, account_repo) = spawn_app(invitee_did).await;
    let (account_id, invitee_id, _owner) =
        seed_pending_invite(&user_repo, &account_repo, invitee_did).await;

    let client = client();
    sign_in(&client, &base).await;
    let res = client
        .post(format!(
            "{base}/accounts/{}/invitations/decline",
            *account_id
        ))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations/decline");
    assert_eq!(
        res.status(),
        200,
        "the invitee declines their pending invitation"
    );

    assert!(
        account_repo
            .find_pending_invitation(account_id, invitee_id)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a declined invitation is no longer a live pending offer"
    );
    assert!(
        account_repo
            .role_of(invitee_id, account_id)
            .await
            .expect("role_of")
            .is_none(),
        "declining mints no membership"
    );
}

// AC1 — declining when there is no pending offer for the signed-in user is a 404
// problem+json; there is nothing for them to decline.
#[tokio::test]
async fn declining_with_no_pending_invitation_is_not_found() {
    let did = "did:plc:e2enopending";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    // An account exists, but the signed-in user holds no invitation to it.
    let owner = user_repo
        .provision(&Did::new("did:plc:seedowner".to_string()))
        .await
        .expect("provision owner");
    let (account, owner_membership) = Account::open(
        owner.id,
        Did::new("did:plc:seedacct".to_string()),
        AccountName::try_new("Acme Studio".to_string()).expect("account name"),
        chrono::Utc::now(),
    );
    account_repo
        .create(&account, &owner_membership)
        .await
        .expect("found the account");

    let client = client();
    sign_in(&client, &base).await;
    let res = client
        .post(format!(
            "{base}/accounts/{}/invitations/decline",
            *account.id
        ))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations/decline");
    common::assert_problem(res, 404, "no_pending_invitation").await;
}

#[tokio::test]
async fn invitee_accepts_and_becomes_a_member() {
    let invitee_did = "did:plc:e2eaccepter";
    let (base, user_repo, account_repo) = spawn_app(invitee_did).await;
    let (account_id, invitee_id, _owner) =
        seed_pending_invite(&user_repo, &account_repo, invitee_did).await;

    let client = client();
    sign_in(&client, &base).await;
    let res = client
        .post(format!(
            "{base}/accounts/{}/invitations/accept",
            *account_id
        ))
        .json(&serde_json::json!({ "listed_on_profile": true }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations/accept");
    assert_eq!(res.status(), 200, "the invitee accepts and joins");

    // The invitee is now a member at the offered role.
    let role = account_repo
        .role_of(invitee_id, account_id)
        .await
        .expect("role_of");
    assert!(
        matches!(role, Some(Role::Member(_))),
        "accepting mints a Member membership at the offered role"
    );
}
