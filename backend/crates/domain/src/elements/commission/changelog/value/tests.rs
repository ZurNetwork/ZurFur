use super::*;

// The pointer gate: trims, rejects blank/oversized/control input, and
// applies no scheme allowlist.
#[test]
fn channel_pointer_validates_shape_but_not_scheme() {
    assert_eq!(
        " https://t.me/x "
            .parse::<ChannelPointer>()
            .unwrap()
            .as_str(),
        "https://t.me/x",
    );
    // No scheme allowlist — a bare handle is a fine pointer.
    assert!("@artist on Telegram".parse::<ChannelPointer>().is_ok());
    assert_eq!(
        "   ".parse::<ChannelPointer>(),
        Err(ChannelPointerError::Empty)
    );
    assert_eq!(
        ChannelPointer::try_from("x".repeat(ChannelPointer::MAX_CHARS + 1)),
        Err(ChannelPointerError::TooLong)
    );
    // Exactly at the cap is fine.
    assert!(ChannelPointer::try_from("x".repeat(ChannelPointer::MAX_CHARS)).is_ok());
    for bad in ["a\nb", "a\tb", "a\rb", "a\0b"] {
        assert_eq!(
            bad.parse::<ChannelPointer>(),
            Err(ChannelPointerError::ControlCharacter),
            "control characters are rejected: {bad:?}",
        );
    }
}
