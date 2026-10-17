use super::*;

/// A live commission's summary, titled `title`.
fn summary(title: &str) -> CommissionSummary {
    CommissionSummary {
        id: CommissionId::from(uuid::Uuid::now_v7()),
        title: StoredText::from(title.to_owned()),
        visibility: Visibility::Private,
        archived: false,
    }
}

// A summary not marked archived is live.
#[test]
fn a_summary_not_marked_archived_is_live() {
    let live = summary("Untitled");
    assert!(!live.is_archived());
}

// A summary marked archived is archived.
#[test]
fn a_summary_marked_archived_is_archived() {
    let mut archived = summary("Old sketch");
    archived.archived = true;
    assert!(archived.is_archived());
}
