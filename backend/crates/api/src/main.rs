//! The Zurfur backend binary: the boot sequence and the live adapter wiring
//! (the only place that names production adapters). [`main`] loads
//! [`Config`], builds the profile's app, and serves. The everyday profile
//! stands up Postgres and runs migrations; the `dev-profile` feature swaps in
//! the in-memory fixture world instead, and nothing else chooses it.

use api::Config;
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt as _};

/// Boots the server: load config, init tracing, build the profile's app, then
/// `axum::serve` forever on the configured address. Fails fast on any setup
/// error.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::load()?;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));
    api::logging::subscriber(filter, std::io::stdout).init();

    let http_addr = config.http_addr;
    let env = config.env.clone();
    let app = profile_app(config).await?;

    let listener = tokio::net::TcpListener::bind(http_addr).await?;
    let bound_addr = listener.local_addr()?;
    tracing::info!(addr = %bound_addr, env = ?env, "starting HTTP server");

    axum::serve(listener, app).await?;
    Ok(())
}

/// The everyday profile: the live Postgres and atproto adapters, migrations
/// applied, sessions in Postgres, and the deadline sweeper and session reaper
/// running.
#[cfg(not(feature = "dev-profile"))]
async fn profile_app(config: Config) -> anyhow::Result<axum::Router> {
    use api::AppState;
    use tower_sessions::session_store::ExpiredDeletion;

    // Migrations are the driver's explicit call, so one never runs by accident.
    let app_state: AppState = composition::Runtime::connect(config).await?;
    adapter_pg::migrate(&app_state.pool).await?;
    tracing::info!("migrations applied");

    let store = adapter_pg::PgSessionStore::new(app_state.pool.clone());
    let secure_cookies = app_state.config.env.secure_cookies();
    let session_layer = http_cookies::session_layer(store, secure_cookies);

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
    Ok(app)
}

/// The dev profile: the in-memory fixture world, which refuses to start
/// outside `ZURFUR_ENV=dev`.
#[cfg(feature = "dev-profile")]
async fn profile_app(config: Config) -> anyhow::Result<axum::Router> {
    api::dev_profile::app(config).await
}
