//! `POST /commissions` and `PATCH /commissions/{id}/title` on the PostgreSQL stores over the
//! den world: each answers the commission as stored, a Participant who isn't the owner can't
//! rename, and someone outside the commission gets the answer a made-up id gets, which is
//! also the answer their changelog read gets. So does every other write call and the read.
//! Requires a container runtime socket.

use chrono::{DateTime, Utc};
use domain::elements::commission::CommissionId;
use serde_json::{Value, json};
use test_support::{
    den::{Answer, get},
    pg_app::{self, DenWorld, absent_commission, new_write_calls, read_call},
};

/// Everything create and rename answer about a commission, in one comparable shape.
#[derive(Debug, PartialEq)]
struct CommissionFacts {
    id: CommissionId,
    title: String,
    lifecycle: String,
    visibility: String,
    deadline: Option<DateTime<Utc>>,
    maturity: Option<(String, bool)>,
    direction_status: Option<String>,
    deadline_status: Option<String>,
    linked_channel: Option<String>,
    created_at: DateTime<Utc>,
}

/// A stored commission row's columns, in [`CommissionFacts`]'s order.
type CommissionRow = (
    String,
    String,
    String,
    Option<DateTime<Utc>>,
    Option<String>,
    Option<bool>,
    Option<String>,
    Option<String>,
    Option<String>,
    DateTime<Utc>,
);

/// The commission `id` as its row stores it.
async fn stored(world: &DenWorld, id: CommissionId) -> CommissionFacts {
    let row: CommissionRow = sqlx::query_as(
        "SELECT title, lifecycle, visibility, deadline, maturity, graphic, direction_status, \
         deadline_status, linked_channel, created_at FROM commission WHERE id = $1",
    )
    .bind(uuid::Uuid::from(id))
    .fetch_one(&world.served.runtime.pool)
    .await
    .expect("read the commission's row");
    let (
        title,
        lifecycle,
        visibility,
        deadline,
        maturity,
        graphic,
        direction_status,
        deadline_status,
        linked_channel,
        created_at,
    ) = row;
    let maturity = maturity.zip(graphic);
    CommissionFacts {
        id,
        title,
        lifecycle,
        visibility,
        deadline,
        maturity,
        direction_status,
        deadline_status,
        linked_channel,
        created_at,
    }
}

/// A wire timestamp, parsed at whatever precision it was written.
fn instant(value: &Value) -> Option<DateTime<Utc>> {
    let text = value.as_str()?;
    let parsed = DateTime::parse_from_rfc3339(text).expect("an RFC 3339 timestamp");
    Some(parsed.with_timezone(&Utc))
}

/// A wire string field that may be absent.
fn optional_text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// What a create or rename answer says about its commission.
fn answered(body: &Value) -> CommissionFacts {
    let id = body["id"]
        .as_str()
        .expect("an id")
        .parse()
        .expect("a commission id");
    let maturity = body["maturity"].as_object().map(|maturity| {
        let rating = maturity["rating"].as_str().expect("a rating").to_owned();
        let graphic = maturity
            .get("graphic")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        (rating, graphic)
    });
    CommissionFacts {
        id,
        title: body["title"].as_str().expect("a title").to_owned(),
        lifecycle: body["lifecycle"].as_str().expect("a lifecycle").to_owned(),
        visibility: body["visibility"]
            .as_str()
            .expect("a visibility")
            .to_owned(),
        deadline: instant(&body["deadline"]),
        maturity,
        direction_status: optional_text(&body["directionStatus"]),
        deadline_status: optional_text(&body["deadlineStatus"]),
        linked_channel: optional_text(&body["linkedChannel"]),
        created_at: instant(&body["createdAt"]).expect("a createdAt"),
    }
}

/// `PATCH /commissions/{id}/title` with `title` as `client`.
async fn rename(
    world: &DenWorld,
    client: &reqwest::Client,
    id: CommissionId,
    title: &str,
) -> reqwest::Response {
    let request = json!({ "title": title });
    client
        .patch(format!("{}/commissions/{id}/title", world.base_url()))
        .json(&request)
        .send()
        .await
        .expect("the rename is answered")
}

/// A JSON answer's body, after checking its status is `status`.
async fn json_with_status(response: reqwest::Response, status: u16) -> Value {
    assert_eq!(response.status(), status, "{}", response.url());
    response.json().await.expect("a JSON body")
}

// The clock reads to the nanosecond and PostgreSQL stores to the microsecond, so an answer
// built from the values before they were stored shows a different `createdAt` whenever the
// clock's nanoseconds aren't a whole microsecond: about 999 runs in 1000.
#[tokio::test]
async fn create_answers_what_it_stored() {
    let world = pg_app::den_world(api::app).await;
    let request = json!({
        "title": "  A ref\u{200B} sheet  ",
        "deadline": "2030-06-01T12:00:00.123456Z",
        "maturity": { "rating": "suggestive", "graphic": true },
    });

    let response = world
        .alice
        .post(format!("{}/commissions", world.base_url()))
        .json(&request)
        .send()
        .await
        .expect("the create is answered");

    let body = json_with_status(response, 201).await;
    let answer = answered(&body);
    let row = stored(&world, answer.id).await;
    assert_eq!(answer, row);
    assert_eq!(row.title, "A ref sheet", "the name rule's strip is stored");
}

// Rename reads the row back, so it answers the stored title and `createdAt` at the stored
// precision, never the title it was sent.
#[tokio::test]
async fn rename_answers_what_it_stored() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id;

    let response = rename(&world, &world.alice, untitled, "  Night\u{200B} shift  ").await;

    let body = json_with_status(response, 200).await;
    let answer = answered(&body);
    let row = stored(&world, untitled).await;
    assert_eq!(answer, row);
    assert_eq!(row.title, "Night shift", "the name rule's strip is stored");
}

#[tokio::test]
async fn a_participant_who_is_not_the_owner_gets_forbidden() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id;

    let response = rename(&world, &world.bob, untitled, "Bob's now").await;

    let problem = json_with_status(response, 403).await;
    assert_eq!(problem["code"], "forbidden");
    assert_eq!(stored(&world, untitled).await.title, "Untitled");
}

// `require_owner` reads more for someone else's commission than for an absent id; the
// answers must still be equal, byte for byte.
#[tokio::test]
async fn bob_s_rename_of_alice_s_commission_answers_like_a_made_up_id() {
    let world = pg_app::den_world(api::app).await;
    let old_sketch = world.old_sketch.id;

    let not_yours = rename(&world, &world.bob, old_sketch, "Bob's now").await;
    let absent = rename(&world, &world.bob, absent_commission(), "Bob's now").await;

    let not_yours = Answer::read(not_yours).await;
    let absent = Answer::read(absent).await;
    assert_eq!(not_yours, absent);
    assert_eq!(absent.status, 404);
    let problem: Value = serde_json::from_slice(&absent.body).expect("a JSON body");
    assert_eq!(problem["code"], "commission_not_found");
    assert_eq!(problem.get("instance"), None, "the problem names no path");
    assert_eq!(stored(&world, old_sketch).await.title, "Old sketch");
}

// The rename and the changelog read share one not-found, so a stranger can't learn more by
// writing than by reading.
#[tokio::test]
async fn bob_s_rename_answers_as_his_changelog_read_does() {
    let world = pg_app::den_world(api::app).await;
    let old_sketch = world.old_sketch.id;
    let changelog_path = format!("/commissions/{old_sketch}/changelog");

    let renamed = rename(&world, &world.bob, old_sketch, "Bob's now").await;
    let read = get(&world.bob, world.base_url(), &changelog_path).await;

    let renamed = Answer::read(renamed).await;
    let read = Answer::read(read).await;
    assert_eq!(renamed, read);
    assert_eq!(read.status, 404);
    let problem: Value = serde_json::from_slice(&read.body).expect("a JSON body");
    assert_eq!(problem["code"], "commission_not_found");
    assert_eq!(problem.get("instance"), None, "the problem names no path");
    assert_eq!(stored(&world, old_sketch).await.title, "Old sketch");
}

// The seven new writes and the read answer bob, who takes no part in "Old sketch", exactly as
// they answer a made-up id: status, body and headers, and the code every one carries. A call
// that answered otherwise is named by its label.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_answer_alike_on_every_new_call() {
    let world = pg_app::den_world(api::app).await;
    let old_sketch = world.old_sketch;
    let mut calls = new_write_calls().to_vec();
    calls.push(read_call());

    for call in calls {
        let pair = call.pair(&old_sketch);

        let not_yours = call
            .send(&world.bob, world.base_url(), &pair.not_yours)
            .await;
        let absent = call.send(&world.bob, world.base_url(), &pair.absent).await;

        let not_yours = Answer::read(not_yours).await;
        let absent = Answer::read(absent).await;
        assert_eq!(not_yours, absent, "{}", call.label);
        assert_eq!(absent.status, 404, "{}", call.label);
        let problem: Value = serde_json::from_slice(&absent.body).expect("a JSON body");
        assert_eq!(problem["code"], "commission_not_found", "{}", call.label);
        assert_eq!(
            problem.get("instance"),
            None,
            "{}: the problem names no path",
            call.label
        );
    }
    assert_eq!(stored(&world, old_sketch.id).await.title, "Old sketch");
}

/// `POST /commissions/{id}/slots` with one Slot on a made-up tab, as `client`: a body its own
/// parsing accepts, so the answer is the standing's.
async fn declare(
    world: &DenWorld,
    client: &reqwest::Client,
    id: CommissionId,
) -> reqwest::Response {
    let request = json!([{
        "tab": uuid::Uuid::now_v7(),
        "surface": "content",
        "title": "A probed Slot",
    }]);
    client
        .post(format!("{}/commissions/{id}/slots", world.base_url()))
        .json(&request)
        .send()
        .await
        .expect("the declare is answered")
}

/// How many Slots `id` holds, as their rows store them.
async fn slot_count(world: &DenWorld, id: CommissionId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM commission_slot WHERE commission_id = $1")
        .bind(uuid::Uuid::from(id))
        .fetch_one(&world.served.runtime.pool)
        .await
        .expect("count the Slots")
}

// Declaring Slots is the owner's: bob, a Participant of "Untitled" who isn't its owner, is
// forbidden, and no Slot lands.
#[tokio::test]
async fn a_participant_s_declare_is_forbidden() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id;
    let before = slot_count(&world, untitled).await;

    let response = declare(&world, &world.bob, untitled).await;

    let problem = json_with_status(response, 403).await;
    assert_eq!(problem["code"], "forbidden");
    assert_eq!(slot_count(&world, untitled).await, before);
}

// bob, who takes no part in "Old sketch", gets from the declare exactly what a made-up id gets:
// status, body and headers.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_answer_alike_on_the_declare() {
    let world = pg_app::den_world(api::app).await;
    let old_sketch = world.old_sketch.id;
    let before = slot_count(&world, old_sketch).await;

    let not_yours = declare(&world, &world.bob, old_sketch).await;
    let absent = declare(&world, &world.bob, absent_commission()).await;

    let not_yours = Answer::read(not_yours).await;
    let absent = Answer::read(absent).await;
    assert_eq!(not_yours, absent);
    assert_eq!(absent.status, 404);
    let problem: Value = serde_json::from_slice(&absent.body).expect("a JSON body");
    assert_eq!(problem["code"], "commission_not_found");
    assert_eq!(slot_count(&world, old_sketch).await, before);
}
