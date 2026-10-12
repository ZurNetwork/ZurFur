//! `Accounts::create` over the in-memory fakes: the one implementation every
//! driver calls, exercised branch by branch below the HTTP layer.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::sync::Arc;

use application::account::{self, AccountError};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::user::{User, UserId};
use domain::elements::{
    did::Did,
    handle::{Handle, HandleDomain, HandleError},
    role::Role,
};
use domain::ports::{Database, DidMinter, UnitOfWork};
use test_support::recording::{CallLog, RecordingDidMinter};

/// The configured Zurfur handle namespace, the way `Config::handle_domain`
/// hands it to the use case: already parsed at load.
fn handle_domain() -> HandleDomain {
    "zurfur.app".parse().expect("a valid handle domain")
}

fn command(actor_id: UserId, handle: &str) -> account::create::Command {
    account::create::Command {
        actor_id,
        name: "Acme Studio".parse().expect("a valid name"),
        handle: handle.parse().expect("a valid handle"),
    }
}

async fn recognized(database: &dyn Database, did: &Did) -> User {
    let provisioned = did.clone();
    application::transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&provisioned).await
    })
    .await
    .expect("provision")
}

#[tokio::test]
async fn founding_persists_the_account_and_seats_the_founder_as_owner() {
    let did = Did::from("did:plc:app-founder".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;

    let app = runtime.app();
    let founded = app
        .accounts()
        .create(
            command(user.id.clone(), "acme.zurfur.app"),
            &handle_domain(),
            Utc::now(),
        )
        .await
        .expect("founds");

    assert_eq!(founded.handle.as_str(), "acme.zurfur.app");
    assert_eq!(founded.name.as_str(), "Acme Studio");
    let stored = runtime
        .accounts
        .find(&founded.account_id)
        .await
        .expect("read")
        .expect("persisted");
    // Post DD 57081857 the account's id IS its sovereign DID (no separate
    // `did` field), so the identity round-trip the old assertion checked is
    // now exactly this: the row `find` returns carries the id `create`
    // reported.
    assert_eq!(stored.id, founded.account_id);
    let owner = runtime
        .accounts
        .role_of(&user.id, &founded.account_id)
        .await
        .expect("read")
        .expect("seated");
    assert!(matches!(owner, Role::Owner));
}

#[tokio::test]
async fn a_live_handle_is_taken() {
    let did = Did::from("did:plc:app-taken".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;
    let app = runtime.app();

    app.accounts()
        .create(
            command(user.id.clone(), "acme.zurfur.app"),
            &handle_domain(),
            Utc::now(),
        )
        .await
        .expect("first founds");
    // `account::create::Output` carries no `Debug` impl, so the error is
    // pulled out by structural match rather than `unwrap_err`.
    let Err(error) = app
        .accounts()
        .create(
            command(user.id, "acme.zurfur.app"),
            &handle_domain(),
            Utc::now(),
        )
        .await
    else {
        panic!("a repeat handle must not found a second account");
    };

    assert!(matches!(error, AccountError::HandleTaken));
}

// The claim gate withholds reserved labels in the CONFIGURED namespace: on a
// staging namespace `admin.zurfur.foo` is refused before anything is minted.
#[tokio::test]
async fn a_reserved_label_in_the_configured_namespace_is_refused() {
    let did = Did::from("did:plc:app-reserved".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let mut runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;
    let log = CallLog::default();
    let recording_minter = RecordingDidMinter::new(runtime.did_minter.clone(), log.clone());
    runtime.did_minter = Arc::new(recording_minter);
    let app = runtime.app();
    let staging: HandleDomain = "zurfur.foo".parse().expect("a valid handle domain");

    let Err(error) = app
        .accounts()
        .create(
            command(user.id.clone(), "admin.zurfur.foo"),
            &staging,
            Utc::now(),
        )
        .await
    else {
        panic!("a reserved label in the configured namespace must not found an account");
    };

    let reserved_label_error = HandleError::ReservedLabel("admin".into());
    assert!(
        matches!(&error, AccountError::InvalidHandle(rule) if *rule == reserved_label_error),
        "{error:?}"
    );
    let unclaimed: Handle = "admin.zurfur.foo".parse().expect("a valid handle");
    let holder = runtime
        .accounts
        .find_did_by_handle(&unclaimed)
        .await
        .expect("read");
    assert!(holder.is_none());
    assert!(log.events().is_empty(), "minted: {:?}", log.events());
}

/// A minter that always fails — the fallible, key-generating step the use
/// case runs before any private write.
struct BrokenMinter;

#[async_trait]
impl DidMinter for BrokenMinter {
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        anyhow::bail!("directory unreachable")
    }

    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        anyhow::bail!("directory unreachable")
    }

    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
        anyhow::bail!("directory unreachable")
    }

    async fn update_handle(&self, _did: &Did, _handle: &Handle) -> anyhow::Result<()> {
        anyhow::bail!("directory unreachable")
    }
}

#[tokio::test]
async fn a_mint_failure_persists_nothing() {
    let did = Did::from("did:plc:app-mintfail".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let mut runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;
    // Swap in the broken minter — every other port stays the live in-memory
    // fake, so this is otherwise the same runtime `create` runs against.
    runtime.did_minter = Arc::new(BrokenMinter);

    // `account::create::Output` carries no `Debug` impl, so the error is
    // pulled out by structural match rather than `unwrap_err`.
    let Err(error) = runtime
        .app()
        .accounts()
        .create(
            command(user.id, "acme.zurfur.app"),
            &handle_domain(),
            Utc::now(),
        )
        .await
    else {
        panic!("a broken minter must not persist a founded account");
    };

    assert!(matches!(error, AccountError::Infrastructure(_)));
    let handle: Handle = "acme.zurfur.app".parse().expect("valid");
    let claimed = runtime
        .accounts
        .find_did_by_handle(&handle)
        .await
        .expect("read");
    assert!(
        claimed.is_none(),
        "nothing may be persisted after a failed mint"
    );
}
