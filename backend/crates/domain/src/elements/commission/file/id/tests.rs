use super::*;

#[test]
fn a_file_key_is_opaque_and_round_trips() {
    let raw = uuid::Uuid::now_v7();
    assert_eq!(*FileKey::new(raw), raw);
    assert_ne!(*FileKey::generate(), *FileKey::generate());
}
