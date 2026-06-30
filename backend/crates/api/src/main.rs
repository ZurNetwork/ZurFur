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
use fluent_uri::Uri;
use tower_sessions::{
    Expiry, SessionManagerLayer,
    cookie::{SameSite, time},
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

    let pool = adapter_pg::connect(&config.database_url).await?;
    tracing::info!("database pool established");
    adapter_pg::migrate(&pool).await?;
    tracing::info!("migrations applied");
    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    tracing::info!(addr = %config.http_addr, env = ?config.env, "starting HTTP server");

    // The redirect URI is fixed at client-construction time (jacquard sends
    // redirect_uris[0] in the PAR request), so it must be registered here from the
    // public origin — not overridden per request.
    let redirect_uri =
        Uri::parse(format!("{}/signin-callback", config.public_url)).map_err(|(e, uri)| {
            anyhow::anyhow!("invalid public_url, cannot build redirect URI ({uri}): {e}")
        })?;
    // Build the session layer before moving `pool` and `config` into AppState.
    let store = adapter_pg::PgSessionStore::new(pool.clone());
    let session_layer = SessionManagerLayer::new(store)
        .with_name("zurfur.sid")
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        // Secure cookies are only sent over HTTPS; dev serves plain HTTP on
        // loopback, so setting Secure there would make the browser drop the cookie.
        .with_secure(matches!(config.env, Environment::PROD | Environment::STG))
        .with_expiry(Expiry::OnInactivity(time::Duration::days(7)));

    let app_state = AppState {
        config,
        auth: std::sync::Arc::new(adapter_atproto::AtprotoAuthenticator::new(
            redirect_uri,
            pool.clone(),
        )),
        // Reads go through the pool-backed stores; every private-store write goes
        // through the one `database` factory (also pool-backed) — both built from
        // the same `pool` (DD `24150017`, compile-enforced Unit of Work).
        users: std::sync::Arc::new(adapter_pg::PgUserStore::new(pool.clone())),
        profile_source: std::sync::Arc::new(adapter_atproto::AtprotoProfileSource::new()),
        // Cache profiles for an hour; a staler entry is refetched from the PDS.
        profile_cache: std::sync::Arc::new(adapter_pg::PgProfileCache::new(
            pool.clone(),
            std::time::Duration::from_secs(60 * 60),
        )),
        // The live DID minter is the ZMVP-14 floor stub; the real minter lands as
        // an adapter swap here ("dress when The Who closes").
        did_minter: std::sync::Arc::new(adapter_atproto::StubDidMinter::new()),
        // Account/membership reads off the pool; their writes (and all other
        // private-store writes) flow through the transaction-bound `database`.
        accounts: std::sync::Arc::new(adapter_pg::PgAccountStore::new(pool.clone())),
        database: std::sync::Arc::new(adapter_pg::PgDatabase::new(pool.clone())),
        pool,
    };
    let app = api::app(app_state).layer(session_layer);

    axum::serve(listener, app).await?;
    Ok(())
}
