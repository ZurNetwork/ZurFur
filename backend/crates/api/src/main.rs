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
use base64::Engine as _;
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

    // ZMVP-49: the live DID minter is the REAL one. It generates each account's
    // secp256k1 rotation keys, signs an identity-only PLC genesis operation,
    // custodies the keys envelope-encrypted under a DEV-ONLY root key (KMS is the
    // URGENT follow-up ZMVP-53), and submits to a no-op directory (C2 —
    // `plc_directory_submit` defaults off, so nothing hits canonical plc.directory).
    let root_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(config.did_key_root_key.trim())
        .map_err(|e| anyhow::anyhow!("ZURFUR_DID_KEY_ROOT_KEY must be valid base64: {e}"))?;
    // Boot-time custody guard: refuse to run any configuration that would mint real
    // identities under dev-only key custody (config-root-backed in prod/stg, or
    // submitting under the shipped example key). Enforces "harden before real
    // accounts" — cloud KMS is ZMVP-53.
    api::ensure_custody_hardened(&config.env, &root_key_bytes, config.plc_directory_submit)?;
    let root_key = adapter_pg::RootKey::from_bytes(&root_key_bytes)?;
    let key_store = std::sync::Arc::new(adapter_pg::PgKeyStore::new(pool.clone(), root_key));
    let op_log = std::sync::Arc::new(adapter_pg::PgPlcOperationLog::new(pool.clone()));
    let directory = adapter_atproto::plc_directory_from_config(&adapter_atproto::DirectoryConfig {
        endpoint: config.plc_directory_endpoint.clone(),
        enabled: config.plc_directory_submit,
    });
    let did_minter = std::sync::Arc::new(adapter_atproto::RealDidMinter::new(
        key_store, op_log, directory,
    ));

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
        // The live DID minter is the real minter, built above.
        did_minter,
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
