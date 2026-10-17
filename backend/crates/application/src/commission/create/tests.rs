use chrono::Utc;
use domain::elements::{commission::ChangelogEntryKind, did::Did};

use super::*;

/// A command creating a commission titled `title` for `actor_id`.
fn command(actor_id: UserId, title: &str) -> Command {
    Command {
        actor_id,
        title: title.parse().expect("the title rule keeps it"),
        maturity: None,
        deadline: None,
    }
}

#[tokio::test]
async fn a_new_commission_s_created_entry_carries_no_title() {
    let did = Did::from("did:plc:create-owner".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let ports = crate::Ports::from(&runtime);

    let created = Commissions::new(&ports)
        .create(command(UserId::from(did), "A ref sheet"), Utc::now())
        .await
        .expect("creates");

    let entries = ports
        .changelog
        .entries(&created.id)
        .await
        .expect("the store reads");
    let kinds: Vec<ChangelogEntryKind> = entries.iter().map(|entry| entry.kind).collect();
    assert_eq!(kinds, [ChangelogEntryKind::Created]);
    let empty_payload = serde_json::json!({});
    assert_eq!(entries[0].payload, empty_payload);
}

#[tokio::test]
async fn create_answers_the_row_as_stored() {
    let did = Did::from("did:plc:create-owner".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    let ports = crate::Ports::from(&runtime);
    let owner = UserId::from(did);

    let created = Commissions::new(&ports)
        .create(command(owner.clone(), "Ref\u{200B} sheet"), Utc::now())
        .await
        .expect("creates");

    let stored = ports
        .commissions
        .find(&created.id)
        .await
        .expect("the store reads")
        .expect("the commission exists");
    assert_eq!(created.title.as_str(), "Ref sheet");
    assert_eq!(created.title.as_str(), stored.title.as_str());
    assert_eq!(created.created_at, stored.created_at);
    assert!(stored.is_owned_by(&owner));
}
