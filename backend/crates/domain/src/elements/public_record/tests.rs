use super::*;

#[test]
fn at_uri_round_trips_through_string() {
    let uri = AtUri::new(
        Did::from("did:plc:abc123".to_string()),
        Nsid::new("app.zurfur.feed.post"),
        RecordKey::new("3laa7lepk2c"),
    );
    let s = uri.to_string();
    assert_eq!(s, "at://did:plc:abc123/app.zurfur.feed.post/3laa7lepk2c");
    assert_eq!(AtUri::parse(&s).unwrap(), uri);
}

#[test]
fn at_uri_parse_rejects_malformed() {
    assert_eq!(
        AtUri::parse("did:plc:abc/app.zurfur.feed.post/rk"),
        Err(AtUriParseError::MissingScheme)
    );
    assert_eq!(
        AtUri::parse("at://did:plc:abc/app.zurfur.feed.post"),
        Err(AtUriParseError::Malformed)
    );
}

#[test]
fn at_uri_parse_rejects_query_and_fragment() {
    assert_eq!(
        AtUri::parse("at://did:plc:abc/app.zurfur.feed.post/3laa7lepk2c?x=1"),
        Err(AtUriParseError::Malformed)
    );
    assert_eq!(
        AtUri::parse("at://did:plc:abc/app.zurfur.feed.post/3laa7lepk2c#frag"),
        Err(AtUriParseError::Malformed)
    );
}

#[test]
fn feed_post_fixes_its_collection() {
    let record = PublicRecord::FeedPost(FeedPost {
        text: Some("hi".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: chrono::Utc::now(),
    });
    assert_eq!(record.collection().as_str(), "app.zurfur.feed.post");
}
