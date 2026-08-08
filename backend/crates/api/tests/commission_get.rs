//! ZMVP-163 — `GET /commissions/{id}`: the commission read and the composition's
//! only exit.
//!
//! What this suite pins, over HTTP:
//!
//! - **The shape** — the contract's `GetCommissionResponse` (DD `40992770`):
//!   lowerCamelCase keys (R1), absent optionals omitting theirs (R4), the
//!   envelope flat alongside `tabs` / `surfaces` / `elements`.
//! - **The address model on the wire** — elements and surfaces cite their tab
//!   **by id**, and the ids they cite are the commission's own. This is also the
//!   first route that hands a caller a tab id at all: until now nothing did,
//!   which is why the element-write suite had to read one out of the store.
//! - **What is deliberately NOT there** — no `position`, no `band`, no
//!   `createdBy`, anywhere in an element. R3 forbids a stored ordinal on the
//!   wire and a sparse one would count what the projection removed; `createdBy`
//!   is off v1 by DD `42762241` D5. Their absence is asserted, not assumed,
//!   because adding a field later is additive while removing one is a major.
//! - **The withheld discriminant exists and is honest** — `compositionWithheld`
//!   is minted at birth (D6/R4) and is false for a participant, who is never
//!   withheld from.
//! - **A participant sees everything, whatever the modes say** — the tier v1
//!   serves is `ViewerTier::PARTICIPANT`, so narrowing a tab or surface to the
//!   closed door changes nothing for them. (The tier that *does* filter is
//!   ZMVP-75's; its arithmetic is pinned in the domain's projection unit tests,
//!   where every combination is reachable.)
//! - **Payload fidelity** — opaque JSON round-trips verbatim as a string,
//!   including an integer above 2^53, which is the whole reason the payload is
//!   not a `google.protobuf.Struct` (DD `42762241` D4).
//! - **The closed door** — anonymous is `401`; a non-participant and a
//!   completely absent id get the ONE uniform 404, byte-identical, so the
//!   response is never an existence oracle.
//!
//! Same in-process fakes as the sibling api e2e suites — no network, no
//! database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{
        Commission, CommissionId, CommissionTitle, SKELETON, SurfaceName, VisibilityMode,
    },
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use tower_sessions::{MemoryStore, SessionManagerLayer};

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can seed modes the write surface does not expose
/// yet (ZMVP-74 owns widening).
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let backend = MemBackend::new();
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url: "postgres://unused".to_string(),
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".to_string(),
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        },
        files: backend.file_store(),
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "artist.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        database: backend.database(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
}

/// A cookie-keeping client that does not auto-follow redirects.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Drives the two-step sign-in so the client's cookie jar carries a live
/// session for the app's configured DID.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=artist.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin should redirect to the PDS");
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback should redirect on success");
}

/// Creates a commission over HTTP as the signed-in caller and returns its id.
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");
    let all = backend.all_commissions().await.expect("list commissions");
    *all.last().expect("a commission was persisted").id
}

/// The commission's only tab id, introspected off the backend — the fixture for
/// writing elements. (Reading one back out of the `GET` is what a real client
/// does, and is itself asserted below.)
async fn tab_of(backend: &MemBackend, commission: uuid::Uuid) -> uuid::Uuid {
    let tabs = backend
        .tabs_of(CommissionId::new(commission))
        .await
        .expect("load tabs");
    *tabs.first().expect("every commission has its tabs").id
}

/// The one surface the placeholder skeleton declares.
fn only_surface() -> &'static str {
    SKELETON[0].surfaces[0]
}

/// POSTs a new element carrying `payload` and returns the created element's id.
async fn add_element(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    tab: uuid::Uuid,
    payload: &Value,
) -> uuid::Uuid {
    let body = json!({
        "tab": tab,
        "surface": only_surface(),
        "type": "note",
        "payload": payload,
    });
    let res = client
        .post(format!("{base}/commissions/{commission}/elements"))
        .json(&body)
        .send()
        .await
        .expect("POST element");
    assert_eq!(res.status(), 201, "adding an element returns 201");
    let body: Value = res.json().await.expect("201 body is JSON");
    body["id"]
        .as_str()
        .expect("the body carries the new element id")
        .parse()
        .expect("the id is a UUID")
}

/// `GET /commissions/{id}` as the signed-in caller, asserting `200` and
/// returning the parsed body.
async fn get_commission(client: &reqwest::Client, base: &str, commission: uuid::Uuid) -> Value {
    let res = client
        .get(format!("{base}/commissions/{commission}"))
        .send()
        .await
        .expect("GET commission");
    assert_eq!(res.status(), 200, "the owner reads their own commission");
    res.json().await.expect("200 body is JSON")
}

/// One of the response's repeated fields, as a slice.
///
/// Canonical ProtoJSON **omits an empty repeated field entirely**, so an empty
/// composition arrives with no `elements` key at all rather than with `[]`. That
/// is exactly why `compositionWithheld` exists: "the key is not there" has to
/// keep meaning only "nothing is here" (R4), and withholding says so out loud
/// instead of borrowing that same silence.
fn rows<'a>(body: &'a Value, field: &str) -> &'a [Value] {
    body.get(field).map_or(&[][..], |value| {
        value.as_array().expect("an array").as_slice()
    })
}

/// Seeds a committed commission owned by someone other than the signed-in
/// caller, returning its id.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = "Not yours".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// The shape: the envelope flat, the composition beside it, canonical ProtoJSON
// keys, and the skeleton served whole even before anything is contributed.
#[tokio::test]
async fn the_read_serves_the_envelope_and_the_whole_skeleton() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let body = get_commission(&client, &base, id).await;

    assert_eq!(body["id"], id.to_string());
    assert_eq!(body["title"], "A ref sheet");
    assert_eq!(body["lifecycle"], "draft");
    assert_eq!(body["visibility"], "private");
    assert!(
        body["createdAt"].is_string(),
        "R1: lowerCamelCase, and the timestamp is a string"
    );
    assert!(
        body.get("created_at").is_none() && body.get("direction_status").is_none(),
        "R1: no snake_case key survives anywhere"
    );
    assert!(
        body.get("deadline").is_none() && body.get("maturity").is_none(),
        "R4: an absent optional OMITS its key — never null"
    );

    // The skeleton is served, and it is the skeleton: one tab, its declared
    // surfaces, all born at the closed door.
    let tabs = rows(&body, "tabs");
    assert_eq!(tabs.len(), SKELETON.len(), "one row per declared tab");
    assert_eq!(tabs[0]["tab"], SKELETON[0].tab);
    assert_eq!(tabs[0]["mode"], "total", "every term is born Total");

    let surfaces = rows(&body, "surfaces");
    assert_eq!(
        surfaces.len(),
        SKELETON[0].surfaces.len(),
        "the declared surfaces are served, so the renderer needs no second copy \
         of the skeleton that could drift"
    );
    assert_eq!(surfaces[0]["surface"], only_surface());
    assert_eq!(
        surfaces[0]["mode"], "total",
        "an absent surface-mode row means Total"
    );
    assert_eq!(
        surfaces[0]["tabId"], tabs[0]["id"],
        "a surface cites its tab BY ID"
    );

    // A composition with nothing in it is an empty list, and it says so
    // explicitly rather than by omission being ambiguous with withholding.
    assert!(rows(&body, "elements").is_empty());
    assert_ne!(
        body["compositionWithheld"],
        Value::Bool(true),
        "a participant is never withheld from"
    );
}

// The element on the wire: the envelope a renderer switches on, the payload as
// opaque text — and NOTHING else. The negative half is the load-bearing one.
#[tokio::test]
async fn an_element_carries_its_envelope_and_no_ordinal() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;
    let element = add_element(&client, &base, id, tab, &json!({ "body": "hi" })).await;

    let body = get_commission(&client, &base, id).await;
    let elements = rows(&body, "elements");
    assert_eq!(elements.len(), 1);
    let only = &elements[0];

    assert_eq!(only["id"], element.to_string());
    assert_eq!(
        only["tabId"],
        tab.to_string(),
        "elements address tabs by id"
    );
    assert_eq!(only["surface"], only_surface());
    assert_eq!(
        only["kind"], "note",
        "the type tag the renderer switches on"
    );
    assert_eq!(only["mode"], "total", "every element is born closed");

    // R3 and DD 42762241 D5, asserted as absences: a stored ordinal must never
    // cross the wire (a sparse one would count what the projection removed),
    // and created_by is off v1 (omitting is additive later; exposing is
    // forever). Adding either is a deliberate act, not a slip.
    for forbidden in [
        "position",
        "band",
        "createdBy",
        "created_by",
        "createdAt",
        "effectiveMode",
    ] {
        assert!(
            only.get(forbidden).is_none(),
            "an element must not carry {forbidden:?}: {only}"
        );
    }

    // The whole element object is exactly the six documented keys.
    let mut keys: Vec<&str> = only
        .as_object()
        .expect("an element is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["id", "kind", "mode", "opaqueJson", "surface", "tabId"]
    );
}

// The payload is opaque TEXT, verbatim — including an integer above 2^53, the
// regression that disqualified google.protobuf.Struct (DD 42762241 D4: pbjson
// errors above 2^53 while protobuf-es silently truncates).
#[tokio::test]
async fn the_payload_round_trips_verbatim_beyond_two_to_the_fifty_third() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let huge = "9007199254740993"; // 2^53 + 1
    let payload: Value = serde_json::from_str(&format!(
        "{{\"big\":{huge},\"text\":\"三毛猫 🐾\",\"nested\":{{\"list\":[1,2,3]}}}}"
    ))
    .expect("valid json");
    add_element(&client, &base, id, tab, &payload).await;

    let body = get_commission(&client, &base, id).await;
    let opaque = body["elements"][0]["opaqueJson"]
        .as_str()
        .expect("the payload travels as a JSON STRING, not as structure");
    assert!(
        opaque.contains(huge),
        "the big integer must survive as digits: {opaque}"
    );

    let parsed: Value = serde_json::from_str(opaque).expect("the string parses back as JSON");
    assert_eq!(parsed, payload, "the payload round-trips unmodified");

    // An element with no payload carries the empty object — one spelling for
    // "nothing", so the oneof is never unset and absence never means anything.
    add_element(&client, &base, id, tab, &json!({})).await;
    let body = get_commission(&client, &base, id).await;
    assert_eq!(body["elements"][1]["opaqueJson"], "{}");
}

// The tier v1 serves is PARTICIPANT, and a participant is admitted to
// everything: closing every mode to the closed door changes nothing they see.
// (The tier that filters is ZMVP-75's; the arithmetic is pinned in the domain.)
#[tokio::test]
async fn a_participant_sees_the_whole_composition_whatever_the_modes_say() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;
    add_element(&client, &base, id, tab, &json!({ "body": "the brief" })).await;

    let widened = get_commission(&client, &base, id).await;
    assert_eq!(rows(&widened, "elements").len(), 1);

    // Narrow every term to the closed door. The owner is a participant, so the
    // projection admits all three regardless.
    let surface = only_surface()
        .parse::<SurfaceName>()
        .expect("the skeleton declares valid labels");
    backend.set_tab_mode(
        domain::elements::commission::TabId::new(tab),
        VisibilityMode::Total,
    );
    backend.set_surface_mode(CommissionId::new(id), surface, VisibilityMode::Total);

    let closed = get_commission(&client, &base, id).await;
    assert_eq!(
        rows(&closed, "elements").len(),
        1,
        "Total is participants-only, and the participant is one"
    );
    assert_eq!(rows(&closed, "tabs").len(), 1);
    assert_eq!(rows(&closed, "surfaces").len(), 1);
}

// The closed door: a non-participant and a completely absent id are answered
// identically — same status, byte-identical body — so nothing distinguishes
// "hidden from you" from "does not exist". Never a 403.
#[tokio::test]
async fn the_read_is_not_an_existence_oracle() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    let foreign = seed_foreign_commission(&backend).await;
    let absent = uuid::Uuid::now_v7();

    let hidden = client
        .get(format!("{base}/commissions/{foreign}"))
        .send()
        .await
        .expect("GET foreign commission");
    assert_eq!(hidden.status(), 404, "never a 403 — that would confirm it");
    let hidden_body: Value = hidden.json().await.expect("problem body");

    let missing = client
        .get(format!("{base}/commissions/{absent}"))
        .send()
        .await
        .expect("GET absent commission");
    assert_eq!(missing.status(), 404);
    let missing_body: Value = missing.json().await.expect("problem body");

    assert_eq!(
        hidden_body, missing_body,
        "the two 404s must be indistinguishable"
    );
    assert_eq!(hidden_body["code"], "commission_not_found");
}

// Signed out is 401, not a redirect: the frontend CALLS this endpoint.
#[tokio::test]
async fn an_anonymous_caller_is_refused() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let owner = client();
    sign_in(&owner, &base).await;
    let id = create_commission(&owner, &base, &backend).await;

    let anonymous = client();
    let res = anonymous
        .get(format!("{base}/commissions/{id}"))
        .send()
        .await
        .expect("GET commission");
    assert_eq!(res.status(), 401);
}
