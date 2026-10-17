//! A refused request field is named in the `422` problem: `POST /commissions`
//! with a blank or missing title points at `#/title`, and nothing is persisted.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use domain::elements::{did::Did, profile::Profile};
use serde_json::{Value, json};
use test_support::http::{client, serve, sign_in};

/// Posts `request` to `/commissions` as a signed-in artist and checks the refusal.
async fn refused_at_the_title_field(request: Value) {
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
                "code": "name_empty",
                "detail": "This can't be blank.",
            }
        ],
    });
    assert_eq!(body, expected);
    let all = backend.all_commissions().await.expect("list commissions");
    assert!(all.is_empty(), "a refused create persists nothing");
}

#[tokio::test]
async fn a_blank_title_is_refused_at_the_title_field() {
    refused_at_the_title_field(json!({ "title": "   " })).await;
}

#[tokio::test]
async fn a_missing_title_is_refused_at_the_title_field() {
    refused_at_the_title_field(json!({})).await;
}
