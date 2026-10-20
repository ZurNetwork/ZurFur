use super::*;

/// The nine explicit bidirectional formatting characters.
const BIDI_CONTROLS: [char; 9] = [
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}',
];

/// Characters beside the nine that the bidi rule lets through. U+2029, the nine's other
/// neighbour, is a paragraph separator, which the one-line values refuse on their own rule.
const BIDI_NEIGHBOURS: [char; 3] = ['\u{202F}', '\u{2065}', '\u{206A}'];

/// LINE SEPARATOR and PARAGRAPH SEPARATOR.
const LINE_SEPARATORS: [char; 2] = ['\u{2028}', '\u{2029}'];

/// Text that is empty once trimmed.
const BLANKS: [&str; 4] = ["", " ", "  \u{3000} ", "\u{00A0}"];

/// What each text value makes of `raw`: `Ok` when it accepts it, else whether its refusal is a
/// control character, in the order description, page, page kind, build id, browser.
fn every_text_value(raw: &str) -> [Result<(), bool>; 5] {
    [
        BugDescription::try_from(raw)
            .map(drop)
            .map_err(|error| error == BugDescriptionError::ControlCharacter),
        PageAddress::try_from(raw)
            .map(drop)
            .map_err(|error| error == PageAddressError::ControlCharacter),
        PageKind::try_from(raw)
            .map(drop)
            .map_err(|error| error == PageKindError::ControlCharacter),
        BuildId::try_from(raw)
            .map(drop)
            .map_err(|error| error == BuildIdError::ControlCharacter),
        BrowserText::try_from(raw)
            .map(drop)
            .map_err(|error| error == BrowserTextError::ControlCharacter),
    ]
}

/// What each text value makes of `raw`: `Ok` when it accepts it, else whether its refusal is
/// `Empty`, in the order description, page, page kind, build id, browser.
fn every_text_value_on_empty(raw: &str) -> [Result<(), bool>; 5] {
    [
        BugDescription::try_from(raw)
            .map(drop)
            .map_err(|error| error == BugDescriptionError::Empty),
        PageAddress::try_from(raw)
            .map(drop)
            .map_err(|error| error == PageAddressError::Empty),
        PageKind::try_from(raw)
            .map(drop)
            .map_err(|error| error == PageKindError::Empty),
        BuildId::try_from(raw)
            .map(drop)
            .map_err(|error| error == BuildIdError::Empty),
        BrowserText::try_from(raw)
            .map(drop)
            .map_err(|error| error == BrowserTextError::Empty),
    ]
}

/// Every text value refused the text as a control character.
const ALL_REFUSED_AS_CONTROL: [Result<(), bool>; 5] = [Err(true); 5];

/// Every text value accepted the text.
const ALL_ACCEPTED: [Result<(), bool>; 5] = [Ok(()); 5];

#[test]
fn every_text_value_refuses_each_of_the_nine_bidi_controls() {
    for bidi_control in BIDI_CONTROLS {
        let raw = format!("/a{bidi_control}b");
        assert_eq!(
            every_text_value(&raw),
            ALL_REFUSED_AS_CONTROL,
            "U+{:04X} is refused by every text value",
            u32::from(bidi_control)
        );
    }
}

#[test]
fn every_text_value_lets_the_bidi_neighbours_through() {
    for neighbour in BIDI_NEIGHBOURS {
        let raw = format!("/a{neighbour}b");
        assert_eq!(
            every_text_value(&raw),
            ALL_ACCEPTED,
            "U+{:04X} passes every text value",
            u32::from(neighbour)
        );
    }
}

#[test]
fn every_text_value_refuses_escape_and_a_c1_control() {
    for control in ['\u{001B}', '\u{009B}', '\0', '\u{007F}', '\r'] {
        let raw = format!("/a{control}b");
        assert_eq!(
            every_text_value(&raw),
            ALL_REFUSED_AS_CONTROL,
            "U+{:04X} is refused by every text value",
            u32::from(control)
        );
    }
}

#[test]
fn every_text_value_refuses_empty_and_whitespace_only_text() {
    let all_refused_as_empty = [Err(true); 5];
    for blank in BLANKS {
        assert_eq!(
            every_text_value_on_empty(blank),
            all_refused_as_empty,
            "{blank:?} is empty"
        );
    }
}

#[test]
fn every_text_value_trims() {
    let page = PageAddress::try_from("  /den/commissions \t").expect("a page");
    let page_kind = PageKind::try_from(" den ").expect("a page kind");
    let build_id = BuildId::try_from(" 0.1.1 ").expect("a build id");
    let browser = BrowserText::try_from(" Firefox/131.0 ").expect("a browser");
    let description = BugDescription::try_from("\n  It broke.\n\n").expect("a description");

    assert_eq!(page.as_ref(), "/den/commissions");
    assert_eq!(page_kind.as_ref(), "den");
    assert_eq!(build_id.as_ref(), "0.1.1");
    assert_eq!(browser.as_ref(), "Firefox/131.0");
    assert_eq!(description.to_string(), "It broke.");
}

#[test]
fn the_description_keeps_newline_and_tab() {
    let description = BugDescription::try_from("Steps:\n\t1. open\n\t2. click").expect("kept");

    assert_eq!(description.as_ref(), "Steps:\n\t1. open\n\t2. click");
}

#[test]
fn the_description_refuses_carriage_return() {
    let refused = BugDescription::try_from("one\r\ntwo");

    assert_eq!(refused, Err(BugDescriptionError::ControlCharacter));
}

#[test]
fn the_one_line_values_refuse_newline_and_tab() {
    for control in ['\n', '\t'] {
        let raw = format!("/a{control}b");
        let [_, page, page_kind, build_id, browser] = every_text_value(&raw);
        assert_eq!(
            [page, page_kind, build_id, browser],
            [Err(true); 4],
            "U+{:04X} is refused in a one-line value",
            u32::from(control)
        );
    }
}

#[test]
fn the_one_line_values_refuse_the_line_and_paragraph_separators() {
    for separator in LINE_SEPARATORS {
        let raw = format!("/a{separator}b");
        let page = PageAddress::try_from(raw.as_str());
        let page_kind = PageKind::try_from(raw.as_str());
        let build_id = BuildId::try_from(raw.as_str());
        let browser = BrowserText::try_from(raw.as_str());

        let code_point = u32::from(separator);
        assert_eq!(
            page,
            Err(PageAddressError::ControlCharacter),
            "U+{code_point:04X} in a page"
        );
        assert_eq!(
            page_kind,
            Err(PageKindError::ControlCharacter),
            "U+{code_point:04X} in a page kind"
        );
        assert_eq!(
            build_id,
            Err(BuildIdError::ControlCharacter),
            "U+{code_point:04X} in a build id"
        );
        assert_eq!(
            browser,
            Err(BrowserTextError::ControlCharacter),
            "U+{code_point:04X} in a browser"
        );
    }
}

#[test]
fn the_description_keeps_the_line_and_paragraph_separators() {
    for separator in LINE_SEPARATORS {
        let raw = format!("one{separator}two");

        let description = BugDescription::try_from(raw.as_str());

        let kept = description.map(|kept| kept.to_string());
        assert_eq!(kept, Ok(raw), "U+{:04X}", u32::from(separator));
    }
}

#[test]
fn the_description_holds_4000_code_points_and_no_more() {
    let at_cap = "\u{1F98A}".repeat(4000);
    let over_cap = "\u{1F98A}".repeat(4001);

    assert!(BugDescription::try_from(at_cap).is_ok());
    assert_eq!(
        BugDescription::try_from(over_cap),
        Err(BugDescriptionError::TooLong)
    );
}

#[test]
fn the_page_holds_2048_bytes_and_no_more() {
    let at_cap = format!("/{}", "a".repeat(2047));
    let over_cap = format!("/{}", "a".repeat(2048));
    let over_cap_in_bytes_only = format!("/{}", "\u{00E9}".repeat(1024));

    assert!(PageAddress::try_from(at_cap).is_ok());
    assert_eq!(
        PageAddress::try_from(over_cap),
        Err(PageAddressError::TooLong)
    );
    assert_eq!(
        PageAddress::try_from(over_cap_in_bytes_only),
        Err(PageAddressError::TooLong)
    );
}

#[test]
fn the_page_kind_holds_64_bytes_and_no_more() {
    let at_cap = "k".repeat(64);
    let over_cap = "k".repeat(65);
    let over_cap_in_bytes_only = "\u{00E9}".repeat(33);

    assert!(PageKind::try_from(at_cap).is_ok());
    assert_eq!(PageKind::try_from(over_cap), Err(PageKindError::TooLong));
    assert_eq!(
        PageKind::try_from(over_cap_in_bytes_only),
        Err(PageKindError::TooLong)
    );
}

#[test]
fn the_build_id_holds_64_bytes_and_no_more() {
    let at_cap = "v".repeat(64);
    let over_cap = "v".repeat(65);
    let over_cap_in_bytes_only = "\u{00E9}".repeat(33);

    assert!(BuildId::try_from(at_cap).is_ok());
    assert_eq!(BuildId::try_from(over_cap), Err(BuildIdError::TooLong));
    assert_eq!(
        BuildId::try_from(over_cap_in_bytes_only),
        Err(BuildIdError::TooLong)
    );
}

#[test]
fn the_browser_holds_512_bytes_and_no_more() {
    let at_cap = "b".repeat(512);
    let over_cap = "b".repeat(513);
    let over_cap_in_bytes_only = "\u{00E9}".repeat(257);

    assert!(BrowserText::try_from(at_cap).is_ok());
    assert_eq!(
        BrowserText::try_from(over_cap),
        Err(BrowserTextError::TooLong)
    );
    assert_eq!(
        BrowserText::try_from(over_cap_in_bytes_only),
        Err(BrowserTextError::TooLong)
    );
}

#[test]
fn the_page_starts_with_a_slash() {
    let refused = PageAddress::try_from("den/commissions");

    assert_eq!(refused, Err(PageAddressError::NotAbsolute));
}

#[test]
fn the_page_refuses_a_leading_double_slash() {
    let protocol_relative = PageAddress::try_from("//evil.example/x");
    let double_slash_inside = PageAddress::try_from("/den//x");

    assert_eq!(protocol_relative, Err(PageAddressError::ProtocolRelative));
    let kept = double_slash_inside.map(|page| page.to_string());
    assert_eq!(kept, Ok("/den//x".to_owned()));
}

#[test]
fn the_page_refuses_any_backslash() {
    let another_host = PageAddress::try_from("/\\evil.example/x");
    let inside_a_path = PageAddress::try_from("/a\\b");
    let a_lone_backslash = PageAddress::try_from("\\");

    let backslash_error = Err(PageAddressError::Backslash);
    assert_eq!(another_host, backslash_error);
    assert_eq!(inside_a_path, backslash_error);
    assert_eq!(a_lone_backslash, backslash_error);
}

#[test]
fn the_page_refuses_a_query_and_a_fragment() {
    let with_a_query = PageAddress::try_from("/den?includeDeleted=true");
    let with_a_fragment = PageAddress::try_from("/den#top");
    let with_a_bare_question_mark = PageAddress::try_from("/den?");

    assert_eq!(with_a_query, Err(PageAddressError::QueryOrFragment));
    assert_eq!(with_a_fragment, Err(PageAddressError::QueryOrFragment));
    assert_eq!(
        with_a_bare_question_mark,
        Err(PageAddressError::QueryOrFragment)
    );
}

#[test]
fn a_window_side_is_1_to_100000() {
    let accepted = [1, 100_000].map(WindowSide::try_from);
    let refused = [0, 100_001, -1, i32::MIN].map(WindowSide::try_from);

    let one = accepted[0].map(u32::from);
    let most = accepted[1].map(u32::from);
    assert_eq!(one, Ok(1));
    assert_eq!(most, Ok(100_000));
    assert_eq!(refused, [Err(WindowSideError::OutOfRange); 4]);
}

#[test]
fn an_error_status_is_400_to_599() {
    let accepted = [400, 599].map(ErrorStatus::try_from);
    let refused = [399, 600, -1, 65_936].map(ErrorStatus::try_from);

    let lowest = accepted[0].map(u16::from);
    let highest = accepted[1].map(u16::from);
    assert_eq!(lowest, Ok(400));
    assert_eq!(highest, Ok(599));
    assert_eq!(refused, [Err(ErrorStatusError::OutOfRange); 4]);
}

#[test]
fn no_error_carries_the_input() {
    let secret = "/hunter2\u{202E}";

    let messages = [
        BugDescription::try_from(secret).unwrap_err().to_string(),
        PageAddress::try_from(secret).unwrap_err().to_string(),
        PageKind::try_from(secret).unwrap_err().to_string(),
        BuildId::try_from(secret).unwrap_err().to_string(),
        BrowserText::try_from(secret).unwrap_err().to_string(),
    ];
    for message in messages {
        assert!(
            !message.contains("hunter2"),
            "{message:?} carries the input"
        );
    }
}
