use super::*;

#[test]
fn only_exactly_atproto_is_identity_only() {
    let cases = [
        (Some("atproto"), true),
        (Some(" atproto "), true),
        (Some("atproto transition:generic"), false),
        (Some("transition:generic atproto"), false),
        (Some("atproto repo:app.bsky.feed.post"), false),
        (Some("transition:generic"), false),
        (Some(""), false),
        (None, false),
    ];

    for (granted, expected) in cases {
        assert_eq!(
            grants_identity_only(granted),
            expected,
            "the verdict on {granted:?}"
        );
    }
}
