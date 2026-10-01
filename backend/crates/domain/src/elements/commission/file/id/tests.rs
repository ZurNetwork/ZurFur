use super::*;

#[test]
fn a_file_key_is_opaque_and_round_trips() {
    let raw = uuid::Uuid::now_v7();
    assert_eq!(uuid::Uuid::from(FileKey::from(raw)), raw);
    assert_ne!(
        uuid::Uuid::from(FileKey::generate()),
        uuid::Uuid::from(FileKey::generate())
    );
}
