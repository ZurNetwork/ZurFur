use std::sync::Arc;

use adapter_mem::fixture::{abco, alice, bob, ember, seed_identity};
use chrono::Utc;
use domain::elements::{
    character::{Character, CharacterAttributes, CharacterDescription},
    did::Did,
    handle::Handle,
    user::UserId,
};
use test_support::recording::{CallLog, RecordingCharacterStore, SubjectLog};

use super::*;
use crate::{App, Ports, den::RootNaming};

/// A Character kept by `keeper`, named `name`, at `presence`.
fn character(keeper: UserId, did: &str, name: &str, presence: Presence) -> Character {
    let attributes = CharacterAttributes {
        name: name.parse().expect("a valid character name"),
        description: "".parse::<CharacterDescription>().expect("no description"),
        species: String::new(),
        dynamic_attributes: Vec::new(),
    };
    Character::create(
        keeper,
        presence,
        Did::from(did.to_owned()),
        attributes,
        Utc::now(),
    )
}

/// The fixture world plus alice's Public "Lumen", her "Flipped" with a name the rule refuses
/// and her tombstoned "Gone", the Character store recording its calls.
async fn recorded() -> (App, CallLog, SubjectLog) {
    let mut mem = test_support::runtime::mem(alice().did()).build();
    seed_identity(&mem.backend)
        .await
        .expect("seeds identity's fixtures");
    let public = Presence::Public {
        handle: "lumen.zurfur.app"
            .parse::<Handle>()
            .expect("a valid handle"),
    };
    let lumen = character(alice(), "did:plc:lumen", "Lumen", public);
    let flipped = character(alice(), "did:plc:flipped", "\u{202E}", Presence::Private);
    let gone = character(alice(), "did:plc:gone", "Gone", Presence::Private);
    mem.backend
        .create_character(lumen)
        .await
        .expect("seeds Lumen");
    mem.backend
        .create_character(flipped)
        .await
        .expect("seeds Flipped");
    mem.backend.seed_tombstoned_character(gone);
    let log = CallLog::default();
    let subjects = SubjectLog::default();
    mem.runtime.characters = Arc::new(RecordingCharacterStore::new(
        mem.runtime.characters.clone(),
        log.clone(),
        subjects.clone(),
    ));
    (App::new(Ports::from(&mem.runtime)), log, subjects)
}

fn id(did: &str) -> CharacterId {
    CharacterId::from(Did::from(did.to_owned()))
}

#[tokio::test]
async fn a_kept_character_opens_by_its_keeper_s_standing_at_its_own_level() {
    let (app, _log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    let kept_ember = node(&context, &ember()).await.expect("no store fails");
    let lumen = node(&context, &id("did:plc:lumen"))
        .await
        .expect("no store fails");

    let private_ember = Node {
        place: Place::Character(CharacterPlace::Character(ember())),
        name: names::stored_name("Ember").expect("a valid name"),
        level: Level::Private,
        lifts: vec![STANDING],
        state: NodeState::Live,
    };
    assert_eq!(kept_ember, Some(private_ember));
    assert_eq!(lumen.map(|lumen| lumen.level), Some(Level::Public));
}

#[tokio::test]
async fn a_stored_name_the_rule_refuses_shows_unnamed() {
    let (app, _log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    let flipped = node(&context, &id("did:plc:flipped"))
        .await
        .expect("no store fails")
        .expect("alice keeps Flipped");

    assert_eq!(flipped.name.as_ref(), names::UNNAMED);
}

#[tokio::test]
async fn another_s_a_tombstoned_and_a_missing_character_are_absent_after_the_same_read() {
    let (app, log, subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    for character in [abco(), id("did:plc:gone"), id("did:plc:nope")] {
        let node = node(&context, &character).await.expect("no store fails");

        assert_eq!(node, None, "{character}");
    }
    let alice_did = alice.to_string();
    assert_eq!(log.events(), ["list_kept_by"; 3]);
    assert_eq!(
        subjects.subjects(),
        [alice_did.clone(), alice_did.clone(), alice_did]
    );
}

#[tokio::test]
async fn bob_reaches_only_the_character_he_keeps() {
    let (app, _log, _subjects) = recorded().await;
    let bob = bob();
    let context = DenContext {
        ports: app.ports(),
        viewer: &bob,
        root_naming: RootNaming::LookUp,
    };

    let abco = node(&context, &abco()).await.expect("no store fails");
    let ember = node(&context, &ember()).await.expect("no store fails");

    assert_eq!(
        abco.map(|abco| abco.name.to_string()),
        Some("Abco".to_owned())
    );
    assert_eq!(ember, None);
}

#[tokio::test]
async fn a_character_holds_nothing_yet() {
    let (app, log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };
    let place = CharacterPlace::Character(ember());
    let key = Segment::try_from("art").expect("a valid segment");

    let children = children(&context, &place).await.expect("no store fails");
    let mounts = mounts(&context, &place).await.expect("no store fails");
    let child = child(&context, &place, &key).await.expect("no store fails");

    assert_eq!(children, Vec::new());
    assert_eq!(mounts, Vec::new());
    assert_eq!(child, None);
    assert_eq!(log.events(), Vec::<&str>::new());
}
