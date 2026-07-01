//! The account `did:plc` minter — real ([`RealDidMinter`]) and stub
//! ([`StubDidMinter`]).
//!
//! [`DidMinter`] mints a sovereign `did:plc` for a platform-custodied entity (an
//! Account; see DESIGN/Account). [`RealDidMinter`] is the live implementation
//! (ZMVP-49): it generates per-account secp256k1 rotation keys, builds and signs
//! an **identity-only** genesis operation (no PDS — DD/26935298), derives the DID
//! from its hash, persists the keys envelope-encrypted through a [`KeyStore`], and
//! submits the operation to a PLC directory (a no-op/local one in v1 — C2).
//! [`StubDidMinter`] is kept as a synthetic floor stub for tests/dev.

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

use crate::plc::{GenesisOperation, TombstoneOperation};
use crate::plc_directory::PlcDirectory;

/// The real [`DidMinter`]: mints a genuine, custody-backed `did:plc`.
///
/// Holds the private-store [`KeyStore`] (where the per-account keys land,
/// envelope-encrypted) and a [`PlcDirectory`] (where the signed operation is
/// submitted — a no-op directory in ZMVP-49). Both are injected so the minter is
/// unit-testable against fakes.
pub struct RealDidMinter {
    key_store: Arc<dyn KeyStore>,
    op_log: Arc<dyn PlcOperationLog>,
    directory: Box<dyn PlcDirectory>,
}

impl RealDidMinter {
    /// Build the real minter over a [`KeyStore`] (custody), a [`PlcOperationLog`]
    /// (the chain of operations we've submitted, so the next op knows its `prev`), and
    /// a [`PlcDirectory`] (submission).
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
    /// Mint an identity-only `did:plc` bound to `handle`.
    ///
    /// Steps, in order: (1) generate three secp256k1 keypairs — cold-recovery,
    /// operational, and the `#atproto` signing key; (2) build the identity-only
    /// genesis operation with `rotationKeys = [cold, operational]` (descending
    /// authority) and `alsoKnownAs = [at://<handle>]`; (3) sign the operation's
    /// no-`sig` DAG-CBOR with the **operational** key (any listed rotation key is a
    /// valid genesis signer per the PLC spec; signing with the operational key
    /// keeps the cold-recovery key off the signing path from birth), low-S,
    /// base64url no-pad; (4) derive the DID from the signed operation's hash; (5)
    /// **persist the keys** via the [`KeyStore`] (private, encrypted at rest); then
    /// (6) **submit** the operation to the directory.
    ///
    /// Steps (5) and (6) are two independent writes across the private/public
    /// boundary — never one transaction (DESIGN/no cross-store transaction). Keys
    /// are stored before submission so a submission retry never orphans them.
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

        // rotationKeys in DESCENDING authority: cold-recovery first (index 0),
        // operational second (index 1). Index 0 is reserved above operational for a
        // future user recovery key (ZMVP-52) — DD/26804226 B2.
        let rotation_keys = vec![cold.did(), operational.did()];
        let op = GenesisOperation::identity_only(rotation_keys, signing.did(), handle.as_str());

        // Sign the no-`sig` DAG-CBOR with the operational key. atrium-crypto's
        // secp256k1 `sign` already emits atproto's canonical form (ECDSA-SHA256,
        // low-S, 64-byte r‖s); we base64url no-pad encode it into the operation.
        let signing_bytes = op.signing_bytes()?;
        let sig_bytes = operational.sign(&signing_bytes)?;
        let sig = URL_SAFE_NO_PAD.encode(&sig_bytes);

        let signed = op.into_signed(sig);
        let did = Did::new(signed.did()?);
        // The genesis op's CID — the `prev` a future operation (e.g. the tombstone)
        // will chain onto. Recorded in the operation log below.
        let genesis_cid = signed.cid()?;
        let op_json = signed.to_json()?;

        // Custody: keep every private half, in role order, for future operations.
        let keys = AccountKeys {
            cold_recovery: SecretKey::new(cold.export()),
            operational: SecretKey::new(operational.export()),
            signing: SecretKey::new(signing.export()),
        };

        // (5) Private write — keys encrypted at rest by the KeyStore adapter.
        self.key_store.put(&did, &keys).await?;
        // (5b) Private write — record the genesis op so the chain can be extended.
        self.op_log
            .append(&PlcOperationRecord {
                did: did.clone(),
                cid: genesis_cid,
                op_type: "plc_operation".to_string(),
                prev: None,
                operation_json: op_json.to_string(),
            })
            .await?;
        // (6) Public dual-write — separate, retryable step (no shared transaction).
        self.directory.submit(did.as_str(), &op_json).await?;

        Ok(did)
    }

    /// Tombstone `did` (ZMVP-34 hard-delete): sign a `plc_tombstone` with the
    /// account's **operational** rotation key, chaining onto the DID's most recent
    /// operation.
    ///
    /// Steps: (1) load the custody keys (the operational key signs; the cold-recovery
    /// key stays off the signing path but is retained so a higher-authority reversal is
    /// possible within the ~72h window); (2) read the DID's latest op CID from the log
    /// — the tombstone's mandatory `prev`; (3) build and sign the `plc_tombstone`'s
    /// no-`sig` DAG-CBOR (ECDSA-SHA256, low-S, base64url no-pad — the same procedure as
    /// the genesis op); (4) **submit** it to the directory (public); then (5) **record**
    /// it in the log (private). Submit-before-record — the opposite of [`mint`], where
    /// the genesis must be recorded before the DID is registered — so a failed submit
    /// never advances our local chain: a retry re-reads the correct `prev` (the DID's
    /// still-latest op) and re-signs the *same* tombstone, rather than chaining onto an
    /// unsubmitted one (which the unique `cid` index would also reject). Steps (4) and
    /// (5) are separate writes across the boundary — never one transaction — and this
    /// whole method runs only after the private hard-delete has committed. Fails
    /// (retryably) if the DID has no custody keys or no logged operation to chain onto.
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

        // (4) Public submission FIRST — so a failed submit never advances our local
        // chain. A retry then re-reads the correct `prev` and re-signs the same
        // tombstone (deterministic) rather than chaining onto an unsubmitted op. A
        // separate retryable step across the boundary, never a shared transaction.
        self.directory.submit(did.as_str(), &op_json).await?;
        // (5) Private write — record the now-submitted tombstone (chains onto `prev`).
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
}

/// `did:plc` base32 alphabet (RFC 4648, lowercase, no padding). A real account DID
/// is `did:plc:` followed by 24 of these characters.
const PLC_BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// A synthetic floor stub for [`DidMinter`]: returns a structurally valid-looking
/// but entirely **synthetic** `did:plc` — `did:plc:` plus 24 random lowercase
/// base32 characters — with no keypair, genesis operation, or directory write. It
/// mints nothing real and registers nowhere; kept for dev and for tests that only
/// need a DID-shaped value without the cost of real key generation.
#[derive(Debug, Default, Clone)]
pub struct StubDidMinter;

impl StubDidMinter {
    /// Construct the stub. It is stateless ([`Default`] does the same); `new`
    /// exists for symmetry with the real adapters.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DidMinter for StubDidMinter {
    /// Returns `did:plc:` + 24 random lowercase base32 chars. `handle` is accepted
    /// to match the port but ignored — the stub builds no operation, so there is
    /// no `alsoKnownAs` to bind it into. Purely local: no network, no keypair, so
    /// unlike the real minter it never fails. The value is well-formed but **not**
    /// registered anywhere; resolving it will not work.
    async fn mint(&self, _handle: &Handle) -> anyhow::Result<Did> {
        let mut rng = rand::thread_rng();
        let suffix: String = (0..24)
            .map(|_| PLC_BASE32[rng.gen_range(0..PLC_BASE32.len())] as char)
            .collect();
        Ok(Did::new(format!("did:plc:{suffix}")))
    }

    /// No-op: the stub builds and registers no operation, so there is nothing to
    /// tombstone. Present to satisfy the port.
    async fn tombstone(&self, _did: &Did) -> anyhow::Result<()> {
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
        Handle::try_new("alice.zurfur.app").unwrap()
    }

    /// A directory that records the DID it was asked to submit, then fails — to
    /// prove keys are persisted *before* submission (a retry never orphans them).
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
        let op = GenesisOperation::identity_only(
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
        let op = GenesisOperation::identity_only(
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
}
