//! `AtprotoPublicRecords` against a real, throwaway reference PDS (ZMVP-105).
//!
//! Runs the **same** shared conformance suite the mem fake runs
//! ([`test_support::contract`]) — that both pass is AC5 — plus the atproto-only
//! behaviors that need something off-port: an unreachable endpoint, an
//! authorization refusal, and a raw blob download for a byte-for-byte compare.
//!
//! These need a container runtime socket (podman/docker), like every other
//! testcontainers suite in the workspace.

use adapter_atproto::AtprotoPublicRecords;
use domain::elements::did::Did;
use domain::elements::public_record::{Embed, FeedPost, PublicRecord, SelfLabels};
use domain::ports::{PublicRecords, PublicRecordsError};
use test_support::contract::{TINY_PNG, public_records_contract};
use test_support::{ActingCredential, ThrowawayPds};

/// Provision a fixture account on `pds` and build the Bearer-authed adapter for
/// it, returning the adapter and the acting DID.
async fn adapter_for(pds: &ThrowawayPds, handle: &str) -> (AtprotoPublicRecords, Did) {
    let account = pds
        .provision_account(handle)
        .await
        .expect("provision fixture account");
    let access_jwt = match &account.credential {
        ActingCredential::PdsSession { access_jwt, .. } => access_jwt.clone(),
        _ => unreachable!("new credential variants opt in explicitly"),
    };
    let store = AtprotoPublicRecords::new(&account.endpoint, access_jwt)
        .expect("build atproto public-records adapter");
    (store, Did::new(account.did))
}

/// AC5: the real adapter satisfies the same contract as the fake.
#[tokio::test]
async fn atproto_satisfies_public_records_contract() {
    let pds = ThrowawayPds::boot().await.expect("boot throwaway PDS");
    let (store, actor) = adapter_for(&pds, "alice.test").await;
    public_records_contract(&store, &actor).await;
}

/// AC2 end-to-end: an uploaded blob is downloadable **byte-for-byte identical**
/// from the PDS (`com.atproto.sync.getBlob`), not merely content-address-stable.
#[tokio::test]
async fn uploaded_blob_downloads_byte_identical() {
    let pds = ThrowawayPds::boot().await.expect("boot throwaway PDS");
    let (store, actor) = adapter_for(&pds, "blobby.test").await;

    let blob_ref = store
        .upload_blob(TINY_PNG.to_vec(), "image/png")
        .await
        .expect("upload_blob");
    assert_eq!(blob_ref.size, TINY_PNG.len() as u64);

    // A freshly uploaded blob is temporary until a record references it; the PDS
    // won't serve an unreferenced blob via getBlob. Publish a record embedding it.
    let record = PublicRecord::FeedPost(FeedPost {
        text: None,
        embed: Some(Embed {
            blob: blob_ref.clone(),
            alt: "byte-fidelity probe".to_string(),
            aspect_ratio: None,
        }),
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: chrono::Utc::now(),
    });
    store
        .create_record(&actor, &record)
        .await
        .expect("publish record referencing the blob");

    let url = format!(
        "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
        pds.endpoint(),
        actor.as_str(),
        blob_ref.cid,
    );
    let downloaded = reqwest::get(&url)
        .await
        .expect("getBlob request")
        .bytes()
        .await
        .expect("getBlob body");
    assert_eq!(
        downloaded.as_ref(),
        TINY_PNG,
        "the blob must download byte-for-byte identical to what was uploaded"
    );
}

/// AC4: an unreachable PDS surfaces as `Unreachable` — never a panic, never a
/// silent Ok. No container needed: a closed loopback port refuses the connection.
#[tokio::test]
async fn unreachable_pds_is_a_domain_error() {
    // Port 9 (discard) is not listening for XRPC — the connection is refused.
    let store = AtprotoPublicRecords::new("http://127.0.0.1:9", "not-a-real-token")
        .expect("build adapter for a dead endpoint");
    let record = PublicRecord::FeedPost(FeedPost {
        text: Some("into the void".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: chrono::Utc::now(),
    });

    match store
        .create_record(&Did::new("did:plc:whoever".to_string()), &record)
        .await
    {
        Err(PublicRecordsError::Unreachable(_)) => {}
        other => panic!("an unreachable PDS must be Err(Unreachable), got {other:?}"),
    }
}

/// AC4: the PDS **answering with a refusal** (here: writing to a repo the Bearer
/// token is not authorized for) surfaces as `Rejected`, carrying the status/error
/// — not a silent success.
#[tokio::test]
async fn rejected_write_is_a_domain_error() {
    let pds = ThrowawayPds::boot().await.expect("boot throwaway PDS");
    let (store, _actor) = adapter_for(&pds, "carol.test").await;

    // A syntactically valid DID that is NOT the acting identity: the PDS refuses
    // to let Carol's session write into someone else's repo.
    let foreign_repo = Did::new("did:plc:someoneelserepo000000000".to_string());
    let record = PublicRecord::FeedPost(FeedPost {
        text: Some("not my repo".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: chrono::Utc::now(),
    });

    match store.create_record(&foreign_repo, &record).await {
        Err(PublicRecordsError::Rejected { status, error, .. }) => {
            assert!(
                (400..=403).contains(&status),
                "a foreign-repo write should be refused with a 4xx, got {status} {error}"
            );
        }
        other => panic!("a refused write must be Err(Rejected), got {other:?}"),
    }
}
