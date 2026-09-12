//! End-to-end invitation flow: issuing and issuer-revocation, and the
//! invitee-side accept and decline. Same in-process fakes as the other
//! account e2e suites: no network, no database.
use adapter_mem::MemBackend;
use api::AppState;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    did::Did,
    handle::Handle,
    invitation::{Invitation, InvitationState},
    profile::Profile,
    role::Role,
    user::UserId,
    user_account::UserAccount,
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process, returning the base URL plus
/// typed handles to the repos so a test can introspect them after the flow.
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let test_support::runtime::MemRuntime { runtime, backend } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .profile(Profile::new(Did::new(did.to_string()), "owner.bsky.social"))
            .public_url(format!("http://{addr}"))
            .build();
    let state: AppState = runtime;
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
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
    // A handle is required at founding; derive a valid one from the first word of
    // the name (each test has its own backend, so a per-name handle never collides).
    let handle = format!(
        "{}.zurfur.app",
        name.split_whitespace()
            .next()
            .unwrap_or("acct")
            .to_lowercase()
    );
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": name, "handle": handle }))
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
    let (base, backend) = spawn_app(did).await;
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
    let invitee = backend
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let owner = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let account = AccountId::new(Did::new(account_id));
    let pending = backend
        .find_pending_invitation(&account, &invitee.id)
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
    let (base, backend) = spawn_app(did).await;
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

    let invitee = backend
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision");
    let account = AccountId::new(Did::new(account_id));
    assert!(
        backend
            .find_pending_invitation(&account, &invitee.id)
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
    let (base, backend) = spawn_app(did).await;
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
    let invitee = backend
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let account = AccountId::new(Did::new(account_id));
    let pending = backend
        .find_pending_invitation(&account, &invitee.id)
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
    let (base, backend) = spawn_app(did).await;
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
    let invitee = backend
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let account = AccountId::new(Did::new(account_id));
    assert!(
        backend
            .find_pending_invitation(&account, &invitee.id)
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
    let (base, backend) = spawn_app(did).await;
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

    let invitee = backend
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision the invitee");
    let account = AccountId::new(Did::new(account_id));
    assert!(
        backend
            .find_pending_invitation(&account, &invitee.id)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a refused invitation stores nothing"
    );
}

// An anonymous visitor cannot invite — turned away at 401 before any lookup.
#[tokio::test]
async fn anonymous_visitor_cannot_invite() {
    let (base, _backend) = spawn_app("did:plc:nobody").await;

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
    backend: &MemBackend,
    invitee_did: &str,
) -> (AccountId, UserId, UserId) {
    let owner = backend
        .provision(&Did::new("did:plc:seedowner".to_string()))
        .await
        .expect("provision owner");
    let invitee = backend
        .provision(&Did::new(invitee_did.to_string()))
        .await
        .expect("provision invitee");
    let (account, owner_membership) = Account::open(
        owner.id.clone(),
        Did::new("did:plc:seedacct".to_string()),
        "acme.zurfur.app".parse::<Handle>().unwrap(),
        "Acme Studio".parse::<AccountName>().expect("account name"),
        chrono::Utc::now(),
    );
    backend
        .create(&account, &owner_membership)
        .await
        .expect("found the account");
    let invitation = Invitation::issue(
        account.id.clone(),
        invitee.id.clone(),
        Role::Member,
        owner.id.clone(),
        chrono::Utc::now(),
    );
    backend
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
    let (base, backend) = spawn_app(invitee_did).await;
    let (account_id, invitee_id, _owner) = seed_pending_invite(&backend, invitee_did).await;

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
        backend
            .find_pending_invitation(&account_id, &invitee_id)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "a declined invitation is no longer a live pending offer"
    );
    assert!(
        backend
            .role_of(&invitee_id, &account_id)
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
    let (base, backend) = spawn_app(did).await;
    // An account exists, but the signed-in user holds no invitation to it.
    let owner = backend
        .provision(&Did::new("did:plc:seedowner".to_string()))
        .await
        .expect("provision owner");
    let (account, owner_membership) = Account::open(
        owner.id,
        Did::new("did:plc:seedacct".to_string()),
        "acme.zurfur.app".parse::<Handle>().unwrap(),
        "Acme Studio".parse::<AccountName>().expect("account name"),
        chrono::Utc::now(),
    );
    backend
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
    let (base, backend) = spawn_app(invitee_did).await;
    let (account_id, invitee_id, _owner) = seed_pending_invite(&backend, invitee_did).await;

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
    let role = backend
        .role_of(&invitee_id, &account_id)
        .await
        .expect("role_of");
    assert!(
        matches!(role, Some(Role::Member)),
        "accepting mints a Member membership at the offered role"
    );
}

// Ultrareview round (2026-07-18) — inviting a DID that already belongs to
// ANOTHER actor (here: the account's own DID) is a typed 409, never an opaque
// 500: one DID = one actor (DD 34013187), and the wire contract is
// did_belongs_to_another_actor (Engineer ruling).
#[tokio::test]
async fn inviting_an_accounts_own_did_is_a_did_conflict() {
    let did = "did:plc:conflict-owner";
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Conflict Studio").await;

    let account = backend
        .find(&AccountId::new(Did::new(account_id.clone())))
        .await
        .expect("find")
        .expect("the founded account exists");

    let res = client
        .post(format!("{base}/accounts/{account_id}/invitations"))
        .json(&serde_json::json!({ "user": account.id.as_str(), "role": "member" }))
        .send()
        .await
        .expect("POST invite with an account's DID");
    assert_eq!(
        res.status(),
        409,
        "an existing actor's DID cannot be provisioned as a User"
    );
    let problem: serde_json::Value = res.json().await.expect("problem+json body");
    let conflict_code = problem["code"].as_str().unwrap_or_default().to_string();
    assert_eq!(conflict_code, "did_belongs_to_another_actor");
}

// Ordering guard (security review, PR #196) — the actor's own standing is
// settled BEFORE the target is looked at. Until it was, a signed-in NON-MEMBER
// could tell three states of an arbitrary DID apart through this one route:
// `409 already_member` (a member), `404 member_not_found` (invited, so the
// actor check was reached), `404 no_pending_invitation` (neither). That is a
// membership *and* invitation oracle over an account the caller holds no
// standing in, and a pending invitation is not public. All three must answer
// byte-identically now.
#[tokio::test]
async fn a_non_member_learns_nothing_about_a_target_through_invitation_revoke() {
    let (base, backend) = spawn_app("did:plc:e2eprobe").await;
    let (account_id, invited_id, _owner) =
        seed_pending_invite(&backend, "did:plc:e2eprobe-invited").await;

    // A seated member on the same account — the `already_member` arm.
    let member = backend
        .provision(&Did::new("did:plc:e2eprobe-member".to_string()))
        .await
        .expect("provision the member");
    let membership = UserAccount {
        user_id: member.id.clone(),
        account_id: account_id.clone(),
        alias: None,
        role: Role::Member,
    };
    backend
        .grant_role(&membership)
        .await
        .expect("seat the member");

    let client = client();
    sign_in(&client, &base).await; // did:plc:e2eprobe holds NO role here

    let mut answers = Vec::new();
    for target in [
        member.id.to_string(),                   // a member
        invited_id.to_string(),                  // invited, not yet a member
        "did:plc:e2eprobe-stranger".to_string(), // neither
    ] {
        let res = client
            .delete(format!("{base}/accounts/{}/invitations", *account_id))
            .json(&serde_json::json!({ "user": target }))
            .send()
            .await
            .expect("DELETE /accounts/{id}/invitations");
        assert_eq!(res.status(), 404, "a non-member is refused for {target}");
        answers.push(res.text().await.expect("problem body"));
    }

    let refusal: serde_json::Value =
        serde_json::from_str(&answers[0]).expect("the refusal is problem+json");
    assert_eq!(
        refusal["code"], "member_not_found",
        "the closed door is the actor's own missing membership",
    );
    assert_eq!(
        answers[0], answers[1],
        "a member and an invitee must be indistinguishable to a non-member",
    );
    assert_eq!(
        answers[1], answers[2],
        "an invitee and a stranger must be indistinguishable to a non-member",
    );

    // The probing changed nothing: the pending offer still stands.
    assert!(
        backend
            .find_pending_invitation(&account_id, &invited_id)
            .await
            .expect("find_pending_invitation")
            .is_some(),
        "a refused revoke leaves the offer standing",
    );
}

// Liveness gate (security review, PR #196) — `role_of` reads the membership
// table alone, with no tombstone predicate, so until the gate landed an Owner
// could still issue invitations into their own soft-deleted account (DD
// `23003138`).
#[tokio::test]
async fn a_soft_deleted_account_takes_no_new_invitations() {
    let (base, backend) = spawn_app("did:plc:seedowner").await;
    let (account_id, _invited, _owner) =
        seed_pending_invite(&backend, "did:plc:e2etombstone-invited").await;

    // Tombstone the account. Its memberships and the pending offer survive —
    // only the account row is stamped — which is exactly why standing alone was
    // never a sufficient gate.
    let handle: Handle = "acme.zurfur.app".parse().expect("valid handle");
    backend.seed_soft_deleted_account(&account_id, &handle);
    assert!(
        backend.find(&account_id).await.expect("find").is_none(),
        "the account reads back as gone",
    );

    let client = client();
    sign_in(&client, &base).await; // signed in as the account's Owner

    let newcomer_did = Did::new("did:plc:e2etombstone-newcomer".to_string());
    let res = client
        .post(format!("{base}/accounts/{}/invitations", *account_id))
        .json(&serde_json::json!({ "user": newcomer_did.as_str(), "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/invitations");
    common::assert_problem(res, 404, "account_not_found").await;

    let newcomer = UserId::new(newcomer_did.clone());
    assert!(
        backend
            .find_pending_invitation(&account_id, &newcomer)
            .await
            .expect("find_pending_invitation")
            .is_none(),
        "no offer was minted into a dead account",
    );
    assert!(
        backend
            .find_by_did(&newcomer_did)
            .await
            .expect("find_by_did")
            .is_none(),
        "and no User was provisioned for the would-be invitee",
    );
}

// Liveness gate, invitee side — an offer outliving its account cannot be
// redeemed: accepting a soft-deleted account's invitation would seat a
// membership in something already gone (DD `23003138`).
#[tokio::test]
async fn a_soft_deleted_accounts_invitation_cannot_be_accepted() {
    let invitee_did = "did:plc:e2etombstone-accepter";
    let (base, backend) = spawn_app(invitee_did).await;
    let (account_id, invitee_id, _owner) = seed_pending_invite(&backend, invitee_did).await;

    let handle: Handle = "acme.zurfur.app".parse().expect("valid handle");
    backend.seed_soft_deleted_account(&account_id, &handle);

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
    common::assert_problem(res, 404, "account_not_found").await;

    assert!(
        backend
            .role_of(&invitee_id, &account_id)
            .await
            .expect("role_of")
            .is_none(),
        "a refused accept seats no membership in a dead account",
    );
}
