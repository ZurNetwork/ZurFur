//! In-process harness (ZMVP-201): drive [`cli::dispatch`] over a
//! [`composition::Runtime`] wired to the in-memory fakes — no database, no
//! process spawn. This is where the Engineer's operation commands get their
//! fast tests; the process harness only pins the conventions.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use cli::{Command, ExitClass, commands::session::SessionOp};
use composition::{Config, Environment, Runtime};
use domain::elements::{did::Did, profile::Profile};

/// A runtime over the in-memory fakes, mirroring `api`'s e2e fixtures.
fn mem_runtime() -> Runtime {
    let backend = MemBackend::new();
    let did = Did::new("did:plc:cli-harness".to_string());
    let profile = Profile {
        did: did.clone(),
        handle: "harness.bsky.social".to_string(),
        display_name: None,
        avatar_url: None,
    };
    let config = Config {
        env: Environment::DEV,
        http_addr: "127.0.0.1:0".parse().unwrap(),
        public_url: "http://127.0.0.1:0".to_string(),
        database_url: "postgres://unused".to_string(),
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

// The pre-declared session ops answer an honest not_implemented problem —
// an infra class, never a fake success — until ZMVP-203/204 fill them in.
#[tokio::test]
async fn unbuilt_session_ops_refuse_honestly() {
    let runtime = mem_runtime();
    let command = Command::Session {
        op: SessionOp::Whoami,
    };
    let error = cli::dispatch(&runtime, command)
        .await
        .expect_err("not built yet");
    assert_eq!(error.class(), ExitClass::Infra);
    assert_eq!(error.code(), "not_implemented");
}
