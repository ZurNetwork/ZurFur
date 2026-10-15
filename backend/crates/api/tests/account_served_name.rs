//! The account routes on the in-memory stores serve a stored name as the safe-to-show rule
//! keeps it: a refused name as the Account's handle, a stripped one in its kept form, an
//! accepted one as stored; on `GET /accounts` (Owner and Member) and
//! `PATCH /accounts/{id}/handle`, and never in a founding refusal.

use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountName},
    did::Did,
    handle::Handle,
    profile::Profile,
    role::Role,
    user::UserId,
    user_account::UserAccount,
};
use serde_json::{Value, json};
use test_support::http::{Served, client, serve, sign_in};

/// A name holding a right-to-left override, a bidirectional control the rule refuses.
const WITH_A_BIDI_OVERRIDE: &str = "Studio\u{202E}gnp.exe";

/// The right-to-left override itself.
const RIGHT_TO_LEFT_OVERRIDE: char = '\u{202E}';

/// A name with a zero-width space inside, which the rule strips rather than refuses.
const WITH_A_ZERO_WIDTH_SPACE: &str = "Ref\u{200B}sheet";

/// The same name as the rule keeps it.
const AS_KEPT: &str = "Refsheet";

/// A name holding a `/`, which the rule for a name safe to show accepts.
const WITH_A_SLASH: &str = "AC/DC Studio";

/// A name the rule accepts unchanged.
const ORDINARY: &str = "Ordinary Studio";

/// The signed-in viewer's DID.
const VIEWER: &str = "did:plc:servednameviewer";

/// The signed-in viewer's handle.
const VIEWER_HANDLE: &str = "viewer.bsky.social";

/// The in-memory API, with the viewer signed in on the returned client.
async fn signed_in() -> (Served, reqwest::Client, UserId) {
    let viewer = Did::from(VIEWER.to_owned());
    let profile = Profile::new(viewer.clone(), VIEWER_HANDLE);
    let runtime = test_support::runtime::mem(&viewer).profile(profile);
    let served = serve(runtime, api::app).await;
    let client = client();
    sign_in(&client, &served.base_url, VIEWER_HANDLE).await;
    let user = served
        .backend
        .find_by_did(&viewer)
        .await
        .expect("find the viewer")
        .expect("the viewer is signed in");
    (served, client, user.id)
}

/// Seeds an Account owned by `owner` under `<label>.zurfur.app` whose stored name is `stored`,
/// written straight to the store as a name stored before founding checked it.
async fn seed_account(backend: &MemBackend, owner: UserId, label: &str, stored: &str) -> Account {
    let did = Did::from(format!("did:plc:served{label}"));
    let handle = format!("{label}.zurfur.app")
        .parse::<Handle>()
        .expect("a valid handle");
    let name = stored.parse::<AccountName>().expect("a storable name");
    let (account, owner_seat) = Account::open(owner, did, handle, name, Utc::now());
    backend
        .create(&account, &owner_seat)
        .await
        .expect("seed the Account");
    account
}

/// Seats `member` as a Member of `account`.
async fn seat_member(backend: &MemBackend, member: UserId, account: &Account) {
    let seat = UserAccount {
        user_id: member,
        account_id: account.id.clone(),
        role: Role::Member,
        alias: None,
    };
    backend.grant_role(&seat).await.expect("seat the Member");
}

/// `GET /accounts` as `client`: the raw body and the name served for each handle.
async fn listed_names(client: &reqwest::Client, base: &str) -> (String, Vec<(String, String)>) {
    let response = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts is answered");
    assert_eq!(response.status(), 200);
    let raw = response.text().await.expect("a body");
    let body: Value = serde_json::from_str(&raw).expect("a JSON body");
    let mut names: Vec<(String, String)> = body["accounts"]
        .as_array()
        .expect("an accounts array")
        .iter()
        .map(|listing| {
            let handle = listing["handle"].as_str().expect("a handle").to_owned();
            let name = listing["name"].as_str().expect("a name").to_owned();
            (handle, name)
        })
        .collect();
    names.sort();
    (raw, names)
}

/// Whether `raw` carries a right-to-left override, raw or JSON-escaped.
fn carries_the_override(raw: &str) -> bool {
    raw.contains(RIGHT_TO_LEFT_OVERRIDE) || raw.to_lowercase().contains("\\u202e")
}

#[tokio::test]
async fn the_owner_s_list_serves_each_stored_name_as_the_rule_keeps_it_or_the_handle() {
    let (served, client, viewer) = signed_in().await;
    let backend = &served.backend;
    seed_account(backend, viewer.clone(), "bidi", WITH_A_BIDI_OVERRIDE).await;
    seed_account(backend, viewer.clone(), "kept", WITH_A_ZERO_WIDTH_SPACE).await;
    seed_account(backend, viewer.clone(), "acdc", WITH_A_SLASH).await;
    seed_account(backend, viewer, "ordinary", ORDINARY).await;

    let (raw, names) = listed_names(&client, &served.base_url).await;

    let expected = [
        ("acdc.zurfur.app", WITH_A_SLASH),
        ("bidi.zurfur.app", "bidi.zurfur.app"),
        ("kept.zurfur.app", AS_KEPT),
        ("ordinary.zurfur.app", ORDINARY),
    ]
    .map(|(handle, name)| (handle.to_owned(), name.to_owned()));
    assert_eq!(names, expected);
    assert!(!carries_the_override(&raw), "{raw}");
    assert!(!raw.contains('\u{200B}'), "{raw}");
}

#[tokio::test]
async fn a_member_s_list_serves_a_refused_stored_name_as_the_handle() {
    let (served, client, viewer) = signed_in().await;
    let backend = &served.backend;
    let owner = backend
        .provision(&Did::from("did:plc:servednameowner".to_owned()))
        .await
        .expect("provision the Owner");
    let account = seed_account(backend, owner.id, "bidi", WITH_A_BIDI_OVERRIDE).await;
    seat_member(backend, viewer, &account).await;

    let (raw, names) = listed_names(&client, &served.base_url).await;

    let expected = [("bidi.zurfur.app".to_owned(), "bidi.zurfur.app".to_owned())];
    assert_eq!(names, expected);
    assert!(!carries_the_override(&raw), "{raw}");
}

#[tokio::test]
async fn a_handle_change_serves_a_refused_stored_name_as_the_new_handle() {
    let (served, client, viewer) = signed_in().await;
    let account = seed_account(&served.backend, viewer, "bidi", WITH_A_BIDI_OVERRIDE).await;
    let change = json!({ "handle": "renamed.zurfur.app" });

    let response = client
        .patch(format!(
            "{}/accounts/{}/handle",
            served.base_url, account.id
        ))
        .json(&change)
        .send()
        .await
        .expect("PATCH /accounts/{id}/handle is answered");

    assert_eq!(response.status(), 200);
    let raw = response.text().await.expect("a body");
    let body: Value = serde_json::from_str(&raw).expect("a JSON body");
    assert_eq!(body["handle"], "renamed.zurfur.app");
    assert_eq!(body["name"], "renamed.zurfur.app");
    assert!(!carries_the_override(&raw), "{raw}");
}

#[tokio::test]
async fn a_handle_change_serves_an_accepted_stored_name_in_its_kept_form() {
    let (served, client, viewer) = signed_in().await;
    let account = seed_account(&served.backend, viewer, "kept", WITH_A_ZERO_WIDTH_SPACE).await;
    let change = json!({ "handle": "renamed.zurfur.app" });

    let response = client
        .patch(format!(
            "{}/accounts/{}/handle",
            served.base_url, account.id
        ))
        .json(&change)
        .send()
        .await
        .expect("PATCH /accounts/{id}/handle is answered");

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("a JSON body");
    assert_eq!(body["name"], AS_KEPT);
}

#[tokio::test]
async fn founding_never_serves_a_refused_name_and_serves_an_accepted_one_as_kept() {
    let (served, client, _viewer) = signed_in().await;
    let found = |name: &'static str, label: &'static str| {
        let founding = json!({ "name": name, "handle": format!("{label}.zurfur.app") });
        client
            .post(format!("{}/accounts", served.base_url))
            .json(&founding)
            .send()
    };

    let refused = found(WITH_A_BIDI_OVERRIDE, "refused")
        .await
        .expect("answered");
    let kept = found(WITH_A_ZERO_WIDTH_SPACE, "kept")
        .await
        .expect("answered");
    let slashed = found(WITH_A_SLASH, "acdc").await.expect("answered");

    assert_eq!(refused.status(), 422);
    let refusal = refused.text().await.expect("a body");
    assert!(!carries_the_override(&refusal), "{refusal}");
    assert!(!refusal.contains("gnp.exe"), "{refusal}");
    assert_eq!(kept.status(), 201);
    let kept: Value = kept.json().await.expect("a JSON body");
    assert_eq!(kept["name"], AS_KEPT);
    assert_eq!(slashed.status(), 201);
    let slashed: Value = slashed.json().await.expect("a JSON body");
    assert_eq!(slashed["name"], WITH_A_SLASH);
}
