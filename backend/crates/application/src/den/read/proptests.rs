use domain::elements::name::NodeName;
use proptest::prelude::*;

use super::*;

/// Ordinary visible characters, an emoji base and a heart among them.
fn ordinary() -> impl Strategy<Value = char> {
    prop::sample::select(vec![
        'a',
        'Z',
        '7',
        '.',
        '-',
        'ド',
        '\u{1F98A}',
        '\u{1F468}',
        '\u{2764}',
    ])
}

/// The Mongolian block's letters, with its combining marks and unassigned code points.
fn mongolian_letter() -> impl Strategy<Value = char> {
    prop::char::range('\u{1820}', '\u{18AA}')
}

/// The Mongolian free variation selectors.
fn mongolian_selector() -> impl Strategy<Value = char> {
    prop::sample::select(vec!['\u{180B}', '\u{180C}', '\u{180D}', '\u{180F}'])
}

/// The zero-width joiner and non-joiner.
fn joiner() -> impl Strategy<Value = char> {
    prop::sample::select(vec!['\u{200C}', '\u{200D}'])
}

/// VS1 to VS16 and the variation selectors supplement.
fn variation_selector() -> impl Strategy<Value = char> {
    prop_oneof![
        prop::char::range('\u{FE00}', '\u{FE0F}'),
        prop::char::range('\u{E0100}', '\u{E01EF}'),
    ]
}

/// Combining marks: the common block, the symbol marks and the Mongolian ones.
fn combining_mark() -> impl Strategy<Value = char> {
    prop_oneof![
        prop::char::range('\u{0300}', '\u{036F}'),
        prop::char::range('\u{20D0}', '\u{20F0}'),
        prop::sample::select(vec!['\u{1885}', '\u{1886}', '\u{18A9}']),
    ]
}

/// The fillers, the zero-width spaces, the invisible marks and the soft hyphen.
fn filler() -> impl Strategy<Value = char> {
    prop::sample::select(vec![
        '\u{115F}',
        '\u{1160}',
        '\u{3164}',
        '\u{FFA0}',
        '\u{2800}',
        '\u{1D159}',
        '\u{16FE4}',
        '\u{13441}',
        '\u{13442}',
        '\u{200B}',
        '\u{2060}',
        '\u{FEFF}',
        '\u{034F}',
        '\u{17B4}',
        '\u{17B5}',
        '\u{00AD}',
    ])
}

/// Spaces, ordinary and other, and a tab.
fn space() -> impl Strategy<Value = char> {
    prop::sample::select(vec![' ', '\u{00A0}', '\u{3000}', '\t'])
}

/// Any tag character, outside a flag.
fn loose_tag() -> impl Strategy<Value = char> {
    prop::char::range('\u{E0020}', '\u{E007F}')
}

/// A black flag with up to eight tag digits or lowercase letters after it, cancelled or not:
/// a subdivision flag, or something shaped nearly like one.
fn flag() -> impl Strategy<Value = String> {
    let tag = prop_oneof![
        prop::char::range('\u{E0030}', '\u{E0039}'),
        prop::char::range('\u{E0061}', '\u{E007A}'),
    ];
    let tags = prop::collection::vec(tag, 0..9);
    (tags, any::<bool>()).prop_map(|(tags, cancelled)| {
        let cancel = cancelled.then_some('\u{E007F}');
        std::iter::once('\u{1F3F4}')
            .chain(tags)
            .chain(cancel)
            .collect()
    })
}

/// One piece of a name, weighted toward what the rule strips, keeps only in context, or reads
/// as a unit.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => ordinary().prop_map(String::from),
        4 => mongolian_letter().prop_map(String::from),
        3 => mongolian_selector().prop_map(String::from),
        3 => Just(String::from('\u{180E}')),
        3 => joiner().prop_map(String::from),
        3 => variation_selector().prop_map(String::from),
        2 => flag(),
        1 => loose_tag().prop_map(String::from),
        3 => combining_mark().prop_map(String::from),
        3 => filler().prop_map(String::from),
        2 => space().prop_map(String::from),
    ]
}

/// Text as a store might hold it: up to sixteen pieces.
fn stored_text() -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..16).prop_map(|pieces| pieces.concat())
}

/// An arm's name as `DenHost::found` hands it to the engine.
#[allow(
    clippy::disallowed_methods,
    reason = "the test walks the host's one construction site"
)]
fn handed_over(name: &NodeName) -> DisplayName {
    DisplayName::host_checked(name.to_string())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// Whatever a store holds, the name an arm makes of it leaves the engine as the same name:
    /// the re-check on the way out never refuses or changes what the way in let through.
    #[test]
    fn a_stored_name_survives_the_way_out(text in stored_text()) {
        let stored = names::stored_name(&text)
            .ok_or_else(|| TestCaseError::fail("stored text always has a name"))?;

        let display = handed_over(&stored);
        let shown = den_name(&display)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;

        prop_assert_eq!(shown.as_ref(), stored.as_ref());
    }
}
