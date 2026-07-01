//! Building, signing, and hashing a `did:plc` **genesis operation** — the
//! byte-exact core of the minter.
//!
//! A `did:plc` is *defined by* the hash of its first (genesis) operation, so
//! every byte is load-bearing. Two serializations of the same operation are used,
//! and they are **not** the same bytes:
//!
//! 1. **Signed bytes** — DAG-CBOR of the operation *without* the `sig` field. This
//!    is what the rotation key signs (ECDSA-SHA256, low-S, 64-byte r‖s, then
//!    base64url no-pad).
//! 2. **Identifier bytes** — DAG-CBOR of the operation *including* that `sig`. Its
//!    `sha256`, base32-encoded (lowercase, no pad) and truncated to 24 chars, is
//!    the `did:plc:` suffix.
//!
//! DAG-CBOR (RFC 8949 core-deterministic) canonically **sorts map keys by
//! length-first, then bytewise** on serialize; `serde_ipld_dagcbor` does this for
//! struct keys too, so declaration order below is irrelevant to the output. The
//! [`tests`] module pins the whole pipeline to a real, published vector
//! (`did:plc:ewvi7nxzyoun6zhxrhs64oiz`).
//!
//! Spec: <https://web.plc.directory/spec/v0.1/did-plc>.

use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The fixed `type` discriminant of a PLC operation.
const OP_TYPE: &str = "plc_operation";

/// A PLC service entry as it appears under the operation's `services` map (e.g. an
/// atproto PDS). Identity-only v1 (DD/26935298) emits an **empty** `services` map,
/// so no `PlcService` is constructed by the minter; the type exists so the shape
/// is complete and the vector test can reproduce a service-bearing operation.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct PlcService {
    /// The service type, e.g. `AtprotoPersonalDataServer`.
    #[serde(rename = "type")]
    pub type_: String,
    /// The service endpoint URL.
    pub endpoint: String,
}

/// The DAG-CBOR view of an operation **without** `sig` — the bytes a rotation key
/// signs. `prev` is `None` (serialized as CBOR `null`) for a genesis operation.
#[derive(Serialize)]
struct UnsignedView<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    #[serde(rename = "rotationKeys")]
    rotation_keys: &'a [String],
    #[serde(rename = "verificationMethods")]
    verification_methods: &'a BTreeMap<String, String>,
    #[serde(rename = "alsoKnownAs")]
    also_known_as: &'a [String],
    services: &'a BTreeMap<String, PlcService>,
    prev: Option<&'a str>,
}

/// The DAG-CBOR / JSON view of an operation **including** `sig` — hashed to derive
/// the DID, and serialized to JSON as the directory submission body.
#[derive(Serialize)]
struct SignedView<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    #[serde(rename = "rotationKeys")]
    rotation_keys: &'a [String],
    #[serde(rename = "verificationMethods")]
    verification_methods: &'a BTreeMap<String, String>,
    #[serde(rename = "alsoKnownAs")]
    also_known_as: &'a [String],
    services: &'a BTreeMap<String, PlcService>,
    prev: Option<&'a str>,
    sig: &'a str,
}

/// An unsigned identity-only genesis operation: the owned field data plus the
/// [`signing_bytes`](GenesisOperation::signing_bytes) it must be signed over.
///
/// "Identity-only" means the `services` map is empty — a valid, resolvable DID
/// with **no PDS** (the feed-generator pattern, DD/26935298). Attaching a PDS
/// later is an operation on the same DID, no churn.
pub struct GenesisOperation {
    rotation_keys: Vec<String>,
    verification_methods: BTreeMap<String, String>,
    also_known_as: Vec<String>,
    services: BTreeMap<String, PlcService>,
}

impl GenesisOperation {
    /// Build an **identity-only** genesis operation:
    ///
    /// - `rotation_keys` — the `did:key` multikeys of the rotation keypairs, in
    ///   descending authority (`[cold_recovery, operational]`, DD/26804226 B2).
    /// - `atproto_signing_did` — the `did:key` of the `#atproto` verification
    ///   method, included for forward-compat (B3).
    /// - `handle` — the initial `alsoKnownAs` becomes `at://<handle>`.
    ///
    /// `services` is left empty (no `atproto_pds`). `prev` is `null` (genesis).
    pub fn identity_only(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
        handle: &str,
    ) -> Self {
        let mut verification_methods = BTreeMap::new();
        verification_methods.insert("atproto".to_string(), atproto_signing_did);
        Self {
            rotation_keys,
            verification_methods,
            also_known_as: vec![format!("at://{handle}")],
            services: BTreeMap::new(),
        }
    }

    /// The DAG-CBOR bytes to sign: this operation **without** a `sig` field.
    pub fn signing_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let view = UnsignedView {
            type_: OP_TYPE,
            rotation_keys: &self.rotation_keys,
            verification_methods: &self.verification_methods,
            also_known_as: &self.also_known_as,
            services: &self.services,
            prev: None,
        };
        Ok(serde_ipld_dagcbor::to_vec(&view)?)
    }

    /// Attach a computed signature (base64url-no-pad), yielding the
    /// [`SignedOperation`] whose hash is the DID.
    pub fn into_signed(self, sig: String) -> SignedOperation {
        SignedOperation { op: self, sig }
    }
}

/// A signed genesis operation: the DID is derived from its DAG-CBOR hash, and its
/// JSON is the directory submission body.
pub struct SignedOperation {
    op: GenesisOperation,
    sig: String,
}

impl SignedOperation {
    /// A borrowed `SignedView` over this operation's fields, for both DAG-CBOR
    /// hashing and JSON submission (one source of truth for the byte layout).
    fn view(&self) -> SignedView<'_> {
        SignedView {
            type_: OP_TYPE,
            rotation_keys: &self.op.rotation_keys,
            verification_methods: &self.op.verification_methods,
            also_known_as: &self.op.also_known_as,
            services: &self.op.services,
            prev: None,
            sig: &self.sig,
        }
    }

    /// Derive the `did:plc:` identifier: `base32(sha256(dag_cbor(op incl. sig)))`
    /// lowercased, no padding, truncated to 24 chars. See [`derive_did`].
    pub fn did(&self) -> anyhow::Result<String> {
        let cbor = serde_ipld_dagcbor::to_vec(&self.view())?;
        Ok(derive_did(&cbor))
    }

    /// The signed operation as JSON — the body a PLC directory expects at
    /// `POST /:did`.
    pub fn to_json(&self) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::to_value(self.view())?)
    }
}

/// Derive the `did:plc` string from the DAG-CBOR bytes of a *signed* operation:
/// `did:plc:` + first 24 chars of the lowercase, unpadded base32 of its SHA-256.
///
/// Isolated as a pure function so the safety-net vector test exercises the exact
/// derivation the minter uses.
fn derive_did(signed_op_cbor: &[u8]) -> String {
    let hash = Sha256::digest(signed_op_cbor);
    let b32 = data_encoding::BASE32_NOPAD.encode(&hash).to_lowercase();
    format!("did:plc:{}", &b32[..24])
}

#[cfg(test)]
mod tests {
    use super::*;

    // THE SAFETY NET (ZMVP-49). Derive the DID from a real, published genesis
    // operation and assert it equals the known value. If this fails, the byte
    // pipeline (DAG-CBOR canonical ordering + sha256 + base32/24) is wrong and the
    // minter must NOT ship. Vector: the bsky.social account's genesis operation.
    #[test]
    fn derives_the_known_vector_did() {
        let mut verification_methods = BTreeMap::new();
        verification_methods.insert(
            "atproto".to_string(),
            "did:key:zQ3shXjHeiBuRCKmM36cuYnm7YEMzhGnCmCyW92sRJ9pribSF".to_string(),
        );
        let mut services = BTreeMap::new();
        services.insert(
            "atproto_pds".to_string(),
            PlcService {
                type_: "AtprotoPersonalDataServer".to_string(),
                endpoint: "https://bsky.social".to_string(),
            },
        );
        let rotation_keys = vec![
            "did:key:zQ3shhCGUqDKjStzuDxPkTxN6ujddP4RkEKJJouJGRRkaLGbg".to_string(),
            "did:key:zQ3shpKnbdPx3g3CmPf5cRVTPe1HtSwVn5ish3wSnDPQCbLJK".to_string(),
        ];
        let also_known_as = vec!["at://atprotocol.bsky.social".to_string()];
        let sig = "lza4at_jCtGo_TYgL5PC1ZNP7lhF4DV8H50LWHhvdHcB143x1wEwqZ43xvV36Pws6OOnJLJrkibEUFDFqkhIhg";

        let view = SignedView {
            type_: OP_TYPE,
            rotation_keys: &rotation_keys,
            verification_methods: &verification_methods,
            also_known_as: &also_known_as,
            services: &services,
            prev: None,
            sig,
        };
        let cbor = serde_ipld_dagcbor::to_vec(&view).unwrap();

        assert_eq!(derive_did(&cbor), "did:plc:ewvi7nxzyoun6zhxrhs64oiz");
    }

    // An identity-only operation carries an EMPTY services map (no atproto_pds) —
    // the defining property of DD/26935298. Assert both the map is empty and the
    // serialized JSON has no `atproto_pds` anywhere.
    #[test]
    fn identity_only_op_has_no_pds() {
        let op = GenesisOperation::identity_only(
            vec!["did:key:cold".to_string(), "did:key:hot".to_string()],
            "did:key:sign".to_string(),
            "alice.zurfur.app",
        );
        assert!(
            op.services.is_empty(),
            "identity-only op must have no services"
        );
        assert_eq!(op.also_known_as, vec!["at://alice.zurfur.app".to_string()]);

        let signed = op.into_signed("sig".to_string());
        let json = signed.to_json().unwrap();
        assert!(
            !json.to_string().contains("atproto_pds"),
            "identity-only op JSON must not mention atproto_pds"
        );
        // services present as an (empty) object, per the PLC operation shape.
        assert_eq!(json["services"], serde_json::json!({}));
    }

    // The signed and unsigned serializations are DIFFERENT bytes: signing_bytes
    // omits `sig`, the DID hash includes it. Guard against ever hashing the wrong
    // one (which would derive a DID over bytes nobody signed).
    #[test]
    fn signing_bytes_exclude_sig() {
        let op = GenesisOperation::identity_only(
            vec!["did:key:cold".to_string(), "did:key:hot".to_string()],
            "did:key:sign".to_string(),
            "alice.zurfur.app",
        );
        let unsigned = op.signing_bytes().unwrap();
        let signed_view_cbor =
            serde_ipld_dagcbor::to_vec(&op.into_signed("theSig".to_string()).view()).unwrap();
        assert_ne!(
            unsigned, signed_view_cbor,
            "signed and unsigned CBOR must differ (sig included vs excluded)"
        );
    }
}
