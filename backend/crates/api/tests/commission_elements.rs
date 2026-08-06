//! ZMVP-166 — the owner composes the commission over HTTP.
//!
//! **This suite replaces three.** The retired tree needed `commission_surfaces`
//! (ZMVP-71), `commission_components` (ZMVP-72), and `commission_removal`
//! (ZMVP-73) because it had three write routes; the flat model has one pair, so
//! their acceptance criteria land here together. What each of them pinned still
//! is pinned:
//!
//! - **Birth** — every commission is born with its skeleton tabs (introspected
//!   off the backend — creation itself mints them), all born `Total`. Tabs and
//!   surfaces cannot be removed: no route addresses them at all, because they
//!   are code-declared skeleton rather than elements.
//! - **Add** — the owner contributes an element into a declared surface with
//!   `POST /commissions/{id}/elements`; the `201` body carries the new element's
//!   id, appended within its ordering group.
//! - **Born closed** — every element is born mode `Total`; no mode is accepted
//!   from the client at all (widening is ZMVP-74).
//! - **Opaque payload** — the payload round-trips unmodified, and an omitted one
//!   defaults to the empty object.
//! - **Remove** — `DELETE /commissions/{id}/elements/{element}` answers `204`
//!   and renumbers the ordering group. There is no `409` arm: nothing is
//!   irremovable, because the skeleton is not made of elements.
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a 403,
//!   and byte-identical bodies, so no existence oracle; a fabricated or foreign
//!   tab is a `tab_not_found` 404; an undeclared surface is an
//!   `unknown_surface` 422; a malformed body is a `422`. Composition edits
//!   append **no** changelog entries (not in the frozen taxonomy).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{
        Commission, CommissionId, CommissionTitle, ElementPayload, ElementType, SKELETON,
        VisibilityMode, declared_tabs,
    },
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect the composition that was persisted.
/// `did` is the identity `sign_in` will authenticate as.
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

/// The commission's only tab id, introspected off the backend.
///
/// ⚠️ There is no ROUTE that hands a caller a tab id: reading the composition is
/// ZMVP-163's `GET`. Until it lands, an element write is only exercisable with
/// an id read out of the store like this — which is exactly why the gap is
/// called out in [`api::routes`]'s element module docs rather than papered over.
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

/// POSTs a new element at `(tab, surface)` carrying `payload` and returns the
/// created element's id from the `201` body.
async fn add_element(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    tab: uuid::Uuid,
    payload: &serde_json::Value,
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
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["id"]
        .as_str()
        .expect("the body carries the new element id")
        .parse()
        .expect("the id is a UUID")
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

// Birth — creation itself mints the skeleton tabs, every one Total, with no
// elements. Nothing can ever remove them: no route addresses a tab or a surface
// at all, because they are code-declared skeleton rather than data.
#[tokio::test]
async fn a_created_commission_is_born_with_its_skeleton_tabs() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let tabs = backend
        .tabs_of(CommissionId::new(id))
        .await
        .expect("load tabs");
    let names: Vec<&str> = tabs.iter().map(|tab| tab.tab.as_str()).collect();
    let declared: Vec<String> = declared_tabs()
        .iter()
        .map(|tab| tab.as_str().to_owned())
        .collect();
    assert_eq!(names, declared, "exactly the code-declared skeleton");
    assert!(
        tabs.iter().all(|tab| tab.mode == VisibilityMode::Total),
        "every tab is born Total (the closed door)"
    );
    assert!(
        backend
            .elements_of(CommissionId::new(id))
            .await
            .expect("load elements")
            .is_empty(),
        "a fresh commission is composed of nothing"
    );
}

// Add — the owner contributes elements into a declared surface: they append in
// order within the group, every one is born Total with the creator's envelope,
// and the payload round-trips exactly as sent (nested structure, unicode,
// numbers, booleans, in-payload nulls).
#[tokio::test]
async fn the_owner_contributes_elements_that_append_and_round_trip() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let payload = json!({
        "kind": "text",
        "body": "Reference: 三毛猫 🐾",
        "nested": { "list": [1, 2, 3], "flag": true, "nothing": null },
    });
    let first = add_element(&client, &base, id, tab, &payload).await;
    let second = add_element(&client, &base, id, tab, &json!({})).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    let elements = backend
        .elements_of(CommissionId::new(id))
        .await
        .expect("load elements");
    assert_eq!(elements.len(), 2);
    assert_eq!(*elements[0].id, first, "append order");
    assert_eq!(elements[0].position, 0);
    assert_eq!(*elements[1].id, second);
    assert_eq!(elements[1].position, 1);
    assert_eq!(
        elements[0].payload.as_value(),
        &payload,
        "the payload round-trips unmodified"
    );
    for element in &elements {
        assert_eq!(
            element.mode,
            VisibilityMode::Total,
            "every element is born Total"
        );
        assert_eq!(element.created_by, me.id, "the envelope names the creator");
        assert_eq!(element.address.surface.as_str(), only_surface());
        assert_eq!(*element.address.tab, tab, "addressed by tab id");
        assert_eq!(element.element_type, "note".parse::<ElementType>().unwrap());
    }

    // Composition edits are NOT changelog events (the taxonomy is frozen;
    // ZMVP-87): the stream still holds only the creation entry.
    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(
        entries.len(),
        1,
        "contributing elements appends no changelog entry"
    );
}

// A request that omits the payload creates an element with the empty object
// payload (the untyped v1 default), not an error.
#[tokio::test]
async fn an_omitted_payload_defaults_to_the_empty_object() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": tab, "surface": only_surface(), "type": "note" }))
        .send()
        .await
        .expect("POST element without payload");
    assert_eq!(res.status(), 201);

    let elements = backend
        .elements_of(CommissionId::new(id))
        .await
        .expect("load elements");
    assert_eq!(elements[0].payload.as_value(), &json!({}));
}

// Remove — the owner removes an element with a 204, the ordering group
// renumbers contiguously, and the survivors keep their order. There is no
// irremovable element to refuse: tabs and surfaces are skeleton, so no element
// id addresses one.
#[tokio::test]
async fn the_owner_removes_an_element_and_the_group_renumbers() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let first = add_element(&client, &base, id, tab, &json!({})).await;
    let doomed = add_element(&client, &base, id, tab, &json!({})).await;
    let last = add_element(&client, &base, id, tab, &json!({})).await;

    let res = client
        .delete(format!("{base}/commissions/{id}/elements/{doomed}"))
        .send()
        .await
        .expect("DELETE element");
    assert_eq!(res.status(), 204, "removal answers 204 No Content");

    let elements = backend
        .elements_of(CommissionId::new(id))
        .await
        .expect("load elements");
    let surviving: Vec<(uuid::Uuid, i32)> = elements
        .iter()
        .map(|element| (*element.id, element.position))
        .collect();
    assert_eq!(
        surviving,
        vec![(first, 0), (last, 1)],
        "the survivors renumber contiguously from 0, order preserved"
    );

    // Removal is likewise not a changelog event.
    assert_eq!(
        backend
            .changelog_entries(CommissionId::new(id))
            .await
            .expect("changelog")
            .len(),
        1,
    );
}

// Floor — anonymous callers can't compose: 401 on both routes, and nothing
// changes.
#[tokio::test]
async fn an_anonymous_caller_cannot_compose() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let tab = tab_of(&backend, id).await;
    let element = add_element(&signed_in, &base, id, tab, &json!({})).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": tab, "surface": only_surface(), "type": "note" }))
        .send()
        .await
        .expect("anonymous POST");
    common::assert_problem(res, 401, "not_authenticated").await;

    let res = client()
        .delete(format!("{base}/commissions/{id}/elements/{element}"))
        .send()
        .await
        .expect("anonymous DELETE");
    common::assert_problem(res, 401, "not_authenticated").await;

    assert_eq!(
        backend
            .elements_of(CommissionId::new(id))
            .await
            .expect("load elements")
            .len(),
        1,
        "the anonymous probes changed nothing"
    );
}

// Floor (the closed door) — a signed-in NON-participant probing someone else's
// commission gets the one uniform commission_not_found 404, byte-identical to
// the answer for a commission that does not exist at all. Never a 403: a 403
// would confirm there is something to be forbidden from.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_tab = tab_of(&backend, foreign).await;
    let body = json!({ "tab": foreign_tab, "surface": only_surface(), "type": "note" });

    // Probing a real commission I may not see...
    let hidden = client
        .post(format!("{base}/commissions/{foreign}/elements"))
        .json(&body)
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    // ...answers exactly like probing one that doesn't exist.
    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .post(format!("{base}/commissions/{absent_id}/elements"))
        .json(&body)
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

    // And the probe wrote nothing.
    assert!(
        backend
            .elements_of(CommissionId::new(foreign))
            .await
            .expect("load elements")
            .is_empty()
    );
}

// Floor — the owner naming a tab that doesn't exist in this commission
// (fabricated, or belonging to another commission) gets tab_not_found; the
// foreign case answers identically to the fabricated one, so a tab id is no
// cross-commission oracle.
#[tokio::test]
async fn an_unknown_or_foreign_tab_is_tab_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let fabricated = uuid::Uuid::now_v7();
    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": fabricated, "surface": only_surface(), "type": "note" }))
        .send()
        .await
        .expect("POST fabricated tab");
    common::assert_problem(res, 404, "tab_not_found").await;

    let foreign = seed_foreign_commission(&backend).await;
    let foreign_tab = tab_of(&backend, foreign).await;
    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": foreign_tab, "surface": only_surface(), "type": "note" }))
        .send()
        .await
        .expect("POST foreign tab");
    common::assert_problem(res, 404, "tab_not_found").await;
}

// Floor — a surface the code skeleton does not declare is a 422
// `unknown_surface`, NOT a 404: the surface vocabulary is global and invariant,
// so refusing one leaks nothing about anybody's commission, and hiding it would
// only make an honest client's mistake harder to diagnose.
#[tokio::test]
async fn an_undeclared_surface_is_a_422() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": tab, "surface": "invented", "type": "note" }))
        .send()
        .await
        .expect("POST undeclared surface");
    common::assert_problem(res, 422, "unknown_surface").await;

    assert!(
        backend
            .elements_of(CommissionId::new(id))
            .await
            .expect("load elements")
            .is_empty(),
        "nothing landed, and no surface was invented"
    );
}

// Floor — the skeleton check is on the (tab, surface) PAIR, and the route maps
// it the same way: a surface that is perfectly real under its own tab, addressed
// under a different tab of the same commission, is the same `unknown_surface`
// 422. Not a 404 — the tab is real and this caller may see it, so there is no
// existence to hide; what is being refused is an address the program does not
// describe.
#[tokio::test]
async fn a_real_surface_under_the_wrong_tab_is_a_422() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    // A real tab of this commission whose name the skeleton does not pair with
    // the surface below. The placeholder skeleton declares a single tab, so the
    // shape is seeded; ZMVP-171's real skeleton makes it an ordinary address.
    let other = backend.seed_tab(
        CommissionId::new(id),
        "other".parse().expect("valid tab name"),
    );

    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": *other, "surface": only_surface(), "type": "note" }))
        .send()
        .await
        .expect("POST a wrongly addressed pair");
    common::assert_problem(res, 422, "unknown_surface").await;

    assert!(
        backend
            .elements_of(CommissionId::new(id))
            .await
            .expect("load elements")
            .is_empty(),
        "nothing landed under a place the skeleton never described"
    );
}

// Floor — removing an element that isn't this commission's (fabricated, or
// belonging to another commission) is element_not_found, indistinguishably.
#[tokio::test]
async fn removing_an_unknown_or_foreign_element_is_element_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let fabricated = uuid::Uuid::now_v7();
    let res = client
        .delete(format!("{base}/commissions/{id}/elements/{fabricated}"))
        .send()
        .await
        .expect("DELETE fabricated");
    common::assert_problem(res, 404, "element_not_found").await;

    // A real element — in someone else's commission, reached through mine.
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_tab = tab_of(&backend, foreign).await;
    let foreign_element = {
        use domain::elements::commission::{NewElement, SurfaceAddress, TabId};
        use domain::ports::UnitOfWork;
        let owner = backend
            .find_by_did(&Did::new("did:plc:someone-else".to_string()))
            .await
            .expect("find")
            .expect("provisioned");
        let element = NewElement::contributed(
            CommissionId::new(foreign),
            SurfaceAddress::new(
                TabId::new(foreign_tab),
                only_surface().parse().expect("declared"),
            ),
            "note".parse().expect("valid type"),
            ElementPayload::default(),
            owner.id,
            Utc::now(),
        );
        let element_id = *element.id;
        let database = backend.database();
        let mut uow = database.begin().await.expect("begin");
        UnitOfWork::commissions(&mut *uow)
            .add_element(&element)
            .await
            .expect("seed foreign element");
        uow.commit().await.expect("commit");
        element_id
    };

    let res = client
        .delete(format!(
            "{base}/commissions/{id}/elements/{foreign_element}"
        ))
        .send()
        .await
        .expect("DELETE foreign");
    common::assert_problem(res, 404, "element_not_found").await;

    assert_eq!(
        backend
            .elements_of(CommissionId::new(foreign))
            .await
            .expect("load elements")
            .len(),
        1,
        "the other commission's element is untouched"
    );
}

// Floor — a malformed body (no tab, no surface, no type) is a 422.
#[tokio::test]
async fn a_malformed_body_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let tab = tab_of(&backend, id).await;

    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tabs": "not-a-tab" }))
        .send()
        .await
        .expect("POST malformed");
    common::assert_problem(res, 422, "invalid_request").await;

    // A blank surface label is a malformed request too — the label rules are
    // checked at the boundary, before the skeleton vocabulary is consulted.
    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": tab, "surface": "   ", "type": "note" }))
        .send()
        .await
        .expect("POST blank surface");
    common::assert_problem(res, 422, "invalid_request").await;

    // As is a blank type tag.
    let res = client
        .post(format!("{base}/commissions/{id}/elements"))
        .json(&json!({ "tab": tab, "surface": only_surface(), "type": "" }))
        .send()
        .await
        .expect("POST blank type");
    common::assert_problem(res, 422, "invalid_request").await;
}
