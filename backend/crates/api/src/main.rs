//! The Zurfur backend binary: the boot sequence and the live adapter wiring
//! (the only place that names production adapters). [`main`] loads
//! [`Config`], builds the profile's app, and serves. The everyday profile
//! stands up Postgres and runs the custody migrator; the `dev-profile` feature
//! swaps in the in-memory fixture world instead, and nothing else chooses it.

use api::Config;
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt as _};

/// Boots the server: load config, init tracing, build the profile's app, then
/// serve it on the configured address until a shutdown signal, finishing the
/// requests in flight. Fails fast on any setup error.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::load()?;

    let rust_log = std::env::var(EnvFilter::DEFAULT_ENV).ok();
    let chosen = api::logging::ChosenFilter::new(rust_log.as_deref(), &config.log_level);
    let filter_text = chosen.filter.to_string();
    api::logging::subscriber(&config.env, chosen.filter, std::io::stdout).init();
    tracing::info!(source = %chosen.source, filter = %filter_text, "log filter chosen");

    let http_addr = config.http_addr;
    let env = config.env.clone();
    let app = profile_app(config).await?;

    let listener = tokio::net::TcpListener::bind(http_addr).await?;
    let bound_addr = listener.local_addr()?;
    tracing::info!(addr = %bound_addr, env = ?env, "starting HTTP server");

    http_stack::serve(listener, app, shutdown_signal()).await?;
    tracing::info!("HTTP server stopped");
    Ok(())
}

/// Resolves on Ctrl-C or, on Unix, `SIGTERM`. A signal that can't be listened
/// for is logged and never fires.
async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "can't listen for Ctrl-C");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                terminate.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "can't listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received; finishing the requests in flight");
}

/// The everyday profile: the live Postgres and atproto adapters over every
/// custody purpose, the custody migrator run, sessions in Postgres, and the
/// deadline sweeper and session reaper running.
#[cfg(not(feature = "dev-profile"))]
async fn profile_app(config: Config) -> anyhow::Result<http_stack::Stacked> {
    use api::AppState;
    use domain::elements::custody::Purpose;
    use tower_sessions::session_store::ExpiredDeletion;

    let purposes = [Purpose::DidCustody, Purpose::SigninSecrets, Purpose::DenMac];
    let root = composition::RootSecret::from_env()?;
    let connecting = AppState::connect_migrating(config, root, &purposes);
    let (app_state, custody_migrator) = connecting.await?;
    // Composition builds the migrator; running it is this driver's explicit call.
    let report = custody_migrator.run().await?;
    tracing::info!(
        applied = report.applied,
        version = ?report.version,
        rows_rewrapped = report.rows_rewrapped,
        "migrations applied"
    );

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

    let app = http_stack::stack(
        api::app(app_state),
        session_layer,
        http_stack::Detach::default(),
    );
    Ok(app)
}

/// The dev profile: the in-memory fixture world. Refuses to start unless
/// `ZURFUR_ENV` is set (by the environment or `.env`) and names dev, whatever
/// the config would default to, then unless the custody root is set.
#[cfg(feature = "dev-profile")]
async fn profile_app(config: Config) -> anyhow::Result<http_stack::Stacked> {
    let zurfur_env = std::env::var_os(composition::PROFILE_ENV);
    api::dev_profile::ensure_dev_variable(zurfur_env.as_deref())?;
    let root = composition::RootSecret::from_env()?;
    api::dev_profile::app(config, root).await
}
