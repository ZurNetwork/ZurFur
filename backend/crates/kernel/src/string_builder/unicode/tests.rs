use super::*;

/// Every `char`, in code point order.
fn every_char() -> impl Iterator<Item = char> {
    (0..=u32::from(char::MAX)).filter_map(char::from_u32)
}

#[test]
fn is_bidi_control_holds_for_exactly_the_nine() {
    let bidi_controls: Vec<char> = every_char().filter(|&c| is_bidi_control(c)).collect();

    let the_nine = [
        '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}',
        '\u{2068}', '\u{2069}',
    ];
    assert_eq!(bidi_controls, the_nine);
}

#[test]
fn is_unsafe_to_print_holds_for_controls_formats_ignorables_and_separators() {
    let unsafe_characters = [
        '\u{0000}', // NUL, C0
        '\u{001B}', // ESC, C0
        '\n',       // C0
        '\u{007F}', // DEL
        '\u{0085}', // NEXT LINE, C1
        '\u{009B}', // CONTROL SEQUENCE INTRODUCER, C1
        '\u{202A}',
        '\u{202B}',
        '\u{202C}',
        '\u{202D}',
        '\u{202E}', // embeddings and overrides
        '\u{2066}',
        '\u{2067}',
        '\u{2068}',
        '\u{2069}',  // isolates
        '\u{061C}',  // ARABIC LETTER MARK
        '\u{200E}',  // LEFT-TO-RIGHT MARK
        '\u{200F}',  // RIGHT-TO-LEFT MARK
        '\u{200D}',  // ZERO WIDTH JOINER
        '\u{FEFF}',  // ZERO WIDTH NO-BREAK SPACE
        '\u{00AD}',  // SOFT HYPHEN
        '\u{E0041}', // TAG LATIN CAPITAL LETTER A
        '\u{2028}',  // LINE SEPARATOR
        '\u{2029}',  // PARAGRAPH SEPARATOR
        '\u{FFF9}',  // INTERLINEAR ANNOTATION ANCHOR, format but not default-ignorable
        '\u{0600}',  // ARABIC NUMBER SIGN, format but not default-ignorable
    ];
    for character in unsafe_characters {
        assert!(
            is_unsafe_to_print(character),
            "U+{:04X} is unsafe to print",
            u32::from(character)
        );
    }
}

#[test]
fn is_unsafe_to_print_passes_ordinary_text() {
    let safe_characters = ['a', 'é', '\u{1F98A}', ' '];
    for character in safe_characters {
        assert!(
            !is_unsafe_to_print(character),
            "U+{:04X} is safe to print",
            u32::from(character)
        );
    }
}

#[test]
fn the_tables_ascend_without_overlap() {
    for table in [FORMAT_CHARACTERS, DEFAULT_IGNORABLE] {
        for &(first, last) in table {
            assert!(first <= last, "range {first:#X}..={last:#X} is reversed");
        }
        for pair in table.windows(2) {
            let [(_, previous_last), (next_first, _)] = pair else {
                unreachable!("windows(2) yields pairs");
            };
            assert!(
                previous_last < next_first,
                "{previous_last:#X} and {next_first:#X} overlap or are out of order"
            );
        }
    }
}

#[test]
fn every_bidi_control_is_a_format_character_and_default_ignorable() {
    let bidi_controls = every_char().filter(|&c| is_bidi_control(c));
    for character in bidi_controls {
        assert!(is_format(character));
        assert!(is_default_ignorable(character));
    }
}
