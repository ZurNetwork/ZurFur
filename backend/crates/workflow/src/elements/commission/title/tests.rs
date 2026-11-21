use super::*;
use kernel::elements::name::NodeNameError;

// A title accepts and refuses exactly what a node name does, stripped the same way and refused
// with the same error: the right-to-left override and the blank names included.
#[test]
fn a_title_is_a_node_name() {
    let too_long = "a".repeat(kernel::elements::name::MAX_NAME_BYTES + 1);
    let cases = [
        "A ref sheet",
        "  A ref sheet  ",
        "ドラゴン",
        "🦊",
        "",
        "   ",
        "\u{200B}",
        "\u{3164}",
        "\u{2800}",
        "\u{202E}",
        "a\u{202E}b",
        "\u{202E}gnp.exe",
        "a/b",
        "a\nb",
        "a\u{2028}b",
        "a\u{FFFC}b",
        too_long.as_str(),
    ];
    for case in cases {
        let as_title = case
            .parse::<CommissionTitle>()
            .map(|title| title.to_string());
        let as_node_name = case.parse::<NodeName>().map(|name| name.to_string());
        assert_eq!(as_title, as_node_name, "{case:?}");
    }
}

// A right-to-left override is refused, as a node name refuses it.
#[test]
fn a_title_refuses_a_right_to_left_override() {
    let refused = "a\u{202E}b".parse::<CommissionTitle>();
    assert_eq!(refused, Err(NodeNameError::Invisible));
}

// A title of only blank characters is refused as empty.
#[test]
fn a_blank_title_is_refused_as_empty() {
    for blank in ["   ", "\u{200B}", "\u{3164}"] {
        let refused = blank.parse::<CommissionTitle>();
        assert_eq!(refused, Err(NodeNameError::Empty), "{blank:?}");
    }
}

// A title and its node name convert both ways without change.
#[test]
fn a_title_round_trips_through_its_node_name() {
    let name = "A ref sheet".parse::<NodeName>().unwrap();
    let title = CommissionTitle::from(name.clone());
    let back = NodeName::from(title);
    assert_eq!(back, name);
}

// The store keeps the stripped title.
#[test]
fn the_store_keeps_the_stripped_title() {
    let title = "  A ref sheet  ".parse::<CommissionTitle>().unwrap();
    let stored = StoredText::from(title);
    assert_eq!(stored.as_str(), "A ref sheet");
}
