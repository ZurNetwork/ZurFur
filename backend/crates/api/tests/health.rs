use std::sync::Arc;

use api::{AppState, Config, Environment};
use domain::elements::did::Did;
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
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url,
            log_level: "info".to_string(),
        },
        pool,
        // /health touches neither the PDS nor the repo; the mem adapters keep both
        // out of the test.
        auth: Arc::new(adapter_mem::MemAuthenticator::new(Did::new(
            "did:plc:test".to_string(),
        ))),
        user_repo: Arc::new(adapter_mem::MemUserRepo::new()),
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
