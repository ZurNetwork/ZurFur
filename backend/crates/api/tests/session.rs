//! Exercises the session-gated greeting route against a real server with the
//! session layer installed. The signed-in path needs a live PDS (covered by
//! manual end-to-end verification); what we can assert automatically is that an
//! anonymous visitor to `/me` is bounced back to the sign-in page rather than
//! shown a session that doesn't exist. Requires a container runtime socket.
use std::sync::Arc;

use api::{AppState, Config, Environment};
use domain::elements::did::Did;
use reqwest::redirect::Policy;
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use tower_sessions::{MemoryStore, SessionManagerLayer};

#[tokio::test]
async fn me_redirects_anonymous_visitor_to_sign_in() {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url,
            log_level: "info".to_string(),
        },
        pool,
        // An anonymous /me reaches neither PDS nor repo; the mem adapters suffice.
        auth: Arc::new(adapter_mem::MemAuthenticator::new(Did::new(
            "did:plc:test".to_string(),
        ))),
        user_repo: Arc::new(adapter_mem::MemUserRepo::new()),
    };
    // The store backing this test is irrelevant — PgSessionStore is exercised in
    // adapter-pg's own tests; here we only need the layer present so the `Session`
    // extractor resolves. An in-memory store keeps the test about the route.
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .expect("client builds");
    let response = client
        .get(format!("http://{addr}/me"))
        .send()
        .await
        .expect("GET /me");

    assert_eq!(response.status(), 303, "anonymous /me should redirect");
    assert_eq!(
        response.headers()["location"],
        "/",
        "redirect should target the sign-in page"
    );
}
