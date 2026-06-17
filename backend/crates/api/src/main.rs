use api::{AppState, Config, Environment};
use fluent_uri::Uri;
use tower_sessions::{
    Expiry, SessionManagerLayer,
    cookie::{SameSite, time},
};
use tracing_subscriber::EnvFilter;

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
        pool,
        oauth: adapter_atproto::build_oauth(redirect_uri),
    };
    let app = api::app(app_state).layer(session_layer);

    axum::serve(listener, app).await?;
    Ok(())
}
