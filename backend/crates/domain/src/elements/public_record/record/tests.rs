use super::*;
use crate::elements::public_record::SelfLabels;

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
