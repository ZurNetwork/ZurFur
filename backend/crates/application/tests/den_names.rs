//! Names at write: creating a commission, uploading a file, declaring a slot and creating a
//! Workflow refuse a name the Den's name rule refuses, in each use case's own error, and
//! store an accepted name as the rule keeps it.

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

/// A name with a zero-width space inside, which the Den's rule strips rather than refuses.
const WITH_A_ZERO_WIDTH_SPACE: &str = "Ref\u{200B}sheet";

/// The same name as the Den's rule keeps it.
const AS_KEPT: &str = "Refsheet";

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

#[tokio::test]
async fn a_commission_title_is_stored_as_the_den_s_rule_keeps_it() {
    let runtime = world().await;
    let command = create::Command {
        actor_id: alice(),
        title: WITH_A_ZERO_WIDTH_SPACE.parse().expect("a valid title"),
        maturity: None,
        deadline: None,
    };

    let created = runtime
        .app()
        .commissions()
        .create(command, Utc::now())
        .await
        .expect("the commission is created");
    let stored = runtime
        .commissions
        .find(&created.id)
        .await
        .expect("no store fails")
        .expect("the commission is stored");

    assert_eq!(stored.title.as_str(), AS_KEPT);
}

#[tokio::test]
async fn a_file_name_is_stored_as_the_den_s_rule_keeps_it() {
    let runtime = world().await;
    let command = upload::Command {
        actor_id: alice(),
        commission_id: untitled(),
        filename: Some(format!("{WITH_A_ZERO_WIDTH_SPACE}.png")),
        content_type: Some("image/png".to_owned()),
    };
    let content = Cursor::new(b"some bytes".to_vec());

    let app = runtime.app();
    let uploaded = app
        .commissions()
        .files()
        .upload(command, content, 1024, Utc::now())
        .await
        .expect("the file is uploaded");
    let stored = runtime
        .files
        .filename(uploaded.id)
        .await
        .expect("no store fails")
        .expect("the file is stored");

    assert_eq!(stored.as_str(), format!("{AS_KEPT}.png"));
}

#[tokio::test]
async fn a_slot_title_is_stored_as_the_den_s_rule_keeps_it() {
    let runtime = world().await;
    let composition = runtime
        .commissions
        .load_composition(&untitled())
        .await
        .expect("no store fails")
        .expect("Untitled has its skeleton");
    let tab = composition.tabs.first().expect("Untitled has a tab").id;
    let surface = SurfaceAddress::new(tab, "content".parse().expect("a surface name"));
    let slot = SlotBody {
        tab,
        surface,
        title: WITH_A_ZERO_WIDTH_SPACE.parse().expect("a valid slot title"),
        notes: None,
    };
    let command = declare::Command {
        user_id: alice(),
        commission_id: untitled(),
        slots: vec![slot],
    };

    let app = runtime.app();
    let declared = app
        .commissions()
        .slots()
        .declare(command, Utc::now())
        .await
        .expect("the slot is declared");
    let [slot_id] = declared.slot_ids.as_slice() else {
        panic!("one slot was declared");
    };
    let slots = runtime
        .commissions
        .slots(&untitled())
        .await
        .expect("no store fails");
    let stored = slots
        .iter()
        .find(|slot| slot.element_id == *slot_id)
        .expect("the slot is stored");

    assert_eq!(stored.title.as_str(), AS_KEPT);
}

#[tokio::test]
async fn a_workflow_name_is_stored_as_the_den_s_rule_keeps_it() {
    let runtime = world().await;
    let command = workflow::create::Command {
        actor_id: alice(),
        account_id: supreme_arts(),
        workflow_name: WITH_A_ZERO_WIDTH_SPACE.parse().expect("a valid board name"),
    };

    let app = runtime.app();
    let accounts = app.accounts();
    let created = accounts
        .workflows()
        .create(command)
        .await
        .expect("the Workflow is created");
    let workflows = runtime
        .workflows
        .list_for_account(&supreme_arts())
        .await
        .expect("no store fails");
    let stored = workflows
        .iter()
        .find(|summary| summary.id == created.workflow.id)
        .expect("the Workflow is stored");

    assert_eq!(stored.name.as_str(), AS_KEPT);
}
