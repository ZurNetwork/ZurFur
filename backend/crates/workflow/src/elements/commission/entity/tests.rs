use super::*;

/// A live commission, as a load would return it, owned by a fresh DID.
fn stored_commission() -> Commission {
    let owner = identity::elements::user::UserId::from(kernel::elements::did::Did::from(format!(
        "did:plc:{}",
        uuid::Uuid::now_v7()
    )));
    Commission {
        id: CommissionId::mint(),
        owner_id: owner,
        lifecycle_step: LifecycleStep::Draft,
        deadline: None,
        maturity: None,
        direction_status: None,
        deadline_status: None,
        linked_channel: None,
        archived_at: None,
        created_at: chrono::Utc::now(),
    }
}

// A commission with no archive time is not archived.
#[test]
fn a_commission_with_no_archive_time_is_not_archived() {
    let c = stored_commission();
    assert!(!c.is_archived());
}

// A commission with an archive time is archived.
#[test]
fn a_commission_with_an_archive_time_is_archived() {
    let mut c = stored_commission();
    c.archived_at = Some(chrono::Utc::now());
    assert!(c.is_archived());
}

// A commission is owned by its owner and by no one else.
#[test]
fn a_commission_is_owned_by_its_owner_only() {
    let c = stored_commission();
    let stranger = identity::elements::user::UserId::from(kernel::elements::did::Did::from(
        "did:plc:stranger".to_owned(),
    ));
    assert!(c.is_owned_by(&c.owner_id));
    assert!(!c.is_owned_by(&stranger));
}
