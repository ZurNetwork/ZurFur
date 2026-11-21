use strum::VariantArray;

use super::*;

// The stored words, in declaration order.
#[test]
fn the_words_are_these() {
    let words: Vec<&'static str> = NodeType::VARIANTS
        .iter()
        .map(|node_type| <&'static str>::from(*node_type))
        .collect();

    assert_eq!(
        words,
        [
            "commission",
            "commission.attachments",
            "commission.brief",
            "commission.changelog",
            "commission.participants",
            "commission.products",
            "commission.slots",
            "slot",
        ]
    );
}

// Every word round-trips through Display and both parses.
#[test]
fn every_word_round_trips() {
    for node_type in NodeType::VARIANTS {
        let word = <&'static str>::from(*node_type);

        assert_eq!(node_type.to_string(), word);
        assert_eq!(word.parse::<NodeType>(), Ok(*node_type));
        assert_eq!(NodeType::try_from(word), Ok(*node_type));
    }
}

// Only the eight exact words parse; anything else is the typed refusal.
#[test]
fn a_word_outside_the_eight_is_refused() {
    let refused = [
        "",
        "Commission",
        "COMMISSION",
        "commission ",
        "attachments",
        "commission.",
        "commission.notes",
        "commission_slots",
        "commission.Slots",
        "slots",
        "folder",
    ];
    for word in refused {
        assert_eq!(word.parse::<NodeType>(), Err(UnknownNodeType), "{word:?}");
        assert_eq!(NodeType::try_from(word), Err(UnknownNodeType), "{word:?}");
    }
}
