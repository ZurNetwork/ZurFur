//! The use cases that call out to the DID minter or the blob store do so
//! before their unit of work opens, so no pool connection is held for a unit
//! across a network call. Each test reads the order off a [`CallLog`].

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::io::Cursor;
use std::sync::Arc;

use application::{
    account,
    character::{self, Characters},
    commission::files::upload,
    transaction,
};
use chrono::Utc;
use composition::Runtime;
use domain::{
    elements::{
        character::{CharacterAttributes, CharacterDescription},
        commission::{Commission, CommissionId},
        did::Did,
        handle::HandleDomain,
        user::{User, UserId},
    },
    ports::{Database, UnitOfWork},
};
use test_support::recording::{CallLog, RecordingDatabase, RecordingDidMinter, RecordingFileStore};

fn handle_domain() -> HandleDomain {
    "zurfur.app".parse().expect("a valid handle domain")
}

async fn recognized(database: &dyn Database, did: &Did) -> User {
    let provisioned = did.clone();
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&provisioned).await
    })
    .await
    .expect("provision")
}

/// Routes the runtime's database, DID minter and blob store through one log.
fn record_calls(runtime: &mut Runtime) -> CallLog {
    let log = CallLog::default();
    let database = RecordingDatabase::new(runtime.database.clone(), log.clone());
    runtime.database = Arc::new(database);
    let did_minter = RecordingDidMinter::new(runtime.did_minter.clone(), log.clone());
    runtime.did_minter = Arc::new(did_minter);
    let files = RecordingFileStore::new(runtime.files.clone(), log.clone());
    runtime.files = Arc::new(files);
    log
}

#[tokio::test]
async fn creating_an_account_mints_the_did_before_the_unit_opens() {
    let did = Did::from("did:plc:order-create".to_string());
    let mut runtime = test_support::runtime::mem(&did).build().runtime;
    let user = recognized(&*runtime.database, &did).await;
    let log = record_calls(&mut runtime);

    let command = account::create::Command {
        actor_id: user.id,
        name: "Acme Studio".parse().expect("a valid name"),
        handle: "acme.zurfur.app".parse().expect("a valid handle"),
    };
    runtime
        .app()
        .accounts()
        .create(command, &handle_domain(), Utc::now())
        .await
        .expect("founds");

    assert_eq!(log.events(), vec!["mint", "begin", "commit"]);
}

#[tokio::test]
async fn changing_a_handle_updates_the_did_document_before_the_unit_opens() {
    let did = Did::from("did:plc:order-change".to_string());
    let mut runtime = test_support::runtime::mem(&did).build().runtime;
    let user = recognized(&*runtime.database, &did).await;
    let founding = account::create::Command {
        actor_id: user.id.clone(),
        name: "Acme Studio".parse().expect("a valid name"),
        handle: "acme.zurfur.app".parse().expect("a valid handle"),
    };
    let founded = runtime
        .app()
        .accounts()
        .create(founding, &handle_domain(), Utc::now())
        .await
        .expect("founds");
    let log = record_calls(&mut runtime);

    let renaming = account::change_handle::Command {
        account_id: founded.account_id,
        actor_id: user.id,
        handle: "acme-two.zurfur.app".parse().expect("a valid handle"),
    };
    runtime
        .app()
        .accounts()
        .change_handle(renaming, &handle_domain(), Utc::now())
        .await
        .expect("renames");

    assert_eq!(log.events(), vec!["update_handle", "begin", "commit"]);
}

#[tokio::test]
async fn creating_a_character_mints_the_did_before_the_unit_opens() {
    let did = Did::from("did:plc:order-character".to_string());
    let mut runtime = test_support::runtime::mem(&did).build().runtime;
    let user = recognized(&*runtime.database, &did).await;
    let log = record_calls(&mut runtime);

    let attributes = CharacterAttributes {
        name: "Fenn".parse().expect("a valid name"),
        description: "".parse::<CharacterDescription>().expect("no description"),
        species: "fox".to_string(),
        dynamic_attributes: Vec::new(),
    };
    let command = character::create::Command {
        actor_id: user.id,
        handle: None,
        attributes,
    };
    let app = runtime.app();
    Characters::new(app.ports())
        .create(command, &handle_domain(), Utc::now())
        .await
        .expect("creates");

    assert_eq!(log.events(), vec!["mint_handleless", "begin", "commit"]);
}

#[tokio::test]
async fn uploading_a_file_writes_the_blob_before_the_unit_opens() {
    let did = Did::from("did:plc:order-upload".to_string());
    let mut runtime = test_support::runtime::mem(&did).build().runtime;
    let user = recognized(&*runtime.database, &did).await;
    let owner_id = user.id.clone();
    let commission_id = seed_commission(&*runtime.database, owner_id).await;
    let log = record_calls(&mut runtime);

    let command = upload::Command {
        actor_id: user.id,
        commission_id,
        filename: Some("ref.png".to_string()),
        content_type: Some("image/png".to_string()),
    };
    runtime
        .app()
        .commissions()
        .files()
        .upload(command, Cursor::new(b"bytes".to_vec()), 1024, Utc::now())
        .await
        .expect("uploads");

    assert_eq!(log.events(), vec!["put", "begin", "commit"]);
}

/// Seeds a committed commission owned by `owner_id`.
async fn seed_commission(database: &dyn Database, owner_id: UserId) -> CommissionId {
    let title = "Ref sheet".parse().expect("valid title");
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let commission = Commission::create(title, owner_id, Utc::now(), None);
        let id = commission.id;
        uow.commissions().create(&commission).await?;
        Ok(id)
    })
    .await
    .expect("seed a commission")
}
