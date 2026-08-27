//! `zurfur account create` in process (ZMVP-205 slice 4): the CLI calls the
//! same `application::account::create_account` as `POST /accounts`, and
//! projects it with the same keys.

use std::path::PathBuf;

use cli::{BackendCommand, ExitClass, commands::account::AccountOp, identity};
use composition::Runtime;
use domain::elements::did::Did;
use domain::ports::UnitOfWork;
use test_support::runtime::DATABASE_URL;

const DID: &str = "did:plc:cli-account";

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
    let did = Did::new(DID.to_string());
    runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&did).await?;
            Ok(())
        })
        .await
        .unwrap();
    let (dir, path) = identity_path();
    identity::save(&path, &identity::Identity::new(DID, DATABASE_URL)).unwrap();
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
    let keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
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
