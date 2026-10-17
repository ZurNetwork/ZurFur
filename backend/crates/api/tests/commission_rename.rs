//! `PATCH /commissions/{id}/title`: the owner renames a commission, end to end over HTTP on
//! the in-memory runtime. Every title the name rule refuses names `#/title` with its own
//! code and fixed detail; the path's id wins over the body's; and the route sits behind the
//! cross-site guard. The rename's log is checked in `commission_rename_log.rs`.

use adapter_mem::MemBackend;
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle},
    did::Did,
    profile::Profile,
};
use reqwest::header::ORIGIN;
use serde_json::{Value, json};
use test_support::http::{bare_client, client, serve, sign_in};

mod common;

/// The artist's DID: the one user the in-memory runtime signs in.
const ARTIST: &str = "did:plc:artist";

/// The artist's handle.
const ARTIST_HANDLE: &str = "artist.bsky.social";

/// The title every commission here is created with.
const FIRST_TITLE: &str = "A ref sheet";

/// The in-memory app, served, with the artist's profile.
async fn world() -> (String, MemBackend) {
    let artist = Did::from(ARTIST.to_owned());
    let profile = Profile::new(artist.clone(), ARTIST_HANDLE);
    let served = serve(
        test_support::runtime::mem(&artist).profile(profile),
        api::app,
    )
    .await;
    (served.base_url, served.backend)
}

/// A client signed in as the artist.
async fn as_artist(base: &str) -> reqwest::Client {
    let session = client();
    sign_in(&session, base, ARTIST_HANDLE).await;
    session
}

/// Creates a commission titled [`FIRST_TITLE`] as `session` and returns its create answer.
async fn create(session: &reqwest::Client, base: &str) -> Value {
    let request = json!({ "title": FIRST_TITLE });
    let response = session
        .post(format!("{base}/commissions"))
        .json(&request)
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(response.status(), 201, "creating a commission returns 201");
    response.json().await.expect("a JSON body")
}

/// A create answer's commission id.
fn id_of(answer: &Value) -> CommissionId {
    answer["id"]
        .as_str()
        .expect("an id")
        .parse()
        .expect("a commission id")
}

/// `PATCH /commissions/{id}/title` with `body` as `session`.
async fn rename(
    session: &reqwest::Client,
    base: &str,
    id: impl std::fmt::Display,
    body: &Value,
) -> reqwest::Response {
    session
        .patch(format!("{base}/commissions/{id}/title"))
        .json(body)
        .send()
        .await
        .expect("PATCH /commissions/{id}/title")
}

/// The title `backend` stores for `id`.
async fn stored_title(backend: &MemBackend, id: CommissionId) -> String {
    let stored = backend
        .find_commission(id)
        .await
        .expect("find the commission")
        .expect("the commission is stored");
    stored.title.as_str().to_owned()
}

/// Seeds a commission owned by someone other than the artist and returns its id.
async fn seed_foreign_commission(backend: &MemBackend) -> CommissionId {
    let owner = backend
        .provision(&Did::from("did:plc:someone-else".to_owned()))
        .await
        .expect("provision the foreign owner");
    let title = "Not yours"
        .parse::<CommissionTitle>()
        .expect("a valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    backend
        .create_commission(&commission)
        .await
        .expect("seed the foreign commission");
    commission.id
}

/// A title the name rule refuses, the code it answers and that code's fixed detail.
struct RefusedTitle {
    label: &'static str,
    title: String,
    code: &'static str,
    detail: &'static str,
}

/// Every refusal of a title, one per code; blank and zero-width-only both answer
/// `name_empty`, the first at the wire's trim and the second at the name rule.
fn refused_titles() -> Vec<RefusedTitle> {
    vec![
        RefusedTitle {
            label: "blank",
            title: "   ".to_owned(),
            code: "name_empty",
            detail: "This can't be blank.",
        },
        RefusedTitle {
            label: "zero-width only",
            title: "\u{200B}".to_owned(),
            code: "name_empty",
            detail: "This can't be blank.",
        },
        RefusedTitle {
            label: "too long",
            title: "a".repeat(2049),
            code: "name_too_long",
            detail: "This is too long. Shorten it.",
        },
        RefusedTitle {
            label: "slash",
            title: "Ref/sheet".to_owned(),
            code: "name_slash",
            detail: "Leave out \u{201c}/\u{201d}.",
        },
        RefusedTitle {
            label: "right-to-left override",
            title: "photo\u{202E}gpj.exe".to_owned(),
            code: "name_invisible",
            detail: "Leave out invisible and direction-changing characters.",
        },
        RefusedTitle {
            label: "line separator",
            title: "Ref\u{2028}sheet".to_owned(),
            code: "name_separator",
            detail: "Keep it on one line.",
        },
        RefusedTitle {
            label: "object replacement",
            title: "Ref\u{FFFC}sheet".to_owned(),
            code: "name_reserved",
            detail: "Leave out characters that can't be shown.",
        },
    ]
}

#[tokio::test]
async fn the_owner_renames_and_gets_the_whole_commission() {
    let (base, backend) = world().await;
    let session = as_artist(&base).await;
    let created = create(&session, &base).await;
    let id = id_of(&created);
    let request = json!({ "title": "  Night shift  " });

    let response = rename(&session, &base, id, &request).await;

    assert_eq!(response.status(), 200);
    let renamed: Value = response.json().await.expect("a JSON body");
    let expected = json!({
        "id": id.to_string(),
        "title": "Night shift",
        "lifecycle": "draft",
        "visibility": "private",
        "createdAt": created["createdAt"],
    });
    assert_eq!(renamed, expected);
    assert_eq!(stored_title(&backend, id).await, "Night shift");
}

#[tokio::test]
async fn a_rename_to_the_same_title_answers_200() {
    let (base, backend) = world().await;
    let session = as_artist(&base).await;
    let created = create(&session, &base).await;
    let id = id_of(&created);
    let request = json!({ "title": FIRST_TITLE });

    let response = rename(&session, &base, id, &request).await;

    assert_eq!(response.status(), 200);
    let renamed: Value = response.json().await.expect("a JSON body");
    assert_eq!(
        renamed, created,
        "an unchanged rename answers the same commission"
    );
    assert_eq!(stored_title(&backend, id).await, FIRST_TITLE);
}

#[tokio::test]
async fn a_refused_title_gets_a_field_error_at_title() {
    let (base, backend) = world().await;
    let session = as_artist(&base).await;
    let id = id_of(&create(&session, &base).await);

    for refused in refused_titles() {
        let request = json!({ "title": refused.title });

        let response = rename(&session, &base, id, &request).await;

        assert_eq!(response.status(), 422, "{}", refused.label);
        let body: Value = response.json().await.expect("a JSON body");
        let expected = json!({
            "type": "urn:zurfur:error:invalid-request",
            "code": "invalid_request",
            "title": "Invalid request",
            "detail": "Some fields need changing; each one says why.",
            "status": 422,
            "errors": [
                {
                    "pointer": "#/title",
                    "code": refused.code,
                    "detail": refused.detail,
                }
            ],
        });
        assert_eq!(body, expected, "{}", refused.label);
        assert_eq!(
            stored_title(&backend, id).await,
            FIRST_TITLE,
            "{}: a refused rename changes nothing",
            refused.label
        );
    }
}

#[tokio::test]
async fn the_path_s_commission_is_renamed_never_the_body_s() {
    let (base, backend) = world().await;
    let session = as_artist(&base).await;
    let in_path = id_of(&create(&session, &base).await);
    let in_body = id_of(&create(&session, &base).await);
    let request = json!({ "id": in_body.to_string(), "title": "Renamed" });

    let response = rename(&session, &base, in_path, &request).await;

    assert_eq!(response.status(), 200);
    let renamed: Value = response.json().await.expect("a JSON body");
    assert_eq!(renamed["id"], in_path.to_string());
    assert_eq!(stored_title(&backend, in_path).await, "Renamed");
    assert_eq!(stored_title(&backend, in_body).await, FIRST_TITLE);
}

#[tokio::test]
async fn a_stranger_and_a_made_up_id_get_the_same_404() {
    let (base, backend) = world().await;
    let session = as_artist(&base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let made_up = uuid::Uuid::now_v7();
    let request = json!({ "title": "Mine now" });

    let not_yours = rename(&session, &base, foreign, &request).await;
    let absent = rename(&session, &base, made_up, &request).await;

    assert_eq!(not_yours.status(), 404);
    assert_eq!(absent.status(), 404);
    let not_yours_body = not_yours.bytes().await.expect("a body");
    let absent_body = absent.bytes().await.expect("a body");
    assert_eq!(not_yours_body, absent_body, "the two not-founds read alike");
    let problem: Value = serde_json::from_slice(&absent_body).expect("a JSON body");
    assert_eq!(problem["code"], "commission_not_found");
    assert_eq!(stored_title(&backend, foreign).await, "Not yours");
}

#[tokio::test]
async fn a_malformed_body_answers_invalid_request() {
    let (base, backend) = world().await;
    let session = as_artist(&base).await;
    let id = id_of(&create(&session, &base).await);

    let response = session
        .patch(format!("{base}/commissions/{id}/title"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body("{ not json")
        .send()
        .await
        .expect("PATCH /commissions/{id}/title");

    common::assert_problem(response, 422, "invalid_request").await;
    assert_eq!(stored_title(&backend, id).await, FIRST_TITLE);
}

#[tokio::test]
async fn a_cross_site_rename_is_refused() {
    let (base, backend) = world().await;
    let id = id_of(&create(&as_artist(&base).await, &base).await);
    let owner = bare_client();
    sign_in(&owner, &base, ARTIST_HANDLE).await;
    let url = format!("{base}/commissions/{id}/title");
    let request = json!({ "title": "Renamed elsewhere" });

    let foreign_origin = owner
        .patch(&url)
        .header(ORIGIN, "https://evil.example")
        .json(&request);
    let no_origin = owner.patch(&url).json(&request);
    let null_origin = owner.patch(&url).header(ORIGIN, "null").json(&request);
    for refused in [foreign_origin, no_origin, null_origin] {
        let response = refused.send().await.expect("the rename is answered");
        common::assert_problem(response, 403, "cross_origin").await;
    }
    assert_eq!(stored_title(&backend, id).await, FIRST_TITLE);

    let signed_out_same_origin = bare_client()
        .patch(&url)
        .header("sec-fetch-site", "same-origin")
        .json(&request)
        .send()
        .await
        .expect("the rename is answered");
    common::assert_problem(signed_out_same_origin, 401, "not_authenticated").await;
}
