use std::collections::BTreeSet;

use strum::VariantArray;

use super::*;

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

// The persisted tokens, pinned literally and in declaration order.
#[test]
fn kind_tokens_are_pinned_literally() {
    let expected_tokens = [
        "created",
        "lifecycle_moved",
        "status_changed",
        "deadline_set",
        "deadline_extended",
        "delayed",
        "late",
        "seat_declared",
        "seat_invited",
        "seat_applied",
        "seat_accepted",
        "seat_declined",
        "seat_left",
        "seat_evicted",
        "ceiling_changed",
        "view_grant_issued",
        "view_grant_revoked",
        "admin_granted",
        "admin_revoked",
        "ownership_transferred",
        "tree_attached",
        "tree_detached",
        "phase_checked_off",
        "phase_approved",
        "file_added",
        "markup_added",
        "invoice_issued",
        "invoice_voided",
        "invoice_marked_paid",
        "invoice_payment_sent",
        "snapshot_published",
        "archived",
        "unarchived",
        "note",
        "channel_linked",
        "channel_unlinked",
    ];

    let tokens: Vec<&'static str> = ChangelogEntryKind::VARIANTS
        .iter()
        .map(|kind| <&'static str>::from(*kind))
        .collect();

    assert_eq!(tokens, expected_tokens);
}
