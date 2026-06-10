use std::net::SocketAddr;

use axum::{Router, http::StatusCode, routing::get};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
enum Environment {
    DEV,
    STG,
    PROD,
}

#[derive(Deserialize)]
struct Config {
    pub env: Environment,
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    pub log_level: String,
}

fn default_http_addr() -> SocketAddr {
    "127.0.0.1:3621".parse().unwrap()
}

impl Config {
    pub fn load() -> Result<Self, figment::Error> {
        let profile = std::env::var("ZURFUR_ENV").unwrap_or_else(|_| "dev".into());

        Figment::new()
            .merge(Toml::file(format!("config/{profile}.toml")))
            .merge(Env::prefixed("ZURFUR_"))
            .extract()
    }
}

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

    let app = Router::new().route("/health", get(health));
    let listener = tokio::net::TcpListener::bind(config.http_addr).await?;
    tracing::info!(addr = %config.http_addr, env = ?config.env, "starting HTTP server");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> StatusCode {
    StatusCode::OK
}
