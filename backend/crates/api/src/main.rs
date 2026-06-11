use api::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    let pool = adapter_pg::connect(&config.database_url).await?;
    tracing::info!("database pool established");
    adapter_pg::migrate(&pool).await?;
    tracing::info!("migrations applied");

    let app = api::app(pool);
    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    tracing::info!(addr = %config.http_addr, env = ?config.env, "starting HTTP server");
    axum::serve(listener, app).await?;
    Ok(())
}
