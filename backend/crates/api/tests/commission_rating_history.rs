//! A rating given at create is recorded like any later rating, over HTTP on both stores.
//!
//! A create whose body carries a rating appends, right after its `created` entry, one
//! `maturity_rated` entry with the rating's literal payload, by the creator; a later
//! `PUT …/maturity` appends its own. A create with no rating appends `created` alone. The
//! changelog reads in the order the entries were appended. The PostgreSQL tests require a
//! container runtime socket.

use domain::elements::{did::Did, profile::Profile};
use serde_json::{Value, json};
use test_support::{
    den::get,
    http::{Served, client, serve, sign_in},
    pg_app,
};

/// The creator's DID on the in-memory store.
const ARTIST: &str = "did:plc:rating-history-artist";

/// The creator's handle on the in-memory store.
const ARTIST_HANDLE: &str = "rating-history.bsky.social";

/// `POST {base}/commissions` as `client` with `body`; the created commission's id.
async fn create(client: &reqwest::Client, base: &str, body: &Value) -> String {
    let response = client
        .post(format!("{base}/commissions"))
        .json(body)
        .send()
        .await
        .expect("the create is answered");
    assert_eq!(response.status(), 201, "{body}");
    let created: Value = response.json().await.expect("a JSON body");
    created["id"].as_str().expect("the created id").to_owned()
}

/// `PUT {base}/commissions/{commission}/maturity` as `client` with `body`, answered `204`.
async fn rate(client: &reqwest::Client, base: &str, commission: &str, body: &Value) {
    let response = client
        .put(format!("{base}/commissions/{commission}/maturity"))
        .json(body)
        .send()
        .await
        .expect("the rating is answered");
    assert_eq!(response.status(), 204, "{body}");
}

/// `commission`'s changelog as `client` reads it, in stream order.
async fn changelog(client: &reqwest::Client, base: &str, commission: &str) -> Vec<Value> {
    let path = format!("/commissions/{commission}/changelog");
    let response = get(client, base, &path).await;
    assert_eq!(response.status(), 200, "{path}");
    let body: Value = response.json().await.expect("a JSON body");
    body.as_array().expect("an array of entries").clone()
}

/// Each entry's kind and payload, in stream order, after checking each is by `creator` and
/// carries no note.
fn kinds_and_payloads(entries: &[Value], creator: &str) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|entry| {
            assert_eq!(entry["actor_id"], creator, "{entry}");
            assert!(entry["note"].is_null(), "{entry}");
            let kind = entry["kind"].as_str().expect("a kind").to_owned();
            (kind, entry["payload"].to_string())
        })
        .collect()
}

/// Heron's probe as `client` on `base`, the signed-in `creator`: a create rated adult and
/// Graphic, then a rating to safe and not Graphic, records `created`, the create's rating and
/// the later one, in that order, each with its literal payload.
async fn the_create_s_rating_is_kept(client: &reqwest::Client, base: &str, creator: &str) {
    let rated = json!({
        "title": "A ref sheet",
        "maturity": { "rating": "adult", "graphic": true },
    });
    let commission = create(client, base, &rated).await;
    let safe = json!({ "rating": "safe", "graphic": false });

    rate(client, base, &commission, &safe).await;

    let entries = changelog(client, base, &commission).await;
    let recorded = kinds_and_payloads(&entries, creator);
    let expected = [
        ("created", r#"{}"#),
        ("maturity_rated", r#"{"graphic":true,"rating":"adult"}"#),
        ("maturity_rated", r#"{"graphic":false,"rating":"safe"}"#),
    ]
    .map(|(kind, payload)| (kind.to_owned(), payload.to_owned()));
    assert_eq!(recorded, expected);
}

/// A create as `client` on `base`, the signed-in `creator`, with no rating records `created`
/// alone.
async fn an_unrated_create_records_created_alone(
    client: &reqwest::Client,
    base: &str,
    creator: &str,
) {
    let unrated = json!({ "title": "A ref sheet" });

    let commission = create(client, base, &unrated).await;

    let entries = changelog(client, base, &commission).await;
    let recorded = kinds_and_payloads(&entries, creator);
    let expected = [("created".to_owned(), "{}".to_owned())];
    assert_eq!(recorded, expected);
}

/// An in-memory server, and a client signed in to it as [`ARTIST`].
async fn signed_in_on_memory() -> (reqwest::Client, Served) {
    let did = Did::from(ARTIST.to_owned());
    let runtime =
        test_support::runtime::mem(&did).profile(Profile::new(did.clone(), ARTIST_HANDLE));
    let served = serve(runtime, api::app).await;
    let client = client();
    sign_in(&client, &served.base_url, ARTIST_HANDLE).await;
    (client, served)
}

#[tokio::test]
async fn a_rating_given_at_create_is_recorded_before_a_later_one() {
    let (client, served) = signed_in_on_memory().await;

    the_create_s_rating_is_kept(&client, &served.base_url, ARTIST).await;
}

#[tokio::test]
async fn a_rating_given_at_create_is_recorded_before_a_later_one_on_postgres() {
    let world = pg_app::den_world(api::app).await;
    let alice = adapter_mem::fixture::alice().to_string();

    the_create_s_rating_is_kept(&world.alice, world.base_url(), &alice).await;
}

#[tokio::test]
async fn a_create_with_no_rating_records_created_alone() {
    let (client, served) = signed_in_on_memory().await;

    an_unrated_create_records_created_alone(&client, &served.base_url, ARTIST).await;
}

#[tokio::test]
async fn a_create_with_no_rating_records_created_alone_on_postgres() {
    let world = pg_app::den_world(api::app).await;
    let alice = adapter_mem::fixture::alice().to_string();

    an_unrated_create_records_created_alone(&world.alice, world.base_url(), &alice).await;
}
