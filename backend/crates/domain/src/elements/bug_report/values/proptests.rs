use super::*;
use proptest::prelude::*;

/// One of the nine explicit bidirectional formatting characters.
fn bidi_control() -> impl Strategy<Value = char> {
    prop::sample::select(vec![
        '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}',
        '\u{2068}', '\u{2069}',
    ])
}

proptest! {
    #[test]
    fn a_bidi_control_anywhere_is_refused_by_every_text_value(
        before in "[a-z/]{0,8}",
        control in bidi_control(),
        after in "[a-z]{0,8}",
    ) {
        let raw = format!("/{before}{control}{after}");

        prop_assert_eq!(BugDescription::try_from(raw.as_str()), Err(BugDescriptionError::ControlCharacter));
        prop_assert_eq!(PageAddress::try_from(raw.as_str()), Err(PageAddressError::ControlCharacter));
        prop_assert_eq!(PageKind::try_from(raw.as_str()), Err(PageKindError::ControlCharacter));
        prop_assert_eq!(BuildId::try_from(raw.as_str()), Err(BuildIdError::ControlCharacter));
        prop_assert_eq!(BrowserText::try_from(raw.as_str()), Err(BrowserTextError::ControlCharacter));
    }

    #[test]
    fn an_accepted_description_is_its_trimmed_input(raw in "\\PC{1,64}") {
        prop_assume!(!raw.trim().is_empty());
        prop_assume!(!raw.chars().any(crate::string_builder::is_bidi_control));

        let description = BugDescription::try_from(raw.as_str());

        prop_assert_eq!(description.map(|kept| kept.to_string()), Ok(raw.trim().to_owned()));
    }

    #[test]
    fn a_window_side_accepts_exactly_1_to_100000(raw in any::<i32>()) {
        let side = WindowSide::try_from(raw);

        prop_assert_eq!(side.is_ok(), (1..=100_000).contains(&raw));
    }

    #[test]
    fn an_error_status_accepts_exactly_400_to_599(raw in any::<i32>()) {
        let status = ErrorStatus::try_from(raw);

        prop_assert_eq!(status.is_ok(), (400..=599).contains(&raw));
    }
}
