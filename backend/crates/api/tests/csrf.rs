//! CSRF on the cookie surface. A state-changing request passes only with a
//! first-party signal: an `Origin` equal to the configured origin, or no
//! `Origin` and `Sec-Fetch-Site: same-origin`. Everything else — no signal at
//! all, a foreign, `null` or empty `Origin`, a loopback neighbour's `Origin`,
//! an `Origin` equal to a loopback `Host`, any other fetch-metadata value —
//! is refused with `403 cross_origin` before the handler runs, in every
//! environment. GET, HEAD and OPTIONS pass; `/health` and `/.well-known` sit
//! outside the guard, and a path no route matches is a bare `404` whatever
//! its signal. Layers on top of the session cookie's `SameSite=Lax`.
//! The no-signal case sweeps every write the API serves: the paths come from
//! each `.route(` in the crate's source, their methods from axum's own `Allow`
//! on a `405`, and only GET, HEAD and OPTIONS are left out.
//! Same in-process fakes as the other api e2e tests, no database; the clients
//! here are bare (no default signal), so each request carries exactly the
//! signal it names.
use std::path::{Path, PathBuf};

use domain::elements::{did::Did, profile::Profile};
use reqwest::{
    Method, RequestBuilder, StatusCode,
    header::{ALLOW, CACHE_CONTROL, HOST, ORIGIN},
};
use serde_json::json;
use test_support::http::{Served, bare_client, client_with_origin, serve, sign_in};

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

/// The methods the guard lets through without a signal.
const SAFE_METHODS: [Method; 3] = [Method::GET, Method::HEAD, Method::OPTIONS];

/// Calls that mount routes without a `.route(` path the scan can read.
const UNREADABLE_CALLS: [&str; 3] = [".nest(", ".nest_service(", ".route_service("];

/// A route's path as its `.route(` call writes it, e.g. `/commissions/{id}/maturity`.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display)]
struct RouteTemplate(String);

impl RouteTemplate {
    /// The template with each `{name}` and `{*name}` segment filled with `probe`.
    fn probe_path(&self) -> String {
        let segments = self.0.split('/').map(|segment| {
            let captured = segment.starts_with('{') && segment.ends_with('}');
            if captured { "probe" } else { segment }
        });
        segments.collect::<Vec<_>>().join("/")
    }
}

/// A route the scan read, and the `file:line` of its `.route(` call.
struct ScannedRoute {
    template: RouteTemplate,
    written_at: String,
}

/// What the source scan read: every route path, and every call it can't read.
#[derive(Default)]
struct RouteScan {
    routes: Vec<ScannedRoute>,
    unreadable: Vec<String>,
}

impl RouteScan {
    /// Reads each `.route(` call in `text`, the source of `file`, and flags each
    /// call whose routes it can't read.
    fn read(&mut self, file: &str, text: &str) {
        let code = code_lines(text);
        for (offset, call) in code.match_indices(".route(") {
            let written_at = format!("{file}:{}", line_of(&code, offset));
            let argument = code[offset + call.len()..].trim_start();
            let Some(template) = string_literal(argument) else {
                let unreadable = format!(
                    "{written_at}: a `.route(` whose first argument isn't a string literal"
                );
                self.unreadable.push(unreadable);
                continue;
            };
            let route = ScannedRoute {
                template,
                written_at,
            };
            self.routes.push(route);
        }
        for call in UNREADABLE_CALLS {
            for (offset, _) in code.match_indices(call) {
                let unreadable = format!(
                    "{file}:{}: `{call}` mounts routes the scan can't read",
                    line_of(&code, offset)
                );
                self.unreadable.push(unreadable);
            }
        }
    }
}

/// `text` with each `//` comment line (`///` and `//!` docs included) blanked,
/// so line numbers still match the source.
fn code_lines(text: &str) -> String {
    let lines = text.lines().map(|line| {
        let comment = line.trim_start().starts_with("//");
        if comment { "" } else { line }
    });
    lines.collect::<Vec<_>>().join("\n")
}

/// The 1-based line of the byte at `offset` in `code`.
fn line_of(code: &str, offset: usize) -> usize {
    code[..offset].matches('\n').count() + 1
}

/// The plain string literal that opens `text`, without escapes.
fn string_literal(text: &str) -> Option<RouteTemplate> {
    let (literal, _) = text.strip_prefix('"')?.split_once('"')?;
    let plain = !literal.contains('\\');
    plain.then(|| RouteTemplate(literal.to_owned()))
}

/// The api crate's source folder.
fn source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Pushes every `.rs` file under `dir` onto `out`, recursively, but unit-test
/// files (`tests.rs`, `proptests.rs`).
fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("a dir entry").path();
        let unit_tests = path
            .file_name()
            .is_some_and(|name| name == "tests.rs" || name == "proptests.rs");
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") && !unit_tests {
            out.push(path);
        }
    }
}

/// Scans every source file of the api crate for its routes.
fn scan_source() -> RouteScan {
    let source = source_dir();
    let mut paths = Vec::new();
    source_files(&source, &mut paths);
    paths.sort();
    let mut scan = RouteScan::default();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", path.display()));
        let file = path
            .strip_prefix(&source)
            .expect("a scanned file sits under src")
            .display()
            .to_string();
        scan.read(&file, &text);
    }
    scan
}

/// The methods axum serves at `url`, read off the `Allow` of the `405` a
/// first-party `PROPFIND` gets; otherwise, how the `PROPFIND` was answered.
async fn allowed_methods(first_party: &reqwest::Client, url: &str) -> Result<Vec<Method>, String> {
    let propfind = Method::from_bytes(b"PROPFIND").expect("a method");
    let response = first_party
        .request(propfind, url)
        .send()
        .await
        .expect("the PROPFIND is answered");
    let status = response.status();
    if status != StatusCode::METHOD_NOT_ALLOWED {
        return Err(format!("answered {}", status.as_u16()));
    }
    let allow = response
        .headers()
        .get(ALLOW)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let methods: Vec<Method> = allow
        .split(',')
        .map(str::trim)
        .filter(|method| !method.is_empty())
        .map(|method| Method::from_bytes(method.as_bytes()).expect("an Allow method"))
        .collect();
    if methods.is_empty() {
        return Err("answered 405 with no Allow".to_owned());
    }
    Ok(methods)
}

/// How `request` was answered, unless it was `403 cross_origin`.
async fn unless_refused(request: RequestBuilder) -> Option<String> {
    let response = request.send().await.expect("the write is answered");
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    let code = body["code"]
        .as_str()
        .unwrap_or("with no problem code")
        .to_owned();
    let refused = status == StatusCode::FORBIDDEN && code == "cross_origin";
    (!refused).then(|| format!("answered {} {code}", status.as_u16()))
}

/// What the sweep found wrong, each list naming every case.
#[derive(Default)]
struct SweepReport {
    /// Calls the scan can't read.
    unreadable: Vec<String>,
    /// Scanned paths axum doesn't serve as a route.
    unserved: Vec<String>,
    /// Writes answered without a first-party signal.
    escaped: Vec<String>,
}

impl SweepReport {
    /// Whether the sweep found nothing wrong.
    fn is_clean(&self) -> bool {
        self.unreadable.is_empty() && self.unserved.is_empty() && self.escaped.is_empty()
    }
}

impl std::fmt::Display for SweepReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sections = [
            (
                "the route scan can't read these calls; teach it the new shape",
                &self.unreadable,
            ),
            (
                "scanned paths axum doesn't serve (a first-party PROPFIND should answer 405 with an Allow)",
                &self.unserved,
            ),
            (
                "writes not refused with 403 cross_origin when sent with no Origin and no Sec-Fetch-Site",
                &self.escaped,
            ),
        ];
        for (heading, lines) in sections.into_iter().filter(|(_, lines)| !lines.is_empty()) {
            writeln!(f, "{heading}:")?;
            for line in lines {
                writeln!(f, "  {line}")?;
            }
        }
        Ok(())
    }
}

#[tokio::test]
async fn a_write_with_no_origin_and_no_fetch_metadata_is_refused() {
    let served = served().await;
    let base = served.base_url.as_str();
    let first_party = client_with_origin(base);
    let unsignalled = bare_client();
    let scan = scan_source();
    assert!(
        !scan.routes.is_empty(),
        "the route scan found no `.route(` under src/; teach it where routes live now"
    );

    let mut report = SweepReport {
        unreadable: scan.unreadable,
        ..SweepReport::default()
    };
    let mut swept = 0_usize;
    for route in &scan.routes {
        let url = format!("{base}{}", route.template.probe_path());
        let methods = allowed_methods(&first_party, &url)
            .await
            .unwrap_or_else(|answer| {
                let unserved = format!(
                    "{} from {} ({answer} to a first-party PROPFIND)",
                    route.template, route.written_at
                );
                report.unserved.push(unserved);
                Vec::new()
            });
        let writes = methods
            .into_iter()
            .filter(|method| !SAFE_METHODS.contains(method));
        for method in writes {
            swept += 1;
            let request = unsignalled.request(method.clone(), &url);
            if let Some(answer) = unless_refused(request).await {
                let escaped = format!("{method} {} ({answer})", route.template);
                report.escaped.push(escaped);
            }
        }
    }

    assert!(report.is_clean(), "{report}");
    assert!(swept > 0, "the sweep found no write route to check");
}

#[test]
fn the_route_scan_reads_a_multi_line_route_and_a_catch_all() {
    let source = concat!(
        "Router::new()\n",
        "    // .route(\"/commented\", get(gone))\n",
        "    .route(\"/den/{*path}\", get(read_node))\n",
        "    .route(\n",
        "        \"/commissions/{id}/files/{file_id}\",\n",
        "        get(download),\n",
        "    )",
    );
    let mut scan = RouteScan::default();

    scan.read("fixture.rs", source);

    let templates: Vec<String> = scan.routes.iter().map(|r| r.template.to_string()).collect();
    let probe_paths: Vec<String> = scan
        .routes
        .iter()
        .map(|r| r.template.probe_path())
        .collect();
    let written_at: Vec<&str> = scan.routes.iter().map(|r| r.written_at.as_str()).collect();
    assert_eq!(
        templates,
        ["/den/{*path}", "/commissions/{id}/files/{file_id}"]
    );
    assert_eq!(
        probe_paths,
        ["/den/probe", "/commissions/probe/files/probe"]
    );
    assert_eq!(written_at, ["fixture.rs:3", "fixture.rs:4"]);
    assert!(scan.unreadable.is_empty(), "{:?}", scan.unreadable);
}

#[test]
fn the_route_scan_flags_every_call_it_cannot_read() {
    let source = concat!(
        "Router::new()\n",
        "    .route(PATH, get(x))\n",
        "    .nest(\"/a\", inner)\n",
        "    .nest_service(\"/b\", service)\n",
        "    .route_service(\"/c\", service)\n",
        "    .route(r\"/d\", get(x))",
    );
    let mut scan = RouteScan::default();

    scan.read("fixture.rs", source);

    let flagged = [
        "fixture.rs:2: a `.route(` whose first argument isn't a string literal",
        "fixture.rs:6: a `.route(` whose first argument isn't a string literal",
        "fixture.rs:3: `.nest(` mounts routes the scan can't read",
        "fixture.rs:4: `.nest_service(` mounts routes the scan can't read",
        "fixture.rs:5: `.route_service(` mounts routes the scan can't read",
    ];
    assert_eq!(scan.unreadable, flagged);
    assert!(scan.routes.is_empty(), "the scan read no route path");
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
