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
