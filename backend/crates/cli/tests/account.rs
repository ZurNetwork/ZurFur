//! `zurfur account create` / `delete` in process: the CLI calls the same `application::account::Accounts::{create,
//! delete}` as `POST /accounts` / `DELETE /accounts/{id}`, and projects them
//! with the same keys; a failure of either logs no DID above debug.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use cli::{BackendCommand, ExitClass, commands::account::AccountOp, identity};
use composition::Runtime;
use domain::elements::{account::AccountId, did::Did, handle::Handle};
use domain::ports::{Database, DidMinter, UnitOfWork};
use test_support::log_capture::{self, CapturedLog};
use test_support::recording::{CallLog, RecordingDidMinter};
use test_support::runtime::DATABASE_URL;
use tracing::subscriber::DefaultGuard;
use uuid::Uuid;

const DID: &str = "did:plc:cli-account";
/// A second recognized visitor, used to act as someone who holds no role on
/// the account under test.
const OTHER_DID: &str = "did:plc:cli-account-other";

fn mem_runtime() -> Runtime {
    test_support::runtime::mem(&Did::from(DID.to_string()))
        .build()
        .runtime
}

fn identity_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(identity::IDENTITY_FILE_NAME);
    (dir, path)
}

async fn signed_in(runtime: &Runtime) -> (tempfile::TempDir, PathBuf) {
    signed_in_as(runtime, DID).await
}

/// Provision `did` and hand back an identity file recorded against it — the
/// one seam `Principal::resolve` reads, so this is how a test chooses who acts.
async fn signed_in_as(runtime: &Runtime, did: &str) -> (tempfile::TempDir, PathBuf) {
    let provisioned = Did::from(did.to_string());
    runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&provisioned).await?;
            Ok(())
        })
        .await
        .unwrap();
    let (dir, path) = identity_path();
    identity::save(&path, &identity::Identity::new(did, DATABASE_URL)).unwrap();
    (dir, path)
}

fn create(name: &str, handle: &str) -> BackendCommand {
    BackendCommand::Account {
        op: AccountOp::Create {
            name: name.to_string(),
            handle: handle.to_string(),
        },
    }
}

/// `delete` with the confirmation waived — the scripted spelling. Every
/// in-process case passes `yes: true` on purpose: the *unconfirmed* path
/// reads the process's real stdin/stderr, and under `cargo test` from a
/// terminal both of those ARE the terminal (measured: libtest captures at the
/// macro level, not the fd level), so an in-process case without `--yes`
/// would prompt the developer and block. That path is pinned deterministically
/// instead by `tests/account_process.rs`, which pipes stdin, and by the unit
/// tests in `src/confirm.rs`.
fn delete(account_id: AccountId) -> BackendCommand {
    BackendCommand::Account {
        op: AccountOp::Delete {
            account_id,
            yes: true,
        },
    }
}

/// Found an account as the signed-in identity at `path` and hand back its id.
async fn founded_account(runtime: &Runtime, path: &Path, handle: &str) -> AccountId {
    let founded = cli::dispatch(runtime, path, create("Acme Studio", handle))
        .await
        .unwrap();
    founded["id"].as_str().unwrap().parse().unwrap()
}

/// A syntactically valid account id that names no live account — a random
/// did:plc nothing ever mints (`AccountId` is itself a DID, not a
/// bare UUID paired with one).
fn unknown_account_id() -> AccountId {
    AccountId::from(Did::from(format!("did:plc:{}", Uuid::now_v7())))
}

#[tokio::test]
async fn create_founds_an_account_and_projects_it_like_post_accounts() {
    let runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;

    let value = cli::dispatch(&runtime, &path, create("Acme Studio", "acme.zurfur.app"))
        .await
        .unwrap();

    assert_eq!(value["handle"], "acme.zurfur.app");
    assert_eq!(value["name"], "Acme Studio");
    assert!(value["did"].as_str().unwrap().starts_with("did:plc:"));
    assert!(value["id"].as_str().is_some());
    // Sorted before comparing: serde_json's Map iteration order is
    // feature-dependent (`preserve_order`), and this pins the key SET.
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["did", "handle", "id", "name"]);
}

#[tokio::test]
async fn create_without_an_identity_is_not_authenticated() {
    let runtime = mem_runtime();
    let (_dir, path) = identity_path();

    let error = cli::dispatch(&runtime, &path, create("Acme", "acme.zurfur.app"))
        .await
        .unwrap_err();

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "not_authenticated");
}

#[tokio::test]
async fn a_bad_handle_is_invalid_request_before_anything_is_minted() {
    let runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;

    let error = cli::dispatch(&runtime, &path, create("Acme", "xn--80ak6aa92e.zurfur.app"))
        .await
        .unwrap_err();

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "invalid_request");
}

// The configured namespace withholds its reserved labels and its apex: the
// claim is refused as `invalid_request` and no DID is minted for it.
#[tokio::test]
async fn a_reserved_handle_is_invalid_request_before_anything_is_minted() {
    let mut runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;
    let log = CallLog::default();
    let recording_minter = RecordingDidMinter::new(runtime.did_minter.clone(), log.clone());
    runtime.did_minter = Arc::new(recording_minter);

    for withheld in ["admin.zurfur.app", "zurfur.app"] {
        let error = cli::dispatch(&runtime, &path, create("Acme", withheld))
            .await
            .unwrap_err();

        assert_eq!(error.class(), ExitClass::Domain, "{withheld}");
        assert_eq!(error.code(), "invalid_request", "{withheld}");
    }
    assert!(log.events().is_empty(), "minted: {:?}", log.events());
}

#[tokio::test]
async fn a_blank_name_is_invalid_request() {
    let runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;

    let error = cli::dispatch(&runtime, &path, create("   ", "acme.zurfur.app"))
        .await
        .unwrap_err();

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "invalid_request");
}

#[tokio::test]
async fn a_taken_handle_is_handle_taken() {
    let runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;
    cli::dispatch(&runtime, &path, create("First", "acme.zurfur.app"))
        .await
        .unwrap();

    let error = cli::dispatch(&runtime, &path, create("Second", "acme.zurfur.app"))
        .await
        .unwrap_err();

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "handle_taken");
}

#[tokio::test]
async fn the_owner_deletes_an_empty_account_hard() {
    let runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;
    let account_id = founded_account(&runtime, &path, "acme.zurfur.app").await;

    let value = cli::dispatch(&runtime, &path, delete(account_id))
        .await
        .unwrap();

    // A freshly founded Account holds no fact, so its deletion is hard.
    assert_eq!(value["outcome"], "hard");
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["outcome"]);
}

#[tokio::test]
async fn deleting_an_unknown_account_is_not_found() {
    // `Accounts::delete` looks the account up before it weighs the caller's
    // standing, so a missing account answers `account_not_found` rather than
    // borrowing the refusal meant for a caller who has no role on a real one.
    let runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;

    let error = cli::dispatch(&runtime, &path, delete(unknown_account_id()))
        .await
        .unwrap_err();

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "account_not_found");
}

#[tokio::test]
async fn a_non_member_deleting_is_forbidden() {
    let runtime = mem_runtime();
    let (_owner_dir, owner_path) = signed_in(&runtime).await;
    let account_id = founded_account(&runtime, &owner_path, "acme.zurfur.app").await;

    // A second recognized user who holds no role on the account: authority is
    // the use case's, and a non-member has none.
    let (_other_dir, other_path) = signed_in_as(&runtime, OTHER_DID).await;
    let error = cli::dispatch(&runtime, &other_path, delete(account_id))
        .await
        .unwrap_err();

    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "forbidden");
}

#[tokio::test]
async fn delete_without_an_identity_is_not_authenticated() {
    let runtime = mem_runtime();
    let (_dir, path) = identity_path();

    let error = cli::dispatch(&runtime, &path, delete(unknown_account_id()))
        .await
        .unwrap_err();

    // The principal is resolved before the account is loaded, so an
    // unrecognized caller never learns whether the id names anything.
    assert_eq!(error.class(), ExitClass::Domain);
    assert_eq!(error.code(), "not_authenticated");
}

// --- the error log of a failed create or delete ----------------------------------

/// The DID a refusing store names in its error, as the PLC directory's refusal names one.
const REFUSED_DID: &str = "did:plc:cliaccountrefused7q2x";

/// The text a refusal carries, naming [`REFUSED_DID`].
fn refusal() -> anyhow::Error {
    anyhow::anyhow!("PLC directory rejected {REFUSED_DID}: 400 Bad Request")
}

/// A [`DidMinter`] whose every call is refused with [`refusal`].
struct RefusingMinter;

#[async_trait]
impl DidMinter for RefusingMinter {
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        Err(refusal())
    }

    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        Err(refusal())
    }

    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
        Err(refusal())
    }

    async fn update_handle(&self, _did: &Did, _handle: &Handle) -> anyhow::Result<()> {
        Err(refusal())
    }
}

/// A [`Database`] that can't open a unit of work, refusing with [`refusal`].
struct RefusingDatabase;

#[async_trait]
impl Database for RefusingDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        Err(refusal())
    }
}

/// Captures this thread's log at `trace` until the guard drops.
fn captured_log() -> (CapturedLog, DefaultGuard) {
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(log.clone())
        .with_ansi(false)
        .finish();
    let guard = log_capture::set_default(subscriber);
    (log, guard)
}

/// The lines of `text` that name [`REFUSED_DID`] at info or above, or on a line with no
/// level of its own, such as a multi-line cause's continuation.
fn lines_naming_the_did_above_debug(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.contains(REFUSED_DID))
        .filter(|line| {
            let level = line.split_whitespace().nth(1);
            !matches!(level, Some("DEBUG" | "TRACE"))
        })
        .collect()
}

/// Whether a line of `text` holds every one of `parts`.
fn some_line_has(text: &str, parts: &[&str]) -> bool {
    text.lines()
        .any(|line| parts.iter().all(|part| line.contains(part)))
}

// A refused founding logs a fixed line at error and its cause at debug only, so no line at
// info or above names the DID the refusal carries.
#[tokio::test]
async fn a_failed_create_logs_no_did_above_debug() {
    let mut runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;
    runtime.did_minter = Arc::new(RefusingMinter);
    let (log, guard) = captured_log();

    let error = cli::dispatch(&runtime, &path, create("Acme Studio", "acme.zurfur.app"))
        .await
        .unwrap_err();
    drop(guard);

    assert_eq!(error.class(), ExitClass::Infra);
    assert_eq!(error.code(), "internal_error");
    let text = log.text();
    assert!(
        some_line_has(&text, &["ERROR", "founding the account failed"]),
        "the failure is logged at error: {text}"
    );
    let leaked = lines_naming_the_did_above_debug(&text);
    assert!(leaked.is_empty(), "a DID above debug: {leaked:#?}");
    assert!(
        some_line_has(&text, &["DEBUG", "the founding's failure", REFUSED_DID]),
        "the cause is logged at debug: {text}"
    );
}

// A store failure during a delete logs a fixed line at error and its cause at debug only, so
// no line at info or above names the DID the failure carries.
#[tokio::test]
async fn a_failed_delete_logs_no_did_above_debug() {
    let mut runtime = mem_runtime();
    let (_dir, path) = signed_in(&runtime).await;
    let account_id = founded_account(&runtime, &path, "acme.zurfur.app").await;
    runtime.database = Arc::new(RefusingDatabase);
    let (log, guard) = captured_log();

    let error = cli::dispatch(&runtime, &path, delete(account_id))
        .await
        .unwrap_err();
    drop(guard);

    assert_eq!(error.class(), ExitClass::Infra);
    assert_eq!(error.code(), "internal_error");
    let text = log.text();
    assert!(
        some_line_has(&text, &["ERROR", "deleting the account failed"]),
        "the failure is logged at error: {text}"
    );
    let leaked = lines_naming_the_did_above_debug(&text);
    assert!(leaked.is_empty(), "a DID above debug: {leaked:#?}");
    assert!(
        some_line_has(&text, &["DEBUG", "the delete's failure", REFUSED_DID]),
        "the cause is logged at debug: {text}"
    );
}
