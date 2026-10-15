use super::*;

fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

#[test]
fn the_root_is_empty_and_shows_as_a_slash() {
    let root = RealPath::default();
    assert_eq!(root.depth(), 0);
    assert_eq!(root.to_string(), "/");
}

#[test]
fn join_adds_one_segment_below() {
    let commission = RealPath::default().join(segment("commission"));
    let node = commission.join(segment("01a0ef9c"));
    assert_eq!(node.depth(), 2);
    assert_eq!(node.to_string(), "/commission/01a0ef9c");
    let expected_segments = [segment("commission"), segment("01a0ef9c")];
    assert_eq!(node.segments(), expected_segments);
}

#[test]
fn collecting_segments_builds_the_same_path() {
    let collected: RealPath = ["user", "did:plc:alice"].into_iter().map(segment).collect();
    let joined = RealPath::default()
        .join(segment("user"))
        .join(segment("did:plc:alice"));
    assert_eq!(collected, joined);
}
