//! A hard delete tombstones the Account's DID only once the delete has
//! committed: inside `tombstone`, a read through the pool (READ COMMITTED)
//! already finds the Account gone.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::sync::{Arc, Mutex};

use application::{account, transaction};
use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{account::AccountId, did::Did, handle::Handle, user::User},
    ports::{AccountStore, Database, DidMinter, UnitOfWork},
};

/// The live minter, reading the tombstoned DID's Account through the pool when
/// asked to tombstone it.
struct ReadingMinter {
    inner: Arc<dyn DidMinter>,
    accounts: Arc<dyn AccountStore>,
    found_at_tombstone: Mutex<Vec<bool>>,
}

#[async_trait]
impl DidMinter for ReadingMinter {
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did> {
        self.inner.mint(handle).await
    }

    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        self.inner.mint_handleless().await
    }

    async fn tombstone(&self, did: &Did) -> anyhow::Result<()> {
        let account = AccountId::from(did.clone());
        let found = self.accounts.find(&account).await?.is_some();
        self.found_at_tombstone
            .lock()
            .expect("unpoisoned")
            .push(found);
        self.inner.tombstone(did).await
    }

    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()> {
        self.inner.update_handle(did, handle).await
    }
}

async fn recognized(database: &dyn Database, did: &Did) -> User {
    let provisioned = did.clone();
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.users().provision(&provisioned).await
    })
    .await
    .expect("provision")
}

#[tokio::test]
async fn the_tombstone_finds_the_hard_deleted_account_already_gone() {
    let pg = test_support::pg_app::pg_runtime().await;
    let mut runtime = pg.runtime.clone();
    let did = Did::from("did:plc:delete-order-owner".to_string());
    let user = recognized(&*runtime.database, &did).await;
    let handle_domain = "zurfur.app".parse().expect("a valid handle domain");
    let founding = account::create::Command {
        actor_id: user.id.clone(),
        name: "Acme Studio".parse().expect("a valid name"),
        handle: "acme.zurfur.app".parse().expect("a valid handle"),
    };
    let founded = runtime
        .app()
        .accounts()
        .create(founding, &handle_domain, Utc::now())
        .await
        .expect("founds");
    let reading = Arc::new(ReadingMinter {
        inner: Arc::clone(&runtime.did_minter),
        accounts: Arc::clone(&runtime.accounts),
        found_at_tombstone: Mutex::default(),
    });
    runtime.did_minter = reading.clone();

    let deleting = account::delete::Command {
        actor_id: user.id,
        account_id: founded.account_id,
    };
    let deleted = runtime
        .app()
        .accounts()
        .delete(deleting)
        .await
        .expect("deletes");

    assert!(matches!(
        deleted.outcome,
        account::delete::DeleteOutcome::Hard
    ));
    let found_at_tombstone = reading
        .found_at_tombstone
        .lock()
        .expect("unpoisoned")
        .clone();
    assert_eq!(
        found_at_tombstone,
        [false],
        "the Account was gone when its DID was tombstoned"
    );
}
