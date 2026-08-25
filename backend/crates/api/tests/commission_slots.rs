//! ZMVP-77 — the owner declares Slots (Character positions; fill deferred) over
//! HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams are
//! covered in `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — the owner declares Slots with `POST /commissions/{id}/slots`,
//!   whose body is an **array** of Slot objects (PR #108 ruling: Slots usually
//!   arrive several at a time; the batch lands all-or-nothing, an empty array
//!   is a `422`): each with a required title (trimmed, blank refused with a
//!   `422`) and optional freeform notes (trimmed; blank normalizes to absent).
//!   Each Slot is carried by an ordinary element in the composition, its
//!   title/notes riding in the satellite.
//! - **AC2** — a commission holds zero or more Slots; an empty (unfilled) Slot
//!   is a valid, permanent state — there is no occupant anywhere to be missing.
//! - **AC3** — filling is not offered: no fill surface exists on this route (or
//!   any other), and no occupant is representable in the read-back shape.
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a 403,
//!   and byte-identical bodies, so no existence oracle; a fabricated/foreign
//!   tab is a `tab_not_found` 404; an undeclared surface is a `422`
//!   `unknown_surface`; a malformed body is a `422`. Declaring a Slot
//!   appends **no** changelog entry — the frozen ZMVP-87 taxonomy carries
//!   `seat_declared` for Seats but no Slot variant.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use adapter_mem::MemBackend;
use api::AppState;
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, ElementId, ElementType, SKELETON},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect the composition and slots that were
/// persisted. `did` is the identity `sign_in` will authenticate as.
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let test_support::runtime::MemRuntime { runtime, backend } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .profile(Profile::new(
                Did::new(did.to_string()),
                "artist.bsky.social",
            ))
            .public_url(format!("http://{addr}"))
            .build();
    let state: AppState = runtime;
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

/// Drives the two-step sign-in so the client's cookie jar carries a live session
/// for the app's configured DID.
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

/// Creates a commission over HTTP as the signed-in caller and returns its id
/// (introspected off the backend — the route returns a bare `201`).
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

/// The commission's only tab id, introspected off the backend. There is no
/// ROUTE that hands a caller a tab id yet — reading the composition is
/// ZMVP-163's `GET` — so tests read it from the store.
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

/// POSTs a Slot-declaration batch (a JSON array of Slot objects) and returns
/// the carrying elements' ids from the `201` body, in request order.
async fn declare_slots(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    body: &serde_json::Value,
) -> Vec<uuid::Uuid> {
    let res = client
        .post(format!("{base}/commissions/{commission}/slots"))
        .json(body)
        .send()
        .await
        .expect("POST slots");
    assert_eq!(res.status(), 201, "declaring slots returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["ids"]
        .as_array()
        .expect("the body carries the new element ids")
        .iter()
        .map(|id| {
            id.as_str()
                .expect("each id is a string")
                .parse()
                .expect("each id is a UUID")
        })
        .collect()
}

/// Seeds a committed commission owned by a directly-provisioned user (someone
/// other than the signed-in caller), returning its id.
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

// AC1/AC2 — the owner declares Slots into a declared surface: each lands as an
// ordinary element (born Total, empty payload, the owner's envelope)
// whose satellite carries the trimmed title and the notes (present on one,
// absent on the other) — a commission going from zero Slots to two. No
// changelog entry is appended (the frozen taxonomy has no Slot variant).
#[tokio::test]
async fn the_owner_declares_slots_with_title_and_optional_notes() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    // Zero Slots is a valid state (AC2).
    assert!(
        backend
            .slots_of(CommissionId::new(id))
            .await
            .expect("list slots")
            .is_empty(),
        "a fresh commission holds zero Slots"
    );

    // One request declares both (the array contract, PR #108 ruling); the 201
    // ids come back in request order.
    let ids = declare_slots(
        &client,
        &base,
        id,
        &json!([
            { "tab": tab, "surface": only_surface(), "title": "  The knight  ", "notes": "  full plate, no cape  " },
            { "tab": tab, "surface": only_surface(), "title": "The mage" },
        ]),
    )
    .await;
    let (noted, bare) = (ids[0], ids[1]);

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // The composition half: both slots ride ordinary elements, typed `slot`.
    let elements = backend
        .elements_of(CommissionId::new(id))
        .await
        .expect("load elements");
    assert_eq!(elements.len(), 2);
    assert_eq!(
        *elements[0].id, noted,
        "the 201 ids reappear in the composition, in request order"
    );
    assert_eq!(*elements[1].id, bare);
    for element in &elements {
        assert_eq!(
            element.element_type,
            ElementType::slot(),
            "the Slot's carrying element is typed `slot`"
        );
        assert_eq!(element.created_by, me.id, "the envelope names the creator");
        assert_eq!(
            element.payload.as_value(),
            &json!({}),
            "the carrying element's payload is empty — the Slot lives in the satellite"
        );
    }

    // The satellite half: trimmed title, notes present/absent as declared.
    let noted_slot = backend
        .find_slot(ElementId::new(noted))
        .await
        .expect("find slot")
        .expect("the declared slot has its satellite");
    assert_eq!(noted_slot.title.as_str(), "The knight", "title is trimmed");
    assert_eq!(
        noted_slot.notes.as_deref(),
        Some("full plate, no cape"),
        "notes are trimmed and kept"
    );
    assert_eq!(noted_slot.commission_id, CommissionId::new(id));

    let bare_slot = backend
        .find_slot(ElementId::new(bare))
        .await
        .expect("find slot")
        .expect("satellite exists");
    assert_eq!(bare_slot.title.as_str(), "The mage");
    assert!(bare_slot.notes.is_none(), "omitted notes stay absent");

    // Zero or more: the commission now counts exactly two (AC2).
    let slots = backend
        .slots_of(CommissionId::new(id))
        .await
        .expect("list slots");
    assert_eq!(slots.len(), 2, "the commission holds two declared Slots");

    // Declaring Slots appends NO changelog entry (the taxonomy's seat_declared
    // is seat-specific; no Slot variant exists): only creation is in the stream.
    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(
        entries.len(),
        1,
        "slot declaration is not changelog-recorded"
    );
}

// AC1 — the title is required: a blank title (and a missing one) is a 422, and
// nothing lands — no element, no satellite.
#[tokio::test]
async fn a_blank_or_missing_title_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "tab": tab, "surface": only_surface(), "title": "   " }]))
        .send()
        .await
        .expect("POST blank title");
    common::assert_problem(res, 422, "invalid_request").await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "tab": tab, "surface": only_surface() }]))
        .send()
        .await
        .expect("POST missing title");
    common::assert_problem(res, 422, "invalid_request").await;

    // Declaring nothing is malformed, not a no-op (the array contract).
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([]))
        .send()
        .await
        .expect("POST empty batch");
    common::assert_problem(res, 422, "invalid_request").await;

    assert!(
        backend
            .elements_of(CommissionId::new(id))
            .await
            .expect("load elements")
            .is_empty(),
        "no refused write landed"
    );
    assert!(
        backend
            .slots_of(CommissionId::new(id))
            .await
            .expect("list slots")
            .is_empty()
    );
}

// AC1 — notes are optional freeform: blank notes normalize to absent rather
// than storing whitespace.
#[tokio::test]
async fn blank_notes_normalize_to_absent() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let element = declare_slots(
        &client,
        &base,
        id,
        &json!([{ "tab": tab, "surface": only_surface(), "title": "The bard", "notes": "   " }]),
    )
    .await[0];

    let slot = backend
        .find_slot(ElementId::new(element))
        .await
        .expect("find slot")
        .expect("satellite exists");
    assert!(slot.notes.is_none(), "blank notes are not stored");
}

// AC1/ZMVP-166 — declaring into a surface the skeleton does not declare is
// rejected with a 422 unknown_surface, and the batch is ALL-OR-NOTHING: a
// refusing entry takes the valid ones with it. (The retired tree's
// "declaring under a component" case has no analogue: nothing is ever an
// element's child, so there is no illegal parent left to name.)
#[tokio::test]
async fn an_undeclared_surface_is_rejected_and_takes_the_batch_with_it() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;
    declare_slots(
        &client,
        &base,
        id,
        &json!([{ "tab": tab, "surface": only_surface(), "title": "First" }]),
    )
    .await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "tab": tab, "surface": "invented", "title": "Nowhere?" }]))
        .send()
        .await
        .expect("POST undeclared surface");
    common::assert_problem(res, 422, "unknown_surface").await;

    // All-or-nothing: a refusing slot mid-batch takes the valid one with it.
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([
            { "tab": tab, "surface": only_surface(), "title": "Would be fine alone" },
            { "tab": uuid::Uuid::now_v7(), "surface": only_surface(), "title": "Doomed" },
        ]))
        .send()
        .await
        .expect("POST mixed batch");
    common::assert_problem(res, 404, "tab_not_found").await;

    let slots = backend
        .slots_of(CommissionId::new(id))
        .await
        .expect("list slots");
    assert_eq!(
        slots.len(),
        1,
        "only the first slot exists — no refused batch left its valid half behind"
    );
}

// Floor — anonymous callers can't declare slots: 401.
#[tokio::test]
async fn an_anonymous_caller_cannot_declare_a_slot() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "tab": tab, "surface": only_surface(), "title": "The knight" }]))
        .send()
        .await
        .expect("anonymous POST");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// Floor (the closed door) — a signed-in NON-participant probing someone else's
// commission gets the one uniform commission_not_found 404, byte-identical to
// the answer for a commission that does not exist at all. Never a 403.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_tab = tab_of(&backend, foreign).await;

    let hidden = client
        .post(format!("{base}/commissions/{foreign}/slots"))
        .json(&json!([{ "tab": foreign_tab, "surface": only_surface(), "title": "Probe" }]))
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .post(format!("{base}/commissions/{absent_id}/slots"))
        .json(&json!([{ "tab": foreign_tab, "surface": only_surface(), "title": "Probe" }]))
        .send()
        .await
        .expect("probe absent");
    let absent_status = absent.status().as_u16();
    let absent_body: serde_json::Value = absent.json().await.expect("problem body");

    assert_eq!(hidden_status, 404, "hidden = not found, never 403");
    assert_eq!(hidden_body["code"], "commission_not_found");
    assert_eq!(
        (hidden_status, &hidden_body),
        (absent_status, &absent_body),
        "hidden and absent are indistinguishable (no existence oracle)"
    );

    assert!(
        backend
            .slots_of(CommissionId::new(foreign))
            .await
            .expect("list slots")
            .is_empty(),
        "the probe wrote nothing"
    );
}

// Floor — the owner naming a tab that doesn't exist in this commission
// (fabricated, or belonging to another commission) gets tab_not_found; the
// foreign case answers identically to the fabricated one.
#[tokio::test]
async fn an_unknown_or_foreign_tab_is_tab_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "tab": uuid::Uuid::now_v7(), "surface": only_surface(), "title": "The knight" }]))
        .send()
        .await
        .expect("POST fabricated tab");
    common::assert_problem(res, 404, "tab_not_found").await;

    let foreign = seed_foreign_commission(&backend).await;
    let foreign_tab = tab_of(&backend, foreign).await;
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "tab": foreign_tab, "surface": only_surface(), "title": "The knight" }]))
        .send()
        .await
        .expect("POST foreign tab");
    common::assert_problem(res, 404, "tab_not_found").await;
}

// Floor — a malformed body (no address) is a 422.
#[tokio::test]
async fn a_malformed_body_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "title": "The knight" }]))
        .send()
        .await
        .expect("POST malformed");
    common::assert_problem(res, 422, "invalid_request").await;
}
