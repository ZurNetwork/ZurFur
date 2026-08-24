//! In-process harness (ZMVP-201/203): drive [`cli::dispatch`] over a
//! [`composition::Runtime`] wired to the in-memory fakes and an identity file
//! in a temp dir — no database, no process spawn. This is where the
//! Engineer's operation commands get their fast tests; the process harness
//! only pins the conventions.

use std::path::PathBuf;
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use cli::{
    BackendCommand, ExitClass, commands::session::SessionOp, identity, principal::Principal,
};
use composition::{Config, Environment, Runtime};
use domain::elements::{did::Did, profile::Profile};
use domain::ports::UnitOfWork;

const DATABASE_URL: &str = "postgres://harness@127.0.0.1:5432/zurfur_harness";
const DID: &str = "did:plc:cli-harness";

/// A runtime over the in-memory fakes, mirroring `api`'s e2e fixtures.
fn mem_runtime() -> Runtime {
    let backend = MemBackend::new();
    let did = Did::new(DID.to_string());
    let profile = Profile {
        did: did.clone(),
        handle: "harness.bsky.social".to_string(),
        display_name: Some("The Harness".to_string()),
        avatar_url: None,
    };
    let config = Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().unwrap(),
        public_url: "http://127.0.0.1:0".to_string(),
        database_url: DATABASE_URL.to_string(),
        log_level: "info".to_string(),
        handle_domain: "zurfur.app".to_string(),
        did_key_root_key: "unused-in-tests".to_string(),
        plc_directory_endpoint: "https://plc.directory".to_string(),
        plc_directory_submit: false,
        deadline_sweep_interval_secs: 60,
        max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
    };
    Runtime {
        config,
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(did)),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(profile)),
        profile_cache: backend.profile_cache(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
        database: backend.database(),
        did_minter: Arc::new(MemDidMinter::new()),
    }
}

/// A fresh identity file location per test.
fn identity_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(identity::IDENTITY_FILE_NAME);
    (dir, path)
}

/// Recognize the harness DID as a User — the write `login` will do.
async fn provision(runtime: &Runtime) {
    let did = Did::new(DID.to_string());
    runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&did).await?;
            Ok(())
        })
        .await
        .unwrap();
}

fn whoami() -> BackendCommand {
    BackendCommand::Session {
        op: SessionOp::Whoami,
    }
}

// The probe's own failure (a pool that exists but never answers) is
// `service_unavailable` like a connect failure — the API's one code for a
// down dependency — with the `detail` telling the two apart.
#[tokio::test]
async fn a_pool_that_never_answers_is_service_unavailable() {
    let mut runtime = mem_runtime();
    // Nobody listens on loopback port 1; the lazy pool defers the failure to
    // the probe, which is exactly the arm under test.
    runtime.pool = adapter_pg::lazy_pool("postgres://nobody@127.0.0.1:1/nothing").unwrap();
    let nowhere = std::path::Path::new("/nonexistent");
    let error = cli::dispatch(&runtime, nowhere, BackendCommand::Health)
        .await
        .unwrap_err();
    assert_eq!(error.class(), ExitClass::Infra);
    assert_eq!(error.code(), "service_unavailable");
    assert!(
        error.problem().detail.contains("health query"),
        "{:?}",
        error.problem()
    );
}

#[tokio::test]
async fn login_is_still_an_honest_not_implemented() {
    let runtime = mem_runtime();
    let (_dir, path) = identity_path();
    let command = BackendCommand::Session {
        op: SessionOp::Login,
    };
    let error = cli::dispatch(&runtime, &path, command).await.unwrap_err();
    assert_eq!(error.class(), ExitClass::Infra);
    assert_eq!(error.code(), "not_implemented");
}

#[tokio::test]
async fn whoami_without_an_identity_is_not_authenticated() {
    let runtime = mem_runtime();
    let (_dir, path) = identity_path();
    let error = cli::dispatch(&runtime, &path, whoami()).await.unwrap_err();
    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "not_authenticated");
}

#[tokio::test]
async fn whoami_projects_the_user_like_get_me() {
    let runtime = mem_runtime();
    provision(&runtime).await;
    let (_dir, path) = identity_path();
    identity::save(&path, &identity::Identity::new(DID, DATABASE_URL)).unwrap();

    let value = cli::dispatch(&runtime, &path, whoami()).await.unwrap();
    assert_eq!(value["did"], DID);
    assert_eq!(value["handle"], "harness.bsky.social");
    assert_eq!(value["displayName"], "The Harness");
    // An absent optional is omitted, never `null` (GET /me parity).
    assert!(value.get("avatarUrl").is_none(), "{value}");
}

#[tokio::test]
async fn an_identity_from_another_database_is_refused() {
    let runtime = mem_runtime();
    provision(&runtime).await;
    let (_dir, path) = identity_path();
    let elsewhere = identity::Identity::new(DID, "postgres://x@other.host:5432/other");
    identity::save(&path, &elsewhere).unwrap();

    let error = Principal::resolve(&runtime, &path).await.unwrap_err();
    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "identity_mismatch");
}

#[tokio::test]
async fn an_identity_unknown_to_the_database_is_not_authenticated() {
    let runtime = mem_runtime(); // nobody provisioned
    let (_dir, path) = identity_path();
    identity::save(&path, &identity::Identity::new(DID, DATABASE_URL)).unwrap();
    let error = Principal::resolve(&runtime, &path).await.unwrap_err();
    assert_eq!(error.code(), "not_authenticated");
}

#[tokio::test]
async fn logout_is_idempotent_and_forgets_the_identity() {
    let runtime = mem_runtime();
    provision(&runtime).await;
    let (_dir, path) = identity_path();
    identity::save(&path, &identity::Identity::new(DID, DATABASE_URL)).unwrap();
    let logout = || BackendCommand::Session {
        op: SessionOp::Logout,
    };

    let first = cli::dispatch(&runtime, &path, logout()).await.unwrap();
    assert_eq!(first["hadIdentity"], true);
    let second = cli::dispatch(&runtime, &path, logout()).await.unwrap();
    assert_eq!(second["hadIdentity"], false);
    assert_eq!(second["loggedOut"], true);

    let error = cli::dispatch(&runtime, &path, whoami()).await.unwrap_err();
    assert_eq!(error.code(), "not_authenticated");
}
