//! `PUT`/`DELETE /commissions/{id}/dispute` on PostgreSQL: any Participant raises a dispute
//! from the step it holds and any Participant clears it back to that step, each with one
//! `lifecycle_moved` entry; a second raise and a clear of nothing answer their codes; and a
//! stranger and a made-up id get the same 404, byte for byte, on both calls.
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

/// `method {base}/commissions/{commission}/dispute` as `client`, with no body.
async fn dispute_call(
    client: &reqwest::Client,
    world: &DenWorld,
    method: reqwest::Method,
    commission: &str,
) -> reqwest::Response {
    client
        .request(
            method,
            format!("{}/commissions/{commission}/dispute", world.base_url()),
        )
        .send()
        .await
        .expect("…/dispute is answered")
}

/// `PUT …/dispute` as `client`: raise a dispute.
async fn raise(client: &reqwest::Client, world: &DenWorld, commission: &str) -> reqwest::Response {
    dispute_call(client, world, reqwest::Method::PUT, commission).await
}

/// `DELETE …/dispute` as `client`: clear the dispute.
async fn clear(client: &reqwest::Client, world: &DenWorld, commission: &str) -> reqwest::Response {
    dispute_call(client, world, reqwest::Method::DELETE, commission).await
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

/// The one entry `commission`'s changelog gained past `before`, as `client` reads it.
async fn the_new_entry(
    client: &reqwest::Client,
    world: &DenWorld,
    commission: &str,
    before: usize,
) -> Value {
    let entries = changelog(client, world, commission).await;
    let [entry] = &entries[before..] else {
        panic!("one new entry: {:#?}", &entries[before..]);
    };
    entry.clone()
}

// bob, a Participant who isn't the owner, raises a dispute on "Untitled" at Draft, and alice,
// the owner, clears it: 204 each, the commission Disputed then Draft again, and one
// `lifecycle_moved` entry each, by its caller, naming both steps and nothing else.
#[tokio::test]
async fn any_participant_raises_and_clears_back_to_draft() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let before = changelog(&world.bob, &world, &untitled).await.len();

    let raised = raise(&world.bob, &world, &untitled).await;

    assert_eq!(raised.status(), 204);
    let entry = the_new_entry(&world.bob, &world, &untitled, before).await;
    let bob = adapter_mem::fixture::bob().to_string();
    assert_eq!(entry["kind"], "lifecycle_moved");
    assert_eq!(entry["actor_id"], bob.as_str());
    assert_eq!(
        entry["payload"],
        json!({ "from": "draft", "to": "disputed" })
    );
    assert_eq!(
        lifecycle_of(&world.alice, &world, &untitled).await,
        "disputed"
    );

    let cleared = clear(&world.alice, &world, &untitled).await;

    assert_eq!(cleared.status(), 204);
    let entry = the_new_entry(&world.alice, &world, &untitled, before + 1).await;
    let alice = adapter_mem::fixture::alice().to_string();
    assert_eq!(entry["kind"], "lifecycle_moved");
    assert_eq!(entry["actor_id"], alice.as_str());
    assert_eq!(
        entry["payload"],
        json!({ "from": "disputed", "to": "draft" })
    );
    assert_eq!(lifecycle_of(&world.bob, &world, &untitled).await, "draft");
}

// A raise while Disputed answers 409 `commission_disputed`, and a clear when not Disputed 409
// `commission_not_disputed`; neither writes anything.
#[tokio::test]
async fn a_second_raise_and_a_clear_of_nothing_answer_their_codes() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let before = changelog(&world.bob, &world, &untitled).await.len();

    let nothing_to_clear = clear(&world.bob, &world, &untitled).await;

    common::assert_problem(nothing_to_clear, 409, "commission_not_disputed").await;
    assert_eq!(changelog(&world.bob, &world, &untitled).await.len(), before);
    let raised = raise(&world.alice, &world, &untitled).await;
    assert_eq!(raised.status(), 204);

    let raised_again = raise(&world.bob, &world, &untitled).await;

    common::assert_problem(raised_again, 409, "commission_disputed").await;
    assert_eq!(
        changelog(&world.bob, &world, &untitled).await.len(),
        before + 1
    );
    assert_eq!(
        lifecycle_of(&world.bob, &world, &untitled).await,
        "disputed"
    );
}

// A stranger's raise or clear on alice's commission and anyone's on a made-up id answer the one
// 404, byte for byte (status, headers and body), and write nothing.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_get_the_same_404_on_both_calls() {
    let world = pg_app::den_world(api::app).await;
    let untitled = world.untitled.id.to_string();
    let absent = absent_commission().to_string();
    let before = changelog(&world.alice, &world, &untitled).await.len();

    for method in [reqwest::Method::PUT, reqwest::Method::DELETE] {
        let not_yours = dispute_call(&world.stranger, &world, method.clone(), &untitled).await;
        let made_up = dispute_call(&world.stranger, &world, method.clone(), &absent).await;

        let not_yours = Answer::read(not_yours).await;
        let made_up = Answer::read(made_up).await;
        assert_eq!(not_yours.status, 404, "{method}");
        assert_eq!(not_yours, made_up, "{method}");
        let problem: Value = serde_json::from_slice(&not_yours.body).expect("a problem body");
        assert_eq!(problem["code"], "commission_not_found", "{method}");
    }
    assert_eq!(lifecycle_of(&world.alice, &world, &untitled).await, "draft");
    assert_eq!(
        changelog(&world.alice, &world, &untitled).await.len(),
        before
    );
}
