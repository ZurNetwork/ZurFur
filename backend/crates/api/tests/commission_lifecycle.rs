//! `PUT /commissions/{id}/lifecycle` on PostgreSQL: a Participant moves the lifecycle step,
//! a move its rule refuses answers its code, an unknown step is refused at `#/to` before
//! anyone's standing, and a stranger gets the made-up id's 404 byte for byte.
//! Requires a container runtime socket.

use serde_json::{Value, json};
use test_support::{
    den::{Answer, get},
    pg_app::{self, DenWorld, absent_commission},
};

mod common;

/// A `200` answer's JSON body.
async fn ok_body(response: reqwest::Response) -> Value {
    assert_eq!(response.status(), 200, "{}", response.url());
    response.json().await.expect("a JSON body")
}

/// `PUT {base}/commissions/{commission}/lifecycle` as `client`, with `body`.
async fn put_lifecycle(
    client: &reqwest::Client,
    world: &DenWorld,
    commission: &str,
    body: &Value,
) -> reqwest::Response {
    client
        .put(format!(
            "{}/commissions/{commission}/lifecycle",
            world.base_url()
        ))
        .json(body)
        .send()
        .await
        .expect("PUT …/lifecycle is answered")
}

/// `commission`'s changelog as `client` reads it.
async fn changelog(client: &reqwest::Client, world: &DenWorld, commission: &str) -> Vec<Value> {
    let path = format!("/commissions/{commission}/changelog");
    let response = get(client, world.base_url(), &path).await;
    let body = ok_body(response).await;
    body.as_array().expect("an array of entries").clone()
}

/// `commission`'s lifecycle step as `client` reads it.
async fn lifecycle_of(client: &reqwest::Client, world: &DenWorld, commission: &str) -> String {
    let path = format!("/commissions/{commission}");
    let response = get(client, world.base_url(), &path).await;
    let body = ok_body(response).await;
    body["lifecycle"].as_str().expect("a lifecycle").to_owned()
}

// On the wire, no move enters Draft, so alice's move of a commission that left it back
// into Draft answers 409 `lifecycle_move_refused` and leaves the step where it was.
#[tokio::test]
async fn a_move_into_draft_answers_its_code() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let to_active = json!({ "to": "active" });
    let moved = put_lifecycle(&world.alice, &world, &untitled, &to_active).await;
    assert_eq!(moved.status(), 204);

    let to_draft = json!({ "to": "draft" });
    let refused = put_lifecycle(&world.alice, &world, &untitled, &to_draft).await;

    common::assert_problem(refused, 409, "lifecycle_move_refused").await;
    assert_eq!(
        lifecycle_of(&world.alice, &world, &untitled).await,
        "active"
    );
}

// bob, a Participant who isn't the owner, moves "Untitled" from Draft to Active: 204, and one
// `lifecycle_moved` entry by him naming both steps.
#[tokio::test]
async fn a_participant_moves_draft_to_active_with_one_entry() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let before = changelog(&world.bob, &world, &untitled).await.len();
    let to_active = json!({ "to": "active" });

    let moved = put_lifecycle(&world.bob, &world, &untitled, &to_active).await;

    assert_eq!(moved.status(), 204);
    let entries = changelog(&world.bob, &world, &untitled).await;
    let [entry] = &entries[before..] else {
        panic!("one new entry: {:#?}", &entries[before..]);
    };
    let bob = adapter_mem::fixture::bob().to_string();
    let payload = json!({ "from": "draft", "to": "active" });
    assert_eq!(entry["kind"], "lifecycle_moved");
    assert_eq!(entry["actor_id"], bob.as_str());
    assert_eq!(entry["payload"], payload);
    assert_eq!(lifecycle_of(&world.bob, &world, &untitled).await, "active");
}

// A move to the step held answers 204 and records nothing.
#[tokio::test]
async fn a_move_to_the_held_step_records_nothing() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let before = changelog(&world.bob, &world, &untitled).await.len();
    let to_draft = json!({ "to": "draft" });

    let held = put_lifecycle(&world.bob, &world, &untitled, &to_draft).await;

    assert_eq!(held.status(), 204);
    assert_eq!(changelog(&world.bob, &world, &untitled).await.len(), before);
}

// A token outside the vocabulary, a different case of a real one and a missing `to` are each
// refused at `#/to` with the fixed detail, never echoing the token, and before anyone's standing:
// the stranger and a made-up id get the same 422.
#[tokio::test]
async fn an_unknown_step_is_refused_at_to() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let absent = absent_commission().to_string();
    let bodies = [
        json!({ "to": "shipped<script>" }),
        json!({ "to": "Active" }),
        json!({}),
    ];
    let expected_errors = json!([{
        "pointer": "#/to",
        "code": "unknown_value",
        "detail": "Choose one of the listed values.",
    }]);

    for body in &bodies {
        for (client, commission) in [
            (&world.bob, &untitled),
            (&world.stranger, &untitled),
            (&world.stranger, &absent),
        ] {
            let refused = put_lifecycle(client, &world, commission, body).await;

            assert_eq!(refused.status(), 422, "{body}");
            let problem: Value = refused.json().await.expect("a problem body");
            assert_eq!(problem["code"], "invalid_request", "{body}");
            assert_eq!(problem["errors"], expected_errors, "{body}");
            assert!(
                !problem.to_string().contains("shipped"),
                "the problem echoes the token: {problem}"
            );
        }
    }
    assert_eq!(lifecycle_of(&world.bob, &world, &untitled).await, "draft");
}

// A stranger's move of alice's commission and anyone's move of a made-up id answer the one 404,
// byte for byte, and move nothing.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_get_the_same_404() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let absent = absent_commission().to_string();
    let to_active = json!({ "to": "active" });

    let not_yours = put_lifecycle(&world.stranger, &world, &untitled, &to_active).await;
    let made_up = put_lifecycle(&world.stranger, &world, &absent, &to_active).await;

    let not_yours = Answer::read(not_yours).await;
    let made_up = Answer::read(made_up).await;
    assert_eq!(not_yours.status, 404);
    assert_eq!(not_yours, made_up);
    let problem: Value = serde_json::from_slice(&not_yours.body).expect("a problem body");
    assert_eq!(problem["code"], "commission_not_found");
    assert_eq!(lifecycle_of(&world.alice, &world, &untitled).await, "draft");
}

// While Disputed no move is made, the held step's included: 409 `commission_disputed`.
#[tokio::test]
async fn a_disputed_commission_answers_its_code() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    world
        .set_lifecycle_step(
            world.untitled.id,
            domain::elements::commission::LifecycleStep::Disputed,
        )
        .await;
    let to_active = json!({ "to": "active" });

    let refused = put_lifecycle(&world.bob, &world, &untitled, &to_active).await;

    common::assert_problem(refused, 409, "commission_disputed").await;
}
