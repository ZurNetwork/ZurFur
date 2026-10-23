//! The dev profile: the server on `adapter-mem`'s stores seeded with the
//! fixture world, its sessions in memory, every sign-in alice, and the Den's
//! MAC key derived from the custody root. Compiled only with the `dev-profile`
//! feature; it refuses every environment but `DEV`.

use std::ffi::OsStr;
use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, seed_commission, seed_identity, supreme_arts};
use adapter_mem::{FixedSignIn, MemBackend, MemDidMinter, MemIdentityResolver, MemProfileSource};
use composition::{Custody, RootSecret};
use domain::elements::custody::Purpose;
use domain::ports::ScopedMac;
use http_stack::{Detach, Stacked};
use tower_sessions::MemoryStore;

use crate::{AppState, Config, Environment};

/// The pool's URL: `.invalid` never resolves, so nothing here reaches a database.
const UNREACHABLE_DATABASE_URL: &str = "postgres://dev-profile.invalid/none";

/// Why the dev profile would not start: it signs everyone in as a fixture
/// user, so it starts only with `ZURFUR_ENV` set to dev.
#[derive(Debug, thiserror::Error)]
pub enum OutsideDev {
    /// `ZURFUR_ENV` is not set at all.
    #[error(
        "refusing to start the dev profile with ZURFUR_ENV unset: it signs everyone in as a \
         fixture user, so it starts only with ZURFUR_ENV=dev"
    )]
    Unset,
    /// `ZURFUR_ENV` names another environment.
    #[error(
        "refusing to start the dev profile with ZURFUR_ENV={0:?}: it signs everyone in as a \
         fixture user, so it starts only with ZURFUR_ENV=dev"
    )]
    Named(String),
    /// The resolved configuration is not `DEV`.
    #[error(
        "refusing to start the dev profile in {0:?}: it signs everyone in as a fixture user, \
         so it starts only with ZURFUR_ENV=dev"
    )]
    Resolved(Environment),
}

/// `Ok` only when `zurfur_env`, the raw `ZURFUR_ENV`, is set and names dev
/// (`dev` or `DEV`, the spellings the config accepts).
pub fn ensure_dev_variable(zurfur_env: Option<&OsStr>) -> Result<(), OutsideDev> {
    let zurfur_env = zurfur_env.ok_or(OutsideDev::Unset)?;
    if zurfur_env == "dev" || zurfur_env == "DEV" {
        Ok(())
    } else {
        Err(OutsideDev::Named(zurfur_env.to_string_lossy().into_owned()))
    }
}

/// The dev profile's app as served, its in-memory session layer included, its Den MAC
/// key derived from `root`. Refuses with [`OutsideDev`] before anything else unless
/// `config.env` is `DEV`.
pub async fn app(config: Config, root: RootSecret) -> anyhow::Result<Stacked> {
    ensure_dev(&config.env)?;
    let den_mac = den_mac(root, &config.env, config.plc_directory_submit)?;
    tracing::warn!(
        "dev profile: the in-memory fixture world, where every sign-in is a fixture user"
    );

    let secure_cookies = config.env.secure_cookies();
    let runtime = runtime(config, den_mac).await?;
    let sessions = http_cookies::session_layer(MemoryStore::default(), secure_cookies);
    let app = http_stack::stack(crate::app(runtime), sessions, Detach::default());
    Ok(app)
}

/// `Ok` only in `DEV`.
fn ensure_dev(env: &Environment) -> Result<(), OutsideDev> {
    if matches!(env, Environment::DEV) {
        Ok(())
    } else {
        Err(OutsideDev::Resolved(env.clone()))
    }
}

/// The Den's MAC handle from `root`, behind the custody guard: the one purpose the dev
/// profile holds.
fn den_mac(
    root: RootSecret,
    env: &Environment,
    plc_directory_submit: bool,
) -> anyhow::Result<ScopedMac> {
    let custody = Custody::derive(root, env, plc_directory_submit, &[Purpose::DenMac])?;
    Ok(custody.den_mac().clone())
}

/// Every port on `adapter-mem` over one backend seeded with identity's and
/// commission's fixtures; sign-in always answers alice.
async fn runtime(config: Config, den_mac: ScopedMac) -> anyhow::Result<AppState> {
    let backend = MemBackend::new();
    seed_identity(&backend).await?;
    seed_commission(&backend, &alice(), &bob(), &supreme_arts()).await?;

    let public_url: &url::Url = config.public_url.as_ref();
    let callback = public_url.join("/signin-callback")?;
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
        profile_lookups: Default::default(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        workflows: backend.workflow_store(),
        columns: backend.column_store(),
        characters: backend.character_store(),
        bug_reports: backend.bug_report_store(),
        files: backend.file_store(),
        database: backend.database(),
        did_minter: Arc::new(MemDidMinter::new()),
        den_mac,
    };
    Ok(runtime)
}

#[cfg(test)]
mod tests;
