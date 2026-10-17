use super::*;

#[test]
fn the_reference_is_the_id_s_last_eight_characters() {
    let uuid = uuid::Uuid::parse_str("0193a1b2-c3d4-7e5f-8a6b-1c2d3e4f5a6b").expect("a valid UUID");
    let id = BugReportId::from(uuid);

    let reference = BugReportReference::from(&id);

    assert_eq!(reference.as_ref(), "3e4f5a6b");
}

#[test]
fn a_minted_id_is_a_version_seven_uuid_whose_reference_ends_it() {
    let id = BugReportId::from(uuid::Uuid::now_v7());

    let reference = BugReportReference::from(&id);

    let uuid: uuid::Uuid = id.into();
    assert_eq!(uuid.get_version_num(), 7);
    assert_eq!(reference.to_string().chars().count(), 8);
    assert!(id.to_string().ends_with(reference.as_ref()));
}
