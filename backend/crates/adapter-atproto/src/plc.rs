//! Building, signing, and hashing `did:plc` operations — `plc_operation`
//! (genesis and handle update, one builder) and `plc_tombstone`.
//!
//! Two DAG-CBOR serializations per operation, not the same bytes: *without*
//! `sig` is what the rotation key signs; *with* `sig` is what is hashed into the
//! DID or CID. Spec: <https://web.plc.directory/spec/v0.1/did-plc>.

use std::collections::BTreeMap;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The fixed `type` discriminant of a PLC operation.
const OP_TYPE: &str = "plc_operation";

/// A PLC service entry under the operation's `services` map (e.g. an atproto
/// PDS). Never constructed by the minter — v1 operations are identity-only, with
/// an empty `services` map.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct PlcService {
    /// The service type, e.g. `AtprotoPersonalDataServer`.
    #[serde(rename = "type")]
    pub type_: String,
    /// The service endpoint URL.
    pub endpoint: String,
}

/// The DAG-CBOR view of an operation without `sig` — the bytes a rotation key
/// signs. `prev` is `None` (CBOR `null`) for a genesis operation.
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

/// The DAG-CBOR / JSON view of an operation including `sig` — hashed to derive
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

/// An unsigned identity-only `plc_operation`. One builder covers both kinds of
/// this shape — [`identity_only`](PlcOperation::identity_only) (genesis) and
/// [`update_handle`](PlcOperation::update_handle) — so there is a single
/// DAG-CBOR path. Identity-only means an empty `services` map: no PDS.
pub struct PlcOperation {
    rotation_keys: Vec<String>,
    verification_methods: BTreeMap<String, String>,
    also_known_as: Vec<String>,
    services: BTreeMap<String, PlcService>,
    prev: Option<String>,
}

impl PlcOperation {
    /// Build an identity-only genesis operation (`prev = null`, empty
    /// `services`). `rotation_keys` are `did:key` multikeys in descending
    /// authority; `handle` becomes the sole `alsoKnownAs`.
    pub fn identity_only(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
        handle: &str,
    ) -> Self {
        Self::build(
            rotation_keys,
            atproto_signing_did,
            vec![format!("at://{handle}")],
            None,
        )
    }

    /// Build an identity-only genesis operation with no alias (`alsoKnownAs`
    /// empty, `prev = null`, empty `services`).
    pub fn identity_only_handleless(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
    ) -> Self {
        Self::build(rotation_keys, atproto_signing_did, Vec::new(), None)
    }

    /// Build an identity-only handle update chaining onto `prev` (the CID of the
    /// DID's latest operation). `alsoKnownAs` is REPLACED with the new handle —
    /// the old alias is dropped, never retained.
    pub fn update_handle(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
        handle: &str,
        prev: String,
    ) -> Self {
        Self::build(
            rotation_keys,
            atproto_signing_did,
            vec![format!("at://{handle}")],
            Some(prev),
        )
    }

    /// The one constructor every op kind funnels through; only `also_known_as`
    /// and `prev` differ.
    fn build(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
        also_known_as: Vec<String>,
        prev: Option<String>,
    ) -> Self {
        let mut verification_methods = BTreeMap::new();
        verification_methods.insert("atproto".to_string(), atproto_signing_did);
        Self {
            rotation_keys,
            verification_methods,
            also_known_as,
            services: BTreeMap::new(),
            prev,
        }
    }

    /// The DAG-CBOR bytes to sign: this operation without a `sig` field.
    pub fn signing_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let view = UnsignedView {
            type_: OP_TYPE,
            rotation_keys: &self.rotation_keys,
            verification_methods: &self.verification_methods,
            also_known_as: &self.also_known_as,
            services: &self.services,
            prev: self.prev.as_deref(),
        };
        Ok(serde_ipld_dagcbor::to_vec(&view)?)
    }

    /// Attach a computed signature (base64url-no-pad).
    pub fn into_signed(self, sig: String) -> SignedOperation {
        SignedOperation { op: self, sig }
    }
}

/// A signed `plc_operation` (genesis or handle update): its JSON is the
/// directory submission body and its [`cid`](SignedOperation::cid) is what the
/// next operation chains onto.
pub struct SignedOperation {
    op: PlcOperation,
    sig: String,
}

impl SignedOperation {
    /// One borrowed view over the fields, shared by DAG-CBOR hashing and JSON
    /// submission, so the byte layout has a single source.
    fn view(&self) -> SignedView<'_> {
        SignedView {
            type_: OP_TYPE,
            rotation_keys: &self.op.rotation_keys,
            verification_methods: &self.op.verification_methods,
            also_known_as: &self.op.also_known_as,
            services: &self.op.services,
            prev: self.op.prev.as_deref(),
            sig: &self.sig,
        }
    }

    /// Derive the `did:plc:` identifier from this operation's DAG-CBOR bytes.
    /// Only a genesis operation defines a DID — on a handle update the value
    /// identifies nothing.
    pub fn did(&self) -> anyhow::Result<String> {
        let cbor = serde_ipld_dagcbor::to_vec(&self.view())?;
        Ok(derive_did(&cbor))
    }

    /// The signed operation as JSON — the body a PLC directory expects at
    /// `POST /:did`.
    pub fn to_json(&self) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::to_value(self.view())?)
    }

    /// This operation's CID (CIDv1 / dag-cbor / sha-256) — recorded in the
    /// operation log so a later operation can reference it as `prev`. Distinct
    /// from [`did`](SignedOperation::did).
    pub fn cid(&self) -> anyhow::Result<String> {
        let cbor = serde_ipld_dagcbor::to_vec(&self.view())?;
        Ok(cid(&cbor))
    }
}

/// The fixed `type` discriminant of a PLC tombstone operation.
const TOMBSTONE_TYPE: &str = "plc_tombstone";

/// The DAG-CBOR view of a tombstone without `sig` — the bytes a rotation key
/// signs. Only `type` and a mandatory (never null) `prev`.
#[derive(Serialize)]
struct TombstoneUnsignedView<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    prev: &'a str,
}

/// The DAG-CBOR / JSON view of a tombstone including `sig` — the body submitted
/// to the directory to deactivate the DID.
#[derive(Serialize)]
struct TombstoneSignedView<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    prev: &'a str,
    sig: &'a str,
}

/// An unsigned `plc_tombstone`: permanently deactivates a DID, chaining onto its
/// most recent operation via `prev` (a mandatory CID). Signed with a rotation
/// key exactly like a genesis operation.
pub struct TombstoneOperation {
    prev: String,
}

impl TombstoneOperation {
    /// Build a tombstone chaining onto `prev` — the CID of the DID's latest operation.
    pub fn new(prev: String) -> Self {
        Self { prev }
    }

    /// The DAG-CBOR bytes to sign: this tombstone without a `sig` field.
    pub fn signing_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let view = TombstoneUnsignedView {
            type_: TOMBSTONE_TYPE,
            prev: &self.prev,
        };
        Ok(serde_ipld_dagcbor::to_vec(&view)?)
    }

    /// Attach a computed signature (base64url-no-pad).
    pub fn into_signed(self, sig: String) -> SignedTombstone {
        SignedTombstone { op: self, sig }
    }
}

/// A signed `plc_tombstone`: its JSON is the directory submission body, its CID
/// the last link in the audit chain.
pub struct SignedTombstone {
    op: TombstoneOperation,
    sig: String,
}

impl SignedTombstone {
    /// One borrowed view over the fields, shared by DAG-CBOR hashing and JSON
    /// submission, so the byte layout has a single source.
    fn view(&self) -> TombstoneSignedView<'_> {
        TombstoneSignedView {
            type_: TOMBSTONE_TYPE,
            prev: &self.op.prev,
            sig: &self.sig,
        }
    }

    /// This tombstone's CID (CIDv1 / dag-cbor / sha-256).
    pub fn cid(&self) -> anyhow::Result<String> {
        let cbor = serde_ipld_dagcbor::to_vec(&self.view())?;
        Ok(cid(&cbor))
    }

    /// The signed tombstone as JSON — the body a PLC directory expects at
    /// `POST /:did`.
    pub fn to_json(&self) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::to_value(self.view())?)
    }
}

/// Derive the `did:plc` string from a signed operation's DAG-CBOR bytes:
/// `did:plc:` + the first 24 chars of lowercase unpadded base32 of its SHA-256.
fn derive_did(signed_op_cbor: &[u8]) -> String {
    let hash = Sha256::digest(signed_op_cbor);
    let b32 = data_encoding::BASE32_NOPAD.encode(&hash).to_lowercase();
    format!("did:plc:{}", &b32[..24])
}

/// Compute the CID of a signed operation's DAG-CBOR bytes — the value a
/// subsequent operation references as its `prev`. A full multiformats CID
/// (`bafyrei…`), unlike [`derive_did`]'s truncated bare base32 hash.
pub fn cid(signed_op_cbor: &[u8]) -> String {
    let hash = Sha256::digest(signed_op_cbor);
    // multibase `b` (base32) over: CIDv1 (0x01), dag-cbor (0x71), then the multihash
    // (sha2-256 = 0x12, length 0x20 = 32 bytes, then the 32 hash bytes).
    let mut bytes = Vec::with_capacity(4 + hash.len());
    bytes.extend_from_slice(&[0x01, 0x71, 0x12, 0x20]);
    bytes.extend_from_slice(&hash);
    format!(
        "b{}",
        data_encoding::BASE32_NOPAD.encode(&bytes).to_lowercase()
    )
}

#[cfg(test)]
mod proptests;
#[cfg(test)]
mod tests;
