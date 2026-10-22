#![allow(
    clippy::disallowed_methods,
    reason = "test seeding recognizes the owner"
)]

use chrono::Utc;
use domain::{
    elements::{commission::ChangelogEntryKind, did::Did},
    ports::UnitOfWork,
};

use super::*;
use crate::{commission::CommissionError, common_error::CommonError};

/// A command creating a commission titled `title` for `actor_id`.
fn command(actor_id: UserId, title: &str) -> Command {
    Command {
        actor_id,
        title: title.parse().expect("the title rule keeps it"),
        maturity: None,
        deadline: None,
    }
}

/// The in-memory runtime's ports, with `did` recognized as a User.
async fn ports_for(did: &Did) -> crate::Ports {
    let runtime = test_support::runtime::mem(did).build().runtime;
    let recognized = did.clone();
    crate::transaction(&*runtime.database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&recognized).await?;
        Ok(())
    })
    .await
    .expect("the owner is recognized");
    crate::Ports::from(&runtime)
}

#[tokio::test]
async fn a_new_commission_s_created_entry_carries_no_title() {
    let did = Did::from("did:plc:create-owner".to_owned());
    let ports = ports_for(&did).await;

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
    let ports = ports_for(&did).await;
    let owner = UserId::from(did);
    let deadline = Utc::now() + chrono::Duration::days(30);
    let chosen = Command {
        deadline: Some(deadline),
        ..command(owner.clone(), "Ref\u{200B} sheet")
    };

    let created = Commissions::new(&ports)
        .create(chosen, Utc::now())
        .await
        .expect("creates");

    let stored = ports
        .commissions
        .find(&created.id)
        .await
        .expect("the store reads")
        .expect("the commission exists");
    let summary = ports
        .commissions
        .find_summary(&created.id)
        .await
        .expect("the store reads")
        .expect("the commission exists");
    assert_eq!(created.title.as_str(), "Ref sheet");
    assert_eq!(created.title.as_str(), summary.title.as_str());
    assert_eq!(created.visibility, summary.visibility);
    assert_eq!(created.lifecycle, stored.lifecycle_step);
    assert_eq!(created.deadline, Some(deadline));
    assert_eq!(created.created_at, stored.created_at);
    assert!(stored.is_owned_by(&owner));
}

// A create by someone no User holds is the store's refusal, and keeps nothing.
#[tokio::test]
async fn a_create_for_an_unknown_user_is_a_store_failure() {
    let did = Did::from("did:plc:create-owner".to_owned());
    let ports = ports_for(&did).await;
    let unknown = UserId::from(Did::from("did:plc:create-unknown".to_owned()));

    let created = Commissions::new(&ports)
        .create(command(unknown.clone(), "Nobody's"), Utc::now())
        .await;

    assert!(
        matches!(
            created,
            Err(CommissionError::CommonError(CommonError::Infrastructure(_)))
        ),
        "{created:?}"
    );
    let owned = ports
        .commissions
        .list_owned_by(&unknown)
        .await
        .expect("the store reads");
    assert!(owned.is_empty());
}
