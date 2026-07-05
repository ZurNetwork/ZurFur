//! `MemPublicRecords` conformance + mem-specific fidelity (ZMVP-105).
//!
//! The shared suite ([`test_support::contract`]) is the same body the real
//! atproto adapter runs; the extra tests here pin the mem fake's own fidelity
//! guarantees (stable content-addressing, idempotent put) that the real boundary
//! gets from the PDS.

use adapter_mem::MemPublicRecords;
use domain::elements::did::Did;
use domain::elements::public_record::{FeedPost, PublicRecord, SelfLabels};
use domain::ports::PublicRecords;
use test_support::contract::{TINY_PNG, public_records_contract};

fn a_post() -> PublicRecord {
    PublicRecord::FeedPost(FeedPost {
        text: Some("idempotent".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: chrono::DateTime::parse_from_rfc3339("2026-07-05T12:00:00.000Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    })
}

#[tokio::test]
async fn mem_satisfies_public_records_contract() {
    let store = MemPublicRecords::new();
    let actor = Did::new("did:plc:memactor".to_string());
    public_records_contract(&store, &actor).await;
}

#[tokio::test]
async fn blob_cid_is_content_addressed_and_stable() {
    let store = MemPublicRecords::new();

    let first = store
        .upload_blob(TINY_PNG.to_vec(), "image/png")
        .await
        .unwrap();
    let same = store
        .upload_blob(TINY_PNG.to_vec(), "image/png")
        .await
        .unwrap();
    assert_eq!(first.cid, same.cid, "identical bytes → identical CID");

    let different = store
        .upload_blob(b"totally different bytes".to_vec(), "image/png")
        .await
        .unwrap();
    assert_ne!(first.cid, different.cid, "different bytes → different CID");
    assert_eq!(different.size, b"totally different bytes".len() as u64);
}

#[tokio::test]
async fn put_is_idempotent() {
    let store = MemPublicRecords::new();
    let actor = Did::new("did:plc:memactor".to_string());
    let record = a_post();

    let created = store.create_record(&actor, &record).await.unwrap();
    let put_once = store.put_record(&created.uri, &record).await.unwrap();
    let put_twice = store.put_record(&created.uri, &record).await.unwrap();

    assert_eq!(
        put_once.cid, put_twice.cid,
        "putting identical content yields the same content-address CID"
    );
    assert_eq!(store.get_record(&created.uri).await.unwrap(), record);
}
