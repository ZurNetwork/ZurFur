//! Names at write: creating a commission, uploading a file, declaring a slot and creating a
//! Workflow refuse a name the Den's name rule refuses, and founding an Account refuses a name
//! that isn't safe to show while it accepts a `/`, in each use case's own error; each stores
//! an accepted name as its rule keeps it.

use std::{io::Cursor, sync::Arc};

use adapter_mem::fixture::{alice, seed_commission, seed_identity, supreme_arts, untitled};
use application::{
    account::{self, AccountError, workflow},
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
    handle::{Handle, HandleDomain},
};
use test_support::recording::{CallLog, RecordingDidMinter};
use uuid::Uuid;

/// A name made only of the zero-width joiner, which the Den's rule counts as empty.
const JOINER_ALONE: &str = "\u{200D}";

/// A name with a zero-width space inside, which the Den's rule strips rather than refuses.
const WITH_A_ZERO_WIDTH_SPACE: &str = "Ref\u{200B}sheet";

/// The same name as the Den's rule keeps it.
const AS_KEPT: &str = "Refsheet";

/// A name holding a right-to-left override, a bidirectional control the Den's rule refuses.
const WITH_A_BIDI_OVERRIDE: &str = "Studio\u{202E}gnp.exe";

/// An Account name holding a `/`, which founding accepts and stores as given.
const WITH_A_SLASH: &str = "AC/DC Studio";

/// The handle each Account founded here asks for.
const FOUNDED_HANDLE: &str = "named.zurfur.app";

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

/// The command founding an Account named `name` as alice, under [`FOUNDED_HANDLE`].
fn founding(name: &str) -> account::create::Command {
    account::create::Command {
        actor_id: alice(),
        name: name.parse().expect("the Account name type keeps it"),
        handle: FOUNDED_HANDLE.parse().expect("a valid handle"),
    }
}

/// The Zurfur handle namespace the founded handles sit under.
fn handle_domain() -> HandleDomain {
    "zurfur.app".parse().expect("a valid handle domain")
}

#[tokio::test]
async fn an_account_named_with_a_joiner_alone_is_refused_as_empty() {
    let runtime = world().await;

    let app = runtime.app();
    let founded = app
        .accounts()
        .create(founding(JOINER_ALONE), &handle_domain(), Utc::now())
        .await;

    assert!(matches!(
        founded,
        Err(AccountError::InvalidName(DenNameError::Empty))
    ));
}

#[tokio::test]
async fn an_account_named_with_a_bidi_override_is_refused_before_any_did_is_minted() {
    let mut runtime = world().await;
    let log = CallLog::default();
    let minter = RecordingDidMinter::new(Arc::clone(&runtime.did_minter), log.clone());
    runtime.did_minter = Arc::new(minter);

    let app = runtime.app();
    let founded = app
        .accounts()
        .create(founding(WITH_A_BIDI_OVERRIDE), &handle_domain(), Utc::now())
        .await;

    assert!(matches!(
        founded,
        Err(AccountError::InvalidName(DenNameError::Invisible))
    ));
    assert_eq!(log.events(), Vec::<&str>::new());
    let handle: Handle = FOUNDED_HANDLE.parse().expect("a valid handle");
    let claimed = runtime
        .accounts
        .find_did_by_handle(&handle)
        .await
        .expect("no store fails");
    assert!(claimed.is_none(), "a refused name founds nothing");
}

#[tokio::test]
async fn an_account_name_is_stored_as_the_den_s_rule_keeps_it() {
    let runtime = world().await;

    let app = runtime.app();
    let founded = app
        .accounts()
        .create(
            founding(WITH_A_ZERO_WIDTH_SPACE),
            &handle_domain(),
            Utc::now(),
        )
        .await
        .expect("the Account is founded");
    let stored = runtime
        .accounts
        .find(&founded.account_id)
        .await
        .expect("no store fails")
        .expect("the Account is stored");

    assert_eq!(founded.name.to_string(), AS_KEPT);
    assert_eq!(stored.name.as_str(), AS_KEPT);
}

#[tokio::test]
async fn an_account_name_may_hold_a_slash_and_is_stored_as_given() {
    let runtime = world().await;

    let app = runtime.app();
    let founded = app
        .accounts()
        .create(founding(WITH_A_SLASH), &handle_domain(), Utc::now())
        .await
        .expect("the Account is founded");
    let stored = runtime
        .accounts
        .find(&founded.account_id)
        .await
        .expect("no store fails")
        .expect("the Account is stored");

    assert_eq!(founded.name.to_string(), WITH_A_SLASH);
    assert_eq!(stored.name.as_str(), WITH_A_SLASH);
}

#[tokio::test]
async fn an_account_name_unsafe_to_show_is_still_refused() {
    let refused = [
        ("Line\u{2028}break", DenNameError::Separator),
        ("Object\u{FFFC}", DenNameError::Reserved),
        ("AC/DC\u{202E}oiduts", DenNameError::Invisible),
        ("\u{200D}", DenNameError::Empty),
    ];
    for (name, refusal) in refused {
        let runtime = world().await;

        let app = runtime.app();
        let founded = app
            .accounts()
            .create(founding(name), &handle_domain(), Utc::now())
            .await;

        let Err(AccountError::InvalidName(refused_as)) = founded else {
            panic!("{name:?} is not refused as an invalid name");
        };
        assert_eq!(refused_as, refusal, "{name:?}");
    }
}

/// The escaper's Unicode tables in `domain` are a copy of Atta's, which depends on no workspace
/// crate; over every `char`, the two copies answer alike.
#[test]
fn the_escaper_s_tables_match_atta_s() {
    let every_char = (0..=u32::from(char::MAX)).filter_map(char::from_u32);
    for character in every_char {
        assert_eq!(
            domain::string_builder::is_format(character),
            atta::is_format(character),
            "is_format disagrees on U+{:04X}",
            u32::from(character)
        );
        assert_eq!(
            domain::string_builder::is_default_ignorable(character),
            atta::is_default_ignorable(character),
            "is_default_ignorable disagrees on U+{:04X}",
            u32::from(character)
        );
    }
}
