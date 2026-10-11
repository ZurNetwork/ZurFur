use super::*;

#[test]
fn ordinary_names_parse() {
    let names = [
        "Untitled",
        "Supreme Arts",
        "ref-abco.png",
        ".theme.yaml",
        "Kael-sona",
        "ドラゴン",
        "🦊",
    ];
    for name in names {
        let parsed = NodeName::try_from(name);
        assert_eq!(parsed.map(|name| name.to_string()), Ok(name.to_owned()));
    }
}

#[test]
fn the_empty_name_is_refused() {
    assert_eq!(NodeName::try_from(""), Err(NodeNameError::Empty));
}

#[test]
fn a_slash_is_refused() {
    assert_eq!(NodeName::try_from("a/b"), Err(NodeNameError::Slash));
}

#[test]
fn control_characters_are_refused() {
    for name in ["a\0b", "a\nb", "a\u{7F}b", "a\u{85}b"] {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Invisible),
            "{name:?}"
        );
    }
}

#[test]
fn format_characters_are_refused_bidirectional_controls_included() {
    let refused = [
        '\u{00AD}',
        '\u{061C}',
        '\u{200E}',
        '\u{200F}',
        '\u{202A}',
        '\u{202E}',
        '\u{2066}',
        '\u{2069}',
        '\u{E0001}',
        '\u{E007F}',
    ];
    for character in refused {
        let name = format!("a{character}b");
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Invisible),
            "{character:?}"
        );
    }
}

#[test]
fn the_two_joiners_are_allowed_beside_other_characters() {
    let family = "👩\u{200D}👧";
    let non_joined = "a\u{200C}b";
    assert!(NodeName::try_from(family).is_ok());
    assert!(NodeName::try_from(non_joined).is_ok());
}

#[test]
fn a_name_of_joiners_alone_is_refused_as_empty() {
    assert_eq!(NodeName::try_from("\u{200D}"), Err(NodeNameError::Empty));
    assert_eq!(
        NodeName::try_from("\u{200C}\u{200D}"),
        Err(NodeNameError::Empty)
    );
}

#[test]
fn a_name_of_exactly_the_cap_parses_and_one_more_byte_is_refused() {
    let at_cap = "a".repeat(MAX_NAME_BYTES);
    let over_cap = "a".repeat(MAX_NAME_BYTES + 1);
    assert!(NodeName::try_from(at_cap).is_ok());
    assert_eq!(NodeName::try_from(over_cap), Err(NodeNameError::TooLong));
}

#[test]
fn the_cap_counts_bytes_not_characters() {
    // "é" is two bytes, so 1025 of them is 2050 bytes.
    let over_cap = "é".repeat(1025);
    assert_eq!(NodeName::try_from(over_cap), Err(NodeNameError::TooLong));
}

#[test]
fn the_format_table_matches_its_known_edges() {
    let inside = [
        '\u{0600}',
        '\u{0605}',
        '\u{110BD}',
        '\u{1343F}',
        '\u{1D17A}',
        '\u{E0020}',
    ];
    let outside = [
        '\u{05FF}',
        '\u{0606}',
        '\u{2065}',
        '\u{E0000}',
        '\u{E0080}',
        'a',
    ];
    for character in inside {
        assert!(is_format(character), "{character:?}");
    }
    for character in outside {
        assert!(!is_format(character), "{character:?}");
    }
}

#[test]
fn a_name_of_white_space_alone_is_refused_as_empty() {
    let blank_names = [
        " ", "   ", "\u{00A0}", "\u{1680}", "\u{2003}", "\u{202F}", "\u{205F}", "\u{3000}",
    ];
    for name in blank_names {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Empty),
            "{name:?}"
        );
    }
}

#[test]
fn a_name_of_fillers_alone_is_refused_as_empty() {
    let filler_names = [
        "\u{115F}",
        "\u{1160}",
        "\u{3164}",
        "\u{FFA0}",
        "\u{2800}",
        "\u{3164}\u{3164}",
    ];
    for name in filler_names {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Empty),
            "{name:?}"
        );
    }
}

#[test]
fn a_name_mixing_white_space_fillers_and_joiners_alone_is_refused_as_empty() {
    let mixed = " \u{3164}\u{200D}\u{2800} \u{200C}";
    assert_eq!(NodeName::try_from(mixed), Err(NodeNameError::Empty));
}

#[test]
fn white_space_and_fillers_beside_a_visible_character_are_allowed() {
    let names = ["Old sketch", " a ", "a\u{3000}b", "\u{3164}a", "a\u{2800}"];
    for name in names {
        assert!(NodeName::try_from(name).is_ok(), "{name:?}");
    }
}

#[test]
fn line_and_paragraph_separators_are_refused_anywhere() {
    let names = ["a\u{2028}b", "a\u{2029}b", "\u{2028}", "ab\u{2029}"];
    for name in names {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Separator),
            "{name:?}"
        );
    }
}

/// Asserts that `name` is refused as [`NodeNameError::Empty`].
fn assert_empty(name: &str) {
    assert_eq!(
        NodeName::try_from(name),
        Err(NodeNameError::Empty),
        "{name:?}"
    );
}

#[test]
fn braille_pattern_blank_alone_is_refused_as_empty() {
    assert_empty("\u{2800}");
}

#[test]
fn hangul_filler_alone_is_refused_as_empty() {
    assert_empty("\u{3164}");
}

#[test]
fn hangul_choseong_filler_alone_is_refused_as_empty() {
    assert_empty("\u{115F}");
}

#[test]
fn hangul_jungseong_filler_alone_is_refused_as_empty() {
    assert_empty("\u{1160}");
}

#[test]
fn halfwidth_hangul_filler_alone_is_refused_as_empty() {
    assert_empty("\u{FFA0}");
}

#[test]
fn musical_symbol_null_notehead_alone_is_refused_as_empty() {
    assert_empty("\u{1D159}");
}

#[test]
fn khitan_small_script_filler_alone_is_refused_as_empty() {
    assert_empty("\u{16FE4}");
}

#[test]
fn the_egyptian_hieroglyph_blanks_alone_are_refused_as_empty() {
    assert_empty("\u{13441}");
    assert_empty("\u{13442}");
}

#[test]
fn default_ignorable_characters_alone_are_refused_as_empty() {
    let ignorable_names = [
        "\u{034F}",
        "\u{17B4}",
        "\u{17B5}",
        "\u{180B}",
        "\u{180F}",
        "\u{FE00}",
        "\u{FE0F}",
        "\u{E0100}",
        "\u{E01EF}",
        "\u{2065}",
        "\u{FFF0}",
        "\u{E0000}",
        "\u{E0FFF}",
    ];
    for name in ignorable_names {
        assert_empty(name);
    }
}

#[test]
fn invisible_format_characters_alone_are_refused_as_empty() {
    for name in ["\u{200B}", "\u{FEFF}", "\u{202E}", "\u{2060}", "\u{00AD}"] {
        assert_empty(name);
    }
}

#[test]
fn control_white_space_alone_is_refused_as_empty() {
    for name in ["\t", "\n", "\r\n", "\u{000B}", "\u{000C}", "\u{0085}"] {
        assert_empty(name);
    }
}

#[test]
fn mixed_blank_names_are_refused_as_empty() {
    let mixed_names = [
        "\t\u{3164}",
        " \u{034F}\u{3164}",
        "\u{FE0F}\u{2800}\t",
        "\u{200B}\u{2800}\u{200D}",
        "\u{1D159} \u{FFA0}\u{E0100}",
        "\u{115F}\u{1160}\u{16FE4}\u{13441}",
    ];
    for name in mixed_names {
        assert_empty(name);
    }
}

#[test]
fn blank_characters_the_other_rules_allow_beside_a_visible_one_are_accepted() {
    let names = [
        "\u{2764}\u{FE0F}",
        "a\u{034F}b",
        "\u{1D159}a",
        "a\u{16FE4}",
        "\u{13441}a",
        "a\u{E0100}",
    ];
    for name in names {
        assert!(NodeName::try_from(name).is_ok(), "{name:?}");
    }
}

#[test]
fn an_invisible_character_inside_a_name_is_still_refused_as_invisible() {
    let names = [
        "a\tb",
        "a\nb",
        "a\u{0}b",
        "a\u{202E}b",
        "a\u{00AD}b",
        "\u{0}\u{3164}",
    ];
    for name in names {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Invisible),
            "{name:?}"
        );
    }
}

#[test]
fn a_separator_in_an_otherwise_blank_name_is_refused_as_a_separator() {
    for name in ["\u{2028}\u{3164}", " \u{2029}", "\u{2800}\u{2028}\u{2800}"] {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Separator),
            "{name:?}"
        );
    }
}

#[test]
fn the_default_ignorable_table_matches_its_known_edges() {
    let inside = [
        '\u{00AD}',
        '\u{034F}',
        '\u{115F}',
        '\u{1160}',
        '\u{200B}',
        '\u{200F}',
        '\u{3164}',
        '\u{FFA0}',
        '\u{FFF8}',
        '\u{E0000}',
        '\u{E0FFF}',
    ];
    let outside = [
        '\u{00AC}',
        '\u{034E}',
        '\u{1161}',
        '\u{17B6}',
        '\u{1810}',
        '\u{2800}',
        '\u{FFF9}',
        '\u{E1000}',
        ' ',
        'a',
    ];
    for character in inside {
        assert!(is_default_ignorable(character), "{character:?}");
    }
    for character in outside {
        assert!(!is_default_ignorable(character), "{character:?}");
    }
}

#[test]
fn each_visible_filler_is_neither_white_space_nor_default_ignorable() {
    for &filler in VISIBLE_FILLERS {
        assert!(!filler.is_whitespace(), "{filler:?}");
        assert!(!is_default_ignorable(filler), "{filler:?}");
    }
}

/// Asserts that `name` parses, and that what it stores is `stored`.
fn assert_stored(name: &str, stored: &str) {
    let parsed = NodeName::try_from(name).map(|name| name.to_string());
    assert_eq!(parsed, Ok(stored.to_owned()), "{name:?}");
}

#[test]
fn white_space_at_both_ends_is_stripped() {
    assert_stored("  a  ", "a");
    assert_stored("\ta\n", "a");
    assert_stored("\u{3000}photo.png\u{00A0}", "photo.png");
}

#[test]
fn fillers_at_both_ends_are_stripped() {
    assert_stored("\u{3164}a\u{2800}", "a");
    assert_stored("\u{115F}\u{1160}a\u{FFA0}\u{1D159}", "a");
}

#[test]
fn invisible_format_characters_at_both_ends_are_stripped() {
    assert_stored("\u{202E}a\u{200B}", "a");
    assert_stored("\u{FEFF}a\u{00AD}", "a");
    assert_stored("\u{2066}a\u{2069}", "a");
}

#[test]
fn a_joiner_or_selector_at_the_start_is_stripped() {
    assert_stored("\u{200D}a", "a");
    assert_stored("\u{FE0F}a", "a");
    assert_stored("\u{200C}\u{E0100}a", "a");
}

#[test]
fn a_joiner_or_selector_after_the_last_character_is_kept() {
    assert_stored("\u{2764}\u{FE0F}", "\u{2764}\u{FE0F}");
    assert_stored("\u{0915}\u{094D}\u{200D}", "\u{0915}\u{094D}\u{200D}");
    assert_stored("\u{1820}\u{180B}", "\u{1820}\u{180B}");
    assert_stored("\u{2764}\u{FE0F} \u{3164}", "\u{2764}\u{FE0F}");
}

#[test]
fn a_joiner_or_selector_after_white_space_at_the_end_is_stripped() {
    assert_stored("a \u{200D}", "a");
    assert_stored("a\u{3164}\u{FE0F}", "a");
}

#[test]
fn fillers_inside_a_name_are_stripped() {
    assert_stored("a\u{3164}b", "ab");
    assert_stored("a\u{2800}b", "ab");
    assert_stored("a\u{115F}\u{1160}\u{FFA0}b", "ab");
    assert_stored("a\u{1D159}\u{16FE4}\u{13441}\u{13442}b", "ab");
}

#[test]
fn zero_width_spaces_inside_a_name_are_stripped() {
    assert_stored("a\u{200B}b", "ab");
    assert_stored("a\u{2060}b", "ab");
    assert_stored("a\u{FEFF}b", "ab");
    assert_stored("a\u{200B}", "a");
}

#[test]
fn ordinary_spaces_inside_a_name_are_kept() {
    assert_stored("Old sketch", "Old sketch");
    assert_stored("a  b", "a  b");
}

#[test]
fn other_space_separators_inside_a_name_are_kept() {
    assert_stored("a\u{00A0}b", "a\u{00A0}b");
    assert_stored("a\u{3000}b", "a\u{3000}b");
    assert_stored("a\u{2003}b", "a\u{2003}b");
}

#[test]
fn emoji_sequences_are_kept_whole() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    assert_stored("\u{1F468}\u{200D}\u{1F469}", "\u{1F468}\u{200D}\u{1F469}");
    assert_stored(family, family);
    assert_stored("\u{2764}\u{FE0F}", "\u{2764}\u{FE0F}");
    assert_stored(
        "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",
        "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}",
    );
}

#[test]
fn a_persian_word_keeps_its_non_joiner() {
    let word = "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}";
    assert_stored(word, word);
}

#[test]
fn selectors_inside_a_name_are_kept() {
    assert_stored("a\u{E0100}b", "a\u{E0100}b");
    assert_stored("\u{1820}\u{180B}\u{1821}", "\u{1820}\u{180B}\u{1821}");
}

#[test]
fn a_name_blank_once_stripped_is_refused_as_empty() {
    let names = [
        "\t\u{3164}",
        "\u{200D}\u{FE0F}",
        " \u{200B} ",
        "\u{FE0F}\u{2800}\u{E0100}",
    ];
    for name in names {
        assert_empty(name);
    }
}

#[test]
fn a_tab_inside_a_name_is_refused() {
    assert_eq!(NodeName::try_from("a\tb"), Err(NodeNameError::Invisible));
}

#[test]
fn a_separator_is_refused_even_where_it_would_be_stripped() {
    for name in ["\u{2028}a", "a\u{2029}", " \u{2028} a"] {
        assert_eq!(
            NodeName::try_from(name),
            Err(NodeNameError::Separator),
            "{name:?}"
        );
    }
}

#[test]
fn the_stripped_form_parses_to_itself() {
    let names = [
        "  a  ",
        "a\u{3164}b",
        "\u{2764}\u{FE0F} ",
        "\u{200D}x\u{200B}y\u{200D}",
    ];
    for name in names {
        let stripped = NodeName::try_from(name).expect("a name that strips to something");
        let reparsed = NodeName::try_from(stripped.as_ref());
        assert_eq!(reparsed, Ok(stripped.clone()), "{name:?}");
    }
}

#[test]
fn from_str_and_try_from_string_strip_alike() {
    let from_str: Result<NodeName, _> = " a\u{3164}b ".parse();
    let from_string = NodeName::try_from(String::from(" a\u{3164}b "));
    assert_eq!(from_str, from_string);
    assert_eq!(from_str.map(|name| name.to_string()), Ok("ab".to_owned()));
}

/// The flag of Scotland: a black flag, the tag letters "gbsct", and the cancel tag.
const SCOTLAND: &str = "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}";

/// Asserts that `name` is refused as [`NodeNameError::Invisible`].
fn assert_invisible(name: &str) {
    assert_eq!(
        NodeName::try_from(name),
        Err(NodeNameError::Invisible),
        "{name:?}"
    );
}

#[test]
fn a_subdivision_flag_alone_is_kept() {
    assert_stored(SCOTLAND, SCOTLAND);
}

#[test]
fn a_subdivision_flag_at_the_end_is_kept() {
    let name = format!("Highlands {SCOTLAND}");
    let padded = format!("Highlands {SCOTLAND} \u{3164}");
    assert_stored(&name, &name);
    assert_stored(&padded, &name);
}

#[test]
fn a_subdivision_flag_inside_a_name_is_kept() {
    let name = format!("{SCOTLAND} sketches");
    let two_flags = format!("{SCOTLAND}{SCOTLAND}");
    assert_stored(&name, &name);
    assert_stored(&two_flags, &two_flags);
}

#[test]
fn tags_not_after_a_black_flag_are_stripped_at_the_end_and_refused_inside() {
    assert_stored("a\u{E0067}\u{E007F}", "a");
    assert_invisible("a\u{E0067}\u{E007F}b");
}

#[test]
fn a_tag_sequence_without_its_cancel_tag_is_not_a_flag() {
    assert_stored("\u{1F3F4}\u{E0067}\u{E0062}", "\u{1F3F4}");
    assert_invisible("\u{1F3F4}\u{E0067}\u{E0062} a");
}

#[test]
fn a_cancel_tag_with_no_tag_letters_is_not_a_flag() {
    assert_stored("\u{1F3F4}\u{E007F}", "\u{1F3F4}");
    assert_invisible("\u{1F3F4}\u{E007F}a");
}

#[test]
fn a_mongolian_vowel_separator_inside_a_mongolian_word_is_kept() {
    let khara = "\u{182C}\u{1820}\u{1837}\u{180E}\u{1820}";
    let after_a_selector = "\u{1828}\u{180B}\u{180E}\u{1820}";
    assert_stored(khara, khara);
    assert_stored(after_a_selector, after_a_selector);
}

#[test]
fn a_mongolian_vowel_separator_outside_a_mongolian_word_is_refused() {
    assert_invisible("a\u{180E}b");
    assert_invisible("\u{1820}\u{180E}b");
    assert_invisible("a\u{180E}\u{1820}");
}

#[test]
fn a_mongolian_vowel_separator_at_the_ends_is_stripped() {
    assert_stored("\u{180E}\u{1820}\u{180E}", "\u{1820}");
}

#[test]
fn an_unassigned_default_ignorable_inside_a_name_is_refused() {
    let names = [
        "a\u{2065}b",
        "a\u{FFF0}b",
        "a\u{FFF8}b",
        "a\u{E0000}b",
        "a\u{E0002}b",
        "a\u{E0080}b",
        "a\u{E01F0}b",
        "a\u{E0FFF}b",
    ];
    for name in names {
        assert_invisible(name);
    }
}

#[test]
fn an_unassigned_default_ignorable_at_the_ends_is_stripped() {
    assert_stored("\u{2065}a\u{FFF8}", "a");
    assert_stored("\u{E0000}a\u{E0FFF}", "a");
}

#[test]
fn the_unassigned_default_ignorable_table_matches_its_known_edges() {
    let inside = [
        '\u{2065}',
        '\u{FFF0}',
        '\u{FFF8}',
        '\u{E0000}',
        '\u{E0002}',
        '\u{E001F}',
        '\u{E0080}',
        '\u{E00FF}',
        '\u{E01F0}',
        '\u{E0FFF}',
    ];
    let outside = [
        '\u{2064}',
        '\u{2066}',
        '\u{FFF9}',
        '\u{E0001}',
        '\u{E0020}',
        '\u{E007F}',
        '\u{E0100}',
        '\u{E01EF}',
        'a',
    ];
    for character in inside {
        assert!(is_unassigned_default_ignorable(character), "{character:?}");
    }
    for character in outside {
        assert!(!is_unassigned_default_ignorable(character), "{character:?}");
    }
}

#[test]
fn every_unassigned_default_ignorable_is_default_ignorable_and_not_a_format_character() {
    let ranges = UNASSIGNED_DEFAULT_IGNORABLE
        .iter()
        .flat_map(|&(first, last)| first..=last)
        .filter_map(char::from_u32);
    for character in ranges {
        assert!(is_default_ignorable(character), "{character:?}");
        assert!(!is_format(character), "{character:?}");
    }
}

#[test]
fn the_toolchain_unicode_version_is_one_the_tables_were_checked_against() {
    let checked_versions = [(16, 0, 0), (17, 0, 0), (18, 0, 0)];
    assert!(
        checked_versions.contains(&char::UNICODE_VERSION),
        "std is on Unicode {:?}: re-check FORMAT_CHARACTERS, DEFAULT_IGNORABLE and \
         UNASSIGNED_DEFAULT_IGNORABLE against its UCD, then add the version here",
        char::UNICODE_VERSION
    );
}
