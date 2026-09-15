use super::*;

// The kind vocabulary is open: any label wraps, trimmed.
#[test]
fn seat_kind_is_an_open_trimmed_vocabulary() {
    assert_eq!(
        "  Creator ".parse::<SeatKind>().unwrap().as_str(),
        "Creator"
    );
    // Not a Role, not a closed list — arbitrary labels are fine.
    assert!("Background artist".parse::<SeatKind>().is_ok());
    assert!("客户".parse::<SeatKind>().is_ok());

    assert_eq!("   ".parse::<SeatKind>(), Err(SeatKindError::Empty));
    assert_eq!(
        SeatKind::try_from("x".repeat(SeatKind::MAX_CHARS + 1)),
        Err(SeatKindError::TooLong)
    );
    assert!(SeatKind::try_from("x".repeat(SeatKind::MAX_CHARS)).is_ok());
    assert_eq!(
        "a\nb".parse::<SeatKind>(),
        Err(SeatKindError::ControlCharacter)
    );
}

// The prompt is multi-line free text: newlines/tabs pass, other control
// characters and blank/oversized input refuse.
#[test]
fn seat_prompt_allows_lines_but_not_injection() {
    let prompt = " Provide:\n\t- two refs\n\t- your rate "
        .parse::<SeatPrompt>()
        .unwrap();
    assert_eq!(prompt.as_str(), "Provide:\n\t- two refs\n\t- your rate");

    assert_eq!("   ".parse::<SeatPrompt>(), Err(SeatPromptError::Empty));
    assert_eq!(
        SeatPrompt::try_from("x".repeat(SeatPrompt::MAX_CHARS + 1)),
        Err(SeatPromptError::TooLong)
    );
    assert!(SeatPrompt::try_from("x".repeat(SeatPrompt::MAX_CHARS)).is_ok());
    assert_eq!(
        "a\0b".parse::<SeatPrompt>(),
        Err(SeatPromptError::ControlCharacter)
    );
    assert_eq!(
        "a\u{1b}b".parse::<SeatPrompt>(),
        Err(SeatPromptError::ControlCharacter)
    );
}

// The link is an opaque pointer: no scheme allowlist, no control chars.
#[test]
fn seat_link_validates_shape_but_not_scheme() {
    assert_eq!(
        " https://forms.example/apply "
            .parse::<SeatLink>()
            .unwrap()
            .as_str(),
        "https://forms.example/apply"
    );
    // No scheme allowlist — a bare pointer is fine.
    assert!("form on my carrd".parse::<SeatLink>().is_ok());

    assert_eq!("   ".parse::<SeatLink>(), Err(SeatLinkError::Empty));
    assert_eq!(
        SeatLink::try_from("x".repeat(SeatLink::MAX_CHARS + 1)),
        Err(SeatLinkError::TooLong)
    );
    assert_eq!(
        "a\tb".parse::<SeatLink>(),
        Err(SeatLinkError::ControlCharacter)
    );
}
