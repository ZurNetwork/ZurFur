//! What the served stack logs per request: a `debug` span `request{id, method, route}` with
//! the route template, never the URI or a header, and its start and finish, outside the detach
//! layer. Alone in its binary, one test: tracing caches a callsite's interest, so a sibling
//! test hitting these callsites first could blind the capture.

use std::sync::{Arc, Mutex};

use axum::{Router, http::StatusCode, routing::get};
use http_stack::{Detach, serve, stack};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tower_sessions::MemoryStore;
use tracing_subscriber::fmt::MakeWriter;

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

/// The served stack on a loopback port: `/things/{id}` answers `204` and logs from inside its
/// handler, and `/boom` panics. Returns its address.
async fn served() -> std::net::SocketAddr {
    let app = Router::new()
        .route(
            "/things/{id}",
            get(|| async {
                tracing::debug!("inside the handler");
                StatusCode::NO_CONTENT
            }),
        )
        .route(
            "/boom",
            get(|| async {
                panic!("the handler blew up");
                #[allow(unreachable_code)]
                StatusCode::NO_CONTENT
            }),
        );
    let sessions = http_cookies::session_layer(MemoryStore::default(), false);
    let stacked = stack(app, sessions, Detach::default());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("binds");
    let address = listener.local_addr().expect("an address");
    tokio::spawn(async move {
        serve(listener, stacked, std::future::pending())
            .await
            .expect("serves");
    });
    address
}

/// Sends `GET <target>` with a planted cookie and bearer token; the response's whole text.
async fn get_planted(address: std::net::SocketAddr, target: &str) -> String {
    let mut stream = TcpStream::connect(address).await.expect("connects");
    let request = format!(
        "GET {target} HTTP/1.1\r\nHost: localhost\r\nCookie: zurfur.sid=planted-session-cookie-value\r\nAuthorization: Bearer planted-bearer-token\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.expect("sends");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .await
        .expect("reads the response");
    response
}

/// The lines of `text` that carry every one of `parts`.
fn lines_with<'t>(text: &'t str, parts: &[&str]) -> Vec<&'t str> {
    text.lines()
        .filter(|line| parts.iter().all(|part| line.contains(part)))
        .collect()
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
async fn each_request_logs_its_template_status_and_latency_and_never_its_uri_or_headers() {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let address = served().await;

    let matched = get_planted(
        address,
        "/things/did:plc:plantedpathdid?code=planted-code&state=planted-state",
    )
    .await;
    let unmatched = get_planted(address, "/elsewhere/alice.example.com?code=x1y2z3").await;
    let panicked = get_planted(address, "/boom").await;

    assert!(matched.starts_with("HTTP/1.1 204"), "{matched}");
    assert!(unmatched.starts_with("HTTP/1.1 404"), "{unmatched}");
    assert!(panicked.starts_with("HTTP/1.1 500"), "{panicked}");
    let text = captured.text();
    let template = "route=\"/things/{id}\"";
    let started = lines_with(&text, &["request started", template, "method=GET"]);
    assert_eq!(started.len(), 1, "{text}");
    let finished = lines_with(
        &text,
        &["request finished", template, "status=204", "DEBUG"],
    );
    assert_eq!(finished.len(), 1, "{text}");
    assert!(finished[0].contains("latency_ms="), "{}", finished[0]);
    let handler = lines_with(&text, &["inside the handler", "request{id=", template]);
    assert_eq!(
        handler.len(),
        1,
        "the detached handling runs in the request span: {text}"
    );
    let unmatched_lines = lines_with(
        &text,
        &["request finished", "route=\"(unmatched)\"", "status=404"],
    );
    assert_eq!(unmatched_lines.len(), 1, "{text}");
    let panic_answer = lines_with(
        &text,
        &["request finished", "route=\"/boom\"", "status=500"],
    );
    assert_eq!(
        panic_answer.len(),
        1,
        "a panic's answer is logged outside the detach layer: {text}"
    );
    for planted in [
        "plantedpathdid",
        "planted-code",
        "planted-state",
        "planted-session-cookie-value",
        "planted-bearer-token",
        "alice.example.com",
        "x1y2z3",
    ] {
        assert!(!text.contains(planted), "the log carries {planted}: {text}");
    }
    let ids = finished_ids(&text);
    assert_eq!(ids.len(), 3, "{ids:?}");
    let process = std::process::id().to_string();
    for id in &ids {
        let (pid, counter) = id.split_once('-').expect("a <pid>-<counter> id");
        assert_eq!(pid, process, "{id}");
        assert!(counter.parse::<u64>().is_ok(), "{id}");
        assert!(
            !matched.contains(id.as_str()),
            "a response carries the id {id}"
        );
    }
    assert!(ids[0] != ids[1] && ids[1] != ids[2], "{ids:?}");
}
