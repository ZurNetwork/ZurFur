use super::*;

#[test]
fn a_minted_id_is_a_version_7_uuid() {
    let minted = NodeId::mint();

    let uuid = uuid::Uuid::from(minted);
    assert_eq!(uuid.get_version_num(), 7);
}

#[test]
fn two_minted_ids_differ() {
    assert_ne!(NodeId::mint(), NodeId::mint());
}
