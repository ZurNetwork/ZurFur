use super::*;
use crate::elements::{commission::CommissionId, did::Did, user::UserId};

// System vs event constructors set the actor arm explicitly.
#[test]
fn constructors_set_the_actor_arm() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let actor = UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())));
    let now = chrono::Utc::now();

    let event = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::Created,
        actor.clone(),
        serde_json::json!({}),
        now,
    );
    assert_eq!(event.actor_id, Some(actor.clone()));

    let system = NewChangelogEntry::system(
        commission,
        ChangelogEntryKind::Late,
        serde_json::json!({}),
        now,
    );
    assert_eq!(system.actor_id, None, "a system entry has no actor");

    let note = NewChangelogEntry::note(commission, actor.clone(), "hi".to_string(), now);
    assert!(matches!(note.kind, ChangelogEntryKind::Note));
    assert_eq!(note.note.as_deref(), Some("hi"));

    let attached = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::PhaseApproved,
        actor,
        serde_json::json!({ "phase": "lineart" }),
        now,
    )
    .with_note("love the colors!".to_string());
    assert_eq!(attached.note.as_deref(), Some("love the colors!"));
}
