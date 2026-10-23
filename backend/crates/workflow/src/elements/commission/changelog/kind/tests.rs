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
        "renamed",
    ];

    let tokens: Vec<&'static str> = ChangelogEntryKind::VARIANTS
        .iter()
        .map(|kind| <&'static str>::from(*kind))
        .collect();

    assert_eq!(tokens, expected_tokens);
}

// The open kinds, pinned literally: each records as the stored kind of its own token, and none
// is a kind the den's write gate builds.
#[test]
fn the_open_kinds_are_pinned_literally_and_record_as_their_own_token() {
    let expected_open = [
        "seat_declared",
        "view_grant_issued",
        "view_grant_revoked",
        "file_added",
        "markup_added",
        "archived",
        "unarchived",
        "channel_linked",
        "channel_unlinked",
    ];

    let open: Vec<&'static str> = OpenKind::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(open, expected_open);

    for kind in OpenKind::VARIANTS {
        let stored = ChangelogEntryKind::from(*kind);
        assert_eq!(<&'static str>::from(stored), <&'static str>::from(kind));
    }
}

// A Late mark records as `late`, and a deadline set (a clear records as one) or extended re-arms
// it, pinned literally; a mark never re-arms itself.
#[test]
fn the_late_kinds_are_pinned_literally() {
    let late_kinds = LateKinds::default();

    let mark = <&'static str>::from(late_kinds.mark());
    let rearming: Vec<&'static str> = late_kinds
        .rearming()
        .iter()
        .map(<&'static str>::from)
        .collect();

    assert_eq!(mark, "late");
    assert_eq!(rearming, ["deadline_set", "deadline_extended"]);
    assert!(!late_kinds.rearming().contains(&late_kinds.mark()));
}
