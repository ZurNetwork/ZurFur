use super::*;

/// A live commission's summary, titled `title`.
fn summary(title: &str) -> CommissionSummary {
    CommissionSummary {
        id: CommissionId::from(uuid::Uuid::now_v7()),
        title: StoredText::from(title.to_owned()),
        visibility: Visibility::Private,
        archived_at: None,
    }
}

// A summary without an archive time is live.
#[test]
fn a_summary_without_an_archive_time_is_not_archived() {
    let live = summary("Untitled");
    assert!(!live.is_archived());
}

// A summary with an archive time is archived.
#[test]
fn a_summary_with_an_archive_time_is_archived() {
    let mut archived = summary("Old sketch");
    archived.archived_at = Some(chrono::Utc::now());
    assert!(archived.is_archived());
}
