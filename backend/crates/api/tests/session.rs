//! Exercises the session-gated greeting route against a real server with the
//! session layer installed. The signed-in path needs a live PDS (covered by
//! manual end-to-end verification); what we can assert automatically is that an
//! anonymous visitor to `/me` is bounced back to the sign-in page rather than
//! shown a session that doesn't exist. Requires a container runtime socket.
use std::sync::Arc;

use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
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
    let backend = adapter_mem::MemBackend::new();
    let state = AppState {
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        database: backend.database(),
        did_minter: Arc::new(adapter_mem::MemDidMinter::new()),
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url,
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".to_string(),
            // ZMVP-49 config (unused by the mem minter in these tests).
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
        },
        pool,
        // An anonymous /me reaches neither PDS nor repo; the mem adapters suffice.
        auth: Arc::new(adapter_mem::MemAuthenticator::new(Did::new(
            "did:plc:test".to_string(),
        ))),
        users: backend.user_store(),
        // An anonymous /me never reaches the profile ports; mem fakes suffice.
        profile_source: Arc::new(adapter_mem::MemProfileSource::new(Profile {
            did: Did::new("did:plc:test".to_string()),
            handle: "test.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
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
