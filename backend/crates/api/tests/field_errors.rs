//! A refused request field is named in the `422` problem: `POST /commissions`
//! with a blank or missing title, or one the name rule refuses, points at
//! `#/title` with its own code and fixed detail, and nothing is persisted.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use domain::elements::{did::Did, profile::Profile};
use serde_json::{Value, json};
use test_support::http::{client, serve, sign_in};

/// Posts `request` to `/commissions` as a signed-in artist and checks it is refused at
/// `#/title` with `code` and its fixed `detail`.
async fn refused_at_the_title_field_with(request: Value, code: &str, detail: &str) {
    let artist = Did::from("did:plc:artist".to_string());
    let profile = Profile::new(artist.clone(), "artist.bsky.social");
    let served = serve(
        test_support::runtime::mem(&artist).profile(profile),
        api::app,
    )
    .await;
    let (base, backend) = (served.base_url, served.backend);
    let client = client();
    sign_in(&client, &base, "artist.bsky.social").await;

    let response = client
        .post(format!("{base}/commissions"))
        .json(&request)
        .send()
        .await
        .expect("POST /commissions");

    assert_eq!(response.status(), 422);
    assert_eq!(
        response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .expect("content-type is set"),
        "application/problem+json",
    );
    let body: Value = response.json().await.expect("problem body is JSON");
    let expected = json!({
        "type": "urn:zurfur:error:invalid-request",
        "code": "invalid_request",
        "title": "Invalid request",
        "detail": "Some fields need changing; each one says why.",
        "status": 422,
        "errors": [
            {
                "pointer": "#/title",
                "code": code,
                "detail": detail,
            }
        ],
    });
    assert_eq!(body, expected, "{request}");
    let all = backend.all_commissions().await.expect("list commissions");
    assert!(all.is_empty(), "a refused create persists nothing");
}

/// Posts `request` and checks it is refused at `#/title` as blank.
async fn refused_at_the_title_field(request: Value) {
    refused_at_the_title_field_with(request, "name_empty", "This can't be blank.").await;
}

#[tokio::test]
async fn a_blank_title_is_refused_at_the_title_field() {
    refused_at_the_title_field(json!({ "title": "   " })).await;
}

#[tokio::test]
async fn a_missing_title_is_refused_at_the_title_field() {
    refused_at_the_title_field(json!({})).await;
}

#[tokio::test]
async fn a_zero_width_only_title_is_refused_as_blank() {
    refused_at_the_title_field(json!({ "title": "\u{200B}" })).await;
}

#[tokio::test]
async fn a_too_long_title_is_refused_at_the_title_field() {
    let title = "a".repeat(2049);
    let request = json!({ "title": title });
    refused_at_the_title_field_with(request, "name_too_long", "This is too long. Shorten it.")
        .await;
}

#[tokio::test]
async fn a_title_with_a_slash_is_refused_at_the_title_field() {
    let request = json!({ "title": "Ref/sheet" });
    refused_at_the_title_field_with(request, "name_slash", "Leave out \u{201c}/\u{201d}.").await;
}

#[tokio::test]
async fn a_title_with_a_direction_override_is_refused_at_the_title_field() {
    let request = json!({ "title": "photo\u{202E}gpj.exe" });
    let detail = "Leave out invisible and direction-changing characters.";
    refused_at_the_title_field_with(request, "name_invisible", detail).await;
}

#[tokio::test]
async fn a_title_with_a_line_separator_is_refused_at_the_title_field() {
    let request = json!({ "title": "Ref\u{2028}sheet" });
    refused_at_the_title_field_with(request, "name_separator", "Keep it on one line.").await;
}

#[tokio::test]
async fn a_title_with_an_unshowable_character_is_refused_at_the_title_field() {
    let request = json!({ "title": "Ref\u{FFFC}sheet" });
    let detail = "Leave out characters that can't be shown.";
    refused_at_the_title_field_with(request, "name_reserved", detail).await;
}
