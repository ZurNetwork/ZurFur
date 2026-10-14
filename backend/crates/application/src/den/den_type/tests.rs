use std::str::FromStr;

use strum::VariantArray;

use super::*;

/// Every type with its kind and whether its content is shown, written out by hand.
const FACTS: [(DenType, NodeKind, bool); 19] = [
    (DenType::TopLevel, NodeKind::Directory, false),
    (DenType::Posts, NodeKind::Directory, true),
    (DenType::User, NodeKind::Directory, false),
    (DenType::UserAccounts, NodeKind::Directory, false),
    (DenType::UserCharacters, NodeKind::Directory, false),
    (DenType::UserCommissions, NodeKind::Directory, false),
    (DenType::Account, NodeKind::Directory, false),
    (DenType::AccountCharacters, NodeKind::Directory, false),
    (DenType::AccountCommissions, NodeKind::Directory, true),
    (DenType::AccountWorkflows, NodeKind::Directory, false),
    (DenType::Character, NodeKind::Directory, true),
    (DenType::Commission, NodeKind::Directory, false),
    (DenType::CommissionAttachments, NodeKind::Directory, false),
    (DenType::CommissionChangelog, NodeKind::File, true),
    (DenType::CommissionProducts, NodeKind::Directory, false),
    (DenType::CommissionSlots, NodeKind::Directory, false),
    (DenType::Slot, NodeKind::Directory, true),
    (DenType::File, NodeKind::File, true),
    (DenType::Workflow, NodeKind::Directory, true),
];

#[test]
fn every_type_has_its_word() {
    let words: Vec<&'static str> = DenType::VARIANTS
        .iter()
        .map(|den_type| den_type.into())
        .collect();

    let expected_words = [
        "top_level",
        "posts",
        "user",
        "user.accounts",
        "user.characters",
        "user.commissions",
        "account",
        "account.characters",
        "account.commissions",
        "account.workflows",
        "character",
        "commission",
        "commission.attachments",
        "commission.changelog",
        "commission.products",
        "commission.slots",
        "slot",
        "file",
        "workflow",
    ];
    assert_eq!(words, expected_words);
}

#[test]
fn every_word_parses_back_to_its_type() {
    for den_type in DenType::VARIANTS {
        let word = den_type.to_string();

        let parsed = DenType::from_str(&word);

        assert_eq!(parsed, Ok(*den_type), "{word}");
    }
}

#[test]
fn an_unknown_word_is_no_type() {
    let parsed = DenType::from_str("commission.participants");

    assert!(parsed.is_err());
}

#[test]
fn the_fact_table_covers_every_type_once() {
    let listed: Vec<DenType> = FACTS.iter().map(|(den_type, _, _)| *den_type).collect();

    assert_eq!(listed, DenType::VARIANTS);
}

#[test]
fn each_type_has_its_kind() {
    for (den_type, kind, _) in FACTS {
        assert_eq!(den_type.kind(), kind, "{den_type}");
    }
}

#[test]
fn each_type_shows_or_hides_its_content() {
    for (den_type, _, content_not_shown) in FACTS {
        assert_eq!(
            den_type.content_not_shown(),
            content_not_shown,
            "{den_type}"
        );
    }
}
