//! End-to-end invitation issuing & revocation (ZMVP-32) — the issuing half of
//! invite-then-accept (acceptance is ZMVP-20). Same in-process fakes as the other
//! account e2e suites: no network, no database.
//!
//! ⚠️ ENGINEER HAND-OFF — these tests are RED until the invitation handler lands.
//! Per the /understand §5 ownership bands, the domain element, the `AccountRepo`
//! ports, and the `adapter-mem` implementation are done and green; the HTTP
//! handler is engineer-owned (the authority policy is a real judgment call — see
//! the OPEN QUESTION below). These tests are the executable contract that handler
//! must satisfy.
//!
//! INTENDED HANDLER (mirror `grant_role` / `revoke_role` in `api/src/lib.rs`):
//!   Routes on `app()`:
//!     POST   /accounts/{id}/invitations                 → issue_invitation
//!     DELETE /accounts/{id}/invitations/{invitation_id} → revoke_invitation
//!
//!   issue_invitation(account_id, { user: DID, role }):
//!     401 no session · 422 bad body / unknown role · 404 missing account
//!     load actor's role (403 if non-member) · 403 unless actor.can_grant(offered)
//!     provision invitee by DID (idempotent, like grant)
//!     409 if invitee is already a member (role_of is Some)
//!     find_pending_invitation(account, invitee):
//!        Some(existing) → idempotent: return it (the "ping", AC5), 2xx, no 2nd row
//!        None           → Invitation::issue(...) → create_invitation → 201
//!     body: { "id", "account", "user", "role", "state": "pending" }
//!
//!   revoke_invitation(account_id, invitation_id):
//!     401 · 404 missing account · 404 unknown invitation
//!     load invitation; 403 unless actor is the inviter (see OPEN QUESTION)
//!     invitation.revoke(now) → 409 if not pending; else revoke_invitation(id) → 200
//!
//! OPEN QUESTION (decide before implementing): the AC says "the *issuing member*
//! may revoke." These tests assert the inviter can. Whether a higher-ranked
//! Owner/Admin may *also* revoke another member's pending invitation is
//! unspecified — confirm the policy, then add/relax a test accordingly.
use std::sync::Arc;

use adapter_mem::{
    MemAccountRepo, MemAuthenticator, MemDidMinter, MemProfileCache, MemProfileSource, MemUserRepo,
};
use api::{AppState, Config, Environment};
use domain::{
    elements::{account::AccountId, did::Did, invitation::InvitationState, profile::Profile},
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
