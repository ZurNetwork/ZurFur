//! A commission route's `{id}` that isn't a commission id answers exactly what a
//! well-formed id that names nothing answers: the one `application/problem+json` 404,
//! byte for byte, never echoing the input. A signed-out caller gets 401 whatever the id.
//! Checked on a write (`PATCH …/title`) and a read (`GET …/changelog`) on the in-memory
//! runtime.

use domain::elements::{did::Did, profile::Profile};
use serde_json::json;
use test_support::{
    den::Answer,
    http::{client, serve, sign_in},
};

mod common;

/// The signed-in user's DID.
const ARTIST: &str = "did:plc:artist";

/// The signed-in user's handle.
const ARTIST_HANDLE: &str = "artist.bsky.social";

/// A well-formed UUIDv7 that names no commission.
const UNKNOWN_ID: &str = "01890a5d-ac96-7d4b-bcce-b302099a8057";

/// A malformed id as it goes in the URL, and the text the route decodes it to.
struct MalformedId {
    in_url: &'static str,
    decoded: &'static str,
}

/// Ids that aren't commission ids: plain text, a colon, a DID, and markup.
fn malformed_ids() -> Vec<MalformedId> {
    vec![
        MalformedId {
            in_url: "abc",
            decoded: "abc",
        },
        MalformedId {
            in_url: "a:b",
            decoded: "a:b",
        },
        MalformedId {
            in_url: "did:plc:abcdefghijklmnopqrstuvwx",
            decoded: "did:plc:abcdefghijklmnopqrstuvwx",
        },
        MalformedId {
            in_url: "%3Cb%3Ehi%3C%2Fb%3E",
            decoded: "<b>hi</b>",
        },
    ]
}

/// The in-memory app, served, with the artist's profile; returns its base URL.
async fn world() -> String {
    let artist = Did::from(ARTIST.to_owned());
    let profile = Profile::new(artist.clone(), ARTIST_HANDLE);
    let served = serve(
        test_support::runtime::mem(&artist).profile(profile),
        api::app,
    )
    .await;
    served.base_url
}

/// A first-party client signed in as the artist.
async fn as_artist(base: &str) -> reqwest::Client {
    let session = client();
    sign_in(&session, base, ARTIST_HANDLE).await;
    session
}

/// `PATCH /commissions/{id}/title` with a title the name rule accepts.
async fn rename(session: &reqwest::Client, base: &str, id: &str) -> reqwest::Response {
    let body = json!({ "id": UNKNOWN_ID, "title": "A new title" });
    session
        .patch(format!("{base}/commissions/{id}/title"))
        .json(&body)
        .send()
        .await
        .expect("PATCH /commissions/{id}/title")
}

/// `GET /commissions/{id}/changelog`.
async fn changelog(session: &reqwest::Client, base: &str, id: &str) -> reqwest::Response {
    session
        .get(format!("{base}/commissions/{id}/changelog"))
        .send()
        .await
        .expect("GET /commissions/{id}/changelog")
}

/// The answer's `content-type`, as text.
fn content_type(answer: &Answer) -> String {
    let (_, value) = answer
        .headers
        .iter()
        .find(|(name, _)| name == "content-type")
        .expect("a content-type header");
    String::from_utf8(value.clone()).expect("an ASCII header")
}

/// Asserts `malformed`'s answer is `unknown`'s, a problem+json 404 that never echoes it.
fn assert_answers_like_unknown(malformed: &MalformedId, answer: &Answer, unknown: &Answer) {
    assert_eq!(
        answer, unknown,
        "{} answers like an id that names nothing",
        malformed.in_url
    );
    assert_eq!(answer.status, 404, "{}", malformed.in_url);
    assert_eq!(
        content_type(answer),
        "application/problem+json",
        "{}",
        malformed.in_url
    );
    let body = String::from_utf8_lossy(&answer.body);
    assert!(
        !body.contains(malformed.decoded) && !body.contains(malformed.in_url),
        "{} is echoed in {body}",
        malformed.in_url
    );
}

#[tokio::test]
async fn a_malformed_id_renames_like_an_id_that_names_nothing() {
    let base = world().await;
    let session = as_artist(&base).await;

    let unknown_response = rename(&session, &base, UNKNOWN_ID).await;
    let unknown = Answer::read(unknown_response).await;

    for malformed in malformed_ids() {
        let response = rename(&session, &base, malformed.in_url).await;
        let answer = Answer::read(response).await;
        assert_answers_like_unknown(&malformed, &answer, &unknown);
    }
}

#[tokio::test]
async fn a_malformed_id_reads_the_changelog_like_an_id_that_names_nothing() {
    let base = world().await;
    let session = as_artist(&base).await;

    let unknown_response = changelog(&session, &base, UNKNOWN_ID).await;
    let unknown = Answer::read(unknown_response).await;

    for malformed in malformed_ids() {
        let response = changelog(&session, &base, malformed.in_url).await;
        let answer = Answer::read(response).await;
        assert_answers_like_unknown(&malformed, &answer, &unknown);
    }
}

#[tokio::test]
async fn a_signed_out_caller_gets_401_whatever_the_id() {
    let base = world().await;
    let signed_out = client();

    for malformed in malformed_ids() {
        let renamed = rename(&signed_out, &base, malformed.in_url).await;
        common::assert_problem(renamed, 401, "not_authenticated").await;

        let read = changelog(&signed_out, &base, malformed.in_url).await;
        common::assert_problem(read, 401, "not_authenticated").await;
    }
}
