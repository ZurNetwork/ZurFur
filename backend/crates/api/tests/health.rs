use std::sync::Arc;

use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// Boots the app against a throwaway PostgreSQL container and expects a green
/// /health. Requires a container runtime socket (DOCKER_HOST honored).
#[tokio::test]
async fn health_is_green_against_fresh_postgres() {
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
        // /health touches neither the PDS nor the repo; the mem adapters keep both
        // out of the test.
        auth: Arc::new(adapter_mem::MemAuthenticator::new(Did::new(
            "did:plc:test".to_string(),
        ))),
        users: backend.user_store(),
        // /health touches neither; mem fakes keep the profile ports out of the test.
        profile_source: Arc::new(adapter_mem::MemProfileSource::new(Profile {
            did: Did::new("did:plc:test".to_string()),
            handle: "test.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
    };
    let app = api::app(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("GET /health");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("JSON body");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "up");
}
