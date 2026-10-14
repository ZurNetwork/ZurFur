//! The dev profile: the server on `adapter-mem`'s stores seeded with the
//! fixture world, its sessions in memory, and every sign-in alice. Compiled
//! only with the `dev-profile` feature; it refuses every environment but `DEV`.

use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, seed_commission, seed_identity, supreme_arts};
use adapter_mem::{FixedSignIn, MemBackend, MemDidMinter, MemIdentityResolver, MemProfileSource};
use axum::Router;
use tower_sessions::MemoryStore;

use crate::{AppState, Config, Environment};

/// The pool's URL: `.invalid` never resolves, so nothing here reaches a database.
const UNREACHABLE_DATABASE_URL: &str = "postgres://dev-profile.invalid/none";

/// Why the dev profile would not start: the environment is not `DEV`.
#[derive(Debug, thiserror::Error)]
#[error(
    "refusing to start the dev profile in {0:?}: it signs everyone in as a fixture user, \
     so it starts only with ZURFUR_ENV=dev"
)]
pub struct OutsideDev(Environment);

/// The dev profile's app, its in-memory session layer included. Refuses with
/// [`OutsideDev`] before anything else unless `config.env` is `DEV`.
pub async fn app(config: Config) -> anyhow::Result<Router> {
    ensure_dev(&config.env)?;
    tracing::info!(
        "dev profile: the in-memory fixture world, where every sign-in is a fixture user"
    );

    let secure_cookies = config.env.secure_cookies();
    let runtime = runtime(config).await?;
    let sessions = http_cookies::session_layer(MemoryStore::default(), secure_cookies);
    let app = crate::app(runtime).layer(sessions);
    Ok(app)
}

/// `Ok` only in `DEV`.
fn ensure_dev(env: &Environment) -> Result<(), OutsideDev> {
    if matches!(env, Environment::DEV) {
        Ok(())
    } else {
        Err(OutsideDev(env.clone()))
    }
}

/// Every port on `adapter-mem` over one backend seeded with identity's and
/// commission's fixtures; sign-in always answers alice.
async fn runtime(config: Config) -> anyhow::Result<AppState> {
    let backend = MemBackend::new();
    seed_identity(&backend).await?;
    seed_commission(&backend, &alice(), &bob(), &supreme_arts()).await?;

    let callback = config.public_url.join("/signin-callback")?;
    let sign_in = FixedSignIn::new(alice().did().clone(), callback);
    let runtime = AppState {
        config,
        pool: adapter_pg::lazy_pool(UNREACHABLE_DATABASE_URL)?,
        auth: Arc::new(sign_in),
        identity_resolver: Arc::new(MemIdentityResolver::new()),
        users: backend.user_store(),
        // The fixture profiles sit in the cache; a miss shows a bare DID, never another's profile.
        profile_source: Arc::new(MemProfileSource::unreachable()),
        profile_cache: backend.profile_cache(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        workflows: backend.workflow_store(),
        columns: backend.column_store(),
        characters: backend.character_store(),
        files: backend.file_store(),
        database: backend.database(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    Ok(runtime)
}

#[cfg(test)]
mod tests;
