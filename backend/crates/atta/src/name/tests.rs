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
        '\u{200B}',
        '\u{200E}',
        '\u{200F}',
        '\u{202A}',
        '\u{202E}',
        '\u{2066}',
        '\u{2069}',
        '\u{FEFF}',
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
fn blank_characters_beside_a_visible_character_are_allowed() {
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
fn an_invisible_character_beside_a_visible_one_is_still_refused_as_invisible() {
    for name in ["\ta", "a\u{200B}", "\u{FEFF}a", "\u{0}\u{3164}"] {
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
