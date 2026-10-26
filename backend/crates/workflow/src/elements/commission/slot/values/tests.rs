use super::*;
use kernel::elements::name::NodeNameError;

// A slot title accepts and refuses exactly what a node name does, stripped the same way.
#[test]
fn a_slot_title_is_a_node_name() {
    let too_long = "a".repeat(kernel::elements::name::MAX_NAME_BYTES + 1);
    let cases = [
        "The knight",
        "  The knight  ",
        "",
        "   \t ",
        "\u{200B}",
        "a\u{202E}b",
        "a/b",
        "a\nb",
        too_long.as_str(),
    ];
    for case in cases {
        let as_title = case.parse::<SlotTitle>().map(|title| title.to_string());
        let as_node_name = case.parse::<NodeName>().map(|name| name.to_string());
        assert_eq!(as_title, as_node_name, "{case:?}");
    }
}

// A blank slot title is refused as empty.
#[test]
fn a_blank_slot_title_is_refused_as_empty() {
    let refused = "   \t ".parse::<SlotTitle>();
    assert_eq!(refused, Err(NodeNameError::Empty));
}

// A slot title's Debug prints no part of the title.
#[test]
fn a_slot_title_s_debug_prints_no_title() {
    let title = "Canary knight 7f3a".parse::<SlotTitle>().unwrap();

    let printed = format!("{title:?}");

    assert_eq!(printed, "SlotTitle(..)");
}
