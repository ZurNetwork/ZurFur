use proptest::prelude::*;

use super::*;

/// Names built from visible characters, ordinary and other spaces, fillers, zero-width spaces,
/// joiners, selectors, format characters and a tab.
fn name() -> impl Strategy<Value = String> {
    let characters = prop::sample::select(vec![
        'a',
        'b',
        ' ',
        '\t',
        '\u{00A0}',
        '\u{3000}',
        '\u{3164}',
        '\u{2800}',
        '\u{115F}',
        '\u{200B}',
        '\u{FEFF}',
        '\u{200C}',
        '\u{200D}',
        '\u{FE0F}',
        '\u{E0100}',
        '\u{180B}',
        '\u{202E}',
        '\u{00AD}',
        '\u{2764}',
        '\u{1F468}',
    ]);
    prop::collection::vec(characters, 0..10).prop_map(|characters| characters.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn stripping_is_idempotent(text in name()) {
        if let Ok(name) = NodeName::try_from(text.as_str()) {
            let reparsed = NodeName::try_from(name.as_ref());
            prop_assert_eq!(reparsed, Ok(name));
        }
    }

    #[test]
    fn a_stored_name_neither_starts_nor_ends_blank(text in name()) {
        if let Ok(name) = NodeName::try_from(text.as_str()) {
            let stored: &str = name.as_ref();
            let first = stored.chars().next().expect("a stored name is not empty");
            let last_visible = stored.chars().rev().find(|&character| !acts_on_previous(character));
            prop_assert!(!is_blank(first));
            prop_assert!(last_visible.is_some_and(|character| !is_blank(character)));
            prop_assert!(!stored.chars().any(is_stripped_inside));
        }
    }
}
