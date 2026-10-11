use chrono::Utc;
use domain::elements::character::{
    CharacterAttributes, CharacterDescription, CharacterName, Presence,
};
use domain::elements::did::Did;

use super::*;

fn user(did: &str) -> UserId {
    UserId::from(Did::from(did.to_string()))
}

fn character(keeper: &UserId, did: &str, name: &str) -> Character {
    let attributes = CharacterAttributes {
        name: name.parse::<CharacterName>().unwrap(),
        description: "".parse::<CharacterDescription>().unwrap(),
        species: String::new(),
        dynamic_attributes: Vec::new(),
    };
    Character::create(
        keeper.clone(),
        Presence::Private,
        Did::from(did.to_string()),
        attributes,
        Utc::now(),
    )
}

fn ids(characters: &[Character]) -> Vec<&str> {
    characters
        .iter()
        .map(|character| AsRef::<str>::as_ref(&character.id))
        .collect()
}

#[tokio::test]
async fn the_keeper_read_lists_only_that_keepers_characters_by_did() {
    let backend = MemBackend::new();
    let keeper = user("did:plc:keeper");
    let stranger = user("did:plc:stranger");
    backend
        .create_character(character(&keeper, "did:plc:zz", "Last"))
        .await
        .unwrap();
    backend
        .create_character(character(&keeper, "did:plc:aa", "First"))
        .await
        .unwrap();
    backend
        .create_character(character(&stranger, "did:plc:mm", "Theirs"))
        .await
        .unwrap();

    let kept = backend
        .character_store()
        .list_kept_by(&keeper)
        .await
        .unwrap();

    let expected_ids = ["did:plc:aa", "did:plc:zz"];
    assert_eq!(ids(&kept), expected_ids);
}

#[tokio::test]
async fn the_keeper_read_leaves_out_tombstoned_characters() {
    let backend = MemBackend::new();
    let keeper = user("did:plc:keeper");
    backend
        .create_character(character(&keeper, "did:plc:live", "Live"))
        .await
        .unwrap();
    backend.seed_tombstoned_character(character(&keeper, "did:plc:gone", "Gone"));

    let kept = backend
        .character_store()
        .list_kept_by(&keeper)
        .await
        .unwrap();

    let expected_ids = ["did:plc:live"];
    assert_eq!(ids(&kept), expected_ids);
}

#[tokio::test]
async fn the_keeper_read_is_empty_for_a_user_who_keeps_nothing() {
    let backend = MemBackend::new();

    let kept = backend
        .character_store()
        .list_kept_by(&user("did:plc:nobody"))
        .await
        .unwrap();

    assert!(kept.is_empty());
}

#[tokio::test]
async fn a_character_created_in_a_unit_is_kept_after_commit() {
    let backend = MemBackend::new();
    let keeper = user("did:plc:keeper");
    let created = character(&keeper, "did:plc:new", "New");

    let mut uow = backend.database().begin().await.unwrap();
    uow.characters().create(created.clone()).await.unwrap();
    uow.commit().await.unwrap();

    let kept = backend
        .character_store()
        .list_kept_by(&keeper)
        .await
        .unwrap();
    let expected = vec![created];
    assert_eq!(kept, expected);
}
