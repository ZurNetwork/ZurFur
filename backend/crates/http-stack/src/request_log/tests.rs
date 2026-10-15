use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::{Router, body::Body, extract::Request, http::StatusCode, routing::get};
use tower_service::Service as _;
use tower_sessions::MemoryStore;
use tracing_subscriber::fmt::MakeWriter;

use crate::{Detach, stack};

/// What a test subscriber wrote; clones share one buffer.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        let bytes = self.0.lock().expect("unpoisoned").clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("unpoisoned").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for Captured {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

/// A debug-level subscriber writing plain text to `captured`.
fn subscriber(captured: &Captured) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish()
}

/// The served router: `/things/{id}` answers `204` and logs from inside its handler.
fn router() -> Router {
    let app = Router::new().route(
        "/things/{id}",
        get(|| async {
            tracing::debug!("inside the handler");
            StatusCode::NO_CONTENT
        }),
    );
    let sessions = http_cookies::session_layer(MemoryStore::default(), false);
    stack(app, sessions, Detach::default()).router
}

/// Sends `request` through `router`.
async fn send(router: &mut Router, request: Request) -> axum::response::Response {
    let Ok::<_, Infallible>(response) = router.call(request).await;
    response
}

/// `GET <uri>` with a planted cookie.
fn get_with_cookie(uri: &str) -> Request {
    Request::get(uri)
        .header("cookie", "zurfur.sid=planted-session-cookie-value")
        .header("authorization", "Bearer planted-bearer-token")
        .body(Body::empty())
        .expect("a valid request")
}

const PLANTED_PATH: &str = "/things/did:plc:plantedpathdid?code=planted-code&state=planted-state";

#[tokio::test]
async fn a_request_logs_its_route_template_method_status_and_latency_and_never_its_uri() {
    let captured = Captured::default();
    let _guard = tracing::subscriber::set_default(subscriber(&captured));
    let mut router = router();

    let response = send(&mut router, get_with_cookie(PLANTED_PATH)).await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let text = captured.text();
    let started = text
        .lines()
        .find(|line| line.contains("request started"))
        .unwrap_or_else(|| panic!("no start line: {text}"));
    assert!(started.contains("route=\"/things/{id}\""), "{started}");
    assert!(started.contains("method=GET"), "{started}");
    let finished = text
        .lines()
        .find(|line| line.contains("request finished"))
        .unwrap_or_else(|| panic!("no finish line: {text}"));
    assert!(finished.contains("DEBUG"), "{finished}");
    assert!(finished.contains("status=204"), "{finished}");
    assert!(finished.contains("latency_ms="), "{finished}");
    for planted in [
        "plantedpathdid",
        "planted-code",
        "planted-state",
        "planted-session-cookie-value",
        "planted-bearer-token",
    ] {
        assert!(!text.contains(planted), "the log carries {planted}: {text}");
    }
}

#[tokio::test]
async fn an_unmatched_request_logs_a_fixed_route_and_never_its_path() {
    let captured = Captured::default();
    let _guard = tracing::subscriber::set_default(subscriber(&captured));
    let mut router = router();

    let response = send(
        &mut router,
        get_with_cookie("/elsewhere/alice.example.com?code=x1y2z3"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let text = captured.text();
    let finished = text
        .lines()
        .find(|line| line.contains("request finished"))
        .unwrap_or_else(|| panic!("no finish line: {text}"));
    assert!(finished.contains("route=\"(unmatched)\""), "{finished}");
    assert!(finished.contains("status=404"), "{finished}");
    assert!(!text.contains("alice.example.com"), "{text}");
    assert!(!text.contains("x1y2z3"), "{text}");
}

#[tokio::test]
async fn the_handling_inside_the_detach_layer_runs_in_the_request_span() {
    let captured = Captured::default();
    let _guard = tracing::subscriber::set_default(subscriber(&captured));
    let mut router = router();

    send(&mut router, get_with_cookie("/things/one")).await;

    let text = captured.text();
    let handler_line = text
        .lines()
        .find(|line| line.contains("inside the handler"))
        .unwrap_or_else(|| panic!("no handler line: {text}"));
    assert!(handler_line.contains("request{id="), "{handler_line}");
    assert!(
        handler_line.contains("route=\"/things/{id}\""),
        "{handler_line}"
    );
}

/// The `id` field of every `request finished` line in `text`.
fn finished_ids(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.contains("request finished"))
        .filter_map(|line| line.split("request{id=").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

#[tokio::test]
async fn each_request_gets_its_own_short_id_and_no_response_header_carries_it() {
    let captured = Captured::default();
    let _guard = tracing::subscriber::set_default(subscriber(&captured));
    let mut router = router();

    let first = send(&mut router, get_with_cookie("/things/one")).await;
    send(&mut router, get_with_cookie("/things/two")).await;

    let ids = finished_ids(&captured.text());
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert_ne!(ids[0], ids[1]);
    let process = std::process::id().to_string();
    for id in &ids {
        let (pid, counter) = id.split_once('-').expect("a <pid>-<counter> id");
        assert_eq!(pid, process, "{id}");
        assert!(counter.parse::<u64>().is_ok(), "{id}");
    }
    for value in first.headers().values() {
        let value = value.to_str().unwrap_or_default();
        assert!(
            !value.contains(ids[0].as_str()),
            "a header carries the id: {value}"
        );
    }
}
