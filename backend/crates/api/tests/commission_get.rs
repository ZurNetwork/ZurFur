//! `GET /commissions/{id}` on PostgreSQL: the fields every opener may see, the deadline status
//! derived at the request's time, never the linked channel nor the owner, and the acts a role
//! holder may take, each one the den's write agrees with.
//! Requires a container runtime socket.

use domain::elements::level::Level;
use serde_json::{Value, json};
use test_support::{
    den::{Answer, get},
    pg_app::{self, DenWorld, absent_commission},
};

/// A `200` answer's JSON body.
async fn ok_body(response: reqwest::Response) -> Value {
    assert_eq!(response.status(), 200, "{}", response.url());
    response.json().await.expect("a JSON body")
}

/// The linked channel the tests link, which no read may serve.
const CHANNEL: &str = "https://t.me/zurfur_get_probe";

/// A deadline long past.
const PAST: &str = "2020-01-01T00:00:00Z";

/// A deadline far ahead.
const FUTURE: &str = "2099-01-01T00:00:00Z";

/// `client`'s `method` on `{base}{path}`, with `body` when there is one.
async fn send(
    client: &reqwest::Client,
    world: &DenWorld,
    method: reqwest::Method,
    path: &str,
    body: Option<&Value>,
) -> reqwest::Response {
    let request = client.request(method, format!("{}{path}", world.base_url()));
    let request = match body {
        Some(body) => request.json(body),
        None => request,
    };
    request.send().await.expect("the request is answered")
}

/// `GET /commissions/{commission}` as `client`, answered `200`.
async fn read(client: &reqwest::Client, world: &DenWorld, commission: &str) -> Value {
    let path = format!("/commissions/{commission}");
    let response = get(client, world.base_url(), &path).await;
    ok_body(response).await
}

/// alice links [`CHANNEL`] to `commission`.
async fn link_channel(world: &DenWorld, commission: &str) {
    let path = format!("/commissions/{commission}/channel");
    let body = json!({ "channel": CHANNEL });
    let linked = send(
        &world.alice,
        world,
        reqwest::Method::PUT,
        &path,
        Some(&body),
    )
    .await;
    assert_eq!(linked.status(), 204, "alice links the channel");
}

/// `client` sets `commission`'s deadline to `deadline`.
async fn set_deadline(
    client: &reqwest::Client,
    world: &DenWorld,
    commission: &str,
    deadline: &str,
) {
    let path = format!("/commissions/{commission}/deadline");
    let body = json!({ "deadline": deadline });
    let set = send(client, world, reqwest::Method::PUT, &path, Some(&body)).await;
    assert_eq!(set.status(), 204, "the deadline is set");
}

/// The keys of a JSON object, sorted.
fn keys(body: &Value) -> Vec<String> {
    let mut keys: Vec<String> = body
        .as_object()
        .expect("a JSON object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

// bob, a Participant, reads "Untitled": every field as stored, its deadline status `late`,
// derived at the request's time from a deadline long past (nothing stores it), no owner, no
// linked channel though one is linked, and the acts the den's write would pass for him: no
// rename, which is the owner's; neither Delayed act while it stands late.
#[tokio::test]
async fn a_participant_reads_the_commission() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    link_channel(&world, &untitled).await;
    set_deadline(&world.alice, &world, &untitled, PAST).await;
    let status_path = format!("/commissions/{untitled}/status/direction");
    let status = json!({ "status": "waiting_for_input" });
    let directed = send(
        &world.alice,
        &world,
        reqwest::Method::PUT,
        &status_path,
        Some(&status),
    )
    .await;
    assert_eq!(directed.status(), 204);

    let body = read(&world.bob, &world, &untitled).await;

    let created_at = body["createdAt"].as_str().expect("a creation time");
    chrono::DateTime::parse_from_rfc3339(created_at).expect("an RFC 3339 time");
    let expected = json!({
        "id": untitled,
        "title": "Untitled",
        "lifecycle": "draft",
        "visibility": "private",
        "deadline": PAST,
        "directionStatus": "waiting_for_input",
        "deadlineStatus": "late",
        "createdAt": created_at,
        "acts": [
            "move_lifecycle",
            "raise_dispute",
            "set_direction_status",
            "clear_direction_status",
            "set_deadline",
            "clear_deadline",
        ],
        "lifecycleTargets": ["batched", "active", "completed", "cancelled"],
    });
    assert_eq!(body, expected);
    assert!(
        !body.to_string().contains(CHANNEL),
        "the read serves the channel"
    );
    let alice = adapter_mem::fixture::alice().to_string();
    assert!(
        !body.to_string().contains(&alice),
        "the read names the owner"
    );
}

// The owner reads her own archived commission: `archived` set, and the rename among her acts.
#[tokio::test]
async fn the_owner_reads_her_archived_commission_with_the_rename() {
    let world = pg_app::den_world(api::app).await;
    let old_sketch = world.old_sketch.id.to_string();

    let body = read(&world.alice, &world, &old_sketch).await;

    assert_eq!(body["title"], "Old sketch");
    assert_eq!(body["archived"], true);
    assert_eq!(body["acts"][0], "rename");
}

// A stranger on a Public commission, whose node the Den opens to everyone, reads it with no
// linked channel, no owner, no acts and no lifecycle targets: the every-opener fields alone.
#[tokio::test]
async fn a_stranger_on_a_public_commission_reads_no_channel_and_no_acts() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    link_channel(&world, &untitled).await;
    world.set_visibility(world.untitled.id, Level::Public).await;

    let body = read(&world.stranger, &world, &untitled).await;

    let expected_keys = ["createdAt", "id", "lifecycle", "title", "visibility"];
    assert_eq!(keys(&body), expected_keys);
    assert_eq!(body["visibility"], "public");
    assert!(
        !body.to_string().contains(CHANNEL),
        "the read serves the channel"
    );
    let alice = adapter_mem::fixture::alice().to_string();
    assert!(
        !body.to_string().contains(&alice),
        "the read names the owner"
    );
}

// A stranger on a Private commission and anyone on a made-up id get the one 404, byte for byte.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_get_the_same_404() {
    let world = pg_app::den_world(api::app).await;
    let untitled = format!("/commissions/{}", world.untitled.id);
    let absent = format!("/commissions/{}", absent_commission());

    let not_yours = get(&world.stranger, world.base_url(), &untitled).await;
    let made_up = get(&world.stranger, world.base_url(), &absent).await;

    let not_yours = Answer::read(not_yours).await;
    let made_up = Answer::read(made_up).await;
    assert_eq!(not_yours.status, 404);
    assert_eq!(not_yours, made_up);
}

/// One call an act's word stands for, made as some client.
struct Call {
    word: &'static str,
    method: reqwest::Method,
    path: &'static str,
    body: Option<Value>,
}

/// The call of `word` with `body`, `path` below the commission's.
fn call(
    word: &'static str,
    method: reqwest::Method,
    path: &'static str,
    body: Option<Value>,
) -> Call {
    Call {
        word,
        method,
        path,
        body,
    }
}

/// Reads the acts `client` may take on `commission`, makes `the_call` for real, and checks the
/// read listed it exactly when the call succeeds; a refusal is the act's own `403` or `409`.
/// Answers whether it was listed.
async fn agrees(
    client: &reqwest::Client,
    world: &DenWorld,
    commission: &str,
    the_call: Call,
    label: &str,
) -> bool {
    let read_before = read(client, world, commission).await;
    let acts: Vec<&str> = read_before["acts"]
        .as_array()
        .map(|acts| acts.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let listed = acts.contains(&the_call.word);
    let body = match (the_call.word, &the_call.body) {
        ("move_lifecycle", _) => {
            let target = read_before["lifecycleTargets"][0]
                .as_str()
                .unwrap_or("active");
            Some(json!({ "to": target }))
        }
        (_, body) => body.clone(),
    };
    let path = format!("/commissions/{commission}{}", the_call.path);

    let response = send(client, world, the_call.method, &path, body.as_ref()).await;

    let status = response.status().as_u16();
    if listed {
        assert!(
            (200..300).contains(&status),
            "{label}: {} listed, answered {status}",
            the_call.word
        );
    } else {
        assert!(
            [403, 409].contains(&status),
            "{label}: {} unlisted, answered {status}",
            the_call.word
        );
    }
    listed
}

// Through a run of states, Draft to Active, with no deadline, one ahead, one past, then
// Disputed through its raise and back through its clear, every act bob and alice could call,
// the owner's rating among them, is listed exactly when the call succeeds.
#[tokio::test]
async fn the_acts_equal_the_gate_s_verdicts() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let rename = || {
        call(
            "rename",
            reqwest::Method::PATCH,
            "/title",
            Some(json!({ "title": "Untitled" })),
        )
    };
    let set_delayed = || {
        call(
            "set_delayed",
            reqwest::Method::PUT,
            "/status/deadline",
            Some(json!({ "status": "delayed" })),
        )
    };
    let clear_delayed = || {
        call(
            "clear_delayed",
            reqwest::Method::DELETE,
            "/status/deadline",
            None,
        )
    };
    let set_future = || {
        call(
            "set_deadline",
            reqwest::Method::PUT,
            "/deadline",
            Some(json!({ "deadline": FUTURE })),
        )
    };
    let set_past = || {
        call(
            "set_deadline",
            reqwest::Method::PUT,
            "/deadline",
            Some(json!({ "deadline": PAST })),
        )
    };
    let clear_deadline = || call("clear_deadline", reqwest::Method::DELETE, "/deadline", None);
    let set_direction = || {
        call(
            "set_direction_status",
            reqwest::Method::PUT,
            "/status/direction",
            Some(json!({ "status": "changes_requested" })),
        )
    };
    let clear_direction = || {
        call(
            "clear_direction_status",
            reqwest::Method::DELETE,
            "/status/direction",
            None,
        )
    };
    let move_lifecycle = || call("move_lifecycle", reqwest::Method::PUT, "/lifecycle", None);
    let raise_dispute = || call("raise_dispute", reqwest::Method::PUT, "/dispute", None);
    let clear_dispute = || call("clear_dispute", reqwest::Method::DELETE, "/dispute", None);
    let rate = || {
        call(
            "set_maturity",
            reqwest::Method::PUT,
            "/maturity",
            Some(json!({ "rating": "suggestive", "graphic": true })),
        )
    };

    let (bob, alice) = (&world.bob, &world.alice);
    let run = [
        (bob, set_delayed(), "draft, no deadline"),
        (bob, clear_delayed(), "draft, no deadline"),
        (bob, rename(), "a Participant's rename"),
        (alice, rename(), "the owner's rename"),
        (bob, rate(), "a Participant's rating"),
        (alice, rate(), "the owner's rating"),
        (bob, clear_deadline(), "draft, no deadline"),
        (bob, set_future(), "draft, no deadline"),
        (bob, set_delayed(), "draft, a deadline ahead"),
        (bob, set_direction(), "draft, a deadline ahead"),
        (bob, clear_direction(), "draft, a status set"),
        (bob, move_lifecycle(), "draft"),
        (bob, set_past(), "moved, a deadline ahead"),
        (bob, set_delayed(), "moved, late"),
        (bob, clear_delayed(), "moved, late"),
        (bob, clear_deadline(), "moved, late"),
    ];
    let mut verdicts = Vec::new();
    for (client, the_call, label) in run {
        verdicts.push(agrees(client, &world, &untitled, the_call, label).await);
    }

    set_deadline(&world.alice, &world, &untitled, FUTURE).await;
    let disputed = [
        (bob, clear_dispute(), "moved, not disputed"),
        (bob, raise_dispute(), "moved, a deadline ahead"),
        (bob, raise_dispute(), "disputed"),
        (bob, move_lifecycle(), "disputed"),
        (bob, set_past(), "disputed"),
        (bob, clear_deadline(), "disputed"),
        (bob, set_direction(), "disputed"),
        (bob, set_delayed(), "disputed, a deadline ahead"),
        (bob, clear_delayed(), "disputed, the flag raised"),
        (alice, rename(), "disputed, the owner's rename"),
        (alice, rate(), "disputed, the owner's rating"),
        (alice, clear_dispute(), "disputed, the owner's clear"),
    ];
    for (client, the_call, label) in disputed {
        verdicts.push(agrees(client, &world, &untitled, the_call, label).await);
    }
    let listed = verdicts.iter().filter(|listed| **listed).count();
    assert!(
        listed > 0 && listed < verdicts.len(),
        "the run makes listed and unlisted calls: {verdicts:?}"
    );
}
