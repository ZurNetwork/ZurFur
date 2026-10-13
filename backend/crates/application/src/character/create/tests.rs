use std::sync::Arc;

use chrono::Utc;
use domain::elements::{
    character::{CharacterDescription, CharacterName},
    did::Did,
};
use test_support::recording::{CallLog, RecordingDidMinter};

use super::*;
use crate::{character::CharacterError, den::DenNameError};

fn handle_domain() -> HandleDomain {
    "zurfur.app".parse().expect("a valid handle domain")
}

/// A command creating a handle-less Character named `name`.
fn command(name: &str) -> Command {
    let attributes = CharacterAttributes {
        name: name.parse::<CharacterName>().expect("a character name"),
        description: "".parse::<CharacterDescription>().expect("no description"),
        species: String::new(),
        dynamic_attributes: Vec::new(),
    };
    Command {
        actor_id: UserId::from(Did::from("did:plc:keeper".to_owned())),
        handle: None,
        attributes,
    }
}

#[tokio::test]
async fn a_name_of_one_joiner_alone_is_refused_as_empty() {
    let did = Did::from("did:plc:keeper".to_owned());
    let mut runtime = test_support::runtime::mem(&did).build().runtime;
    let minted = CallLog::default();
    runtime.did_minter = Arc::new(RecordingDidMinter::new(
        runtime.did_minter.clone(),
        minted.clone(),
    ));
    let app = crate::App::new(crate::Ports::from(&runtime));

    for joiner in ["\u{200D}", "\u{200C}"] {
        let created = Characters::new(app.ports())
            .create(command(joiner), &handle_domain(), Utc::now())
            .await;

        assert!(
            matches!(created, Err(CharacterError::Name(DenNameError::Empty))),
            "{joiner:?}: {created:?}"
        );
    }
    let kept = app
        .ports()
        .characters
        .list_kept_by(&UserId::from(did))
        .await
        .expect("the store reads");
    assert!(kept.is_empty(), "no Character was created");
    assert_eq!(minted.events(), Vec::<&str>::new(), "no DID was minted");
}

#[tokio::test]
async fn a_name_the_rule_accepts_creates_the_character() {
    let did = Did::from("did:plc:keeper".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let app = crate::App::new(crate::Ports::from(&runtime));

    let created = Characters::new(app.ports())
        .create(
            command("\u{1F469}\u{200D}\u{1F3A8}"),
            &handle_domain(),
            Utc::now(),
        )
        .await;

    assert!(created.is_ok(), "{created:?}");
}

#[tokio::test]
async fn a_name_is_stored_as_the_den_s_rule_keeps_it() {
    let did = Did::from("did:plc:keeper".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let app = crate::App::new(crate::Ports::from(&runtime));

    Characters::new(app.ports())
        .create(command("Ad\u{200B}min"), &handle_domain(), Utc::now())
        .await
        .expect("creates");

    let kept = app
        .ports()
        .characters
        .list_kept_by(&UserId::from(did))
        .await
        .expect("the store reads");
    let names: Vec<&str> = kept
        .iter()
        .map(|character| character.attributes.name.as_ref())
        .collect();
    assert_eq!(names, ["Admin"]);
}
