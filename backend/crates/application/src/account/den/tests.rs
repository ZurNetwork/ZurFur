use std::sync::Arc;

use adapter_mem::{
    MemBackend,
    fixture::{alice, bob, seed_identity, supreme_arts},
};
use chrono::Utc;
use domain::{
    elements::{
        account::AccountName, did::Did, handle::Handle, role::Role, user::UserId,
        user_account::UserAccount,
    },
    ports::UnitOfWork,
};
use test_support::recording::{CallLog, RecordingAccountStore, SubjectLog};

use super::*;
use crate::{App, Ports, den::RootNaming};

/// The deactivated Account "Closed Studio": alice its Owner, bob a Member.
fn closed_studio() -> AccountId {
    AccountId::from(Did::from("did:plc:c1osedstudiofixtureaaaaa".to_owned()))
}

/// Seeds an Account founded by alice, with bob a Member, and deactivates it when asked.
#[allow(clippy::disallowed_methods, reason = "test seeding")]
async fn seed_account(app: &App, id: AccountId, name: &str, handle: &str, deactivate: bool) {
    let handle = handle.parse::<Handle>().expect("a valid handle");
    let name = name.parse::<AccountName>().expect("a valid name");
    let (account, ownership) = Account::open(alice(), id.did().clone(), handle, name, Utc::now());
    let member = UserAccount {
        user_id: bob(),
        account_id: id.clone(),
        role: Role::Member,
        alias: None,
    };
    crate::transaction(
        &*app.ports().database,
        async move |uow: &mut dyn UnitOfWork| {
            uow.accounts().create(&account, &ownership).await?;
            uow.accounts().grant_role(&member).await?;
            if deactivate {
                uow.accounts().soft_delete(&id).await?;
            }
            Ok(())
        },
    )
    .await
    .expect("seeds an Account");
}

/// The fixture world plus "Closed Studio", the Account store recording its calls and whose
/// data each call asked for.
async fn recorded() -> (App, MemBackend, CallLog, SubjectLog) {
    let mut mem = test_support::runtime::mem(alice().did()).build();
    seed_identity(&mem.backend)
        .await
        .expect("seeds identity's fixtures");
    seed_account(
        &App::new(Ports::from(&mem.runtime)),
        closed_studio(),
        "Closed Studio",
        "closed-studio.zurfur.app",
        true,
    )
    .await;
    let log = CallLog::default();
    let subjects = SubjectLog::default();
    mem.runtime.accounts = Arc::new(RecordingAccountStore::new(
        mem.runtime.accounts.clone(),
        log.clone(),
        subjects.clone(),
    ));
    (
        App::new(Ports::from(&mem.runtime)),
        mem.backend,
        log,
        subjects,
    )
}

/// The calls one `node` read made, and whose data they asked for; the logs are emptied by
/// taking a fresh pair for each read.
async fn calls_for(viewer: &UserId, account: &AccountId) -> (Vec<&'static str>, Vec<String>) {
    let (app, _backend, log, subjects) = recorded().await;
    let context = DenContext {
        ports: app.ports(),
        viewer,
        root_naming: RootNaming::LookUp,
    };
    node(&context, account).await.expect("no store fails");
    (log.events(), subjects.subjects())
}

#[tokio::test]
async fn every_cause_runs_the_same_reads_in_the_same_order() {
    let missing = AccountId::from(Did::from("did:plc:nope".to_owned()));
    let causes = [
        (bob(), missing.clone(), "absent"),
        (bob(), supreme_arts(), "a stranger's"),
        (bob(), closed_studio(), "deactivated, not its Owner"),
        (alice(), supreme_arts(), "live, a member"),
        (alice(), closed_studio(), "deactivated, its Owner"),
        (alice(), missing, "absent, to a member elsewhere"),
    ];

    for (viewer, account, cause) in causes {
        let (events, subjects) = calls_for(&viewer, &account).await;

        let viewer_text = viewer.to_string();
        assert_eq!(
            events,
            ["find", "role_of", "list_deactivated_for_owner"],
            "{cause}"
        );
        assert_eq!(subjects, [viewer_text.clone(), viewer_text], "{cause}");
    }
}

#[tokio::test]
async fn a_member_opens_a_live_account_by_standing() {
    let (app, _backend, _log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    let account = node(&context, &supreme_arts())
        .await
        .expect("no store fails");

    let expected = Node {
        place: Place::Account(AccountPlace::Account(supreme_arts())),
        name: NodeName::try_from("Supreme Arts").expect("a valid name"),
        level: Level::Private,
        lifts: vec![STANDING],
        state: NodeState::Live,
    };
    assert_eq!(account, Some(expected));
}

#[tokio::test]
async fn the_owner_opens_a_deactivated_account_marked() {
    let (app, _backend, _log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    let account = node(&context, &closed_studio())
        .await
        .expect("no store fails");

    let expected = Node {
        place: Place::Account(AccountPlace::Account(closed_studio())),
        name: NodeName::try_from("Closed Studio").expect("a valid name"),
        level: Level::Private,
        lifts: vec![STANDING],
        state: NodeState::Deactivated {
            viewer_is_owner: true,
        },
    };
    assert_eq!(account, Some(expected));
}

#[tokio::test]
async fn a_stranger_a_non_owner_and_a_missing_account_are_all_absent() {
    let (app, _backend, _log, _subjects) = recorded().await;
    let bob = bob();
    let context = DenContext {
        ports: app.ports(),
        viewer: &bob,
        root_naming: RootNaming::LookUp,
    };
    let missing = AccountId::from(Did::from("did:plc:nope".to_owned()));

    for account in [supreme_arts(), closed_studio(), missing] {
        let node = node(&context, &account).await.expect("no store fails");

        assert_eq!(node, None, "{account}");
    }
}

#[tokio::test]
async fn a_stored_name_the_rule_refuses_shows_the_handle() {
    let (app, _backend, _log, _subjects) = recorded().await;
    let flipped = AccountId::from(Did::from("did:plc:f1ippedfixtureaaaaaaaaa".to_owned()));
    seed_account(
        &app,
        flipped.clone(),
        "Flip\u{202E}ped",
        "flipped.zurfur.app",
        false,
    )
    .await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    let account = node(&context, &flipped)
        .await
        .expect("no store fails")
        .expect("alice's Account");

    assert_eq!(account.name.as_ref(), "flipped.zurfur.app");
}

#[tokio::test]
async fn a_stored_name_with_a_slash_shows_the_handle() {
    let (app, _backend, _log, _subjects) = recorded().await;
    let slashed = AccountId::from(Did::from("did:plc:s1ashedfixtureaaaaaaaaa".to_owned()));
    seed_account(
        &app,
        slashed.clone(),
        "AC/DC Studio",
        "acdc.zurfur.app",
        false,
    )
    .await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };

    let account = node(&context, &slashed)
        .await
        .expect("no store fails")
        .expect("alice's Account");

    assert_eq!(account.name.as_ref(), "acdc.zurfur.app");
}

#[tokio::test]
async fn an_account_holds_its_three_system_folders_found_by_key() {
    let (app, _backend, log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };
    let account = AccountPlace::Account(supreme_arts());

    let folders = children(&context, &account).await.expect("no store fails");
    let workflows = child(
        &context,
        &account,
        &names::fixed_segment(WORKFLOWS).expect("a word"),
    )
    .await
    .expect("no store fails");

    let places: Vec<Place> = folders.iter().map(|folder| folder.place.clone()).collect();
    let expected_places = [
        Place::Account(AccountPlace::Characters(supreme_arts())),
        Place::Commission(CommissionPlace::AccountCommissions(supreme_arts())),
        Place::Workflow(WorkflowPlace::Workflows(supreme_arts())),
    ];
    assert_eq!(places, expected_places);
    for folder in &folders {
        assert_eq!(folder.level, Level::Private);
        assert_eq!(folder.lifts, Vec::new());
        assert_eq!(folder.state, NodeState::Live);
    }
    assert_eq!(
        workflows.map(|folder| folder.place),
        Some(expected_places[2].clone())
    );
    assert_eq!(log.events(), Vec::<&str>::new());
}

/// Pins the named exemption: an Account's shared Characters aren't modeled yet, so its
/// `characters` folder is empty. The slice that models them replaces this test.
#[tokio::test]
async fn an_account_s_characters_folder_is_empty_until_shared_characters_are_modeled() {
    let (app, _backend, _log, _subjects) = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
        root_naming: RootNaming::LookUp,
    };
    let characters = AccountPlace::Characters(supreme_arts());

    let children = children(&context, &characters)
        .await
        .expect("no store fails");
    let mounts = mounts(&context, &characters).await.expect("no store fails");

    assert_eq!(children, Vec::new());
    assert_eq!(mounts, Vec::new());
}
