//! The golden wire-shape guard for the nine `/api/v1` contract endpoints
//! (DD 40992770; `contract/VERSIONING.md` §7.7's golden test, first cut).
//!
//! Pins the three mint rulings as assertions, so a serializer change that
//! `buf breaking` structurally cannot see (it diffs schemas, not emitted
//! bytes) fails a test instead of shipping:
//! - **R1** — keys are lowerCamelCase (`displayName`, never `display_name`);
//! - **R4** — an absent optional OMITS its key; `null` is never emitted;
//! - **R7** — listings are wrapped objects (`{"accounts": [...]}`), never
//!   bare top-level arrays.
//!
//! Volatile values (ids, DIDs, timestamps) are normalized to a placeholder
//! before comparison — the guard pins the SHAPE: exact key sets, wrapping,
//! omission, and stable vocabulary values. ZMVP-160/161 upgrade this to
//! byte-exact golden files once both codecs are generated (the serializer
//! option set is contract text; only byte comparison pins it fully).

use adapter_mem::MemBackend;
use api::AppState;
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use tower_sessions::{MemoryStore, SessionManagerLayer};

/// Boots the app with in-process fakes; `profile` controls what `/me` resolves.
async fn spawn_app(did: &str, profile: Profile) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let test_support::runtime::MemRuntime { runtime, backend } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .profile(profile)
            .public_url(format!("http://{addr}"))
            .build();
    let state: AppState = runtime;
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

async fn sign_in(client: &reqwest::Client, base: &str, handle: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("handle={handle}"))
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303);
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
}

/// Replace volatile leaf values with placeholders, recursively — key SETS and
/// stable values survive, which is what the guard pins. Volatility is keyed by
/// name, so a renamed key stops being normalized and shows up as a diff (the
/// desired failure).
///
/// Timestamps are NOT fully erased: only the digits are replaced, so the
/// ENCODING — canonical ProtoJSON, `Z`-normalized, never `+00:00` — stays
/// pinned. The epic gate caught exactly this hole: the ZMVP-160 codec swap
/// moved `createdAt`/`deadline` from `Z` to `+00:00` and the old
/// all-`<VOLATILE>` normalization was blind to it.
fn normalized(value: &Value) -> Value {
    const VOLATILE_KEYS: &[&str] = &["id", "did"];
    const TIMESTAMP_KEYS: &[&str] = &["createdAt", "deadline"];
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, val)| {
                    if VOLATILE_KEYS.contains(&key.as_str()) {
                        (key.clone(), json!("<VOLATILE>"))
                    } else if TIMESTAMP_KEYS.contains(&key.as_str()) {
                        (key.clone(), normalized_timestamp(val))
                    } else {
                        (key.clone(), normalized(val))
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
        other => other.clone(),
    }
}

/// Normalize a timestamp value to `"<TS>"` + its offset suffix (`Z`, or
/// whatever non-canonical form actually got emitted): the instant is volatile,
/// the encoding is contract text. A non-string (e.g. a null) passes through
/// untouched and fails the comparison — also the desired failure.
fn normalized_timestamp(value: &Value) -> Value {
    let Value::String(text) = value else {
        return value.clone();
    };
    let date_and_time = &text[..text.len().min(19)];
    let offset_suffix =
        &text[text.len().min(19)..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let is_timestamp_shaped = date_and_time.len() == 19
        && date_and_time.as_bytes()[4] == b'-'
        && date_and_time.as_bytes()[10] == b'T';
    if is_timestamp_shaped {
        json!(format!("<TS>{offset_suffix}"))
    } else {
        value.clone()
    }
}

/// `GET /me`, resolved profile: every key camelCase (R1), all four present.
#[tokio::test]
async fn me_wire_shape_resolved() {
    let profile = Profile {
        did: Did::new("did:plc:golden".to_string()),
        handle: "golden.bsky.social".to_string(),
        display_name: Some("Golden".to_string()),
        avatar_url: Some("https://pds.example/a.jpg".to_string()),
    };
    let (base, _backend) = spawn_app("did:plc:golden", profile).await;
    let c = client();
    sign_in(&c, &base, "golden.bsky.social").await;

    let body: Value = c
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me")
        .json()
        .await
        .expect("json");
    let expected = json!({
        "did": "<VOLATILE>",
        "handle": "golden.bsky.social",
        "displayName": "Golden",
        "avatarUrl": "https://pds.example/a.jpg",
    });
    assert_eq!(normalized(&body), expected, "R1: camelCase keys, exact set");
}

/// `GET /me` with a handle-only profile (no display name, no avatar): the two
/// unset optional KEYS ARE ABSENT (R4) — not null, not empty. `handle` is
/// present because the mem profile source always resolves one; the fully
/// unresolved arm is the session suite's omission test.
#[tokio::test]
async fn me_wire_shape_partial_profile_omits_unset_keys() {
    let profile = Profile {
        did: Did::new("did:plc:bare".to_string()),
        handle: "bare.bsky.social".to_string(),
        display_name: None,
        avatar_url: None,
    };
    let (base, backend) = spawn_app("did:plc:bare", profile).await;
    // Poison the profile source AFTER seeding nothing in the cache: the mem
    // source returns the profile above, so to exercise the unresolved arm we
    // assert on the None fields it carries instead — display/avatar absent.
    let c = client();
    sign_in(&c, &base, "bare.bsky.social").await;
    let _ = backend; // cache stays cold for the optional fields

    let body: Value = c
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me")
        .json()
        .await
        .expect("json");
    let expected = json!({
        "did": "<VOLATILE>",
        "handle": "bare.bsky.social",
    });
    assert_eq!(
        normalized(&body),
        expected,
        "R4: absent optionals omit their keys — no null, no empty string"
    );
}

/// The account trio: create (bare resource), list (wrapped, role riding),
/// delete (the outcome says which deletion happened).
#[tokio::test]
async fn account_wire_shapes() {
    let profile = Profile {
        did: Did::new("did:plc:acctgold".to_string()),
        handle: "acctgold.bsky.social".to_string(),
        display_name: None,
        avatar_url: None,
    };
    let (base, _backend) = spawn_app("did:plc:acctgold", profile).await;
    let c = client();
    sign_in(&c, &base, "acctgold.bsky.social").await;

    // Create: the bare resource, 201.
    let res = c
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": "Golden Studio", "handle": "golden.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    let created: Value = res.json().await.expect("json");
    let expected = json!({
        "id": "<VOLATILE>",
        "did": "<VOLATILE>",
        "handle": "golden.zurfur.app",
        "name": "Golden Studio",
    });
    assert_eq!(normalized(&created), expected, "create: bare resource");
    let account_id = created["id"].as_str().expect("id").to_string();

    // List: wrapped (R7), the caller's own role riding flat on each row.
    let body: Value = c
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts")
        .json()
        .await
        .expect("json");
    let expected = json!({
        "accounts": [{
            "id": "<VOLATILE>",
            "did": "<VOLATILE>",
            "handle": "golden.zurfur.app",
            "name": "Golden Studio",
            "role": "owner",
        }],
    });
    assert_eq!(
        normalized(&body),
        expected,
        "R7: wrapped object, never a bare array"
    );

    // Delete: the wire SAYS which deletion happened (ruling 2026-07-25).
    let res = c
        .delete(format!("{base}/accounts/{account_id}"))
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

/// The commission pair: create returns the created resource (ruling
/// 2026-07-25), list wraps; absent optionals omit keys; nested maturity is
/// camelCase-clean.
#[tokio::test]
async fn commission_wire_shapes() {
    let profile = Profile {
        did: Did::new("did:plc:commgold".to_string()),
        handle: "commgold.bsky.social".to_string(),
        display_name: None,
        avatar_url: None,
    };
    let (base, _backend) = spawn_app("did:plc:commgold", profile).await;
    let c = client();
    sign_in(&c, &base, "commgold.bsky.social").await;

    // Create with a maturity but no deadline: the response carries the full
    // envelope; deadline / statuses / channel are ABSENT, not null (R4).
    let res = c
        .post(format!("{base}/commissions"))
        .json(&json!({
            "title": "A golden ref sheet",
            "maturity": { "rating": "safe", "graphic": false },
        }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201);
    let created: Value = res.json().await.expect("json");
    let expected = json!({
        "id": "<VOLATILE>",
        "title": "A golden ref sheet",
        "lifecycle": "draft",
        "visibility": "private",
        // `graphic` is ABSENT here although the request sent `false`: implicit-
        // presence defaults are omitted (canonical ProtoJSON; the contract's
        // §7.7 canonical settings). Absent ⇒ false, and a generated client's
        // parse restores the default structurally. This hunk moved at the
        // ZMVP-160 codec adoption, deliberately — the golden records it.
        "maturity": { "rating": "safe" },
        // `<TS>Z` pins the ENCODING: canonical ProtoJSON is Z-normalized —
        // an emitter drifting to `+00:00` (pbjson's raw Timestamp serializer
        // does exactly that) fails here instead of shipping.
        "createdAt": "<TS>Z",
    });
    assert_eq!(
        normalized(&created),
        expected,
        "create returns the resource; absent optionals omit keys (R4); \
         implicit-presence defaults omit too (§7.7); timestamps are \
         Z-normalized (§7.3); vocabulary stays lowercase (R8)"
    );

    // List: wrapped (R7).
    let body: Value = c
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions")
        .json()
        .await
        .expect("json");
    let rows = body["commissions"].as_array().expect("wrapped rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        normalized(&rows[0]),
        expected,
        "the listing row matches the created resource, shape-for-shape"
    );
}
