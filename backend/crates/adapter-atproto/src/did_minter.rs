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
    },
    ports::{DidMinter, KeyStore},
};
use rand::Rng;
use std::sync::Arc;

use crate::plc::GenesisOperation;
use crate::plc_directory::PlcDirectory;

/// The real [`DidMinter`]: mints a genuine, custody-backed `did:plc`.
///
/// Holds the private-store [`KeyStore`] (where the per-account keys land,
/// envelope-encrypted) and a [`PlcDirectory`] (where the signed operation is
/// submitted — a no-op directory in ZMVP-49). Both are injected so the minter is
/// unit-testable against fakes.
pub struct RealDidMinter {
    key_store: Arc<dyn KeyStore>,
    directory: Box<dyn PlcDirectory>,
}

impl RealDidMinter {
    /// Build the real minter over a [`KeyStore`] and a [`PlcDirectory`].
    pub fn new(key_store: Arc<dyn KeyStore>, directory: Box<dyn PlcDirectory>) -> Self {
        Self {
            key_store,
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

        // Custody: keep every private half, in role order, for future operations.
        let keys = AccountKeys {
            cold_recovery: SecretKey::new(cold.export()),
            operational: SecretKey::new(operational.export()),
            signing: SecretKey::new(signing.export()),
        };

        // (5) Private write — keys encrypted at rest by the KeyStore adapter.
        self.key_store.put(&did, &keys).await?;
        // (6) Public dual-write — separate, retryable step (no shared transaction).
        self.directory.submit(did.as_str(), &signed).await?;

        Ok(did)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plc_directory::NoopPlcDirectory;
    use adapter_mem::MemKeyStore;
    use atrium_crypto::verify::verify_signature;
    use k256::ecdsa::Signature;

    fn handle() -> Handle {
        Handle::try_new("alice.zurfur.app").unwrap()
    }

    // The real minter produces a well-formed did:plc: 24 base32 chars, and it is a
    // REAL derivation (not the stub's random suffix) — proven by the vector test in
    // plc.rs; here we assert shape + that keys were custodied.
    #[tokio::test]
    async fn real_mint_produces_well_formed_did_and_stores_keys() {
        let store = Arc::new(MemKeyStore::new());
        let minter = RealDidMinter::new(store.clone(), Box::new(NoopPlcDirectory));

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
        let minter = RealDidMinter::new(store.clone(), Box::new(NoopPlcDirectory));

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
        let minter = RealDidMinter::new(store.clone(), Box::new(NoopPlcDirectory));
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
}
