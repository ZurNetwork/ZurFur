//! The request log: every request runs in a `debug` span `request{id, method, route}` and logs
//! its start and its finish (status, latency) at `debug`. The route is the matched template,
//! never the URI, which can carry OAuth codes and page tokens; headers are never read.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    http::Response,
};
use tower_http::{
    classify::{ServerErrorsAsFailures, SharedClassifier},
    trace::TraceLayer,
};
use tracing::Span;

/// The route a request that matched none is logged under.
pub const UNMATCHED_ROUTE: &str = "(unmatched)";

/// The counter behind each request's id; ids restart with the process, whose pid they carry.
static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

/// The tower layer that logs each request; [`crate::stack`] puts it outside the detach layer,
/// so the detached handling runs inside the request's span.
pub type RequestLog = TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    fn(&Request) -> Span,
    fn(&Request, &Span),
    fn(&Response<Body>, Duration, &Span),
    (),
    (),
    (),
>;

/// The request log layer.
pub fn request_log() -> RequestLog {
    TraceLayer::new_for_http()
        .make_span_with(request_span as fn(&Request) -> Span)
        .on_request(request_started as fn(&Request, &Span))
        .on_response(request_finished as fn(&Response<Body>, Duration, &Span))
        .on_body_chunk(())
        .on_eos(())
        .on_failure(())
}

/// A request's id: this process's pid and its count of requests, unique within one log file
/// across restarts.
fn next_request_id() -> String {
    let count = NEXT_REQUEST.fetch_add(1, Ordering::Relaxed);
    format!("{}-{count}", std::process::id())
}

/// The span a request runs in: its id, its method and its route template.
fn request_span(request: &Request) -> Span {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or(UNMATCHED_ROUTE, MatchedPath::as_str);
    tracing::debug_span!(
        "request",
        id = %next_request_id(),
        method = %request.method(),
        route = route,
    )
}

/// Logs that a request started, inside its span.
fn request_started(_request: &Request, _span: &Span) {
    tracing::debug!("request started");
}

/// Logs that a request finished, with its status and how long it took to answer.
fn request_finished(response: &Response<Body>, latency: Duration, _span: &Span) {
    tracing::debug!(
        status = response.status().as_u16(),
        latency_ms = latency.as_millis(),
        "request finished"
    );
}
