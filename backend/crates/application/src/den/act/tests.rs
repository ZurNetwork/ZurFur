use adapter_mem::fixture::{alice, untitled};
use chrono::Utc;

use super::*;

// Each gated entry, built on a commission, by an actor, at a time, carries its own kind, that
// commission, actor and time, an empty JSON object as its payload and no note: the entries the
// gate stores stay as they are.
#[test]
fn each_gated_entry_builds_its_own_kind_with_an_empty_payload_and_no_note() {
    let now = Utc::now();
    let gated = [
        (GatedEntry::CREATED, ChangelogEntryKind::Created),
        (GatedEntry::RENAMED, ChangelogEntryKind::Renamed),
    ];
    let empty_payload = serde_json::json!({});

    for (entry, kind) in gated {
        let built = entry.construct(untitled(), alice(), now);

        assert_eq!(entry.kind(), kind);
        assert_eq!(built.kind(), kind);
        assert_eq!(built.commission_id(), untitled(), "{kind}");
        assert_eq!(built.actor_id(), Some(&alice()), "{kind}");
        assert_eq!(built.created_at(), now, "{kind}");
        assert_eq!(built.payload(), &empty_payload, "{kind}");
        assert_eq!(built.note_text(), None, "{kind}");
    }
}

#[test]
fn an_on_change_rule_names_its_entry_only_when_the_act_changed_its_node() {
    let always = EntryRule::Always(GatedEntry::CREATED);
    let on_change = EntryRule::OnChange(GatedEntry::RENAMED);

    let kinds = [
        always.entry(true).map(GatedEntry::kind),
        always.entry(false).map(GatedEntry::kind),
        on_change.entry(true).map(GatedEntry::kind),
        on_change.entry(false).map(GatedEntry::kind),
    ];

    let expected = [
        Some(ChangelogEntryKind::Created),
        Some(ChangelogEntryKind::Created),
        Some(ChangelogEntryKind::Renamed),
        None,
    ];
    assert_eq!(kinds, expected);
}
