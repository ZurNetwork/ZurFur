use std::convert::Infallible;
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::{
    Json,
    extract::Request,
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;
use tokio_util::task::TaskTracker;
use tower_layer::Layer;
use tower_service::Service;
use tracing::{Instrument as _, Span};

/// How many requests are handled at once; the next one waits for a permit.
pub const REQUEST_PERMITS: usize = 256;

/// Runs each request's handling in its own tracked task, so dropping the
/// request's future (a client that disconnects) leaves the handling running to
/// its end. A permit taken before the spawn bounds how many run at once.
#[derive(Clone, Debug)]
pub struct Detach {
    permits: Arc<Semaphore>,
    tasks: TaskTracker,
}

impl Detach {
    /// A detach layer letting `permits` requests run at once. Inherent: a
    /// bound to build with, not a conversion.
    pub fn new(permits: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(permits)),
            tasks: TaskTracker::new(),
        }
    }

    /// How many detached requests are still running.
    pub fn in_flight(&self) -> usize {
        self.tasks.len()
    }

    /// Waits until every detached request has finished, or `deadline` passes.
    pub async fn drain(&self, deadline: Instant) -> Result<(), Unfinished> {
        self.tasks.close();
        tokio::time::timeout_at(deadline, self.tasks.wait())
            .await
            .map_err(|_| Unfinished {
                in_flight: self.in_flight(),
            })
    }
}

impl Default for Detach {
    /// A detach layer with [`REQUEST_PERMITS`] permits.
    fn default() -> Self {
        Self::new(REQUEST_PERMITS)
    }
}

/// Requests still running when the drain's deadline passed.
#[derive(Debug, thiserror::Error)]
#[error("{in_flight} requests were still running at the deadline")]
pub struct Unfinished {
    /// How many.
    pub in_flight: usize,
}

impl<S> Layer<S> for Detach {
    type Service = Detached<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Detached {
            inner,
            detach: self.clone(),
        }
    }
}

/// The service [`Detach`] wraps around `S`.
#[derive(Clone, Debug)]
pub struct Detached<S> {
    inner: S,
    detach: Detach,
}

impl<S> Service<Request> for Detached<S>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    /// Always ready: the inner service is readied inside the task.
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let inner = self.inner.clone();
        let Detach { permits, tasks } = self.detach.clone();
        let span = Span::current();
        Box::pin(async move {
            let Ok(permit) = permits.acquire_owned().await else {
                return Ok(panic_answer());
            };
            let handling = handle(inner, request, permit).instrument(span);
            let handled = tasks.spawn(handling).await;
            let response = handled.unwrap_or_else(|join_error| {
                tracing::error!(%join_error, "a request's handling panicked or was cancelled");
                panic_answer()
            });
            Ok(response)
        })
    }
}

/// Runs `request` through `inner` to its end, holding `permit` until then.
async fn handle<S>(mut inner: S, request: Request, permit: OwnedSemaphorePermit) -> Response
where
    S: Service<Request, Response = Response, Error = Infallible>,
{
    let Ok(()) = poll_fn(|cx| inner.poll_ready(cx)).await;
    let Ok(response) = inner.call(request).await;
    drop(permit);
    response
}

/// The `500` problem document a request whose handling panicked answers: the
/// API's internal error, unchanged.
pub fn panic_answer() -> Response {
    let problem = serde_json::json!({
        "type": "urn:zurfur:error:internal",
        "code": "internal_error",
        "title": "Internal error",
        "detail": "The request couldn't be completed. Please try again.",
        "status": 500,
    });
    let mut response = (StatusCode::INTERNAL_SERVER_ERROR, Json(problem)).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    response
}

#[cfg(test)]
mod tests;
