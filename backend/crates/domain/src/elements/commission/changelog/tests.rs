use std::collections::BTreeSet;

use strum::VariantArray;

use super::*;
use crate::elements::did::Did;

// The storage tokens round-trip and never collide.
#[test]
fn kind_tokens_round_trip_and_never_collide() {
    let mut seen = BTreeSet::new();
    for kind in ChangelogEntryKind::VARIANTS {
        let token = <&'static str>::from(*kind);
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(
            token.parse::<ChangelogEntryKind>().ok(),
            Some(*kind),
            "token {token:?} must parse back to its kind",
        );
    }
}

// A token outside the vocabulary is refused, not guessed at.
#[test]
fn unknown_tokens_do_not_parse() {
    assert_eq!("placement_changed".parse::<ChangelogEntryKind>().ok(), None);
    assert_eq!("".parse::<ChangelogEntryKind>().ok(), None);
    assert_eq!("CREATED".parse::<ChangelogEntryKind>().ok(), None);
}

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
