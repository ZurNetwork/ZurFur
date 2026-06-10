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
    tokio::spawn(async move {
        axum::serve(listener, api::app(pool)).await.unwrap();
    });

    let response = reqwest::get(format!("http://{addr}/health"))
        .await
        .expect("GET /health");

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.expect("JSON body");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "up");
}
