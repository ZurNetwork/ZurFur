//! Names at write: creating a commission, uploading a file, declaring a slot and creating a
//! Workflow refuse a name the Den's name rule refuses, in each use case's own error.

use std::io::Cursor;

use adapter_mem::fixture::{alice, seed_commission, seed_identity, supreme_arts, untitled};
use application::{
    account::{AccountError, workflow},
    commission::{
        CommissionError, create,
        files::upload,
        slots::declare::{self, SlotBody},
    },
    den::DenNameError,
};
use chrono::Utc;
use domain::elements::{
    commission::{SurfaceAddress, TabId},
    did::Did,
};
use uuid::Uuid;

/// A name made only of the zero-width joiner, which the Den's rule counts as empty.
const JOINER_ALONE: &str = "\u{200D}";

/// The in-memory runtime with the fixture world seeded.
async fn world() -> composition::Runtime {
    let did = Did::from("did:plc:den-names-test".to_owned());
    let built = test_support::runtime::mem(&did).build();
    seed_identity(&built.backend)
        .await
        .expect("identity's fixture seeds");
    seed_commission(
        &built.backend,
        &alice(),
        &adapter_mem::fixture::bob(),
        &supreme_arts(),
    )
    .await
    .expect("commission's fixture seeds");
    built.runtime
}

#[tokio::test]
async fn a_commission_titled_with_a_joiner_alone_is_refused_as_empty() {
    let runtime = world().await;
    let command = create::Command {
        actor_id: alice(),
        title: JOINER_ALONE.parse().expect("the title rule keeps a joiner"),
        maturity: None,
        deadline: None,
    };

    let created = runtime
        .app()
        .commissions()
        .create(command, Utc::now())
        .await;

    assert!(matches!(
        created,
        Err(CommissionError::InvalidName(DenNameError::Empty))
    ));
}

#[tokio::test]
async fn a_file_named_with_a_joiner_alone_is_refused_as_empty() {
    let runtime = world().await;
    let command = upload::Command {
        actor_id: alice(),
        commission_id: untitled(),
        filename: Some(JOINER_ALONE.to_owned()),
        content_type: Some("image/png".to_owned()),
    };
    let content = Cursor::new(b"some bytes".to_vec());

    let app = runtime.app();
    let uploaded = app
        .commissions()
        .files()
        .upload(command, content, 1024, Utc::now())
        .await;

    assert!(matches!(
        uploaded,
        Err(CommissionError::InvalidName(DenNameError::Empty))
    ));
}

#[tokio::test]
async fn a_slot_titled_with_a_joiner_alone_is_refused_as_empty() {
    let runtime = world().await;
    let tab = TabId::from(Uuid::from_u128(1));
    let surface = SurfaceAddress::new(tab, "content".parse().expect("a surface name"));
    let slot = SlotBody {
        tab,
        surface,
        title: JOINER_ALONE
            .parse()
            .expect("the slot title rule keeps a joiner"),
        notes: None,
    };
    let command = declare::Command {
        user_id: alice(),
        commission_id: untitled(),
        slots: vec![slot],
    };

    let app = runtime.app();
    let declared = app.commissions().slots().declare(command, Utc::now()).await;

    assert!(matches!(
        declared,
        Err(CommissionError::InvalidName(DenNameError::Empty))
    ));
}

#[tokio::test]
async fn a_workflow_named_with_a_joiner_alone_is_refused_as_empty() {
    let runtime = world().await;
    let command = workflow::create::Command {
        actor_id: alice(),
        account_id: supreme_arts(),
        workflow_name: JOINER_ALONE
            .parse()
            .expect("the board name rule keeps a joiner"),
    };

    let app = runtime.app();
    let accounts = app.accounts();
    let created = accounts.workflows().create(command).await;

    assert!(matches!(
        created,
        Err(AccountError::InvalidName(DenNameError::Empty))
    ));
}

#[tokio::test]
async fn an_ordinary_name_still_creates() {
    let runtime = world().await;
    let command = create::Command {
        actor_id: alice(),
        title: "Ref sheet".parse().expect("a valid title"),
        maturity: None,
        deadline: None,
    };

    let created = runtime
        .app()
        .commissions()
        .create(command, Utc::now())
        .await;

    assert!(created.is_ok());
}
