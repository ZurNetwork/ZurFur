//! `GET /den` and `GET /den/{*path}` on the in-memory fixture world, signed in as alice: the
//! session first, then the query, then the path; every miss the same 404, byte for byte;
//! the raw path split before each piece is decoded once; `private, no-store` on every
//! answer; and the API reaching Atta only through the application.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use adapter_mem::{
    MemBackend,
    fixture::{
        abco, alice, bob, old_sketch, seed_commission, seed_identity, supreme_arts, untitled,
        untitled_files,
    },
};
use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        handle::Handle,
        profile::Profile,
        role::Role,
        user::{User, UserId},
    },
    ports::{UnitOfWork, UserStore},
};
use serde_json::{Value, json};
use test_support::{
    den::{Answer, PRIVATE_NO_STORE, cache_control, entry_names, get, ok_json},
    http::{client, serve, sign_in},
    log_capture::CapturedLog,
};
use tracing_subscriber::EnvFilter;

mod common;
use common::assert_problem;

/// A deactivated Account owned by bob, with alice a Member: she may never see it.
const CLOSED_STUDIO_DID: &str = "did:plc:c1osedstudiofixtureaaaaa";

/// A well-formed page token, far past the end of any fixture listing.
const PAST_THE_END: &str = "0100000000000003e8";

/// The Den's engine, which the API reaches only through the application.
const ENGINE: &str = "atta";

/// A served app on the fixture world, the deactivated Account seeded beside it.
async fn world() -> (String, MemBackend) {
    world_over(api::app).await
}

/// [`world`], with `app` building the router over the runtime.
async fn world_over(
    app: impl FnOnce(composition::Runtime) -> axum::Router,
) -> (String, MemBackend) {
    let alice_profile =
        Profile::new(alice().did().clone(), "alice.test").with_display_name("Alice");
    let builder = test_support::runtime::mem(alice().did()).profile(alice_profile);
    let served = serve(builder, app).await;
    seed_identity(&served.backend)
        .await
        .expect("seeds identity's fixtures");
    seed_commission(&served.backend, &alice(), &bob(), &supreme_arts())
        .await
        .expect("seeds commission's fixtures");
    seed_closed_studio(&served.backend).await;
    (served.base_url, served.backend)
}

/// Seeds "Closed Studio", bob its Owner and alice a Member, and deactivates it.
async fn seed_closed_studio(backend: &MemBackend) {
    let closed_studio = AccountId::from(Did::from(CLOSED_STUDIO_DID.to_owned()));
    let handle = "closed-studio.zurfur.app"
        .parse::<Handle>()
        .expect("a valid handle");
    let name = "Closed Studio"
        .parse::<AccountName>()
        .expect("a valid name");
    let (account, ownership) =
        Account::open(bob(), closed_studio.did().clone(), handle, name, Utc::now());
    let database = backend.database();
    application::transaction(&*database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create(&account, &ownership).await?;
        test_support::membership::seat_member(
            uow,
            &closed_studio,
            &alice(),
            Role::Member,
            &bob(),
            Utc::now(),
        )
        .await?;
        uow.accounts().soft_delete(&closed_studio).await
    })
    .await
    .expect("seeds a deactivated Account");
}

/// The fixture world with a client signed in as alice.
async fn as_alice() -> (String, reqwest::Client) {
    let (base, _backend) = world().await;
    let alice = client();
    sign_in(&alice, &base, "alice.test").await;
    (base, alice)
}

#[tokio::test]
async fn no_session_is_401_before_the_query_and_the_path() {
    let (base, _backend) = world().await;
    let anonymous = client();
    let paths = [
        "/den".to_owned(),
        "/den/".to_owned(),
        "/den/accounts".to_owned(),
        "/den?nope=1".to_owned(),
        "/den/nope?includeDeleted=yes".to_owned(),
        "/den/%FF?pageToken=zz".to_owned(),
        format!("/den/{}", "accounts/".repeat(17)),
    ];

    for path in paths {
        let response = get(&anonymous, &base, &path).await;

        assert_eq!(cache_control(&response), PRIVATE_NO_STORE, "{path}");
        assert_problem(response, 401, "not_authenticated").await;
    }
}

#[tokio::test]
async fn an_unknown_parameter_is_400_whatever_the_path() {
    let (base, alice) = as_alice().await;
    let bob_did = bob().to_string();
    let paths = [
        "/den?nope=1".to_owned(),
        "/den/?nope=1".to_owned(),
        "/den/nope?nope=1".to_owned(),
        "/den/accounts?includeDeleted=yes&nope".to_owned(),
        format!("/den?viewer={bob_did}"),
        format!("/den/accounts?root={bob_did}"),
    ];

    for path in paths {
        let response = get(&alice, &base, &path).await;

        assert_eq!(cache_control(&response), PRIVATE_NO_STORE, "{path}");
        assert_problem(response, 400, "unknown_parameter").await;
    }
}

#[tokio::test]
async fn a_bad_query_value_is_422_before_the_path_is_read() {
    let (base, alice) = as_alice().await;
    let paths = [
        "/den?includeDeleted=yes".to_owned(),
        "/den/nope?includeDeleted=yes".to_owned(),
        "/den/nope?includeDeleted".to_owned(),
        format!("/den/nope?pageToken={PAST_THE_END}&pageToken={PAST_THE_END}"),
        "/den/nope?includeDeleted=false&includeDeleted=false".to_owned(),
        "/den/nope?pageToken=zz".to_owned(),
        "/den/nope?pageToken=".to_owned(),
        "/den/?pageToken=zz".to_owned(),
        "/den/%FF?pageToken=zz".to_owned(),
    ];

    for path in paths {
        let response = get(&alice, &base, &path).await;

        assert_eq!(cache_control(&response), PRIVATE_NO_STORE, "{path}");
        assert_problem(response, 422, "invalid_request").await;
    }
}

#[tokio::test]
async fn every_miss_is_the_same_404_byte_for_byte() {
    let (base, alice) = as_alice().await;
    let bob_did = bob().to_string();
    let abco_did = abco().to_string();
    let supreme_arts_did = supreme_arts().to_string();
    let untitled = untitled().to_string();
    let [attachment, _] = untitled_files();
    let causes = [
        // A nonexistent child.
        "/den/nope".to_owned(),
        "/den/accounts/did:plc:nope".to_owned(),
        // Malformed pieces: a space, a non-ASCII letter, bytes that aren't UTF-8, a broken
        // escape, an empty piece.
        "/den/accounts/a%20b".to_owned(),
        "/den/accounts/%C3%A9".to_owned(),
        "/den/accounts/%FF".to_owned(),
        "/den/accounts/%2".to_owned(),
        "/den/accounts/".to_owned(),
        "/den//accounts".to_owned(),
        // Seventeen pieces.
        format!("/den/{}accounts", "accounts/".repeat(16)),
        // The empty path.
        "/den/".to_owned(),
        // A path that continues past a file.
        format!("/den/commissions/{untitled}/changelog/more"),
        format!("/den/commissions/{untitled}/attachments/{attachment}/more"),
        // Another user's DID as a piece, under alice's session.
        format!("/den/{bob_did}"),
        format!("/den/accounts/{bob_did}"),
        format!("/den/{bob_did}/accounts"),
        // A character that exists but isn't hers, beside one that doesn't exist.
        format!("/den/characters/{abco_did}"),
        "/den/characters/did:plc:nope".to_owned(),
        // A deactivated Account typed by a Member who isn't its Owner.
        format!("/den/accounts/{CLOSED_STUDIO_DID}"),
        format!("/den/accounts/{CLOSED_STUDIO_DID}?includeDeleted=true"),
        // An encoded slash stays inside its piece: split first, then decode.
        format!("/den/accounts%2F{supreme_arts_did}"),
        // Decoded once only: `%252F` is the valid, absent piece `accounts%2F<did>`.
        format!("/den/accounts%252F{supreme_arts_did}"),
        "/den/%2561ccounts".to_owned(),
    ];
    let first = Answer::read(get(&alice, &base, "/den/nope").await).await;
    let expected_body = json!({
        "type": "urn:zurfur:error:node-not-found",
        "code": "node_not_found",
        "title": "Node not found",
        "detail": "No such node.",
        "status": 404,
    });

    let body: Value = serde_json::from_slice(&first.body).expect("a JSON body");
    assert_eq!(first.status, 404);
    assert_eq!(body, expected_body);
    let cache_header = (
        "cache-control".to_owned(),
        PRIVATE_NO_STORE.as_bytes().to_vec(),
    );
    assert!(first.headers.contains(&cache_header), "{first:?}");
    for cause in causes {
        let answer = Answer::read(get(&alice, &base, &cause).await).await;

        assert_eq!(answer, first, "{cause}");
    }
}

#[tokio::test]
async fn each_piece_is_decoded_once_after_the_split() {
    let (base, alice) = as_alice().await;
    let supreme_arts_did = supreme_arts().to_string();

    let encoded_folder = ok_json(get(&alice, &base, "/den/%61ccounts").await).await;
    let encoded_account = ok_json(
        get(
            &alice,
            &base,
            &format!("/den/accounts/{}", supreme_arts_did.replace(':', "%3A")),
        )
        .await,
    )
    .await;

    assert_eq!(encoded_folder["node"]["segments"], json!(["accounts"]));
    assert_eq!(encoded_account["node"]["name"], "Supreme Arts");
}

#[tokio::test]
async fn alice_s_root_is_hers_and_holds_her_three_open_folders_and_posts() {
    let (base, alice) = as_alice().await;
    let expected_node = json!({
        "name": "Alice",
        "type": "user",
        "kind": "directory",
        "open": { "ownLevel": "private" },
    });
    let expected_posts = json!({
        "segments": ["posts"],
        "name": "posts",
        "type": "posts",
        "kind": "directory",
        "mount": true,
        "open": { "ownLevel": "public", "contentNotShown": true },
    });

    let root = ok_json(get(&alice, &base, "/den").await).await;

    assert_eq!(root["node"], expected_node);
    let listing = &root["listing"];
    assert_eq!(
        entry_names(listing),
        ["accounts", "characters", "commissions", "posts"]
    );
    assert!(listing.get("nextPageToken").is_none(), "{listing}");
    let entries = listing["entries"].as_array().expect("entries");
    let (folders, posts) = entries.split_at(3);
    for entry in folders {
        let name = entry["name"].clone();
        assert_eq!(entry["segments"], json!([name]), "{entry}");
        assert_eq!(entry["kind"], "directory", "{entry}");
        assert_eq!(entry["open"]["ownLevel"], "private", "{entry}");
        assert!(entry.get("mount").is_none(), "{entry}");
    }
    assert_eq!(posts, [expected_posts]);
}

#[tokio::test]
async fn alice_s_posts_open_public_and_empty_with_their_content_not_shown() {
    let (base, alice) = as_alice().await;
    let expected_node = json!({
        "segments": ["posts"],
        "name": "posts",
        "type": "posts",
        "kind": "directory",
        "mount": true,
        "open": { "ownLevel": "public", "contentNotShown": true },
    });
    let expected_crumbs = json!([{ "name": "Alice", "type": "user" }]);

    let posts = ok_json(get(&alice, &base, "/den/posts").await).await;

    assert_eq!(posts["node"], expected_node);
    assert_eq!(posts["crumbs"], expected_crumbs);
    assert_eq!(posts["listing"], json!({}));
}

#[tokio::test]
async fn every_path_below_posts_is_the_one_404_byte_for_byte() {
    let alice_did = alice().to_string();
    let (base, alice) = as_alice().await;
    let below = [
        "/den/posts/anything".to_owned(),
        "/den/posts/anything/deeper".to_owned(),
        format!("/den/posts/{alice_did}"),
        "/den/posts/anything?includeDeleted=true".to_owned(),
    ];
    let not_found = Answer::read(get(&alice, &base, "/den/nope").await).await;

    assert_eq!(not_found.status, 404);
    for path in below {
        let answer = Answer::read(get(&alice, &base, &path).await).await;

        assert_eq!(answer, not_found, "{path}");
    }
}

#[tokio::test]
async fn alice_s_accounts_mounts_supreme_arts() {
    let (base, alice) = as_alice().await;
    let supreme_arts_did = supreme_arts().to_string();
    let expected_crumbs = json!([{ "name": "Alice", "type": "user" }]);

    let accounts = ok_json(get(&alice, &base, "/den/accounts").await).await;

    assert_eq!(accounts["node"]["segments"], json!(["accounts"]));
    assert_eq!(accounts["crumbs"], expected_crumbs);
    let listing = &accounts["listing"];
    assert_eq!(entry_names(listing), ["Supreme Arts"]);
    let supreme_arts = &listing["entries"][0];
    assert_eq!(supreme_arts["mount"], true);
    assert_eq!(supreme_arts["type"], "account");
    assert_eq!(
        supreme_arts["segments"],
        json!(["accounts", supreme_arts_did])
    );
    assert!(supreme_arts.get("open").is_some(), "{supreme_arts}");
}

#[tokio::test]
async fn a_page_past_the_end_is_empty_and_has_no_token() {
    let (base, alice) = as_alice().await;
    let path = format!("/den?includeDeleted=false&pageToken={PAST_THE_END}");

    let root = ok_json(get(&alice, &base, &path).await).await;

    assert_eq!(root["listing"], json!({}));
    assert_eq!(root["node"]["name"], "Alice");
}

#[tokio::test]
async fn include_deleted_shows_alice_nothing_she_may_not_open() {
    let (base, alice) = as_alice().await;

    let accounts = ok_json(get(&alice, &base, "/den/accounts?includeDeleted=true").await).await;

    assert_eq!(entry_names(&accounts["listing"]), ["Supreme Arts"]);
}

#[tokio::test]
async fn old_sketch_opens_marked_archived_without_asking_for_deleted_ones() {
    let (base, alice) = as_alice().await;
    let path = format!("/den/commissions/{}", old_sketch());

    let old_sketch = ok_json(get(&alice, &base, &path).await).await;

    assert_eq!(old_sketch["node"]["name"], "Old sketch");
    assert_eq!(old_sketch["node"]["type"], "commission");
    assert_eq!(old_sketch["node"]["open"]["softDelete"], "archived");
}

#[tokio::test]
async fn the_past_a_file_causes_continue_past_real_files() {
    let (base, alice) = as_alice().await;
    let untitled = untitled();
    let [attachment, _] = untitled_files();
    let files = [
        format!("/den/commissions/{untitled}/changelog"),
        format!("/den/commissions/{untitled}/attachments/{attachment}"),
    ];

    for path in files {
        let file = ok_json(get(&alice, &base, &path).await).await;

        assert_eq!(file["node"]["kind"], "file", "{path}");
        assert_eq!(file["file"], json!({}), "{path}");
    }
}

/// The keys an open node names its thing by.
const THING_KEYS: [&str; 2] = ["accountDid", "commissionId"];

/// Asserts the open node `node` names no thing.
fn assert_no_thing(node: &Value, label: &str) {
    let open = node.get("open").expect("an open node");
    for key in THING_KEYS {
        assert!(open.get(key).is_none(), "{label}: {node}");
    }
}

/// The listing entry of `listing` named `name`.
fn entry<'a>(listing: &'a Value, name: &str) -> &'a Value {
    let entries = listing["entries"].as_array().expect("an entries array");
    entries
        .iter()
        .find(|entry| entry["name"] == name)
        .expect("the entry is listed")
}

#[tokio::test]
async fn an_open_commission_carries_its_id() {
    let (base, alice) = as_alice().await;
    let untitled = untitled().to_string();
    let expected_open = json!({ "ownLevel": "private", "commissionId": untitled });

    let commissions = ok_json(get(&alice, &base, "/den/commissions").await).await;
    let opened = ok_json(get(&alice, &base, &format!("/den/commissions/{untitled}")).await).await;

    let listed = entry(&commissions["listing"], "Untitled");
    assert_eq!(listed["mount"], true);
    assert_eq!(listed["open"], expected_open);
    assert_eq!(opened["node"]["mount"], true);
    assert_eq!(opened["node"]["open"], expected_open);
}

#[tokio::test]
async fn an_account_mount_carries_the_account_s_did_not_the_viewer_s() {
    let (base, alice) = as_alice().await;
    let supreme_arts_did = supreme_arts().to_string();
    let expected_thing = json!(supreme_arts_did);

    let accounts = ok_json(get(&alice, &base, "/den/accounts").await).await;
    let opened =
        ok_json(get(&alice, &base, &format!("/den/accounts/{supreme_arts_did}")).await).await;

    let listed = entry(&accounts["listing"], "Supreme Arts");
    assert_eq!(listed["mount"], true);
    assert_eq!(listed["open"]["accountDid"], expected_thing);
    assert!(listed["open"].get("commissionId").is_none(), "{listed}");
    assert_eq!(opened["node"]["open"]["accountDid"], expected_thing);
    assert!(
        opened["node"]["open"].get("commissionId").is_none(),
        "{opened}"
    );
}

#[tokio::test]
async fn system_folders_and_commission_entries_carry_no_thing() {
    let (base, alice) = as_alice().await;
    let untitled = untitled().to_string();
    let supreme_arts_did = supreme_arts().to_string();
    let nodes = [
        "/den/commissions".to_owned(),
        "/den/characters".to_owned(),
        "/den/accounts".to_owned(),
        format!("/den/commissions/{untitled}/attachments"),
        format!("/den/commissions/{untitled}/changelog"),
        format!("/den/commissions/{untitled}/slots"),
        format!("/den/accounts/{supreme_arts_did}/characters"),
        format!("/den/accounts/{supreme_arts_did}/commissions"),
        format!("/den/accounts/{supreme_arts_did}/workflows"),
    ];
    let listings = [
        format!("/den/commissions/{untitled}"),
        format!("/den/accounts/{supreme_arts_did}"),
    ];

    let root = ok_json(get(&alice, &base, "/den").await).await;

    assert_no_thing(&root["node"], "/den");
    for listed in root["listing"]["entries"].as_array().expect("entries") {
        assert_no_thing(listed, "/den's entries");
    }
    for path in nodes {
        let answer = ok_json(get(&alice, &base, &path).await).await;

        assert_no_thing(&answer["node"], &path);
    }
    for path in listings {
        let answer = ok_json(get(&alice, &base, &path).await).await;

        let entries = answer["listing"]["entries"].as_array().expect("entries");
        assert!(!entries.is_empty(), "{path}");
        for listed in entries {
            assert_no_thing(listed, &path);
        }
    }
}

#[tokio::test]
async fn an_archived_commission_opened_with_include_deleted_carries_its_id() {
    let (base, alice) = as_alice().await;
    let old_sketch = old_sketch().to_string();
    let expected_thing = json!(old_sketch);

    let commissions =
        ok_json(get(&alice, &base, "/den/commissions?includeDeleted=true").await).await;
    let opened = ok_json(
        get(
            &alice,
            &base,
            &format!("/den/commissions/{old_sketch}?includeDeleted=true"),
        )
        .await,
    )
    .await;

    let listed = entry(&commissions["listing"], "Old sketch");
    assert_eq!(listed["open"]["softDelete"], "archived");
    assert_eq!(listed["open"]["commissionId"], expected_thing);
    assert_eq!(opened["node"]["open"]["softDelete"], "archived");
    assert_eq!(opened["node"]["open"]["commissionId"], expected_thing);
}

#[tokio::test]
async fn every_crumb_carries_its_type() {
    let (base, alice) = as_alice().await;
    let untitled = untitled().to_string();
    let [attachment, _] = untitled_files();
    let supreme_arts_did = supreme_arts().to_string();
    let reads = [
        (
            format!("/den/commissions/{untitled}"),
            vec!["user", "user.commissions"],
        ),
        (
            format!("/den/commissions/{untitled}/attachments/{attachment}"),
            vec![
                "user",
                "user.commissions",
                "commission",
                "commission.attachments",
            ],
        ),
        (
            format!("/den/accounts/{supreme_arts_did}/characters"),
            vec!["user", "user.accounts", "account"],
        ),
    ];

    for (path, expected_types) in reads {
        let answer = ok_json(get(&alice, &base, &path).await).await;

        let crumbs = answer["crumbs"].as_array().expect("crumbs");
        let types: Vec<&str> = crumbs
            .iter()
            .map(|crumb| crumb["type"].as_str().expect("a type"))
            .collect();
        assert_eq!(types, expected_types, "{path}");
    }
}

/// The title the twin commissions share; no fixture commission uses it.
const TWIN_TITLE: &str = "Twin sketch";

/// Creates a commission titled `title` as `client`, answering its id.
async fn create_commission(client: &reqwest::Client, base: &str, title: &str) -> String {
    let body = json!({ "title": title });
    let response = client
        .post(format!("{base}/commissions"))
        .json(&body)
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(response.status(), 201, "creates the commission");
    let created: Value = response.json().await.expect("a JSON body");
    created["id"].as_str().expect("an id").to_owned()
}

/// Asserts no crumb of `answer` carries a name ordinal.
fn assert_no_crumb_numbered(answer: &Value, path: &str) {
    let crumbs = answer["crumbs"].as_array().expect("crumbs");
    for crumb in crumbs {
        assert!(crumb.get("nameOrdinal").is_none(), "{path}: {crumb}");
    }
}

#[tokio::test]
async fn a_twin_carries_name_ordinal_and_others_omit_it() {
    let (base, alice) = as_alice().await;
    let first = create_commission(&alice, &base, TWIN_TITLE).await;
    let second = create_commission(&alice, &base, TWIN_TITLE).await;
    // The later UUIDv7 is the newer twin, whatever order the two creates landed in.
    let newer = first.max(second);
    let newer_path = format!("/den/commissions/{newer}");
    let attachments_path = format!("{newer_path}/attachments");

    let commissions = ok_json(get(&alice, &base, "/den/commissions").await).await;
    let twin = ok_json(get(&alice, &base, &newer_path).await).await;
    let attachments = ok_json(get(&alice, &base, &attachments_path).await).await;

    let entries = commissions["listing"]["entries"]
        .as_array()
        .expect("an entries array");
    let numbered: Vec<(&str, &Value)> = entries
        .iter()
        .filter_map(|entry| {
            let ordinal = entry.get("nameOrdinal")?;
            let segments = entry["segments"].as_array().expect("segments");
            let last = segments.last().and_then(Value::as_str).expect("a segment");
            Some((last, ordinal))
        })
        .collect();
    let two = json!(2);
    let expected_numbered = vec![(newer.as_str(), &two)];
    assert_eq!(numbered, expected_numbered, "{commissions}");
    let twins = entries
        .iter()
        .filter(|entry| entry["name"] == TWIN_TITLE)
        .count();
    assert_eq!(twins, 2, "{commissions}");

    assert_eq!(twin["node"]["nameOrdinal"], 2, "{twin}");
    assert_no_crumb_numbered(&twin, &newer_path);

    let crumb_ordinals: Vec<Option<&Value>> = attachments["crumbs"]
        .as_array()
        .expect("crumbs")
        .iter()
        .map(|crumb| crumb.get("nameOrdinal"))
        .collect();
    let expected_crumb_ordinals = vec![None, None, Some(&two)];
    assert_eq!(crumb_ordinals, expected_crumb_ordinals, "{attachments}");
    assert!(
        attachments["node"].get("nameOrdinal").is_none(),
        "{attachments}"
    );
}

/// `text` without its terminal color codes.
fn without_colors(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_escape = false;
    for character in text.chars() {
        match (in_escape, character) {
            (false, '\u{1b}') => in_escape = true,
            (false, _) => plain.push(character),
            (true, 'm') => in_escape = false,
            (true, _) => {}
        }
    }
    plain
}

/// A user store that answers from the real one until it is switched to failing.
struct SwitchableUsers {
    inner: Arc<dyn UserStore>,
    failing: Arc<AtomicBool>,
}

impl SwitchableUsers {
    fn check(&self) -> anyhow::Result<()> {
        if self.failing.load(Ordering::SeqCst) {
            let failure = anyhow::anyhow!("the user store failed reading /user/{}", alice());
            return Err(failure);
        }
        Ok(())
    }
}

#[async_trait]
impl UserStore for SwitchableUsers {
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        self.check()?;
        self.inner.find(id).await
    }

    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        self.check()?;
        self.inner.find_by_did(did).await
    }
}

#[tokio::test]
async fn a_store_failure_is_the_house_500_logged_by_its_route_template_alone() {
    let log = CapturedLog::default();
    let subscriber =
        api::logging::subscriber(&api::Environment::DEV, EnvFilter::new("info"), log.clone());
    let _guard = tracing::subscriber::set_default(subscriber);
    let failing = Arc::new(AtomicBool::new(false));
    let switch = Arc::clone(&failing);
    let (base, _backend) = world_over(move |mut runtime| {
        let users = SwitchableUsers {
            inner: Arc::clone(&runtime.users),
            failing: switch,
        };
        runtime.users = Arc::new(users);
        api::app(runtime)
    })
    .await;
    let session = client();
    sign_in(&session, &base, "alice.test").await;
    failing.store(true, Ordering::SeqCst);
    let page = format!("?pageToken={PAST_THE_END}");

    let root = get(&session, &base, &format!("/den{page}")).await;
    let node = get(&session, &base, &format!("/den/accounts{page}")).await;

    for response in [root, node] {
        assert_eq!(cache_control(&response), PRIVATE_NO_STORE);
        assert_problem(response, 500, "internal_error").await;
    }
    let text = without_colors(&log.text());
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("a den read failed in a store"))
        .collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].contains("route=\"/den\""), "{text}");
    assert!(lines[1].contains("route=\"/den/{*path}\""), "{text}");
    assert!(
        !text.contains(PAST_THE_END),
        "the log carries the page token: {text}"
    );
    for line in lines {
        for leak in [
            alice().to_string(),
            "accounts".to_owned(),
            "failed reading".to_owned(),
        ] {
            assert!(!line.contains(&leak), "the den line carries {leak}: {line}");
        }
    }
}

/// `cargo tree` for the API's own dependencies over normal edges.
fn api_direct_dependencies() -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--package",
            "api",
            "--edges",
            "normal",
            "--depth",
            "1",
            "--prefix",
            "none",
            "--locked",
        ])
        .output()
        .expect("cargo tree runs");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("a UTF-8 tree");
    assert!(
        tree.lines().any(|line| line.starts_with("application ")),
        "cargo tree printed no application:\n{tree}"
    );
    tree
}

#[test]
fn the_api_does_not_depend_on_atta() {
    let tree = api_direct_dependencies();

    let named = tree
        .lines()
        .any(|line| line.split_whitespace().next() == Some(ENGINE));

    assert!(!named, "the api depends on atta directly:\n{tree}");
}

/// Every Rust source under `directory`, recursively.
fn rust_sources(directory: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(directory).expect("a readable directory");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            files.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}

#[test]
fn no_rust_source_of_the_api_names_atta() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let this_guard = crate_root.join("tests").join("den.rs");

    let naming: Vec<_> = rust_sources(crate_root)
        .into_iter()
        .filter(|path| *path != this_guard)
        .filter(|path| {
            let source = std::fs::read_to_string(path).expect("a readable source");
            source
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .any(|word| word == ENGINE)
        })
        .collect();

    assert!(naming.is_empty(), "these sources name atta: {naming:?}");
}
