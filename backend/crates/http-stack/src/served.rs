use std::future::{Future, IntoFuture as _};
use std::io;
use std::pin::pin;
use std::time::Duration;

use axum::{Router, extract::Request, response::IntoResponse, routing::Route};
use http_cookies::SessionLayer;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::Instant;
use tower_http::timeout::RequestBodyTimeoutLayer;
use tower_layer::Layer;
use tower_service::Service;
use tower_sessions::SessionStore;

use crate::Detach;

/// How long shutdown waits, from its signal, for open connections and then
/// detached requests to finish.
pub const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(40);

/// How long a request body may go without a frame before reading it fails.
pub const BODY_IDLE_TIMEOUT: Duration = Duration::from_secs(20);

/// The app as served: under the session layer, both inside a [`Detach`].
/// Only [`stack`] builds one, and [`serve`] serves nothing else.
#[derive(Clone, Debug)]
pub struct Stacked {
    router: Router,
    detach: Detach,
}

impl Stacked {
    /// Wraps the whole stack in `outer`, outside the detach layer.
    pub fn layer<L>(self, outer: L) -> Self
    where
        L: Layer<Route> + Clone + Send + Sync + 'static,
        L::Service: Service<Request> + Clone + Send + Sync + 'static,
        <L::Service as Service<Request>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request>>::Error: Into<std::convert::Infallible> + 'static,
        <L::Service as Service<Request>>::Future: Send + 'static,
    {
        Self {
            router: self.router.layer(outer),
            detach: self.detach,
        }
    }

    /// The detach layer this stack runs its requests in.
    pub fn detach(&self) -> &Detach {
        &self.detach
    }
}

/// `app` under `sessions`, both inside `detach`: the session layer's save
/// after the handler runs to its end with it. A request body idle for
/// [`BODY_IDLE_TIMEOUT`] fails its read, so a stalled body frees its permit.
pub fn stack<S>(app: Router, sessions: SessionLayer<S>, detach: Detach) -> Stacked
where
    S: SessionStore + Clone,
{
    let router = app
        .layer(sessions)
        .layer(RequestBodyTimeoutLayer::new(BODY_IDLE_TIMEOUT))
        .layer(detach.clone());
    Stacked { router, detach }
}

/// Serves `stacked` on `listener` until `shutdown` resolves, then waits for
/// open connections and every detached request to finish, until
/// [`SHUTDOWN_DEADLINE`] after the signal. What is still running then is
/// logged and left.
pub async fn serve(
    listener: TcpListener,
    stacked: Stacked,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let Stacked { router, detach } = stacked;
    let (deadline_sender, mut deadline_set) = oneshot::channel();
    let signal = async move {
        shutdown.await;
        let _ = deadline_sender.send(Instant::now() + SHUTDOWN_DEADLINE);
    };

    let mut server = pin!(
        axum::serve(listener, router)
            .with_graceful_shutdown(signal)
            .into_future()
    );
    let signalled = tokio::select! {
        served = &mut server => {
            served?;
            None
        }
        Ok(deadline) = &mut deadline_set => Some(deadline),
    };
    let deadline = match signalled {
        Some(deadline) => {
            let closed = tokio::time::timeout_at(deadline, &mut server).await;
            match closed {
                Ok(served) => served?,
                Err(_) => tracing::warn!("connections were still open at the shutdown deadline"),
            }
            deadline
        }
        // The server only stops after the signal, so its deadline is waiting.
        None => deadline_set
            .await
            .unwrap_or_else(|_| Instant::now() + SHUTDOWN_DEADLINE),
    };

    if let Err(unfinished) = detach.drain(deadline).await {
        tracing::warn!(
            in_flight = unfinished.in_flight,
            "requests were still running at the shutdown deadline"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
