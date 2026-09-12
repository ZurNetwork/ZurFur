//! The account `did:plc` minter — real ([`RealDidMinter`]) and stub
//! ([`StubDidMinter`]).
//!
//! The real minter generates per-account secp256k1 rotation keys, signs an
//! identity-only genesis operation, custodies the keys through a [`KeyStore`],
//! and submits to a [`PlcDirectory`]. The stub mints a synthetic DID only.

use async_trait::async_trait;
use atrium_crypto::keypair::{Did as _, Export as _, Secp256k1Keypair};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    elements::{
        account_keys::{AccountKeys, SecretKey},
        did::Did,
        handle::Handle,
        plc_operation::PlcOperationRecord,
    },
    ports::{DidMinter, KeyStore, PlcOperationLog},
};
use rand::Rng;
use std::sync::Arc;

use crate::plc::{PlcOperation, TombstoneOperation};
use crate::plc_directory::PlcDirectory;

/// The real [`DidMinter`]: mints a genuine, custody-backed `did:plc` over an
/// injected [`KeyStore`], [`PlcOperationLog`] and `PlcDirectory`.
pub struct RealDidMinter {
    key_store: Arc<dyn KeyStore>,
    op_log: Arc<dyn PlcOperationLog>,
    directory: Box<dyn PlcDirectory>,
}

impl RealDidMinter {
    /// Build the real minter over custody, the operation log (so the next op
    /// knows its `prev`), and the submission directory.
    pub fn new(
        key_store: Arc<dyn KeyStore>,
        op_log: Arc<dyn PlcOperationLog>,
        directory: Box<dyn PlcDirectory>,
    ) -> Self {
        Self {
            key_store,
            op_log,
            directory,
        }
    }
}

#[async_trait]
impl DidMinter for RealDidMinter {
    /// Mint an identity-only `did:plc` bound to `handle`: generate the three
    /// keypairs, sign the genesis operation with the operational key, derive the
    /// DID, custody the keys and log the op, then submit to the directory.
    /// Keys are stored before submission, so a submission retry never orphans them.
    async fn mint(&self, handle: &Handle) -> anyhow::Result<Did> {
        // Generate keys in a block so the non-`Send` `ThreadRng` is dropped before
        // any `.await` below (the keypairs themselves are `Send`).
        let (cold, operational, signing) = {
            let mut rng = rand::thread_rng();
            (
                Secp256k1Keypair::create(&mut rng),
                Secp256k1Keypair::create(&mut rng),
                Secp256k1Keypair::create(&mut rng),
            )
        };

        // rotationKeys in DESCENDING authority: cold-recovery, then operational.
        let rotation_keys = vec![cold.did(), operational.did()];
        let op = PlcOperation::identity_only(rotation_keys, signing.did(), handle.as_str());

        // Signing with the operational key keeps cold-recovery off the signing
        // path; atrium-crypto already emits atproto's canonical low-S form.
        let signing_bytes = op.signing_bytes()?;
        let sig_bytes = operational.sign(&signing_bytes)?;
        let sig = URL_SAFE_NO_PAD.encode(&sig_bytes);

        let signed = op.into_signed(sig);
        let did = Did::new(signed.did()?);
        // The genesis op's CID — the `prev` a future operation chains onto.
        let genesis_cid = signed.cid()?;
        let op_json = signed.to_json()?;

        let keys = AccountKeys {
            cold_recovery: SecretKey::new(cold.export()),
            operational: SecretKey::new(operational.export()),
            signing: SecretKey::new(signing.export()),
        };

        // Private writes first: custody, then the genesis op.
        self.key_store.put(&did, &keys).await?;
        self.op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: genesis_cid,
                op_type: "plc_operation".to_string(),
                prev: None,
                operation_json: op_json.to_string(),
            })
            .await?;
        // Public dual-write — a separate retryable step, never a shared transaction.
        self.directory.submit(did.as_str(), &op_json).await?;

        Ok(did)
    }

    /// Tombstone `did`: sign a `plc_tombstone` with the operational rotation
    /// key, chaining onto the DID's latest logged operation, then submit before
    /// recording — so a failed submit never advances the local chain. Fails
    /// retryably if the DID has no custody keys or no op to chain onto.
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()> {
        let keys = self
            .key_store
            .get(did)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no custody keys to tombstone {}", did.as_str()))?;
        let prev = self.op_log.latest_cid(did).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "no prior PLC operation to chain a tombstone onto for {}",
                did.as_str()
            )
        })?;

        let operational = Secp256k1Keypair::import(keys.operational.expose())?;
        let op = TombstoneOperation::new(prev.clone());
        let sig_bytes = operational.sign(&op.signing_bytes()?)?;
        let signed = op.into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes));
        let cid = signed.cid()?;
        let op_json = signed.to_json()?;

        // Submission first, so a failed submit never advances the local chain.
        self.directory.submit(did.as_str(), &op_json).await?;
        self.op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid,
                op_type: "plc_tombstone".to_string(),
                prev: Some(prev),
                operation_json: op_json.to_string(),
            })
            .await?;

        Ok(())
    }

    /// Re-point `did`'s `alsoKnownAs` to `handle`: sign a `plc_operation` with
    /// the operational rotation key, chaining onto the DID's latest logged op,
    /// which also supplies the carried-forward public document fields; a
    /// non-identity-only prior op is rejected, never rewritten. Submits before
    /// recording; an identical replay is idempotent and a competing update on the
    /// same `prev` errors rather than forking the chain.
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()> {
        // The latest op's `cid` is our `prev`; its JSON holds the public document
        // fields we preserve, never re-derived from the custodied private keys.
        let prior = self.op_log.latest_op(did).await?.ok_or_else(|| {
            anyhow::anyhow!(
                "no prior PLC operation to chain an update onto for {}",
                did.as_str()
            )
        })?;
        // An update rebuilds an identity-only op, so a prior op of any other
        // shape must fail loud rather than silently drop its fields.
        if prior.op_type != "plc_operation" {
            anyhow::bail!(
                "cannot update {}: its latest op is `{}`, not a chainable plc_operation",
                did.as_str(),
                prior.op_type
            );
        }
        let prior_json: serde_json::Value = serde_json::from_str(&prior.operation_json)?;
        let rotation_keys = string_array(&prior_json, "rotationKeys")?;
        let services_empty = prior_json["services"]
            .as_object()
            .is_none_or(serde_json::Map::is_empty);
        let verification_only_atproto = prior_json["verificationMethods"]
            .as_object()
            .is_some_and(|vm| vm.len() == 1 && vm.contains_key("atproto"));
        if !(services_empty && verification_only_atproto) {
            anyhow::bail!(
                "cannot update {}: its latest op is not identity-only (unexpected services or \
                 verification methods); carrying those forward is not implemented",
                did.as_str()
            );
        }
        let atproto_signing_did = prior_json["verificationMethods"]["atproto"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "prior op for {} has no verificationMethods.atproto",
                    did.as_str()
                )
            })?
            .to_string();

        // Only the operational key is decrypted; the others stay sealed.
        let keys = self
            .key_store
            .get(did)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no custody keys to update {}", did.as_str()))?;
        let operational = Secp256k1Keypair::import(keys.operational.expose())?;

        let op = PlcOperation::update_handle(
            rotation_keys,
            atproto_signing_did,
            handle.as_str(),
            prior.cid.clone(),
        );
        let sig_bytes = operational.sign(&op.signing_bytes()?)?;
        let signed = op.into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes));
        let cid = signed.cid()?;
        let op_json = signed.to_json()?;

        // Submission first, so a failed submit never advances the local chain.
        self.directory.submit(did.as_str(), &op_json).await?;
        let append = self
            .op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: cid.clone(),
                op_type: "plc_operation".to_string(),
                prev: Some(prior.cid),
                operation_json: op_json.to_string(),
            })
            .await;
        if let Err(err) = append {
            // Benign only if the log's tip already IS our exact op (an identical
            // writer landed it); otherwise propagate so the caller retries onto
            // the new tip rather than forking.
            if self.op_log.latest_cid(did).await?.as_deref() == Some(cid.as_str()) {
                return Ok(());
            }
            return Err(err);
        }
        Ok(())
    }
}

/// Extract a JSON string array as `Vec<String>`, erroring if the field is
/// missing, not an array, or holds a non-string element.
fn string_array(value: &serde_json::Value, field: &str) -> anyhow::Result<Vec<String>> {
    value[field]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("prior op field `{field}` is missing or not an array"))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("prior op field `{field}` has a non-string element"))
        })
        .collect()
}

/// `did:plc` base32 alphabet (RFC 4648, lowercase, no padding).
const PLC_BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// A synthetic floor stub for [`DidMinter`]: a well-formed but entirely
/// synthetic `did:plc`, with no keypair, operation, or directory write. It
/// registers nowhere, for dev and tests that only need a DID-shaped value.
#[derive(Debug, Default, Clone)]
pub struct StubDidMinter;

impl StubDidMinter {
    /// Construct the stub; it is stateless.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DidMinter for StubDidMinter {
    /// Return `did:plc:` + 24 random lowercase base32 chars. `handle` is ignored
    /// and the DID resolves nowhere; purely local, so it never fails.
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        let mut rng = rand::thread_rng();
        let suffix: String = (0..24)
            .map(|_| PLC_BASE32[rng.gen_range(0..PLC_BASE32.len())] as char)
            .collect();
        Ok(Did::new(format!("did:plc:{suffix}")))
    }

    /// No-op: the stub registers no operation, so there is nothing to tombstone.
    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
        Ok(())
    }

    /// No-op: the stub custodies no keys and has no `alsoKnownAs` to re-point.
    async fn update_handle(&self, _did: &Did, _handle: &Handle) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plc_directory::{NoopPlcDirectory, PlcDirectory};
    use adapter_mem::{MemKeyStore, MemPlcOperationLog};
    use atrium_crypto::verify::verify_signature;
    use k256::ecdsa::Signature;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn handle() -> Handle {
        "alice.zurfur.app".parse::<Handle>().unwrap()
    }

    /// A directory that records the DID it was asked to submit, then fails.
    struct FailingPlcDirectory {
        seen_did: Arc<Mutex<Option<String>>>,
    }
    #[async_trait]
    impl PlcDirectory for FailingPlcDirectory {
        async fn submit(&self, did: &str, _operation: &serde_json::Value) -> anyhow::Result<()> {
            *self.seen_did.lock().unwrap() = Some(did.to_string());
            anyhow::bail!("simulated directory failure")
        }
    }

    /// A directory that records whether it was reached at all.
    struct RecordingPlcDirectory {
        called: Arc<AtomicBool>,
    }
    #[async_trait]
    impl PlcDirectory for RecordingPlcDirectory {
        async fn submit(&self, _did: &str, _operation: &serde_json::Value) -> anyhow::Result<()> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    /// A KeyStore whose write always fails — to prove submission is not reached.
    struct FailingKeyStore;
    #[async_trait]
    impl KeyStore for FailingKeyStore {
        async fn put(&self, _did: &Did, _keys: &AccountKeys) -> anyhow::Result<()> {
            anyhow::bail!("simulated key-store failure")
        }
        async fn get(&self, _did: &Did) -> anyhow::Result<Option<AccountKeys>> {
            Ok(None)
        }
    }

    // The real minter produces a well-formed did:plc: 24 base32 chars, and it is a
    // REAL derivation (not the stub's random suffix) — proven by the vector test in
    // plc.rs; here we assert shape + that keys were custodied.
    #[tokio::test]
    async fn real_mint_produces_well_formed_did_and_stores_keys() {
        let store = Arc::new(MemKeyStore::new());
        let minter = RealDidMinter::new(
            store.clone(),
            Arc::new(MemPlcOperationLog::new()),
            Box::new(NoopPlcDirectory),
        );

        let did = minter.mint(&handle()).await.unwrap();

        assert!(did.starts_with("did:plc:"));
        let suffix = &did.as_str()["did:plc:".len()..];
        assert_eq!(suffix.len(), 24, "did:plc suffix is 24 base32 chars");
        assert!(suffix.bytes().all(|b| PLC_BASE32.contains(&b)));

        // Keys were persisted for exactly this DID.
        let keys = store.get(&did).await.unwrap().expect("keys custodied");
        assert_eq!(keys.cold_recovery.expose().len(), 32);
        assert_eq!(keys.operational.expose().len(), 32);
        assert_eq!(keys.signing.expose().len(), 32);
        // The three keys are distinct — never a shared key.
        assert_ne!(keys.cold_recovery, keys.operational);
        assert_ne!(keys.operational, keys.signing);
    }

    // Distinct mints get distinct DIDs and distinct keys — no account shares a
    // sovereign identity or a rotation key.
    #[tokio::test]
    async fn distinct_mints_are_independent() {
        let store = Arc::new(MemKeyStore::new());
        let minter = RealDidMinter::new(
            store.clone(),
            Arc::new(MemPlcOperationLog::new()),
            Box::new(NoopPlcDirectory),
        );

        let a = minter.mint(&handle()).await.unwrap();
        let b = minter.mint(&handle()).await.unwrap();

        assert_ne!(a, b);
        let ka = store.get(&a).await.unwrap().unwrap();
        let kb = store.get(&b).await.unwrap().unwrap();
        assert_ne!(
            ka.operational, kb.operational,
            "per-account keys, never shared"
        );
    }

    // The genesis operation is signed by a listed rotation key (the operational
    // key, index 1), low-S, 64-byte — a valid, verifiable genesis signature. We
    // rebuild the exact signed operation from the stored keys and check it.
    #[tokio::test]
    async fn genesis_signature_is_valid_low_s_and_from_a_rotation_key() {
        let store = Arc::new(MemKeyStore::new());
        let minter = RealDidMinter::new(
            store.clone(),
            Arc::new(MemPlcOperationLog::new()),
            Box::new(NoopPlcDirectory),
        );
        let h = handle();

        let did = minter.mint(&h).await.unwrap();
        let keys = store.get(&did).await.unwrap().unwrap();

        // Reconstruct the keypairs and re-derive the operation the minter signed.
        let operational = Secp256k1Keypair::import(keys.operational.expose()).unwrap();
        let cold = Secp256k1Keypair::import(keys.cold_recovery.expose()).unwrap();
        let signing = Secp256k1Keypair::import(keys.signing.expose()).unwrap();
        let op = PlcOperation::identity_only(
            vec![cold.did(), operational.did()],
            signing.did(),
            h.as_str(),
        );
        let signing_bytes = op.signing_bytes().unwrap();
        let sig_bytes = operational.sign(&signing_bytes).unwrap();

        // 64-byte compact r‖s, and already low-S (normalize_s is a no-op).
        assert_eq!(sig_bytes.len(), 64, "compact 64-byte r‖s");
        let s = Signature::from_slice(&sig_bytes).unwrap();
        assert!(s.normalize_s().is_none(), "signature must already be low-S");

        // Verifies under the operational rotation key's did:key.
        verify_signature(&operational.did(), &signing_bytes, &sig_bytes).unwrap();
    }

    // Closes the view()-mapping gap: the vector test derives via `derive_did`
    // directly, so the PRODUCTION path (identity_only → sign → into_signed →
    // SignedOperation::did()/view()) is otherwise pinned only by shape. Re-signing
    // is deterministic (RFC 6979), so reconstructing the op from the stored keys must
    // reproduce the *exact* minted DID — proving the whole production field-mapping.
    #[tokio::test]
    async fn minted_did_reproduces_from_stored_keys() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let store = Arc::new(MemKeyStore::new());
        let minter = RealDidMinter::new(
            store.clone(),
            Arc::new(MemPlcOperationLog::new()),
            Box::new(NoopPlcDirectory),
        );
        let h = handle();

        let did = minter.mint(&h).await.unwrap();
        let keys = store.get(&did).await.unwrap().unwrap();

        let operational = Secp256k1Keypair::import(keys.operational.expose()).unwrap();
        let cold = Secp256k1Keypair::import(keys.cold_recovery.expose()).unwrap();
        let signing = Secp256k1Keypair::import(keys.signing.expose()).unwrap();
        let op = PlcOperation::identity_only(
            vec![cold.did(), operational.did()],
            signing.did(),
            h.as_str(),
        );
        let sig_bytes = operational.sign(&op.signing_bytes().unwrap()).unwrap();
        let signed = op.into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes));

        assert_eq!(
            Did::new(signed.did().unwrap()),
            did,
            "the production mint path must reproduce the same did:plc"
        );
    }

    // Failure ordering: keys are persisted BEFORE the operation is submitted, so a
    // submission failure leaves the keys in place (a retry never orphans them).
    #[tokio::test]
    async fn keys_persist_when_directory_submission_fails() {
        let store = Arc::new(MemKeyStore::new());
        let seen_did = Arc::new(Mutex::new(None));
        let directory = Box::new(FailingPlcDirectory {
            seen_did: seen_did.clone(),
        });
        let minter = RealDidMinter::new(
            store.clone(),
            Arc::new(MemPlcOperationLog::new()),
            directory,
        );

        let result = minter.mint(&handle()).await;

        assert!(result.is_err(), "mint fails when submission fails");
        let did = seen_did
            .lock()
            .unwrap()
            .clone()
            .expect("submission was reached (so keys were already written)");
        assert!(
            store.get(&Did::new(did)).await.unwrap().is_some(),
            "custody keys must remain persisted after a submission failure"
        );
    }

    // Failure ordering: if the key write fails, the directory is NEVER reached — no
    // operation is published for a DID whose keys we could not custody.
    #[tokio::test]
    async fn directory_is_not_reached_when_key_write_fails() {
        let called = Arc::new(AtomicBool::new(false));
        let directory = Box::new(RecordingPlcDirectory {
            called: called.clone(),
        });
        let minter = RealDidMinter::new(
            Arc::new(FailingKeyStore),
            Arc::new(MemPlcOperationLog::new()),
            directory,
        );

        let result = minter.mint(&handle()).await;

        assert!(result.is_err(), "mint fails when the key write fails");
        assert!(
            !called.load(Ordering::SeqCst),
            "directory.submit must not run when the key write fails"
        );
    }

    #[tokio::test]
    async fn stub_mint_produces_synthetic_did_plc() {
        let did = StubDidMinter::new().mint(&handle()).await.unwrap();
        let value = did.as_str();
        assert!(
            value.starts_with("did:plc:"),
            "expected did:plc prefix, got {value}"
        );
        assert_eq!(value.len(), 32, "unexpected DID length: {value}");
        let suffix = &value["did:plc:".len()..];
        assert_eq!(suffix.len(), 24);
        assert!(suffix.bytes().all(|b| PLC_BASE32.contains(&b)));
    }

    /// A directory that records the last operation JSON it was asked to submit.
    struct CapturingPlcDirectory {
        last: Arc<Mutex<Option<serde_json::Value>>>,
    }
    #[async_trait]
    impl PlcDirectory for CapturingPlcDirectory {
        async fn submit(&self, _did: &str, operation: &serde_json::Value) -> anyhow::Result<()> {
            *self.last.lock().unwrap() = Some(operation.clone());
            Ok(())
        }
    }

    // The security-critical path: minting records the genesis op, and `tombstone`
    // signs a `plc_tombstone` that (a) chains onto the genesis op's CID as its `prev`
    // and (b) is signed by the OPERATIONAL rotation key — a valid, verifiable, low-S
    // signature. If `prev` or the signing were wrong, the canonical directory would
    // reject the tombstone.
    #[tokio::test]
    async fn tombstone_chains_onto_genesis_and_is_signed_by_the_operational_key() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let last = Arc::new(Mutex::new(None));
        let directory = Box::new(CapturingPlcDirectory { last: last.clone() });
        let minter = RealDidMinter::new(store.clone(), op_log.clone(), directory);

        // Mint (records the genesis op), then tombstone.
        let did = minter.mint(&handle()).await.unwrap();
        let genesis_cid = op_log
            .latest_cid(&did)
            .await
            .unwrap()
            .expect("the genesis op was logged");

        minter.tombstone(&did).await.unwrap();

        // The submitted op is a tombstone chaining onto the genesis CID.
        let submitted = last
            .lock()
            .unwrap()
            .clone()
            .expect("a tombstone was submitted");
        assert_eq!(submitted["type"], "plc_tombstone");
        assert_eq!(
            submitted["prev"], genesis_cid,
            "the tombstone chains onto the genesis op CID"
        );

        // The tombstone was logged as the DID's new latest op (chained on `prev`).
        let latest = op_log.latest_cid(&did).await.unwrap().unwrap();
        assert_ne!(
            latest, genesis_cid,
            "the log's latest op is now the tombstone"
        );

        // The signature verifies under the operational rotation key's did:key, low-S.
        let keys = store.get(&did).await.unwrap().unwrap();
        let operational = Secp256k1Keypair::import(keys.operational.expose()).unwrap();
        let signing_bytes = TombstoneOperation::new(genesis_cid)
            .signing_bytes()
            .unwrap();
        let sig_bytes = URL_SAFE_NO_PAD
            .decode(submitted["sig"].as_str().unwrap())
            .unwrap();
        assert_eq!(sig_bytes.len(), 64, "compact 64-byte r‖s");
        let s = Signature::from_slice(&sig_bytes).unwrap();
        assert!(s.normalize_s().is_none(), "signature must already be low-S");
        verify_signature(&operational.did(), &signing_bytes, &sig_bytes).unwrap();
    }

    fn new_handle() -> Handle {
        "bob.zurfur.app".parse::<Handle>().unwrap()
    }

    /// The fields of one appended record, as [`RecordingOpLog`] stores them.
    #[derive(Clone)]
    struct StoredOp {
        did: String,
        cid: String,
        op_type: String,
        prev: Option<String>,
        operation_json: String,
    }

    /// An op log keeping full records, mirroring the pg adapter's two integrity
    /// indexes: `UNIQUE(cid)` and `UNIQUE(did, prev)`.
    #[derive(Clone, Default)]
    struct RecordingOpLog {
        /// Appended records, in append order.
        records: Arc<Mutex<Vec<StoredOp>>>,
    }
    #[async_trait]
    impl PlcOperationLog for RecordingOpLog {
        async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
            let mut records = self.records.lock().unwrap();
            if records.iter().any(|stored| stored.cid == record.cid) {
                anyhow::bail!("plc operation {} already logged", record.cid);
            }
            if let Some(prev) = &record.prev
                && records.iter().any(|stored| {
                    stored.did == record.did.as_str() && stored.prev.as_deref() == Some(prev)
                })
            {
                anyhow::bail!("plc operation already chains onto {prev} (chain would fork)");
            }
            records.push(StoredOp {
                did: record.did.to_string(),
                cid: record.cid.clone(),
                op_type: record.op_type.clone(),
                prev: record.prev.clone(),
                operation_json: record.operation_json.clone(),
            });
            Ok(())
        }
        async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>> {
            Ok(self
                .records
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|stored| stored.did == did.as_str())
                .map(|stored| stored.cid.clone()))
        }
        async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>> {
            Ok(self
                .records
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|stored| stored.did == did.as_str())
                .map(|stored| PlcOperationRecord {
                    did: did.clone(),
                    cid: stored.cid.clone(),
                    op_type: stored.op_type.clone(),
                    prev: stored.prev.clone(),
                    operation_json: stored.operation_json.clone(),
                }))
        }
    }

    /// An op log that loses the append race ONCE: the next `append` first lands
    /// the identical record, so the minter's own append hits `UNIQUE(cid)`.
    struct RacingOpLog {
        inner: RecordingOpLog,
        race_pending: AtomicBool,
    }
    #[async_trait]
    impl PlcOperationLog for RacingOpLog {
        async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
            if self.race_pending.swap(false, Ordering::SeqCst) {
                self.inner.append(record).await?;
            }
            self.inner.append(record).await
        }
        async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>> {
            self.inner.latest_cid(did).await
        }
        async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>> {
            self.inner.latest_op(did).await
        }
    }

    /// An op log where a DIFFERENT writer wins the append race: the first
    /// `append` lands a pre-seeded `winner` op chaining the same `prev`, so the
    /// minter's own append hits the `UNIQUE(did, prev)` fork guard.
    struct ForkRaceOpLog {
        inner: RecordingOpLog,
        winner: Mutex<Option<PlcOperationRecord>>,
    }
    #[async_trait]
    impl PlcOperationLog for ForkRaceOpLog {
        async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
            // Take the winner out of the lock BEFORE awaiting (never hold a std Mutex
            // guard across `.await`).
            let winner = self.winner.lock().unwrap().take();
            if let Some(winner) = winner {
                self.inner.append(&winner).await?;
            }
            self.inner.append(record).await
        }
        async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>> {
            self.inner.latest_cid(did).await
        }
        async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>> {
            self.inner.latest_op(did).await
        }
    }

    // The update chains onto the DID's latest LOGGED op (never fetched from the
    // directory): its `prev` is the genesis CID, it REPLACES `alsoKnownAs` with the
    // new handle, and the log advances so the update is now the DID's latest op.
    #[tokio::test]
    async fn update_chains_onto_latest_logged_op() {
        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let last = Arc::new(Mutex::new(None));
        let minter = RealDidMinter::new(
            store.clone(),
            op_log.clone(),
            Box::new(CapturingPlcDirectory { last: last.clone() }),
        );

        let did = minter.mint(&handle()).await.unwrap();
        let genesis_cid = op_log.latest_cid(&did).await.unwrap().unwrap();

        minter.update_handle(&did, &new_handle()).await.unwrap();

        let submitted = last
            .lock()
            .unwrap()
            .clone()
            .expect("an update was submitted");
        assert_eq!(submitted["type"], "plc_operation");
        assert_eq!(
            submitted["prev"], genesis_cid,
            "the update chains onto the DID's latest logged op"
        );
        assert_eq!(
            submitted["alsoKnownAs"],
            serde_json::json!(["at://bob.zurfur.app"]),
            "alsoKnownAs is REPLACED with the new handle"
        );

        let latest = op_log.latest_cid(&did).await.unwrap().unwrap();
        assert_ne!(latest, genesis_cid, "the log's latest op is now the update");
    }

    // The update is signed by the OPERATIONAL rotation key (`rotationKeys[1]`) —
    // a valid, verifiable, low-S signature over the update's no-`sig` DAG-CBOR.
    // We rebuild the exact operation from the stored keys and check the submitted
    // signature against it, mirroring the genesis signature test.
    #[tokio::test]
    async fn update_is_signed_by_the_operational_key() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let last = Arc::new(Mutex::new(None));
        let minter = RealDidMinter::new(
            store.clone(),
            op_log.clone(),
            Box::new(CapturingPlcDirectory { last: last.clone() }),
        );

        let did = minter.mint(&handle()).await.unwrap();
        let genesis_cid = op_log.latest_cid(&did).await.unwrap().unwrap();

        minter.update_handle(&did, &new_handle()).await.unwrap();
        let submitted = last.lock().unwrap().clone().unwrap();

        // Rebuild the exact signed-over bytes from the custodied keys.
        let keys = store.get(&did).await.unwrap().unwrap();
        let operational = Secp256k1Keypair::import(keys.operational.expose()).unwrap();
        let cold = Secp256k1Keypair::import(keys.cold_recovery.expose()).unwrap();
        let signing = Secp256k1Keypair::import(keys.signing.expose()).unwrap();
        let signing_bytes = PlcOperation::update_handle(
            vec![cold.did(), operational.did()],
            signing.did(),
            new_handle().as_str(),
            genesis_cid,
        )
        .signing_bytes()
        .unwrap();

        let sig_bytes = URL_SAFE_NO_PAD
            .decode(submitted["sig"].as_str().unwrap())
            .unwrap();
        assert_eq!(sig_bytes.len(), 64, "compact 64-byte r‖s");
        let s = Signature::from_slice(&sig_bytes).unwrap();
        assert!(s.normalize_s().is_none(), "signature must already be low-S");
        verify_signature(&operational.did(), &signing_bytes, &sig_bytes).unwrap();
    }

    // The update is durably logged: `update_handle` appends a `plc_operation`
    // record whose `prev` is the genesis CID and whose `cid` is exactly the
    // deterministic content id of the signed update.
    #[tokio::test]
    async fn update_appends_a_plc_operation_record() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(RecordingOpLog::default());
        let minter = RealDidMinter::new(store.clone(), op_log.clone(), Box::new(NoopPlcDirectory));

        let did = minter.mint(&handle()).await.unwrap();
        let genesis_cid = op_log.latest_cid(&did).await.unwrap().unwrap();

        minter.update_handle(&did, &new_handle()).await.unwrap();

        // Recompute the expected CID from the custodied keys (signing is
        // deterministic, so this is the exact op the minter signed).
        let keys = store.get(&did).await.unwrap().unwrap();
        let operational = Secp256k1Keypair::import(keys.operational.expose()).unwrap();
        let cold = Secp256k1Keypair::import(keys.cold_recovery.expose()).unwrap();
        let signing = Secp256k1Keypair::import(keys.signing.expose()).unwrap();
        let op = PlcOperation::update_handle(
            vec![cold.did(), operational.did()],
            signing.did(),
            new_handle().as_str(),
            genesis_cid.clone(),
        );
        let sig_bytes = operational.sign(&op.signing_bytes().unwrap()).unwrap();
        let expected_cid = op
            .into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes))
            .cid()
            .unwrap();

        let records = op_log.records.lock().unwrap();
        assert_eq!(records.len(), 2, "genesis + the update");
        let update = &records[1];
        assert_eq!(update.did, did.as_str());
        assert_eq!(update.op_type, "plc_operation");
        assert_eq!(update.prev.as_deref(), Some(genesis_cid.as_str()));
        assert_eq!(
            update.cid, expected_cid,
            "the logged cid is the signed op's content id"
        );
    }

    // IDEMPOTENT REPLAY: signing is deterministic, so an identical update (same
    // prev, handle, keys) has the same CID. If the identical op already landed —
    // here a simulated concurrent retry that wins the append race — the
    // duplicate-cid rejection is treated as SUCCESS (the op IS logged), so the
    // caller can retry blindly: no error, and no second row.
    #[tokio::test]
    async fn replaying_an_identical_update_is_idempotent() {
        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(RacingOpLog {
            inner: RecordingOpLog::default(),
            race_pending: AtomicBool::new(false),
        });
        let minter = RealDidMinter::new(store.clone(), op_log.clone(), Box::new(NoopPlcDirectory));

        let did = minter.mint(&handle()).await.unwrap();

        // The identical op lands first (racing retry); our append hits UNIQUE(cid).
        op_log.race_pending.store(true, Ordering::SeqCst);
        minter
            .update_handle(&did, &new_handle())
            .await
            .expect("a benign replay (identical op already logged) is success, not an error");

        let records = op_log.inner.records.lock().unwrap();
        assert_eq!(
            records.len(),
            2,
            "genesis + exactly ONE update row — the replay appended nothing"
        );
    }

    // RETRYABLE FAILURE: a failed directory submission must not advance the local
    // chain (submit-before-record, like the tombstone) — so a clean retry re-reads
    // the SAME `prev`, re-signs the SAME deterministic op, and lands it once.
    #[tokio::test]
    async fn update_survives_directory_submission_failure_retryably() {
        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let minter = RealDidMinter::new(store.clone(), op_log.clone(), Box::new(NoopPlcDirectory));
        let did = minter.mint(&handle()).await.unwrap();
        let genesis_cid = op_log.latest_cid(&did).await.unwrap().unwrap();

        // First attempt: the directory fails; nothing may be logged.
        let failing = RealDidMinter::new(
            store.clone(),
            op_log.clone(),
            Box::new(FailingPlcDirectory {
                seen_did: Arc::new(Mutex::new(None)),
            }),
        );
        assert!(
            failing.update_handle(&did, &new_handle()).await.is_err(),
            "update fails when submission fails"
        );
        assert_eq!(
            op_log.latest_cid(&did).await.unwrap().unwrap(),
            genesis_cid,
            "a failed submit must not advance the local chain (no orphaned op)"
        );

        // Retry with a healthy directory: same prev → the same op, landed once.
        let last = Arc::new(Mutex::new(None));
        let retrying = RealDidMinter::new(
            store.clone(),
            op_log.clone(),
            Box::new(CapturingPlcDirectory { last: last.clone() }),
        );
        retrying
            .update_handle(&did, &new_handle())
            .await
            .expect("the retry succeeds");

        let submitted = last.lock().unwrap().clone().unwrap();
        assert_eq!(
            submitted["prev"], genesis_cid,
            "the retry chains onto the same prev the failed attempt read"
        );
        assert_ne!(
            op_log.latest_cid(&did).await.unwrap().unwrap(),
            genesis_cid,
            "the retried update is now the DID's latest logged op"
        );
    }

    // INITIAL-MAINTAIN: calling update_handle again after a fully-landed update is
    // NOT a replay — `prev` has advanced, so it signs a NEW op chaining onto the
    // previous update, re-asserting the current handle (the briefing's
    // "initial-maintain" use; DD 27852802).
    #[tokio::test]
    async fn re_asserting_the_current_handle_chains_a_new_op() {
        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let last = Arc::new(Mutex::new(None));
        let minter = RealDidMinter::new(
            store.clone(),
            op_log.clone(),
            Box::new(CapturingPlcDirectory { last: last.clone() }),
        );

        let did = minter.mint(&handle()).await.unwrap();
        minter.update_handle(&did, &new_handle()).await.unwrap();
        let first_update_cid = op_log.latest_cid(&did).await.unwrap().unwrap();

        minter
            .update_handle(&did, &new_handle())
            .await
            .expect("re-asserting the current handle is valid");

        let submitted = last.lock().unwrap().clone().unwrap();
        assert_eq!(
            submitted["prev"], first_update_cid,
            "the re-assertion chains onto the previous update"
        );
        assert_ne!(
            op_log.latest_cid(&did).await.unwrap().unwrap(),
            first_update_cid,
            "the re-assertion is a new op in the chain"
        );
    }

    // update_handle fails retryably when the DID has no custody keys or no logged
    // op to chain onto — never signs an unchainable or unsigned-able update.
    #[tokio::test]
    async fn update_fails_without_custody_or_a_prior_op() {
        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let minter = RealDidMinter::new(store.clone(), op_log.clone(), Box::new(NoopPlcDirectory));

        // Unknown DID: no keys, no ops.
        let unknown = Did::new("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        assert!(minter.update_handle(&unknown, &new_handle()).await.is_err());
    }

    // NO CHAIN FORK (F1): two updates cannot both chain the same `prev`. When a
    // DIFFERENT concurrent writer lands its op onto the DID's tip first, our append
    // hits `UNIQUE(did, prev)`; the tip is not our op, so `update_handle` propagates
    // the error (never silently forks), and a retry re-reads the NEW tip and chains
    // onto it — serializing the two writers into ONE linear chain.
    #[tokio::test]
    async fn a_forking_update_is_rejected_and_the_chain_stays_linear() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(ForkRaceOpLog {
            inner: RecordingOpLog::default(),
            winner: Mutex::new(None),
        });
        let last = Arc::new(Mutex::new(None));
        let minter = RealDidMinter::new(
            store.clone(),
            op_log.clone(),
            Box::new(CapturingPlcDirectory { last: last.clone() }),
        );

        let did = minter.mint(&handle()).await.unwrap();
        let genesis_cid = op_log.latest_cid(&did).await.unwrap().unwrap();

        // Build a DIFFERENT concurrent winner (a change to a THIRD handle) chaining
        // the same genesis `prev`, and arm it to land first on the next append.
        let keys = store.get(&did).await.unwrap().unwrap();
        let operational = Secp256k1Keypair::import(keys.operational.expose()).unwrap();
        let cold = Secp256k1Keypair::import(keys.cold_recovery.expose()).unwrap();
        let signing = Secp256k1Keypair::import(keys.signing.expose()).unwrap();
        let winner_op = PlcOperation::update_handle(
            vec![cold.did(), operational.did()],
            signing.did(),
            "carol.zurfur.app",
            genesis_cid.clone(),
        );
        let winner_sig = operational
            .sign(&winner_op.signing_bytes().unwrap())
            .unwrap();
        let winner_signed = winner_op.into_signed(URL_SAFE_NO_PAD.encode(&winner_sig));
        let winner_cid = winner_signed.cid().unwrap();
        let winner_json = winner_signed.to_json().unwrap();
        *op_log.winner.lock().unwrap() = Some(PlcOperationRecord {
            did: did.clone(),
            cid: winner_cid.clone(),
            op_type: "plc_operation".to_string(),
            prev: Some(genesis_cid.clone()),
            operation_json: winner_json.to_string(),
        });

        // Our update reads prev=genesis, but the winner lands first chaining genesis;
        // our append hits UNIQUE(did, prev) and the error propagates (no silent fork).
        assert!(
            minter.update_handle(&did, &new_handle()).await.is_err(),
            "a fork (a second op chaining the same prev) is rejected, not accepted"
        );
        {
            let records = op_log.inner.records.lock().unwrap();
            assert_eq!(
                records.len(),
                2,
                "genesis + the winner — no forked third op"
            );
            assert_eq!(records[1].cid, winner_cid);
        }
        assert_eq!(
            op_log.latest_cid(&did).await.unwrap().unwrap(),
            winner_cid,
            "the winner is the DID's tip"
        );

        // Retry: now reads prev=winner and chains onto it — one linear chain.
        minter
            .update_handle(&did, &new_handle())
            .await
            .expect("the retry serializes onto the new tip");
        assert_eq!(
            last.lock().unwrap().clone().unwrap()["prev"],
            winner_cid,
            "the retry chains onto the winner, not the stale genesis"
        );
        assert_eq!(
            op_log.inner.records.lock().unwrap().len(),
            3,
            "genesis → winner → the retried update: one linear chain, never a fork"
        );
    }

    // KEY HYGIENE (F2): a routine update needs ONLY the operational key. Here custody
    // holds a valid operational key but GARBAGE (all-zero, un-importable) cold-recovery
    // and signing scalars; the update still succeeds — proving those private keys are
    // never imported — by carrying the public rotationKeys/verificationMethods forward
    // from the logged op, and signs with the operational key alone.
    #[tokio::test]
    async fn update_uses_only_the_operational_key_carrying_public_fields_from_the_log() {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        // A genesis op signed with REAL keys, logged as the DID's only op.
        let cold = Secp256k1Keypair::create(&mut rand::thread_rng());
        let operational = Secp256k1Keypair::create(&mut rand::thread_rng());
        let signing = Secp256k1Keypair::create(&mut rand::thread_rng());
        let genesis = PlcOperation::identity_only(
            vec![cold.did(), operational.did()],
            signing.did(),
            "alice.zurfur.app",
        );
        let g_sig = operational.sign(&genesis.signing_bytes().unwrap()).unwrap();
        let genesis_signed = genesis.into_signed(URL_SAFE_NO_PAD.encode(&g_sig));
        let did = Did::new(genesis_signed.did().unwrap());
        let genesis_cid = genesis_signed.cid().unwrap();
        let genesis_json = genesis_signed.to_json().unwrap();

        // Custody: real operational key, but cold-recovery and signing are GARBAGE —
        // all-zero scalars that would fail `Secp256k1Keypair::import`.
        let store = Arc::new(MemKeyStore::new());
        store
            .put(
                &did,
                &AccountKeys {
                    cold_recovery: SecretKey::new(vec![0u8; 32]),
                    operational: SecretKey::new(operational.export()),
                    signing: SecretKey::new(vec![0u8; 32]),
                },
            )
            .await
            .unwrap();
        let op_log = Arc::new(MemPlcOperationLog::new());
        op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: genesis_cid.clone(),
                op_type: "plc_operation".to_string(),
                prev: None,
                operation_json: genesis_json.to_string(),
            })
            .await
            .unwrap();

        let last = Arc::new(Mutex::new(None));
        let minter = RealDidMinter::new(
            store,
            op_log,
            Box::new(CapturingPlcDirectory { last: last.clone() }),
        );

        // Succeeds despite un-importable cold/signing custody — they are never touched.
        minter
            .update_handle(&did, &new_handle())
            .await
            .expect("update needs only the operational key; the rest is public, read from the log");

        let submitted = last.lock().unwrap().clone().unwrap();
        assert_eq!(
            submitted["rotationKeys"],
            serde_json::json!([cold.did(), operational.did()]),
            "rotationKeys carried forward from the logged op, not re-derived from custody"
        );
        assert_eq!(
            submitted["verificationMethods"]["atproto"],
            signing.did(),
            "verificationMethods carried forward from the logged op"
        );
        // The update still verifies against the operational key over its own bytes.
        let sig_bytes = URL_SAFE_NO_PAD
            .decode(submitted["sig"].as_str().unwrap())
            .unwrap();
        let signing_bytes = PlcOperation::update_handle(
            vec![cold.did(), operational.did()],
            signing.did(),
            new_handle().as_str(),
            genesis_cid,
        )
        .signing_bytes()
        .unwrap();
        verify_signature(&operational.did(), &signing_bytes, &sig_bytes).unwrap();
    }

    // GUARD (review F: no silent field drop): an update refuses a TOMBSTONED DID —
    // its latest op is a `plc_tombstone`, which has no rotationKeys to chain an
    // identity-only update onto — with a clear error, not a confusing parse failure.
    #[tokio::test]
    async fn update_rejects_a_tombstoned_did() {
        let store = Arc::new(MemKeyStore::new());
        let op_log = Arc::new(MemPlcOperationLog::new());
        let minter = RealDidMinter::new(store.clone(), op_log.clone(), Box::new(NoopPlcDirectory));

        let did = minter.mint(&handle()).await.unwrap();
        minter.tombstone(&did).await.unwrap();

        let err = minter
            .update_handle(&did, &new_handle())
            .await
            .expect_err("cannot update a tombstoned DID")
            .to_string();
        assert!(
            err.contains("not a chainable plc_operation"),
            "the rejection names the tombstone clearly, got: {err}"
        );
    }

    // GUARD (review F: no silent field drop): if the prior op is NOT identity-only
    // (here a hand-seeded op carrying a `services` entry), an update REFUSES it rather
    // than rebuilding an empty-`services` op that would silently drop the PDS binding.
    // The guard fires before custody is even loaded (empty key store), so this proves
    // the check, not a downstream failure.
    #[tokio::test]
    async fn update_rejects_a_non_identity_only_prior() {
        let did = Did::new("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa".to_string());
        let op_log = Arc::new(MemPlcOperationLog::new());
        op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: "bafyreiprior".to_string(),
                op_type: "plc_operation".to_string(),
                prev: None,
                operation_json: serde_json::json!({
                    "type": "plc_operation",
                    "rotationKeys": ["did:key:cold", "did:key:hot"],
                    "verificationMethods": {"atproto": "did:key:sign"},
                    "alsoKnownAs": ["at://alice.zurfur.app"],
                    "services": {
                        "atproto_pds": {
                            "type": "AtprotoPersonalDataServer",
                            "endpoint": "https://pds.example"
                        }
                    },
                    "prev": serde_json::Value::Null
                })
                .to_string(),
            })
            .await
            .unwrap();

        // Empty key store — the guard must reject before any custody read.
        let minter = RealDidMinter::new(
            Arc::new(MemKeyStore::new()),
            op_log,
            Box::new(NoopPlcDirectory),
        );
        let err = minter
            .update_handle(&did, &new_handle())
            .await
            .expect_err("cannot update onto a non-identity-only op")
            .to_string();
        assert!(
            err.contains("not identity-only"),
            "the PDS binding is not silently dropped, got: {err}"
        );
    }

    // The stub minter's update is a no-op, matching its mint/tombstone: nothing
    // real was registered, so there is nothing to update.
    #[tokio::test]
    async fn stub_update_handle_is_a_noop() {
        StubDidMinter::new()
            .update_handle(&Did::new("did:plc:stub".to_string()), &new_handle())
            .await
            .unwrap();
    }
}
