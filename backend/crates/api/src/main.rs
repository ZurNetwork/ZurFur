//! The Zurfur backend binary: the boot sequence and the live adapter wiring
//! (the only place that names production adapters). [`main`] loads
//! [`Config`], stands up Postgres, runs migrations, assembles [`AppState`],
//! mounts [`api::app`], and serves.

use api::{AppState, Config, Environment};
use tower_sessions::{
    Expiry, SessionManagerLayer,
    cookie::{SameSite, time},
    session_store::ExpiredDeletion,
};
use tracing_subscriber::EnvFilter;

/// Boots the server: load config, init tracing, connect the pool, run
/// migrations, build the session layer, assemble [`AppState`], then
/// `axum::serve` forever. Fails fast on any setup error.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    // Migrations are the driver's explicit call, so one never runs by accident.
    let app_state: AppState = composition::Runtime::connect(config).await?;
    adapter_pg::migrate(&app_state.pool).await?;
    tracing::info!("migrations applied");
    let http_addr = app_state.config.http_addr;
    let listener = tokio::net::TcpListener::bind(http_addr).await?;
    tracing::info!(addr = %http_addr, env = ?app_state.config.env, "starting HTTP server");

    let store = adapter_pg::PgSessionStore::new(app_state.pool.clone());
    let secure_cookies = matches!(app_state.config.env, Environment::PROD | Environment::STG);
    let session_layer = SessionManagerLayer::new(store)
        .with_name("zurfur.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_secure(secure_cookies)
        .with_expiry(Expiry::OnInactivity(time::Duration::days(7)));

    tokio::spawn(api::run_deadline_sweeper(
        app_state.database.clone(),
        // Needs the pool directly for the advisory lock (single-writer leader election).
        app_state.pool.clone(),
        std::time::Duration::from_secs(app_state.config.deadline_sweep_interval_secs),
    ));

    // Read-time expiry already hides expired rows; this is pure housekeeping
    // (hourly, failed passes logged and retried next tick).
    let session_reaper = adapter_pg::PgSessionStore::new(app_state.pool.clone());
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = session_reaper.delete_expired().await {
                tracing::error!(%error, "session reaper pass failed; retrying next tick");
            }
        }
    });

    let app = api::app(app_state).layer(session_layer);

    axum::serve(listener, app).await?;
    Ok(())
}
