//! Shared axum test-serving fixture: boots a served app against the
//! in-memory runtime and returns a base URL plus a cookie-keeping client, so
//! route tests never re-implement the boot sequence.

use adapter_mem::MemBackend;
use axum::Router;
use composition::Runtime;
use http_stack::{Detach, Stacked};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ORIGIN};
use reqwest::redirect::Policy;
use tower_sessions::MemoryStore;

use crate::runtime::{MemRuntime, MemRuntimeBuilder};

/// A running test server plus the in-memory backend behind it, so a test can
/// introspect the stores it wired after driving the app.
pub struct Served {
    pub base_url: String,
    pub backend: MemBackend,
    /// The detach layer the server runs its requests in.
    pub detach: Detach,
}

/// Boots `app` against an in-memory [`Runtime`] built from `builder`, through
/// the served stack with a fresh session layer issuing the real session
/// cookie, and returns the running server's [`Served`] handle.
pub async fn serve(builder: MemRuntimeBuilder, app: impl FnOnce(Runtime) -> Router) -> Served {
    serve_wrapped(builder, app, |stacked| stacked).await
}

/// As [`serve`], with `wrap` adding layers outside the whole stack.
pub async fn serve_wrapped(
    builder: MemRuntimeBuilder,
    app: impl FnOnce(Runtime) -> Router,
    wrap: impl FnOnce(Stacked) -> Stacked,
) -> Served {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let MemRuntime { runtime, backend } = builder.public_url(format!("http://{addr}")).build();

    // The served app speaks plain HTTP, so its cookies are not `Secure`.
    let session_layer = http_cookies::session_layer(MemoryStore::default(), false);
    let stacked = wrap(http_stack::stack(
        app(runtime),
        session_layer,
        Detach::default(),
    ));
    let detach = stacked.detach().clone();
    tokio::spawn(async move {
        http_stack::serve(listener, stacked, std::future::pending())
            .await
            .unwrap();
    });

    Served {
        base_url: format!("http://{addr}"),
        backend,
        detach,
    }
}

/// A cookie-keeping client that does not auto-follow redirects, so each hop
/// in a multi-step flow is asserted on its own, and that sends
/// `Sec-Fetch-Site: same-origin` on every request, as a browser's fetch from
/// the app's own pages does.
pub fn client() -> reqwest::Client {
    let sec_fetch_site = HeaderName::from_static("sec-fetch-site");
    first_party_client(sec_fetch_site, "same-origin")
}

/// As [`client`], sending `Origin: {origin}` on every request instead.
pub fn client_with_origin(origin: &str) -> reqwest::Client {
    first_party_client(ORIGIN, origin)
}

/// As [`client`], with no first-party signal at all: no `Origin` and no
/// `Sec-Fetch-Site`, unless a request sets one.
pub fn bare_client() -> reqwest::Client {
    browser().build().expect("client builds")
}

/// A browser-like client that sends `name: value` on every request.
fn first_party_client(name: HeaderName, value: &str) -> reqwest::Client {
    let value = HeaderValue::try_from(value).expect("a header value");
    let signal = HeaderMap::from_iter([(name, value)]);
    browser()
        .default_headers(signal)
        .build()
        .expect("client builds")
}

/// A cookie-keeping client builder that does not auto-follow redirects.
fn browser() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
}

/// Drives the two-step sign-in so `client`'s cookie jar carries a live
/// session; returns the callback response so a test can inspect its cookie.
/// The `POST /signin` sends `Origin: {base}`, so any client can sign in.
pub async fn sign_in(client: &reqwest::Client, base: &str, handle: &str) -> reqwest::Response {
    let res = client
        .post(format!("{base}/signin"))
        .header(ORIGIN, base)
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

#[cfg(test)]
mod tests;
