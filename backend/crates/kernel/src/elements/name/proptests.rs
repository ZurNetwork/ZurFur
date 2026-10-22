use proptest::prelude::*;

use super::*;

/// Visible characters, ordinary and other spaces, fillers, zero-width spaces, joiners, selectors,
/// format characters, a tab, the pieces of a subdivision flag, a Mongolian letter and vowel
/// separator, an unassigned default-ignorable, two invisible marks, two combining marks, the line
/// and paragraph separators, the object replacement character and a noncharacter.
fn alphabet() -> Vec<char> {
    vec![
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
        '\u{034F}',
        '\u{17B4}',
        '\u{0301}',
        '\u{20DD}',
        '\u{2028}',
        '\u{2029}',
        '\u{FFFC}',
        '\u{FDD0}',
    ]
}

/// Names of up to ten characters drawn from `alphabet`.
fn name_from(alphabet: Vec<char>) -> impl Strategy<Value = String> {
    let characters = prop::sample::select(alphabet);
    prop::collection::vec(characters, 0..10).prop_map(|characters| characters.into_iter().collect())
}

/// Names built from [`alphabet`], with no `/`.
fn name() -> impl Strategy<Value = String> {
    name_from(alphabet())
}

/// A visible character none of the names here hold, standing in for a `/`.
const STAND_IN: char = 'c';

/// Names built from [`alphabet`] and the `/`.
fn name_with_slashes() -> impl Strategy<Value = String> {
    let mut characters = alphabet();
    characters.push('/');
    name_from(characters)
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
            prop_assert!(!single(first).is_some_and(is_combining_mark));
            prop_assert!(last_visible.is_some_and(|unit| !blank(unit)));
            prop_assert!(!stored.iter().any(stripped_inside));
        }
    }

    #[test]
    fn each_gap_holds_at_most_one_joiner_and_one_selector(text in name()) {
        if let Ok(name) = NodeName::try_from(text.as_str()) {
            let mut joiners = 0;
            let mut selectors = 0;
            for unit in units(name.as_ref()) {
                match modifier(unit) {
                    Some(Modifier::Joiner) => joiners += 1,
                    Some(Modifier::Selector) => selectors += 1,
                    None if !single(unit).is_some_and(is_blank) => {
                        joiners = 0;
                        selectors = 0;
                    }
                    None => {}
                }
                prop_assert!(joiners <= 1 && selectors <= 1, "{name:?}");
            }
        }
    }
}

#[test]
fn the_stand_in_is_never_in_the_alphabet() {
    assert!(!alphabet().contains(&STAND_IN));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn a_showable_name_and_a_node_name_agree_on_text_without_a_slash(text in name()) {
        let node = NodeName::try_from(text.as_str()).map(|name| name.to_string());
        let showable = ShowableName::try_from(text.as_str())
            .map(|name| name.to_string())
            .map_err(NodeNameError::from);
        prop_assert_eq!(node, showable);
    }

    #[test]
    fn a_slash_is_to_a_showable_name_what_any_visible_character_is(text in name_with_slashes()) {
        let stood_in = text.replace('/', &STAND_IN.to_string());
        let showable = ShowableName::try_from(text.as_str())
            .map(|name| name.to_string())
            .map_err(NodeNameError::from);
        let node = NodeName::try_from(stood_in)
            .map(|name| name.to_string().replace(STAND_IN, "/"));
        prop_assert_eq!(showable, node);
    }
}
