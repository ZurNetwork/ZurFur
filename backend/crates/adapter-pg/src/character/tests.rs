use domain::elements::character::{
    CharacterAttributes, CharacterDescription, CharacterName, Presence,
};
use domain::elements::did::Did;

use super::*;

// PostgreSQL stores no Character yet, so the keeper read answers empty for
// anyone. Character storage replaces this test.
#[tokio::test]
async fn the_keeper_read_answers_empty_until_characters_can_be_stored() {
    let keeper = UserId::from(Did::from("did:plc:keeper".to_string()));

    let kept = PgCharacterStore.list_kept_by(&keeper).await.unwrap();

    assert!(kept.is_empty());
}

// Tripwire: character storage makes `create` stop panicking, which fails this
// test and so forces the empty keeper read above to be replaced.
#[tokio::test]
#[should_panic(expected = "character persistence lands with the Character slice")]
async fn character_storage_has_not_landed_yet() {
    let keeper = UserId::from(Did::from("did:plc:keeper".to_string()));
    let attributes = CharacterAttributes {
        name: "Tripwire".parse::<CharacterName>().unwrap(),
        description: "".parse::<CharacterDescription>().unwrap(),
        species: String::new(),
        dynamic_attributes: Vec::new(),
    };
    let character = Character::create(
        keeper,
        Presence::Private,
        Did::from("did:plc:tripwire".to_string()),
        attributes,
        chrono::Utc::now(),
    );

    let _ = PgCharacterWrites.create(character).await;
}
