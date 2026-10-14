//! identity's Accounts through the Den on the PostgreSQL stores, over the den world: a
//! deactivated Account opens only to its Owner, another's Account and a missing one answer
//! alike with the same store calls, no Den answer names another user, a stored name the
//! Den's rule refuses shows the Account's handle, founding refuses a name unsafe to show with
//! today's `422`, and founding accepts a `/`, which the Den names by the Account's handle.
//! Requires a container runtime socket.

use std::sync::Arc;

use domain::elements::{account::AccountId, user::UserId};
use serde_json::{Value, json};
use test_support::{
    den::{Answer, entry_names, get, ok_json},
    pg_app::{self, DenWorld, absent_account, stranger},
    recording::{
        CallLog, RecordingAccountStore, RecordingCommissionStore, RecordingProfileCache, SubjectLog,
    },
};

/// A name holding a right-to-left override, a bidirectional control the Den's rule refuses.
const WITH_A_BIDI_OVERRIDE: &str = "Studio\u{202E}gnp.exe";

/// A name made only of the zero-width joiner, which the Den's rule counts as empty.
const JOINER_ALONE: &str = "\u{200D}";

/// A name with a zero-width space inside, which the Den's rule strips rather than refuses.
const WITH_A_ZERO_WIDTH_SPACE: &str = "Ref\u{200B}sheet";

/// The same name as the Den's rule keeps it.
const AS_KEPT: &str = "Refsheet";

/// A name holding a line separator, which founding refuses.
const WITH_A_LINE_SEPARATOR: &str = "Ref\u{2028}sheet";

/// A name holding U+FFFC OBJECT REPLACEMENT CHARACTER, which founding refuses.
const WITH_AN_OBJECT_REPLACEMENT: &str = "Ref\u{FFFC}sheet";

/// A name holding a `/`, which founding accepts and the Den's rule refuses.
const WITH_A_SLASH: &str = "AC/DC Studio";

/// The one not-found, as `viewer` reads it at a path that names nothing.
async fn the_one_404(world: &DenWorld, viewer: &reqwest::Client) -> Answer {
    let response = get(viewer, world.base_url(), "/den/nope").await;
    Answer::read(response).await
}

/// The path to `account` in a viewer's `accounts` folder.
fn account_path(account: &AccountId) -> String {
    format!("/den/accounts/{account}")
}

#[tokio::test]
async fn a_deactivated_account_appears_to_its_owner_and_not_to_its_member() {
    let world = pg_app::den_world(api::app).await;
    let closed_studio = world.seed_closed_studio().await;
    let closed_studio_path = account_path(&closed_studio);
    let everything = "/den/accounts?includeDeleted=true";

    let alice_s = ok_json(get(&world.alice, world.base_url(), everything).await).await;
    let alice_s_live = ok_json(get(&world.alice, world.base_url(), "/den/accounts").await).await;
    let opened = ok_json(get(&world.alice, world.base_url(), &closed_studio_path).await).await;
    let bob_s = ok_json(get(&world.bob, world.base_url(), everything).await).await;
    let bob_s_read =
        Answer::read(get(&world.bob, world.base_url(), &closed_studio_path).await).await;
    let bob_s_deleted_path = format!("{closed_studio_path}?includeDeleted=true");
    let bob_s_deleted_read =
        Answer::read(get(&world.bob, world.base_url(), &bob_s_deleted_path).await).await;

    let mut alice_s_names = entry_names(&alice_s["listing"]);
    alice_s_names.sort();
    assert_eq!(alice_s_names, ["Closed Studio", "Supreme Arts"]);
    assert_eq!(entry_names(&alice_s_live["listing"]), ["Supreme Arts"]);
    assert_eq!(opened["node"]["name"], "Closed Studio");
    assert!(
        opened["node"]["open"].get("softDelete").is_some(),
        "{opened}"
    );
    assert_eq!(entry_names(&bob_s["listing"]), ["Night Shift"]);
    let the_one_404 = the_one_404(&world, &world.bob).await;
    assert_eq!(bob_s_read, the_one_404);
    assert_eq!(bob_s_deleted_read, the_one_404);
}

/// The recorders the equal-calls test reads, one log per store, so a call moved from one
/// store to another shows even where both name it alike.
#[derive(Clone, Default)]
struct Recorders {
    accounts: CallLog,
    commissions: CallLog,
    profiles: CallLog,
    subjects: SubjectLog,
}

/// What one read did: its answer, each store's calls in order, and whom they asked about.
#[derive(Debug, PartialEq, Eq)]
struct Reads {
    answer: Answer,
    accounts: Vec<&'static str>,
    commissions: Vec<&'static str>,
    profiles: Vec<&'static str>,
    subjects: Vec<String>,
}

#[tokio::test]
async fn another_s_account_and_a_missing_one_give_the_same_answer_and_store_calls() {
    let recorders = Recorders::default();
    let recording = recorders.clone();
    let world = pg_app::den_world(move |mut runtime| {
        let accounts = RecordingAccountStore::new(
            Arc::clone(&runtime.accounts),
            recording.accounts,
            recording.subjects.clone(),
        );
        let commissions =
            RecordingCommissionStore::new(Arc::clone(&runtime.commissions), recording.commissions);
        let profiles = RecordingProfileCache::new(
            Arc::clone(&runtime.profile_cache),
            recording.profiles,
            recording.subjects,
        );
        runtime.accounts = Arc::new(accounts);
        runtime.commissions = Arc::new(commissions);
        runtime.profile_cache = Arc::new(profiles);
        api::app(runtime)
    })
    .await;
    let closed_studio = world.seed_closed_studio().await;
    let viewers = [
        (
            "the stranger",
            &world.stranger,
            stranger(),
            &world.supreme_arts,
        ),
        (
            "bob",
            &world.bob,
            adapter_mem::fixture::bob(),
            &world.supreme_arts,
        ),
        (
            "bob, a Member",
            &world.bob,
            adapter_mem::fixture::bob(),
            &closed_studio,
        ),
        (
            "alice",
            &world.alice,
            adapter_mem::fixture::alice(),
            &world.night_shift,
        ),
    ];

    for (label, viewer, viewer_id, another_s) in viewers {
        assert_profile_cached(&world, viewer, &viewer_id).await;
        let expected = the_one_404(&world, viewer).await;
        for suffix in ["", "/characters", "/workflows"] {
            let not_yours = format!("{}{suffix}", account_path(another_s));
            let absent = format!("{}{suffix}", account_path(&absent_account()));

            let not_yours_reads = reads_of(&recorders, viewer, &world, &not_yours).await;
            let absent_reads = reads_of(&recorders, viewer, &world, &absent).await;

            assert_eq!(absent_reads.answer, expected, "{label}: {absent}");
            assert_eq!(not_yours_reads, absent_reads, "{label}: {not_yours}");
            assert!(
                !absent_reads.accounts.is_empty(),
                "{label}: no Account-store call"
            );
            assert!(
                !absent_reads.profiles.contains(&"put_profile"),
                "{label}: {absent_reads:?}"
            );
            let viewer_did = viewer_id.to_string();
            assert!(
                absent_reads
                    .subjects
                    .iter()
                    .all(|subject| *subject == viewer_did),
                "{label}: {absent_reads:?}"
            );
        }
    }
}

/// Fixes the profile cache's state before any call is recorded: `viewer` reads their root,
/// and their profile is then cached, so no read below can start a lookup that fills it.
async fn assert_profile_cached(world: &DenWorld, viewer: &reqwest::Client, viewer_id: &UserId) {
    ok_json(get(viewer, world.base_url(), "/den").await).await;
    let cached = world
        .served
        .runtime
        .profile_cache
        .get(viewer_id.did())
        .await
        .expect("the profile cache answers");
    assert!(cached.is_some(), "{viewer_id}'s profile is cached");
}

/// What `GET {path}` as `viewer` answered and recorded.
async fn reads_of(
    recorders: &Recorders,
    viewer: &reqwest::Client,
    world: &DenWorld,
    path: &str,
) -> Reads {
    let accounts = recorders.accounts.events().len();
    let commissions = recorders.commissions.events().len();
    let profiles = recorders.profiles.events().len();
    let subjects = recorders.subjects.subjects().len();
    let answer = Answer::read(get(viewer, world.base_url(), path).await).await;
    Reads {
        answer,
        accounts: recorders.accounts.events().split_off(accounts),
        commissions: recorders.commissions.events().split_off(commissions),
        profiles: recorders.profiles.events().split_off(profiles),
        subjects: recorders.subjects.subjects().split_off(subjects),
    }
}

/// What names one signed-in user: their DID, their handle and their display name, if any.
struct Named {
    viewer: &'static str,
    words: Vec<String>,
}

#[tokio::test]
async fn no_den_answer_names_another_user() {
    let world = pg_app::den_world(api::app).await;
    world.seed_closed_studio().await;
    world.add_bob_as_member(&world.supreme_arts).await;
    let alice = Named {
        viewer: "alice",
        words: vec![
            adapter_mem::fixture::alice().to_string(),
            "alice.test".to_owned(),
            "Alice".to_owned(),
        ],
    };
    let bob = Named {
        viewer: "bob",
        words: vec![
            adapter_mem::fixture::bob().to_string(),
            "bob.test".to_owned(),
        ],
    };
    let stranger = Named {
        viewer: "the stranger",
        words: vec![stranger().to_string(), "stranger.test".to_owned()],
    };
    let viewers = [
        (
            &world.alice,
            &alice,
            [&bob, &stranger],
            vec!["Closed Studio", "Supreme Arts"],
        ),
        (
            &world.bob,
            &bob,
            [&alice, &stranger],
            vec!["Night Shift", "Supreme Arts"],
        ),
        (&world.stranger, &stranger, [&alice, &bob], vec![]),
    ];

    for (client, viewer, others, reach) in viewers {
        let walk = identity_s_answers(&world, client).await;

        assert_eq!(walk.opened, reach, "{}", viewer.viewer);
        let bodies = walk.bodies;
        assert_eq!(
            bodies.len(),
            2 + reach.len(),
            "{}: {bodies:?}",
            viewer.viewer
        );
        for body in &bodies {
            for other in others {
                for word in &other.words {
                    assert!(
                        !body.contains(word.as_str()),
                        "{}'s Den names {}: {word} in {body}",
                        viewer.viewer,
                        other.viewer,
                    );
                }
            }
        }
    }
}

/// A walk of identity's part of a viewer's Den: every body it read, and the names of the
/// Accounts it opened, sorted.
struct Walk {
    bodies: Vec<String>,
    opened: Vec<String>,
}

/// Every body identity's part of the Den serves `viewer`: their root, their `accounts`
/// folder with deactivated Accounts asked for, and each Account it lists. An empty folder
/// answers an empty listing.
async fn identity_s_answers(world: &DenWorld, viewer: &reqwest::Client) -> Walk {
    let root = ok_json(get(viewer, world.base_url(), "/den").await).await;
    let accounts_path = "/den/accounts?includeDeleted=true";
    let accounts = ok_json(get(viewer, world.base_url(), accounts_path).await).await;
    let mut bodies = vec![root.to_string(), accounts.to_string()];
    let listing = &accounts["listing"];
    let entries = if *listing == json!({}) {
        Vec::new()
    } else {
        listing["entries"]
            .as_array()
            .expect("an entries array")
            .clone()
    };
    let mut opened = Vec::new();
    for entry in entries {
        let path = den_path(&entry);
        let account = ok_json(get(viewer, world.base_url(), &path).await).await;
        let name = account["node"]["name"]
            .as_str()
            .expect("the Account's name");
        opened.push(name.to_owned());
        bodies.push(account.to_string());
    }
    opened.sort();
    Walk { bodies, opened }
}

/// The Den path of a listing entry, from its segments, asking for deactivated nodes too.
fn den_path(entry: &Value) -> String {
    let segments = entry["segments"].as_array().expect("an entry's segments");
    let pieces: Vec<&str> = segments
        .iter()
        .map(|segment| segment.as_str().expect("a segment"))
        .collect();
    format!("/den/{}?includeDeleted=true", pieces.join("/"))
}

#[tokio::test]
async fn a_stored_name_the_rule_refuses_shows_the_account_s_handle_and_still_loads() {
    let world = pg_app::den_world(api::app).await;
    let account = world
        .seed_account_with_stored_name("bidi-studio", WITH_A_BIDI_OVERRIDE)
        .await;
    let handle = "bidi-studio.zurfur.app";

    let listed = ok_json(get(&world.alice, world.base_url(), "/den/accounts").await).await;
    let opened = ok_json(get(&world.alice, world.base_url(), &account_path(&account)).await).await;
    let accounts = world
        .alice
        .get(format!("{}/accounts", world.base_url()))
        .send()
        .await
        .expect("GET /accounts is answered");

    let mut names = entry_names(&listed["listing"]);
    names.sort();
    assert_eq!(names, ["Supreme Arts", handle]);
    assert_eq!(opened["node"]["name"], handle);
    assert_eq!(accounts.status(), 200);
    let body: Value = accounts.json().await.expect("a JSON body");
    let stored = body["accounts"]
        .as_array()
        .expect("an accounts array")
        .iter()
        .find(|listing| listing["handle"] == handle)
        .expect("the Account is listed");
    assert_eq!(stored["name"], WITH_A_BIDI_OVERRIDE);
}

/// `POST /accounts` as alice, founding `name` under `<label>.zurfur.app`.
async fn found(world: &DenWorld, name: &str, label: &str) -> reqwest::Response {
    let founding = json!({ "name": name, "handle": format!("{label}.zurfur.app") });
    world
        .alice
        .post(format!("{}/accounts", world.base_url()))
        .json(&founding)
        .send()
        .await
        .expect("POST /accounts is answered")
}

/// A refusal as a client tells it apart: its status, its content type, and its problem body
/// but for `detail`, the one field that says which rule refused.
async fn refusal(response: reqwest::Response) -> (u16, String, Value) {
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .expect("a content type")
        .to_str()
        .expect("an ASCII header")
        .to_owned();
    let mut body: Value = response.json().await.expect("a problem body");
    body.as_object_mut()
        .expect("a problem object")
        .remove("detail");
    (status, content_type, body)
}

#[tokio::test]
async fn founding_refuses_what_the_rule_refuses_with_today_s_422_and_founds_nothing() {
    let world = pg_app::den_world(api::app).await;
    let blank = refusal(found(&world, "   ", "refused").await).await;

    let bidi = refusal(found(&world, WITH_A_BIDI_OVERRIDE, "refused").await).await;
    let joiner = refusal(found(&world, JOINER_ALONE, "refused").await).await;
    let separator = refusal(found(&world, WITH_A_LINE_SEPARATOR, "refused").await).await;
    let object = refusal(found(&world, WITH_AN_OBJECT_REPLACEMENT, "refused").await).await;
    let afterwards = found(&world, "Ref sheet", "refused").await;

    assert_eq!(blank.0, 422, "{blank:?}");
    assert_eq!(blank.1, "application/problem+json", "{blank:?}");
    assert_eq!(blank.2["code"], "invalid_request", "{blank:?}");
    assert_eq!(bidi, blank);
    assert_eq!(joiner, blank);
    assert_eq!(separator, blank);
    assert_eq!(object, blank);
    assert_eq!(afterwards.status(), 201, "the refusals claimed no handle");
}

#[tokio::test]
async fn founding_stores_an_accepted_name_as_the_rule_keeps_it() {
    let world = pg_app::den_world(api::app).await;

    let founded = found(&world, WITH_A_ZERO_WIDTH_SPACE, "kept").await;

    assert_eq!(founded.status(), 201);
    let body: Value = founded.json().await.expect("a JSON body");
    assert_eq!(body["name"], AS_KEPT);
    let did = body["did"].as_str().expect("the founded DID");
    let path = format!("/den/accounts/{did}");
    let opened = ok_json(get(&world.alice, world.base_url(), &path).await).await;
    assert_eq!(opened["node"]["name"], AS_KEPT);
}

#[tokio::test]
async fn founding_accepts_a_slash_and_the_den_names_the_account_by_its_handle() {
    let world = pg_app::den_world(api::app).await;
    let handle = "acdc.zurfur.app";

    let founded = found(&world, WITH_A_SLASH, "acdc").await;

    assert_eq!(founded.status(), 201);
    let body: Value = founded.json().await.expect("a JSON body");
    assert_eq!(body["name"], WITH_A_SLASH);
    let did = body["did"].as_str().expect("the founded DID");
    let path = format!("/den/accounts/{did}");
    let opened = ok_json(get(&world.alice, world.base_url(), &path).await).await;
    let listed = ok_json(get(&world.alice, world.base_url(), "/den/accounts").await).await;
    let accounts = world
        .alice
        .get(format!("{}/accounts", world.base_url()))
        .send()
        .await
        .expect("GET /accounts is answered");
    assert_eq!(opened["node"]["name"], handle);
    let mut names = entry_names(&listed["listing"]);
    names.sort();
    assert_eq!(names, ["Supreme Arts", handle]);
    let accounts: Value = accounts.json().await.expect("a JSON body");
    let stored = accounts["accounts"]
        .as_array()
        .expect("an accounts array")
        .iter()
        .find(|listing| listing["did"] == did)
        .expect("the Account is listed");
    assert_eq!(stored["name"], WITH_A_SLASH);
}
