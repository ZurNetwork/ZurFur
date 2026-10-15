//! A client that goes away mid-request, and the witness that the server
//! dropped that request's future: what a test of the detach layer cuts with.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::AsyncWriteExt as _;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tower_layer::Layer;
use tower_service::Service;

/// How long a cut test waits for an event that must come.
pub const CUT_WAIT: Duration = Duration::from_secs(20);

/// A layer that signals when a request's future it wraps is dropped before it
/// finished. Mounted outside the detach layer, it proves the server cut the request.
#[derive(Clone, Debug, Default)]
pub struct DropWitness {
    dropped: Arc<Notify>,
}

impl DropWitness {
    /// Waits, at most [`CUT_WAIT`], until a wrapped request's future was dropped unfinished.
    pub async fn dropped(&self) {
        tokio::time::timeout(CUT_WAIT, self.dropped.notified())
            .await
            .expect("the server drops the cut request's future");
    }
}

impl<S> Layer<S> for DropWitness {
    type Service = Witnessed<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Witnessed {
            inner,
            dropped: Arc::clone(&self.dropped),
        }
    }
}

/// The service [`DropWitness`] wraps around `S`.
#[derive(Clone, Debug)]
pub struct Witnessed<S> {
    inner: S,
    dropped: Arc<Notify>,
}

impl<S, R> Service<R> for Witnessed<S>
where
    S: Service<R>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = WitnessedFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: R) -> Self::Future {
        WitnessedFuture {
            inner: Box::pin(self.inner.call(request)),
            dropped: Some(Arc::clone(&self.dropped)),
        }
    }
}

/// A request's future that signals when dropped before it finished.
pub struct WitnessedFuture<F> {
    inner: Pin<Box<F>>,
    dropped: Option<Arc<Notify>>,
}

impl<F: Future> Future for WitnessedFuture<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let polled = self.inner.as_mut().poll(cx);
        if polled.is_ready() {
            self.dropped = None;
        }
        polled
    }
}

impl<F> Drop for WitnessedFuture<F> {
    fn drop(&mut self) {
        if let Some(dropped) = self.dropped.take() {
            dropped.notify_one();
        }
    }
}

/// Opens a connection to `base_url` (`http://host:port`) and sends `request`,
/// a whole HTTP/1.1 request. Dropping the stream cuts the request.
pub async fn send_raw(base_url: &str, request: &str) -> TcpStream {
    let address = base_url
        .strip_prefix("http://")
        .expect("a plain-HTTP base URL");
    let mut stream = TcpStream::connect(address).await.expect("connects");
    stream.write_all(request.as_bytes()).await.expect("sends");
    stream
}
