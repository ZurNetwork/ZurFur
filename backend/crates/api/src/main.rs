//! The Zurfur backend binary: the boot sequence and the live adapter wiring.
//!
//! This is the only place that names the production adapters. [`main`] loads
//! [`Config`], stands up the Postgres pool, runs migrations, builds the session
//! middleware, assembles the [`AppState`] from the pg/atproto adapters, mounts
//! the [`api::app`] router under that layer, and serves. The rest of the crate
//! is adapter-agnostic; swapping an implementation is a change here, nowhere
//! else.
//!
//! References: CLAUDE.md "Architecture"/"Configuration"/"Database".

use api::{AppState, Config, Environment};
use tower_sessions::{
    Expiry, SessionManagerLayer,
    cookie::{SameSite, time},
    session_store::ExpiredDeletion,
};
use tracing_subscriber::EnvFilter;

/// Boots the server, in order: load `.env`, load [`Config`], init tracing
/// (`RUST_LOG` overrides [`Config::log_level`]), connect the pool, run
/// migrations, bind the listener, build the redirect URI and session layer,
/// assemble [`AppState`] from the live adapters, then `axum::serve` forever.
///
/// Fails fast — returns `Err` and exits before serving — if the config won't
/// load, the database is unreachable, a migration fails, the bind fails, or
/// [`Config::public_url`] won't parse into a redirect URI. The redirect URI is
/// fixed at client-construction time (jacquard sends it in the PAR request), so
/// it is registered once here from the public origin, not per request. Cookie
/// `Secure` is on only in [`Environment::STG`]/[`Environment::PROD`]; profiles
/// are cached for one hour.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let config = Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    // The live composition is shared with the CLI (ZMVP-200); migrations are
    // the driver's explicit call, so a driver can never run them by accident.
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
        // The sweeper takes a Postgres advisory lock for single-writer leader election
        // across instances, so it needs the pool directly (not just the port).
        app_state.pool.clone(),
        std::time::Duration::from_secs(app_state.config.deadline_sweep_interval_secs),
    ));

    // Reclaim expired `tower_sessions.session` rows on a schedule. Read-time expiry
    // (`PgSessionStore::load` filters `expiry_date > now()`) already hides them from
    // callers, so this is pure housekeeping — hence a relaxed hourly cadence. Mirrors
    // the deadline sweeper: a failed pass is logged and retried on the next tick, so a
    // transient DB blip never permanently stops the reaper.
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
