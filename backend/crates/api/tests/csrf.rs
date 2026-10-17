//! CSRF on the cookie surface. A state-changing request passes only with a
//! first-party signal: an `Origin` equal to the configured origin, or no
//! `Origin` and `Sec-Fetch-Site: same-origin`. Everything else — no signal at
//! all, a foreign, `null` or empty `Origin`, a loopback neighbour's `Origin`,
//! an `Origin` equal to a loopback `Host`, any other fetch-metadata value —
//! is refused with `403 cross_origin` before the handler runs, in every
//! environment. GET, HEAD and OPTIONS pass; `/health` and `/.well-known` sit
//! outside the guard, and a path no route matches is a bare `404` whatever
//! its signal. Layers on top of the session cookie's `SameSite=Lax`.
//! Same in-process fakes as the other api e2e tests, no database; the clients
//! here are bare (no default signal), so each request carries exactly the
//! signal it names.
use domain::elements::{did::Did, profile::Profile};
use reqwest::{
    Method, RequestBuilder,
    header::{CACHE_CONTROL, HOST, ORIGIN},
};
use serde_json::json;
use test_support::http::{Served, bare_client, serve, sign_in};

mod common;

const HANDLE: &str = "t.bsky.social";
const FOREIGN_ORIGIN: &str = "http://evil.example";
const REFUSED_DETAIL: &str = "This state-changing request didn't come from Zurfur's own pages.";

async fn served() -> Served {
    let did = Did::from("did:plc:test".to_string());
    let builder = test_support::runtime::mem(&did).profile(Profile::new(did.clone(), HANDLE));
    serve(builder, api::app).await
}

/// `POST {base}/signin` starting a sign-in for [`HANDLE`]; a pass is a `303`.
fn signin(client: &reqwest::Client, base: &str) -> RequestBuilder {
    client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("handle={HANDLE}"))
}

/// `POST {base}/accounts`; anonymous, a pass is the handler's `401`.
fn found_account(client: &reqwest::Client, base: &str) -> RequestBuilder {
    client
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": "x" }))
}

/// `POST {base}/commissions` with a valid title.
fn create_commission(client: &reqwest::Client, base: &str) -> RequestBuilder {
    client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
}

/// `POST {base}/logout`.
fn logout(client: &reqwest::Client, base: &str) -> RequestBuilder {
    client.post(format!("{base}/logout"))
}

/// The port `base` is served on.
fn served_port(base: &str) -> u16 {
    let (_, port) = base.rsplit_once(':').expect("a base URL with a port");
    port.parse().expect("a port number")
}

/// A loopback port beside `base`'s own, never equal to it, even at 65535.
fn neighbour_port(base: &str) -> u16 {
    let port = served_port(base);
    port.checked_add(1).unwrap_or(port - 1)
}

async fn assert_refused(request: RequestBuilder) {
    let response = request.send().await.expect("the request is answered");
    common::assert_problem(response, 403, "cross_origin").await;
}

#[tokio::test]
async fn a_write_with_no_origin_and_no_fetch_metadata_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    assert_refused(signin(&client, base)).await;
    assert_refused(found_account(&client, base)).await;
    assert_refused(logout(&client, base)).await;
    assert_refused(create_commission(&client, base)).await;
}

#[tokio::test]
async fn the_refusal_says_the_request_did_not_come_from_zurfurs_pages() {
    let served = served().await;
    let response = found_account(&bare_client(), &served.base_url)
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(response.status(), 403);
    let body: serde_json::Value = response.json().await.expect("a problem body");
    assert_eq!(body["code"], "cross_origin");
    assert_eq!(body["detail"], REFUSED_DETAIL);
}

#[tokio::test]
async fn a_refused_logout_leaves_the_session_alive() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    sign_in(&client, base, HANDLE).await;

    assert_refused(logout(&client, base).header(ORIGIN, FOREIGN_ORIGIN)).await;

    let me = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(me.status(), 200, "the refused logout signed no one out");
}

#[tokio::test]
async fn a_refused_create_writes_no_commission() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    sign_in(&client, base, HANDLE).await;
    let before = served.backend.all_commissions().await.expect("listed");

    assert_refused(create_commission(&client, base).header(ORIGIN, FOREIGN_ORIGIN)).await;

    let after = served.backend.all_commissions().await.expect("listed");
    assert_eq!(
        after.len(),
        before.len(),
        "the refused create wrote nothing"
    );
    // A control: the same request from the app's own pages creates one, so the
    // refused body was one the handler would have taken.
    let created = create_commission(&client, base)
        .header(ORIGIN, base)
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(created.status(), 201);
    let created_count = served.backend.all_commissions().await.expect("listed");
    assert_eq!(created_count.len(), before.len() + 1);
}

#[tokio::test]
async fn a_signed_in_write_without_a_signal_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    sign_in(&client, base, HANDLE).await;

    assert_refused(create_commission(&client, base)).await;
    let commissions = served.backend.all_commissions().await.expect("listed");
    assert!(commissions.is_empty(), "the refused create wrote nothing");
}

#[tokio::test]
async fn a_same_origin_fetch_without_origin_passes() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    let account = found_account(&client, base)
        .header("sec-fetch-site", "same-origin")
        .send()
        .await
        .expect("POST /accounts");
    // The handler's 401 shows the request got past the guard.
    common::assert_problem(account, 401, "not_authenticated").await;
    let started = signin(&client, base)
        .header("sec-fetch-site", "same-origin")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(started.status(), 303);
}

#[tokio::test]
async fn same_site_cross_site_and_none_are_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    for site in ["same-site", "cross-site", "none"] {
        assert_refused(signin(&client, base).header("sec-fetch-site", site)).await;
        assert_refused(found_account(&client, base).header("sec-fetch-site", site)).await;
    }
}

#[tokio::test]
async fn a_null_or_empty_origin_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    for origin in ["null", ""] {
        let request = signin(&client, base)
            .header(ORIGIN, origin)
            .header("sec-fetch-site", "same-origin");
        assert_refused(request).await;
    }
}

#[tokio::test]
async fn a_first_party_origin_passes() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    let account = found_account(&client, base)
        .header(ORIGIN, base)
        .send()
        .await
        .expect("POST /accounts");
    // The handler's 401 shows the request got past the guard.
    common::assert_problem(account, 401, "not_authenticated").await;
    let started = signin(&client, base)
        .header(ORIGIN, base)
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(started.status(), 303);
}

#[tokio::test]
async fn a_foreign_origin_with_same_origin_metadata_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    let signin_request = signin(&client, base)
        .header(ORIGIN, FOREIGN_ORIGIN)
        .header("sec-fetch-site", "same-origin");
    assert_refused(signin_request).await;
    let account_request = found_account(&client, base)
        .header(ORIGIN, FOREIGN_ORIGIN)
        .header("sec-fetch-site", "same-origin");
    assert_refused(account_request).await;
}

#[tokio::test]
async fn an_origin_that_agrees_with_its_host_but_not_config_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let port = base.rsplit_once(':').expect("a base URL with a port").1;
    let rebound_host = format!("evil.example:{port}");
    let rebound_origin = format!("http://{rebound_host}");

    let request = signin(&bare_client(), base)
        .header(HOST, rebound_host)
        .header(ORIGIN, rebound_origin);
    assert_refused(request).await;
}

#[tokio::test]
async fn an_unknown_method_needs_a_first_party_signal() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    let propfind = Method::from_bytes(b"PROPFIND").expect("a method");

    assert_refused(client.request(propfind.clone(), format!("{base}/accounts"))).await;
    let first_party = client
        .request(propfind, format!("{base}/accounts"))
        .header(ORIGIN, base)
        .send()
        .await
        .expect("PROPFIND /accounts");
    // Past the guard, the method router has no PROPFIND.
    assert_eq!(first_party.status(), 405);
}

#[tokio::test]
async fn a_safe_method_is_never_blocked_by_origin() {
    let served = served().await;
    let base = served.base_url.as_str();
    // GET is safe; even a foreign Origin passes the guard. Anonymous /me is a
    // 401 (the session check), never a 403 cross_origin.
    let me = bare_client()
        .get(format!("{base}/me"))
        .header(ORIGIN, FOREIGN_ORIGIN)
        .send()
        .await
        .expect("GET /me");
    common::assert_problem(me, 401, "not_authenticated").await;
}

#[tokio::test]
async fn the_guard_does_not_wrap_health_or_well_known() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();

    for path in ["/health", "/.well-known/atproto-did"] {
        let response = client
            .post(format!("{base}{path}"))
            .header(ORIGIN, FOREIGN_ORIGIN)
            .send()
            .await
            .expect("POST outside the cookie surface");
        assert_eq!(response.status(), 405, "{path} answers before any guard");
    }
}

#[tokio::test]
async fn a_loopback_neighbours_origin_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    let neighbour_origin = format!("http://127.0.0.1:{}", neighbour_port(base));

    let signin_request = signin(&client, base).header(ORIGIN, neighbour_origin.as_str());
    assert_refused(signin_request).await;
    let account_request = found_account(&client, base).header(ORIGIN, neighbour_origin.as_str());
    assert_refused(account_request).await;
}

#[tokio::test]
async fn an_origin_equal_to_a_loopback_host_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    let loopback_host = format!("127.0.0.1:{}", neighbour_port(base));
    let loopback_origin = format!("http://{loopback_host}");

    let signin_request = signin(&client, base)
        .header(HOST, loopback_host.as_str())
        .header(ORIGIN, loopback_origin.as_str());
    assert_refused(signin_request).await;
    let account_request = found_account(&client, base)
        .header(HOST, loopback_host.as_str())
        .header(ORIGIN, loopback_origin.as_str());
    assert_refused(account_request).await;
}

#[tokio::test]
async fn a_path_no_route_matches_is_a_bare_404_whatever_its_signal() {
    let served = served().await;
    let base = served.base_url.as_str();
    let client = bare_client();
    let unmatched = format!("{base}/plugin/v1/x");

    let unsignalled = client.post(&unmatched);
    let foreign = client.post(&unmatched).header(ORIGIN, FOREIGN_ORIGIN);
    for (signal, request) in [("no Origin", unsignalled), ("a foreign Origin", foreign)] {
        let response = request.send().await.expect("POST /plugin/v1/x");
        assert_eq!(response.status(), 404, "with {signal}");
        let no_store = response.headers().get(CACHE_CONTROL);
        assert!(
            no_store.is_none(),
            "with {signal}, outside the no-store layer"
        );
        let body = response.text().await.expect("a body");
        assert_eq!(body, "", "with {signal}, an empty body");
    }
}
