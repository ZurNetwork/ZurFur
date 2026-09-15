//! The golden wire-shape guard for the nine `/api/v1` contract endpoints —
//! `contract/VERSIONING.md`'s golden test, first cut.
//!
//! Pins the three mint rulings as assertions, so a serializer change that
//! `buf breaking` structurally cannot see (it diffs schemas, not emitted
//! bytes) fails a test instead of shipping:
//! - **R1** — keys are lowerCamelCase (`displayName`, never `display_name`);
//! - **R4** — an absent optional OMITS its key; `null` is never emitted;
//! - **R7** — listings are wrapped objects (`{"accounts": [...]}`), never
//!   bare top-level arrays.
//!
//! Volatile values (ids, DIDs, timestamps) are redacted to a placeholder
//! before the snapshot is taken — the guard pins the SHAPE: exact key sets,
//! wrapping, omission, and stable vocabulary values.

use domain::elements::{did::Did, profile::Profile};
use serde_json::{Value, json};
use test_support::http::{client, serve, sign_in};

/// The redaction settings every golden snapshot binds: ids and DIDs go to a
/// fixed placeholder, timestamps keep their encoding but lose their instant.
/// Volatility is keyed by name, so a renamed key stops being redacted and
/// shows up as a diff (the desired failure).
fn golden() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    settings.add_redaction(".**.id", "<VOLATILE>");
    settings.add_redaction(".**.did", "<VOLATILE>");
    settings.add_dynamic_redaction(".**.createdAt", redact_timestamp);
    settings.add_dynamic_redaction(".**.deadline", redact_timestamp);
    settings
}

/// Redact a timestamp value to `"<TS>"` + its offset suffix (`Z`, or
/// whatever non-canonical form actually got emitted): the instant is
/// volatile, the encoding is contract text. A non-string (e.g. a null)
/// passes through untouched and still breaks the snapshot — the desired
/// failure.
fn redact_timestamp(
    value: insta::internals::Content,
    _path: insta::internals::ContentPath<'_>,
) -> insta::internals::Content {
    let Some(text) = value.as_str() else {
        return value;
    };
    let date_and_time = &text[..text.len().min(19)];
    let offset_suffix =
        &text[text.len().min(19)..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let is_timestamp_shaped = date_and_time.len() == 19
        && date_and_time.as_bytes()[4] == b'-'
        && date_and_time.as_bytes()[10] == b'T';
    if is_timestamp_shaped {
        insta::internals::Content::from(format!("<TS>{offset_suffix}"))
    } else {
        value
    }
}

/// `GET /me`, resolved profile: every key camelCase (R1), all four present.
#[tokio::test]
async fn me_wire_shape_resolved() {
    let _golden = golden().bind_to_scope();
    let profile = Profile {
        did: Did::from("did:plc:golden".to_string()),
        handle: "golden.bsky.social".to_string().into(),
        display_name: Some("Golden".to_string()),
        avatar_url: Some("https://pds.example/a.jpg".to_string()),
    };
    let did = profile.did.clone();
    let served = serve(test_support::runtime::mem(&did).profile(profile), api::app).await;
    let c = client();
    sign_in(&c, &served.base_url, "golden.bsky.social").await;

    let body: Value = c
        .get(format!("{}/me", served.base_url))
        .send()
        .await
        .expect("GET /me")
        .json()
        .await
        .expect("json");
    // R1: camelCase keys, exact set.
    insta::assert_json_snapshot!("me_resolved", body);
}

/// `GET /me` with a handle-only profile (no display name, no avatar): the two
/// unset optional KEYS ARE ABSENT (R4) — not null, not empty. `handle` is
/// present because the mem profile source always resolves one; the fully
/// unresolved arm is the session suite's omission test.
#[tokio::test]
async fn me_wire_shape_partial_profile_omits_unset_keys() {
    let _golden = golden().bind_to_scope();
    let profile = Profile {
        did: Did::from("did:plc:bare".to_string()),
        handle: "bare.bsky.social".to_string().into(),
        display_name: None,
        avatar_url: None,
    };
    let did = profile.did.clone();
    let served = serve(test_support::runtime::mem(&did).profile(profile), api::app).await;
    // Poison the profile source AFTER seeding nothing in the cache: the mem
    // source returns the profile above, so to exercise the unresolved arm we
    // assert on the None fields it carries instead — display/avatar absent.
    let c = client();
    sign_in(&c, &served.base_url, "bare.bsky.social").await;
    let _ = served.backend; // cache stays cold for the optional fields

    let body: Value = c
        .get(format!("{}/me", served.base_url))
        .send()
        .await
        .expect("GET /me")
        .json()
        .await
        .expect("json");
    // R4: absent optionals omit their keys — no null, no empty string.
    insta::assert_json_snapshot!("me_partial", body);
}

/// The account trio: create (bare resource), list (wrapped, role riding),
/// delete (the outcome says which deletion happened).
#[tokio::test]
async fn account_wire_shapes() {
    let _golden = golden().bind_to_scope();
    let profile = Profile {
        did: Did::from("did:plc:acctgold".to_string()),
        handle: "acctgold.bsky.social".to_string().into(),
        display_name: None,
        avatar_url: None,
    };
    let did = profile.did.clone();
    let served = serve(test_support::runtime::mem(&did).profile(profile), api::app).await;
    let c = client();
    sign_in(&c, &served.base_url, "acctgold.bsky.social").await;

    // Create: the bare resource, 201.
    let res = c
        .post(format!("{}/accounts", served.base_url))
        .json(&json!({ "name": "Golden Studio", "handle": "golden.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    let created: Value = res.json().await.expect("json");
    // create: bare resource
    insta::assert_json_snapshot!("account_created", created);
    let account_id = created["id"].as_str().expect("id").to_string();

    // List: wrapped (R7), the caller's own role riding flat on each row.
    let body: Value = c
        .get(format!("{}/accounts", served.base_url))
        .send()
        .await
        .expect("GET /accounts")
        .json()
        .await
        .expect("json");
    // R7: wrapped object, never a bare array.
    insta::assert_json_snapshot!("account_listed", body);

    // Delete: the wire SAYS which deletion happened (ruling 2026-07-25).
    let res = c
        .delete(format!("{}/accounts/{account_id}", served.base_url))
        .send()
        .await
        .expect("DELETE /accounts/{id}");
    assert_eq!(res.status(), 200);
    let outcome: Value = res.json().await.expect("json");
    assert_eq!(
        outcome,
        json!({ "outcome": "hard" }),
        "an empty account hard-deletes, and the response carries the fact"
    );
}

/// The commission pair: create returns the created resource,
/// list wraps; absent optionals omit keys; nested maturity is
/// camelCase-clean.
#[tokio::test]
async fn commission_wire_shapes() {
    let _golden = golden().bind_to_scope();
    let profile = Profile {
        did: Did::from("did:plc:commgold".to_string()),
        handle: "commgold.bsky.social".to_string().into(),
        display_name: None,
        avatar_url: None,
    };
    let did = profile.did.clone();
    let served = serve(test_support::runtime::mem(&did).profile(profile), api::app).await;
    let c = client();
    sign_in(&c, &served.base_url, "commgold.bsky.social").await;

    // Create with a maturity but no deadline: the response carries the full
    // envelope; deadline / statuses / channel are ABSENT, not null (R4).
    let res = c
        .post(format!("{}/commissions", served.base_url))
        .json(&json!({
            "title": "A golden ref sheet",
            "maturity": { "rating": "safe", "graphic": false },
        }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201);
    let created: Value = res.json().await.expect("json");
    // create returns the resource; absent optionals omit keys (R4);
    // implicit-presence defaults omit too (§7.7); timestamps are
    // Z-normalized (§7.3); vocabulary stays lowercase (R8).
    insta::assert_json_snapshot!("commission_created", created);

    // List: wrapped (R7).
    let body: Value = c
        .get(format!("{}/commissions", served.base_url))
        .send()
        .await
        .expect("GET /commissions")
        .json()
        .await
        .expect("json");
    let rows = body["commissions"].as_array().expect("wrapped rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0], created,
        "the listing row matches the created resource, shape-for-shape"
    );
}
