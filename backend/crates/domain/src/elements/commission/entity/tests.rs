use super::*;

// A fresh commission carries no deadline status, even born with a deadline.
#[test]
fn a_fresh_commission_has_no_deadline_status() {
    let c = Commission::create(
        "Ref".parse::<CommissionTitle>().unwrap(),
        crate::elements::user::UserId::from(crate::elements::did::Did::from(format!(
            "did:plc:{}",
            uuid::Uuid::now_v7()
        ))),
        chrono::Utc::now(),
        Some(chrono::Utc::now()),
    );
    assert_eq!(c.deadline_status, None);
}

// A fresh commission carries no direction status (the cleared state).
#[test]
fn a_fresh_commission_has_no_direction_status() {
    let c = Commission::create(
        "Ref".parse::<CommissionTitle>().unwrap(),
        crate::elements::user::UserId::from(crate::elements::did::Did::from(format!(
            "did:plc:{}",
            uuid::Uuid::now_v7()
        ))),
        chrono::Utc::now(),
        None,
    );
    assert_eq!(c.direction_status, None);
}

// A new commission carries its checked title, trimmed.
#[test]
fn a_new_commission_carries_its_checked_title() {
    let owner = crate::elements::user::UserId::from(crate::elements::did::Did::from(format!(
        "did:plc:{}",
        uuid::Uuid::now_v7()
    )));
    let title = "  A ref sheet  ".parse::<CommissionTitle>().unwrap();
    let c = Commission::create(title, owner, chrono::Utc::now(), None);
    assert_eq!(c.title.as_str(), "A ref sheet");
}

/// The commission a load would return for `new`, written as it was built.
fn stored(new: NewCommission) -> Commission {
    Commission {
        id: new.id,
        title: StoredText::from(new.title),
        owner_id: new.owner_id,
        lifecycle_step: new.lifecycle_step,
        visibility: new.visibility,
        deadline: new.deadline,
        maturity: new.maturity,
        direction_status: new.direction_status,
        deadline_status: new.deadline_status,
        linked_channel: new.linked_channel,
        archived_at: new.archived_at,
        created_at: new.created_at,
    }
}

/// A freshly created commission, as stored, owned by a fresh DID.
fn fresh_stored_commission() -> Commission {
    let owner = crate::elements::user::UserId::from(crate::elements::did::Did::from(format!(
        "did:plc:{}",
        uuid::Uuid::now_v7()
    )));
    let title = "Ref".parse::<CommissionTitle>().unwrap();
    let new = Commission::create(title, owner, chrono::Utc::now(), None);
    stored(new)
}

// A fresh commission is not archived.
#[test]
fn a_fresh_commission_is_not_archived() {
    let c = fresh_stored_commission();
    assert!(!c.is_archived());
}

// A commission with an archive time is archived.
#[test]
fn a_commission_with_an_archive_time_is_archived() {
    let mut c = fresh_stored_commission();
    c.archived_at = Some(chrono::Utc::now());
    assert!(c.is_archived());
}
