//! `zurfur account create` / `delete` in process (ZMVP-205 slices 4 and 5):
//! the CLI calls the same `application::account::Accounts::{create,
//! delete}` as `POST /accounts` / `DELETE /accounts/{id}`, and projects them
//! with the same keys.

use std::path::{Path, PathBuf};

use cli::{BackendCommand, ExitClass, commands::account::AccountOp, identity};
use composition::Runtime;
use domain::elements::{account::AccountId, did::Did};
use domain::ports::UnitOfWork;
use test_support::runtime::DATABASE_URL;
use uuid::Uuid;

const DID: &str = "did:plc:cli-account";
/// A second recognized visitor, used to act as someone who holds no role on
/// the account under test.
const OTHER_DID: &str = "did:plc:cli-account-other";

fn mem_runtime() -> Runtime {
    test_support::runtime::mem(&Did::new(DID.to_string()))
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
    let provisioned = Did::new(did.to_string());
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
/// did:plc nothing ever mints (`AccountId` is a DID, DD 57081857, not a
/// bare UUID).
fn unknown_account_id() -> AccountId {
    AccountId::new(Did::new(format!("did:plc:{}", Uuid::now_v7())))
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

    // No account-anchored fact store exists yet, so a fresh account is empty
    // and every deletion is hard (`application::account::account_has_facts`).
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
