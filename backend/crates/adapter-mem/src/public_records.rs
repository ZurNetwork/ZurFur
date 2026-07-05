//! In-memory fake of the [`PublicRecords`] port — the atproto (public-boundary)
//! write side, faked in-process so core development and tests run without a PDS.
//!
//! **Fidelity, not realism** (the module-level rule): [`MemPublicRecords`] keeps
//! records in a `repo → collection → rkey → record` map and blobs in a
//! `cid → bytes` map. It does **not** speak real DAG-CBOR or validate against a
//! lexicon — the reference PDS does that behind the real adapter. What it *does*
//! reproduce is the contract downstream code depends on: create mints a fresh
//! rkey and a content-address CID, `upload_blob` content-addresses the bytes (so
//! byte-identical uploads share a stable CID), and put/get/delete behave. The
//! shared conformance suite (`test_support::contract`) runs against this fake and
//! the real adapter alike, which is what makes the fake trustworthy.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use cid::Cid;
use sha2::{Digest, Sha256};

use domain::elements::did::Did;
use domain::elements::public_record::{AtUri, BlobRef, PublicRecord, RecordKey, RecordRef};
use domain::ports::{PublicRecords, PublicRecordsError};

/// The multicodec code for raw binary (`raw`), used for the content-address CIDs.
const RAW_CODEC: u64 = 0x55;
/// The multihash code for SHA-256.
const SHA2_256: u64 = 0x12;

/// Compute a stable CIDv1 (raw codec, SHA-256 multihash) over `bytes` — the mem
/// fake's content-address. Deterministic: identical bytes always yield the
/// identical CID, and any byte change changes it.
fn content_cid(bytes: &[u8]) -> Cid {
    let digest = Sha256::digest(bytes);
    let mh = cid::multihash::Multihash::<64>::wrap(SHA2_256, &digest)
        .expect("SHA-256 digest is 32 bytes, well within the 64-byte multihash budget");
    Cid::new_v1(RAW_CODEC, mh)
}

/// The map key for a stored record: `(repo did, collection nsid, rkey)`, mirroring
/// how the real repo addresses a record.
type RecordAddr = (String, String, String);

/// In-process [`PublicRecords`] fake over shared maps. Cloning shares the maps
/// (the `Arc`s are cloned, not the data), like the other mem stores.
#[derive(Clone, Default)]
pub struct MemPublicRecords {
    /// `(repo did, collection nsid, rkey) → record`.
    records: Arc<Mutex<HashMap<RecordAddr, PublicRecord>>>,
    /// `content-address cid → blob bytes` (kept so a test could read bytes back;
    /// the CID alone already witnesses byte-fidelity).
    blobs: Arc<Mutex<HashMap<Cid, Vec<u8>>>>,
    /// Monotonic source of unique, sort-ordered synthetic rkeys.
    next_rkey: Arc<AtomicU64>,
}

impl MemPublicRecords {
    /// An empty in-memory public-records store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mint a fresh, unique, lexicographically-sortable synthetic rkey — the
    /// fake's stand-in for a TID (the real repo mints a real TID on create).
    fn mint_rkey(&self) -> RecordKey {
        let n = self.next_rkey.fetch_add(1, Ordering::SeqCst);
        RecordKey::new(format!("3mem{n:013}"))
    }

    /// The record content-address the write paths return. Derived from a
    /// deterministic representation of the record, so an identical record puts to
    /// an identical CID (the mem mirror of content-addressed revisions).
    fn record_cid(record: &PublicRecord) -> Cid {
        content_cid(format!("{record:?}").as_bytes())
    }
}

#[async_trait]
impl PublicRecords for MemPublicRecords {
    async fn create_record(
        &self,
        repo: &Did,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError> {
        let collection = record.collection();
        let rkey = self.mint_rkey();
        let uri = AtUri::new(repo.clone(), collection.clone(), rkey.clone());
        self.records
            .lock()
            .expect("MemPublicRecords records mutex poisoned")
            .insert(
                (
                    repo.as_str().to_string(),
                    collection.as_str().to_string(),
                    rkey.as_str().to_string(),
                ),
                record.clone(),
            );
        Ok(RecordRef {
            uri,
            cid: Self::record_cid(record),
        })
    }

    async fn put_record(
        &self,
        uri: &AtUri,
        record: &PublicRecord,
    ) -> Result<RecordRef, PublicRecordsError> {
        self.records
            .lock()
            .expect("MemPublicRecords records mutex poisoned")
            .insert(key_of(uri), record.clone());
        Ok(RecordRef {
            uri: uri.clone(),
            cid: Self::record_cid(record),
        })
    }

    async fn delete_record(&self, uri: &AtUri) -> Result<(), PublicRecordsError> {
        // Idempotent, like the repo: deleting an absent record is a no-op.
        self.records
            .lock()
            .expect("MemPublicRecords records mutex poisoned")
            .remove(&key_of(uri));
        Ok(())
    }

    async fn get_record(&self, uri: &AtUri) -> Result<PublicRecord, PublicRecordsError> {
        self.records
            .lock()
            .expect("MemPublicRecords records mutex poisoned")
            .get(&key_of(uri))
            .cloned()
            .ok_or(PublicRecordsError::NotFound)
    }

    async fn upload_blob(
        &self,
        bytes: Vec<u8>,
        mime_type: &str,
    ) -> Result<BlobRef, PublicRecordsError> {
        let cid = content_cid(&bytes);
        let size = bytes.len() as u64;
        self.blobs
            .lock()
            .expect("MemPublicRecords blobs mutex poisoned")
            .insert(cid, bytes);
        Ok(BlobRef {
            cid,
            mime_type: mime_type.to_string(),
            size,
        })
    }
}

/// The map key for an [`AtUri`]: `(repo did, collection, rkey)`, mirroring how the
/// real repo addresses a record.
fn key_of(uri: &AtUri) -> RecordAddr {
    (
        uri.did.as_str().to_string(),
        uri.collection.as_str().to_string(),
        uri.rkey.as_str().to_string(),
    )
}
