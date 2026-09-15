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
