use std::net::SocketAddr;

use adapter_pg::PgPool;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::get,
};
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub enum Environment {
    DEV,
    STG,
    PROD,
}

#[derive(Deserialize)]
pub struct Config {
    pub env: Environment,
    #[serde(default = "default_http_addr")]
    pub http_addr: SocketAddr,
    pub database_url: String,
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
            .merge(Env::raw().only(&["DATABASE_URL"]))
            .merge(Env::prefixed("ZURFUR_"))
            .extract()
    }
}

pub fn app(pool: PgPool) -> Router {
    Router::new().route("/health", get(health)).with_state(pool)
}

async fn health(State(pool): State<PgPool>) -> (StatusCode, Json<serde_json::Value>) {
    if adapter_pg::is_reachable(&pool).await {
        (
            StatusCode::OK,
            Json(json!({ "status": "ok", "database": "up" })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "degraded", "database": "down" })),
        )
    }
}
