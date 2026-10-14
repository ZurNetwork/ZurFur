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
        user_account::UserAccount,
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
    let member = UserAccount {
        user_id: alice(),
        account_id: closed_studio.clone(),
        role: Role::Member,
        alias: None,
    };
    let database = backend.database();
    application::transaction(&*database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create(&account, &ownership).await?;
        uow.accounts().grant_role(&member).await?;
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
        "type": "post",
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
        "type": "post",
        "kind": "directory",
        "mount": true,
        "open": { "ownLevel": "public", "contentNotShown": true },
    });
    let expected_crumbs = json!([{ "name": "Alice" }]);

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
    let expected_crumbs = json!([{ "name": "Alice" }]);

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
    let subscriber = api::logging::subscriber(EnvFilter::new("info"), log.clone());
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
