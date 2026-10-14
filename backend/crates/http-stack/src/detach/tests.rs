use std::pin::pin;

use axum::{Router, body::Body, routing::get};
use futures_util::FutureExt as _;
use tokio::sync::Notify;

use super::*;

/// How long a test waits for an event that must come.
const WAIT: std::time::Duration = std::time::Duration::from_secs(60);

/// Waits at most [`WAIT`] for `event`, failing with `what` if it never comes.
async fn within<F: Future>(what: &str, event: F) -> F::Output {
    tokio::time::timeout(WAIT, event)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// A route that signals `entered`, waits for `release`, then signals `finished`.
#[derive(Clone, Default)]
struct Held {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    finished: Arc<Notify>,
}

impl Held {
    fn router(&self) -> Router {
        let held = self.clone();
        let handler = move || async move {
            held.entered.notify_one();
            held.release.notified().await;
            held.finished.notify_one();
            StatusCode::NO_CONTENT
        };
        Router::new()
            .route("/held", get(handler))
            .route("/quick", get(|| async { StatusCode::NO_CONTENT }))
            .route("/panics", get(panics))
    }
}

/// A handler that panics before it answers.
async fn panics() -> StatusCode {
    panic!("a handler panicked")
}

fn request(path: &str) -> Request {
    Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("a valid request")
}

#[tokio::test]
async fn a_request_whose_future_is_dropped_still_runs_to_its_end() {
    let held = Held::default();
    let detach = Detach::new(1);
    let mut service = detach.layer(held.router());

    let mut first = Box::pin(service.call(request("/held")));
    assert!(
        first.as_mut().now_or_never().is_none(),
        "the handler is held"
    );
    within("the handler", held.entered.notified()).await;
    drop(first);
    held.release.notify_one();

    within("the handler's end", held.finished.notified()).await;
    let far_off = Instant::now() + WAIT;
    detach.drain(far_off).await.expect("the request finished");
}

#[tokio::test]
async fn with_every_permit_held_the_next_request_waits_and_then_completes() {
    let held = Held::default();
    let detach = Detach::new(1);
    let mut service = detach.layer(held.router());

    let first = tokio::spawn(service.call(request("/held")));
    within("the handler", held.entered.notified()).await;

    let mut second = pin!(service.call(request("/quick")));
    assert!(
        (&mut second).now_or_never().is_none(),
        "the second request waits for the permit"
    );
    assert_eq!(
        detach.in_flight(),
        1,
        "no task is spawned before its permit"
    );

    held.release.notify_one();
    let first = within("the first answer", first)
        .await
        .expect("joins")
        .expect("infallible");
    let second = within("the second answer", second)
        .await
        .expect("infallible");
    assert_eq!(first.status(), StatusCode::NO_CONTENT);
    assert_eq!(second.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_panicking_handler_answers_the_internal_error_problem() {
    let detach = Detach::default();
    let mut service = detach.layer(Held::default().router());

    let response = within("the panic answer", service.call(request("/panics")))
        .await
        .expect("infallible");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let content_type = response.headers().get(CONTENT_TYPE).cloned();
    assert_eq!(
        content_type,
        Some(HeaderValue::from_static("application/problem+json"))
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a body");
    let problem: serde_json::Value = serde_json::from_slice(&body).expect("JSON");
    let internal_error = serde_json::json!({
        "type": "urn:zurfur:error:internal",
        "code": "internal_error",
        "title": "Internal error",
        "detail": "The request couldn't be completed. Please try again.",
        "status": 500,
    });
    assert_eq!(problem, internal_error);
}
