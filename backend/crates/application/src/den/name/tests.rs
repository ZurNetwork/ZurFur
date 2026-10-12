use super::*;

#[test]
fn ordinary_text_is_a_name() {
    let name = DenName::try_from("Untitled");

    assert_eq!(name.as_ref().map(AsRef::as_ref), Ok("Untitled"));
}

#[test]
fn an_emoji_sequence_joined_inside_is_a_name() {
    let artist = "\u{1F469}\u{200D}\u{1F3A8}";

    let name = DenName::try_from(artist);

    assert_eq!(name.as_ref().map(AsRef::as_ref), Ok(artist));
}

#[test]
fn joiners_alone_are_an_empty_name() {
    for text in [
        "\u{200D}",
        "\u{200D}\u{200D}",
        "\u{200C}",
        "\u{200C}\u{200C}",
    ] {
        let name = DenName::try_from(text);

        assert_eq!(name, Err(DenNameError::Empty), "{text:?}");
    }
}

#[test]
fn a_slash_is_refused() {
    let name = DenName::try_from("left/right");

    assert_eq!(name, Err(DenNameError::Slash));
}

#[test]
fn a_control_character_is_refused() {
    let name = DenName::try_from("bell\u{0007}name");

    assert_eq!(name, Err(DenNameError::Invisible));
}

#[test]
fn a_bidirectional_override_is_refused() {
    let name = DenName::try_from("evil\u{202E}txt.exe");

    assert_eq!(name, Err(DenNameError::Invisible));
}

#[test]
fn the_three_doors_agree() {
    let text = "Old sketch";

    let from_str = text.parse::<DenName>();
    let from_borrowed = DenName::try_from(text);
    let from_owned = DenName::try_from(text.to_owned());

    assert_eq!(from_str, from_borrowed);
    assert_eq!(from_borrowed, from_owned);
}

#[test]
fn a_refusal_carries_no_part_of_the_text() {
    let secret = "secret/name";

    let refusal = DenName::try_from(secret)
        .err()
        .map(|error| error.to_string());

    assert!(refusal.is_some_and(|message| !message.contains("secret")));
}
