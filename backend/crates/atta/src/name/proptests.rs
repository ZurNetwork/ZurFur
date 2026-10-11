use proptest::prelude::*;

use super::*;

/// Names built from visible characters, ordinary and other spaces, fillers, zero-width spaces,
/// joiners, selectors, format characters, a tab, the pieces of a subdivision flag, a Mongolian
/// letter and vowel separator, and an unassigned default-ignorable.
fn name() -> impl Strategy<Value = String> {
    let alphabet = vec![
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
        '\u{1F3F4}',
        '\u{E0067}',
        '\u{E007F}',
        '\u{1820}',
        '\u{180E}',
        '\u{2065}',
    ];
    let characters = prop::sample::select(alphabet);
    prop::collection::vec(characters, 0..10).prop_map(|characters| characters.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

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
            let stored: Vec<&str> = units(name.as_ref()).collect();
            let blank = |unit: &&str| single(unit).is_some_and(is_blank);
            let first = stored.first().expect("a stored name is not empty");
            let last_visible = stored
                .iter()
                .rev()
                .find(|unit| !single(unit).is_some_and(acts_on_previous));
            let stripped_inside = |unit: &&str| single(unit).is_some_and(is_stripped_inside);
            prop_assert!(!blank(first));
            prop_assert!(last_visible.is_some_and(|unit| !blank(unit)));
            prop_assert!(!stored.iter().any(stripped_inside));
        }
    }
}
