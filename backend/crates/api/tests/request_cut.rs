//! A client that goes away mid-request can't leave a half-done cross-store
//! write: the request runs to its end in its detached task. Each test cuts the
//! connection the moment the DID minter's call has landed, before the request
//! is done, on the stack the server runs, over PostgreSQL.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use domain::{
    elements::{
        account::{AccountId, ListingScope},
        did::Did,
        handle::Handle,
        profile::Profile,
        user::UserId,
    },
    ports::DidMinter,
};
use test_support::{
    cut::{CUT_WAIT, DropWitness, send_raw},
    pg_app::{self, PgServed, SignedIn},
};
use tokio::sync::Notify;
use tokio::time::Instant;

/// What the minter did, and the gate that holds it right after its call landed.
#[derive(Default)]
struct Probe {
    minted: Mutex<Vec<Did>>,
    tombstoned: Mutex<Vec<Did>>,
    holding: AtomicBool,
    entered: Notify,
    release: Notify,
}

impl Probe {
    /// While holding: signals `entered`, then waits for `release`.
    async fn landed(&self) {
        if self.holding.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }

    /// Waits until a held call has landed.
    async fn entered(&self) {
        tokio::time::timeout(CUT_WAIT, self.entered.notified())
            .await
            .expect("the minter's call lands");
    }

    fn minted(&self) -> Vec<Did> {
        self.minted.lock().expect("unpoisoned").clone()
    }

    fn tombstoned(&self) -> Vec<Did> {
        self.tombstoned.lock().expect("unpoisoned").clone()
    }
}

/// The live minter, recording each mint and tombstone once it has landed,
/// then holding there while the probe holds.
struct HeldMinter {
    inner: Arc<dyn DidMinter>,
    probe: Arc<Probe>,
}

#[async_trait]
impl DidMinter for HeldMinter {
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did> {
        let did = self.inner.mint(handle).await?;
        self.probe
            .minted
            .lock()
            .expect("unpoisoned")
            .push(did.clone());
        self.probe.landed().await;
        Ok(did)
    }

    async fn mint_handleless(&self) -> anyhow::Result<Did> {
        self.inner.mint_handleless().await
    }

    async fn tombstone(&self, did: &Did) -> anyhow::Result<()> {
        self.inner.tombstone(did).await?;
        self.probe
            .tombstoned
            .lock()
            .expect("unpoisoned")
            .push(did.clone());
        self.probe.landed().await;
        Ok(())
    }

    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()> {
        self.inner.update_handle(did, handle).await
    }
}

/// The app on PostgreSQL with the held minter, a drop witness outside the stack.
async fn served(probe: &Arc<Probe>, witness: &DropWitness) -> PgServed {
    let held = Arc::clone(probe);
    let outer = witness.clone();
    pg_app::serve_wrapped(
        move |mut runtime| {
            let minter = HeldMinter {
                inner: Arc::clone(&runtime.did_minter),
                probe: held,
            };
            runtime.did_minter = Arc::new(minter);
            api::app(runtime)
        },
        move |stacked| stacked.layer(outer),
    )
    .await
}

fn owner() -> Profile {
    Profile {
        did: Did::from("did:plc:request-cut-owner".to_string()),
        handle: "owner.test".to_string().into(),
        display_name: None,
        avatar_url: None,
    }
}

/// A raw request from the signed-in owner's browser, `body` as JSON.
fn browser_request(served: &PgServed, owner: &SignedIn, line: &str, body: &str) -> String {
    let host = served
        .base_url
        .strip_prefix("http://")
        .expect("a plain-HTTP base URL");
    format!(
        "{line} HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\nCookie: zurfur.sid={cookie}\r\n\
         Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
        origin = served.base_url,
        cookie = owner.session_cookie,
        length = body.len(),
    )
}

/// Sends `request`, cuts the connection once the minter's call has landed,
/// sees the server drop the request's future, then lets the minter go on and
/// waits for every detached request to finish.
async fn cut(served: &PgServed, probe: &Probe, witness: &DropWitness, request: &str) {
    probe.holding.store(true, Ordering::SeqCst);
    let connection = send_raw(&served.base_url, request).await;
    probe.entered().await;
    drop(connection);
    witness.dropped().await;
    probe.release.notify_one();
    served
        .detach
        .drain(Instant::now() + CUT_WAIT)
        .await
        .expect("the detached request finishes");
}

async fn live_account_dids(served: &PgServed, owner: &Profile) -> Vec<Did> {
    let owner_id = UserId::from(owner.did.clone());
    let memberships = served
        .runtime
        .accounts
        .list_for_user(&owner_id, ListingScope::SelfView)
        .await
        .expect("the store answers");
    memberships
        .into_iter()
        .map(|membership| membership.account.id.did().clone())
        .collect()
}

#[tokio::test]
async fn a_founding_cut_after_the_mint_still_founds_the_account_the_did_was_minted_for() {
    let probe = Arc::new(Probe::default());
    let witness = DropWitness::default();
    let served = served(&probe, &witness).await;
    let owner = owner();
    let signed_in = served.sign_in_with_cookie(owner.clone()).await;
    let founding = browser_request(
        &served,
        &signed_in,
        "POST /accounts",
        r#"{"name":"Cut Studio","handle":"cut-studio.zurfur.app"}"#,
    );

    cut(&served, &probe, &witness, &founding).await;

    let minted = probe.minted();
    assert_eq!(minted.len(), 1, "one DID was minted");
    assert_eq!(live_account_dids(&served, &owner).await, minted);
}

#[tokio::test]
async fn a_delete_cut_after_the_tombstone_leaves_no_live_account_on_a_tombstoned_did() {
    let probe = Arc::new(Probe::default());
    let witness = DropWitness::default();
    let served = served(&probe, &witness).await;
    let owner = owner();
    let signed_in = served.sign_in_with_cookie(owner.clone()).await;
    let founded = signed_in
        .client
        .post(format!("{}/accounts", served.base_url))
        .json(&serde_json::json!({ "name": "Cut Studio", "handle": "cut-studio.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(founded.status(), 201);
    let [account] = <[Did; 1]>::try_from(probe.minted()).expect("one account founded");
    let deleting = browser_request(
        &served,
        &signed_in,
        &format!("DELETE /accounts/{account}"),
        "",
    );

    cut(&served, &probe, &witness, &deleting).await;

    let tombstoned_once = std::slice::from_ref(&account);
    assert_eq!(probe.tombstoned(), tombstoned_once);
    let still_there = served
        .runtime
        .accounts
        .find(&AccountId::from(account))
        .await
        .expect("the store answers");
    assert!(
        still_there.is_none(),
        "a live account stands on a tombstoned DID"
    );
}
