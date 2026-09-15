//! Shared axum test-serving fixture: boots a served app against the
//! in-memory runtime and returns a base URL plus a cookie-keeping client, so
//! route tests never re-implement the boot sequence.

use adapter_mem::MemBackend;
use axum::Router;
use composition::Runtime;
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

use crate::runtime::{MemRuntime, MemRuntimeBuilder};

/// A running test server plus the in-memory backend behind it, so a test can
/// introspect the stores it wired after driving the app.
pub struct Served {
    pub base_url: String,
    pub backend: MemBackend,
}

/// Boots `app` against an in-memory [`Runtime`] built from `builder`,
/// wrapping it in a fresh session layer, and returns the running server's
/// [`Served`] handle.
pub async fn serve(builder: MemRuntimeBuilder, app: impl FnOnce(Runtime) -> Router) -> Served {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let MemRuntime { runtime, backend } = builder.public_url(format!("http://{addr}")).build();

    let router = app(runtime).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    Served {
        base_url: format!("http://{addr}"),
        backend,
    }
}

/// A cookie-keeping client that does not auto-follow redirects, so each hop
/// in a multi-step flow is asserted on its own.
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Drives the two-step sign-in so `client`'s cookie jar carries a live
/// session; returns the callback response so a test can inspect its cookie.
pub async fn sign_in(client: &reqwest::Client, base: &str, handle: &str) -> reqwest::Response {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("handle={handle}"))
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

    res
}
