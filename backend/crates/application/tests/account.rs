//! `create_account` over the in-memory fakes: the one implementation every
//! driver calls (ZMVP-205 AC2), exercised branch by branch below the HTTP
//! layer.

use application::account::{AccountError, AccountPorts, CreateAccountCommand, create_account};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::user::{User, UserId};
use domain::elements::{did::Did, handle::Handle, role::Role};
use domain::ports::{Database, DidMinter, UnitOfWork};

const HANDLE_DOMAIN: &str = "zurfur.app";

fn command(actor: UserId, handle: &str) -> CreateAccountCommand {
    CreateAccountCommand {
        actor,
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
    let did = Did::new("did:plc:app-founder".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;

    let ports = AccountPorts {
        accounts: &*runtime.accounts,
        did_minter: &*runtime.did_minter,
        database: &*runtime.database,
    };
    let founded = create_account(
        command(user.id, "acme.zurfur.app"),
        ports,
        HANDLE_DOMAIN,
        Utc::now(),
    )
    .await
    .expect("founds");

    assert_eq!(founded.handle.as_str(), "acme.zurfur.app");
    assert_eq!(founded.name.as_str(), "Acme Studio");
    let stored = runtime
        .accounts
        .find(founded.account_id)
        .await
        .expect("read")
        .expect("persisted");
    assert_eq!(stored.did, founded.did);
    let owner = runtime
        .accounts
        .role_of(user.id, founded.account_id)
        .await
        .expect("read")
        .expect("seated");
    assert!(matches!(owner, Role::Owner(_)));
}

#[tokio::test]
async fn a_live_handle_is_taken() {
    let did = Did::new("did:plc:app-taken".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;
    let ports = || AccountPorts {
        accounts: &*runtime.accounts,
        did_minter: &*runtime.did_minter,
        database: &*runtime.database,
    };

    create_account(
        command(user.id, "acme.zurfur.app"),
        ports(),
        HANDLE_DOMAIN,
        Utc::now(),
    )
    .await
    .expect("first founds");
    let error = create_account(
        command(user.id, "acme.zurfur.app"),
        ports(),
        HANDLE_DOMAIN,
        Utc::now(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, AccountError::HandleTaken));
}

/// A minter that always fails — the fallible, key-generating step the use
/// case runs before any private write.
struct BrokenMinter;

#[async_trait]
impl DidMinter for BrokenMinter {
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
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
    let did = Did::new("did:plc:app-mintfail".to_string());
    let fixture = test_support::runtime::mem(&did).build();
    let runtime = fixture.runtime;
    let user = recognized(&*runtime.database, &did).await;
    let ports = AccountPorts {
        accounts: &*runtime.accounts,
        did_minter: &BrokenMinter,
        database: &*runtime.database,
    };

    let error = create_account(
        command(user.id, "acme.zurfur.app"),
        ports,
        HANDLE_DOMAIN,
        Utc::now(),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, AccountError::Minter(_)));
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
