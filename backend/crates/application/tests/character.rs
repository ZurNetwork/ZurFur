//! `Characters::create` over the in-memory fakes: a public Character's handle
//! passes the same claim gate as an Account's.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::sync::Arc;

use application::{
    character::{self, CharacterError, Characters},
    common_error::CommonError,
    transaction,
};
use chrono::Utc;
use domain::{
    elements::{
        character::{CharacterAttributes, CharacterDescription},
        did::Did,
        handle::{HandleDomain, HandleError},
        user::User,
    },
    ports::{Database, UnitOfWork},
};
use test_support::recording::{CallLog, RecordingDidMinter};

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

fn attributes() -> CharacterAttributes {
    CharacterAttributes {
        name: "Fenn".parse().expect("a valid name"),
        description: "".parse::<CharacterDescription>().expect("no description"),
        species: "fox".to_string(),
        dynamic_attributes: Vec::new(),
    }
}

// A handle the configured namespace withholds surfaces as `InvalidHandle`,
// carrying the rule, and no DID is minted for it.
#[tokio::test]
async fn a_withheld_handle_is_an_invalid_handle_before_anything_is_minted() {
    let did = Did::from("did:plc:character-reserved".to_string());
    let mut runtime = test_support::runtime::mem(&did).build().runtime;
    let user = recognized(&*runtime.database, &did).await;
    let log = CallLog::default();
    let recording_minter = RecordingDidMinter::new(runtime.did_minter.clone(), log.clone());
    runtime.did_minter = Arc::new(recording_minter);
    let withheld = [
        (
            "admin.zurfur.app",
            HandleError::ReservedLabel("admin".into()),
        ),
        ("zurfur.app", HandleError::NamespaceApex),
    ];

    for (handle, rule) in withheld {
        let command = character::create::Command {
            actor_id: user.id.clone(),
            handle: Some(handle.parse().expect("a valid handle")),
            attributes: attributes(),
        };
        let app = runtime.app();
        let Err(error) = Characters::new(app.ports())
            .create(command, &handle_domain(), Utc::now())
            .await
        else {
            panic!("{handle} must not be claimed by a Character");
        };

        assert!(
            matches!(&error, CharacterError::Common(CommonError::InvalidHandle(refused)) if *refused == rule),
            "{handle}: {error:?}"
        );
    }
    assert!(log.events().is_empty(), "minted: {:?}", log.events());
}
