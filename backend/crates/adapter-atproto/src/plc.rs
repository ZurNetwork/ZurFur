//! Building, signing, and hashing `did:plc` **operations** — the byte-exact core
//! of the minter. One [`PlcOperation`] builder covers both the **genesis** op
//! (`prev = null`) and a **handle update** op (`prev` = the CID of the DID's
//! latest op, `alsoKnownAs` REPLACED with the new handle — ZMVP-50); both are the
//! same `plc_operation` shape and share one serialization path, so the two can
//! never drift byte-wise. The `plc_tombstone` op has its own minimal shape.
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

/// An unsigned identity-only `plc_operation`: the owned field data plus the
/// [`signing_bytes`](PlcOperation::signing_bytes) it must be signed over. One
/// builder covers both op kinds of this shape — a **genesis** op
/// ([`identity_only`](PlcOperation::identity_only), `prev = null`) and a
/// **handle update** op ([`update_handle`](PlcOperation::update_handle),
/// `prev` = a CID) — so there is exactly one DAG-CBOR path that can never drift.
///
/// "Identity-only" means the `services` map is empty — a valid, resolvable DID
/// with **no PDS** (the feed-generator pattern, DD/26935298). Attaching a PDS
/// later is an operation on the same DID, no churn.
pub struct PlcOperation {
    rotation_keys: Vec<String>,
    verification_methods: BTreeMap<String, String>,
    also_known_as: Vec<String>,
    services: BTreeMap<String, PlcService>,
    prev: Option<String>,
}

impl PlcOperation {
    /// Build an **identity-only genesis** operation:
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
        Self::build(rotation_keys, atproto_signing_did, handle, None)
    }

    /// Build an **identity-only handle update** operation (ZMVP-50): the same
    /// `plc_operation` shape as a genesis op, differing in exactly two fields —
    /// `alsoKnownAs` is **REPLACED** with `["at://<handle>"]` (the old alias is
    /// dropped, never retained — DD 27852802 §5) and `prev` is the CID of the
    /// DID's most recent operation, which this op chains onto. The rest of the
    /// DID document (rotation keys, verification methods, empty services) is
    /// reconstructed unchanged from the same key material.
    pub fn update_handle(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
        handle: &str,
        prev: String,
    ) -> Self {
        Self::build(rotation_keys, atproto_signing_did, handle, Some(prev))
    }

    /// The one constructor both op kinds funnel through — genesis passes
    /// `prev = None`, update passes `Some(cid)`; every other field is shaped
    /// identically.
    fn build(
        rotation_keys: Vec<String>,
        atproto_signing_did: String,
        handle: &str,
        prev: Option<String>,
    ) -> Self {
        let mut verification_methods = BTreeMap::new();
        verification_methods.insert("atproto".to_string(), atproto_signing_did);
        Self {
            rotation_keys,
            verification_methods,
            also_known_as: vec![format!("at://{handle}")],
            services: BTreeMap::new(),
            prev,
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
            prev: self.prev.as_deref(),
        };
        Ok(serde_ipld_dagcbor::to_vec(&view)?)
    }

    /// Attach a computed signature (base64url-no-pad), yielding the
    /// [`SignedOperation`] whose hash is the DID (for a genesis op) and whose
    /// CID the next operation chains onto.
    pub fn into_signed(self, sig: String) -> SignedOperation {
        SignedOperation { op: self, sig }
    }
}

/// A signed `plc_operation` (genesis or handle update): for a genesis op the DID
/// is derived from its DAG-CBOR hash; for both kinds the JSON is the directory
/// submission body and the [`cid`](SignedOperation::cid) is what the next
/// operation chains onto.
pub struct SignedOperation {
    op: PlcOperation,
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
            prev: self.op.prev.as_deref(),
            sig: &self.sig,
        }
    }

    /// Derive the `did:plc:` identifier: `base32(sha256(dag_cbor(op incl. sig)))`
    /// lowercased, no padding, truncated to 24 chars. See [`derive_did`].
    ///
    /// Only a **genesis** operation defines a DID — calling this on a handle
    /// update yields a value that identifies nothing.
    pub fn did(&self) -> anyhow::Result<String> {
        let cbor = serde_ipld_dagcbor::to_vec(&self.view())?;
        Ok(derive_did(&cbor))
    }

    /// The signed operation as JSON — the body a PLC directory expects at
    /// `POST /:did`.
    pub fn to_json(&self) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::to_value(self.view())?)
    }

    /// This operation's **CID** (CIDv1 / dag-cbor / sha-256) — recorded in the
    /// operation log so a later operation (e.g. a tombstone) can reference it as
    /// `prev`. Distinct from [`did`](SignedOperation::did) (which truncates a bare
    /// base32 hash to the 24-char DID suffix); see [`cid`].
    pub fn cid(&self) -> anyhow::Result<String> {
        let cbor = serde_ipld_dagcbor::to_vec(&self.view())?;
        Ok(cid(&cbor))
    }
}

/// The fixed `type` discriminant of a PLC tombstone operation.
const TOMBSTONE_TYPE: &str = "plc_tombstone";

/// The DAG-CBOR view of a tombstone **without** `sig` — the bytes a rotation key
/// signs. A tombstone carries no data fields, only `type` and the **mandatory**
/// `prev` (the CID of the DID's most recent operation; not nullable, unlike a
/// genesis op's `prev`).
#[derive(Serialize)]
struct TombstoneUnsignedView<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    prev: &'a str,
}

/// The DAG-CBOR / JSON view of a tombstone **including** `sig` — the body submitted
/// to the directory to deactivate the DID.
#[derive(Serialize)]
struct TombstoneSignedView<'a> {
    #[serde(rename = "type")]
    type_: &'static str,
    prev: &'a str,
    sig: &'a str,
}

/// An unsigned `plc_tombstone` operation: it permanently deactivates a DID, chaining
/// onto the DID's most recent operation via `prev` (a mandatory CID). Signed with a
/// rotation key exactly like a genesis operation — DAG-CBOR without `sig`, then
/// ECDSA-SHA256 low-S, base64url no-pad. Spec: <https://web.plc.directory/spec/v0.1/did-plc>.
pub struct TombstoneOperation {
    prev: String,
}

impl TombstoneOperation {
    /// Build a tombstone chaining onto `prev` — the CID of the DID's latest operation.
    pub fn new(prev: String) -> Self {
        Self { prev }
    }

    /// The DAG-CBOR bytes to sign: this tombstone **without** a `sig` field.
    pub fn signing_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let view = TombstoneUnsignedView {
            type_: TOMBSTONE_TYPE,
            prev: &self.prev,
        };
        Ok(serde_ipld_dagcbor::to_vec(&view)?)
    }

    /// Attach a computed signature (base64url-no-pad), yielding the
    /// [`SignedTombstone`].
    pub fn into_signed(self, sig: String) -> SignedTombstone {
        SignedTombstone { op: self, sig }
    }
}

/// A signed `plc_tombstone`: its JSON is the directory submission body, and its
/// DAG-CBOR hash is its [`cid`](SignedTombstone::cid) (recorded as the last link in
/// the audit chain).
pub struct SignedTombstone {
    op: TombstoneOperation,
    sig: String,
}

impl SignedTombstone {
    /// A borrowed view over this tombstone's fields, for both DAG-CBOR hashing and
    /// JSON submission (one source of truth for the byte layout).
    fn view(&self) -> TombstoneSignedView<'_> {
        TombstoneSignedView {
            type_: TOMBSTONE_TYPE,
            prev: &self.op.prev,
            sig: &self.sig,
        }
    }

    /// This tombstone's CID (CIDv1 / dag-cbor / sha-256) — recorded in the operation
    /// log as the chain's final link. See [`cid`].
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

/// Compute the **CID** of a signed operation's DAG-CBOR bytes — the value a
/// subsequent operation (e.g. a tombstone) references as its `prev`.
///
/// CIDv1, `dag-cbor` codec (`0x71`), `sha-256` multihash (`0x12`), multibase base32
/// (lowercase, `b` prefix): `"b"` + base32(`0x01 0x71 0x12 0x20` ‖ `sha256(bytes)`).
/// This is **not** [`derive_did`] — that truncates a bare base32 hash to 24 chars for
/// the DID *suffix*; a `prev` is a full multiformats CID (`bafyrei…`). Isolated as a
/// pure function so the safety-net vector test pins the exact byte layout.
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
        let op = PlcOperation::identity_only(
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
        let op = PlcOperation::identity_only(
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

    // THE CID SAFETY NET (ZMVP-34). A tombstone's `prev` is the CID of the DID's last
    // operation; a wrong CID computation means an unchainable (directory-rejected)
    // tombstone. Pin `cid()` to the REAL, published genesis-op CID of the vector DID
    // (`did:plc:ewvi7nxzyoun6zhxrhs64oiz`, from its plc.directory audit log) — the same
    // genesis bytes the DID-derivation vector uses. If this fails, the CID byte layout
    // (CIDv1 + dag-cbor + sha-256 multihash + base32) is wrong and no tombstone must ship.
    #[test]
    fn computes_the_known_vector_cid() {
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

        assert_eq!(
            cid(&cbor),
            "bafyreibfvkh3n6odvdpwj54j4xxdsgnn4zo5utbyf7z7nfbyikhtygzjcq"
        );
    }

    /// A prior-op CID for update tests to chain onto (the real vector genesis CID,
    /// so the value is shaped like production data).
    const PREV: &str = "bafyreibfvkh3n6odvdpwj54j4xxdsgnn4zo5utbyf7z7nfbyikhtygzjcq";

    fn update_op() -> PlcOperation {
        PlcOperation::update_handle(
            vec!["did:key:cold".to_string(), "did:key:hot".to_string()],
            "did:key:sign".to_string(),
            "bob.zurfur.app",
            PREV.to_string(),
        )
    }

    // An update signs over bytes that EXCLUDE `sig`, exactly like a genesis op —
    // both serialize through the same UnsignedView/SignedView pair, so this guards
    // the shared path from ever hashing/signing the wrong serialization.
    #[test]
    fn update_signing_bytes_exclude_sig() {
        let op = update_op();
        let unsigned = op.signing_bytes().unwrap();
        let signed_view_cbor =
            serde_ipld_dagcbor::to_vec(&op.into_signed("theSig".to_string()).view()).unwrap();
        assert_ne!(
            unsigned, signed_view_cbor,
            "signed and unsigned update CBOR must differ (sig included vs excluded)"
        );
    }

    // REPLACE semantics (DD 27852802 §5): the update's `alsoKnownAs` is exactly
    // `["at://<new-handle>"]` — the old handle is dropped, never retained as a
    // dead alias (a retained alias fails bidirectional handle verification).
    #[test]
    fn update_replaces_also_known_as() {
        let json = update_op()
            .into_signed("sig".to_string())
            .to_json()
            .unwrap();
        assert_eq!(
            json["alsoKnownAs"],
            serde_json::json!(["at://bob.zurfur.app"]),
            "alsoKnownAs is REPLACED with exactly the new handle"
        );
        assert!(
            !json.to_string().contains("alice.zurfur.app"),
            "no stale alias may survive the update"
        );
    }

    // The update PRESERVES the rest of the DID document: rotationKeys,
    // verificationMethods, and the (empty) services map equal the genesis shape
    // reconstructed from the same keys — only `alsoKnownAs` and `prev` differ.
    #[test]
    fn update_preserves_rotation_keys_and_verification_methods() {
        let genesis = PlcOperation::identity_only(
            vec!["did:key:cold".to_string(), "did:key:hot".to_string()],
            "did:key:sign".to_string(),
            "alice.zurfur.app",
        )
        .into_signed("sig".to_string())
        .to_json()
        .unwrap();
        let update = update_op()
            .into_signed("sig".to_string())
            .to_json()
            .unwrap();

        assert_eq!(
            update["type"], "plc_operation",
            "same discriminant as genesis"
        );
        assert_eq!(update["rotationKeys"], genesis["rotationKeys"]);
        assert_eq!(
            update["verificationMethods"],
            genesis["verificationMethods"]
        );
        assert_eq!(update["services"], serde_json::json!({}));
        assert_ne!(update["alsoKnownAs"], genesis["alsoKnownAs"]);
        assert_ne!(update["prev"], genesis["prev"]);
    }

    // The update chains: its `prev` is the supplied CID of the DID's latest op
    // (a genesis op serializes `prev: null`; an update never does).
    #[test]
    fn update_chains_on_prev() {
        let json = update_op()
            .into_signed("sig".to_string())
            .to_json()
            .unwrap();
        assert_eq!(json["prev"], PREV);
    }

    // Signing is DETERMINISTIC (atrium-crypto uses RFC 6979 + low-S): the same
    // bytes under the same key yield the same signature, so the same
    // (prev, alsoKnownAs, keys) yield the same CID. The whole idempotency model —
    // a replayed identical update dedups on `UNIQUE(cid)` — rests on this test.
    #[test]
    fn signing_is_deterministic() {
        use atrium_crypto::keypair::{Did as _, Secp256k1Keypair};
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

        let key = Secp256k1Keypair::create(&mut rand::thread_rng());
        let build = || {
            PlcOperation::update_handle(
                vec!["did:key:cold".to_string(), key.did()],
                "did:key:sign".to_string(),
                "bob.zurfur.app",
                PREV.to_string(),
            )
        };

        let first_bytes = build().signing_bytes().unwrap();
        let second_bytes = build().signing_bytes().unwrap();
        assert_eq!(first_bytes, second_bytes, "unsigned CBOR is deterministic");

        let first_sig = key.sign(&first_bytes).unwrap();
        let second_sig = key.sign(&second_bytes).unwrap();
        assert_eq!(
            first_sig, second_sig,
            "ECDSA signing must be deterministic (RFC 6979) — idempotency rests on it"
        );

        let first_cid = build()
            .into_signed(URL_SAFE_NO_PAD.encode(&first_sig))
            .cid()
            .unwrap();
        let second_cid = build()
            .into_signed(URL_SAFE_NO_PAD.encode(&second_sig))
            .cid()
            .unwrap();
        assert_eq!(
            first_cid, second_cid,
            "same inputs → same signed op → same CID"
        );
    }

    // A tombstone signs over bytes that EXCLUDE `sig` (like a genesis op), and its JSON
    // is the minimal `{type: plc_tombstone, prev, sig}` — no rotationKeys / alsoKnownAs
    // / services / verificationMethods (per the did:plc spec).
    #[test]
    fn tombstone_shape_and_signing_bytes() {
        let prev = "bafyreibfvkh3n6odvdpwj54j4xxdsgnn4zo5utbyf7z7nfbyikhtygzjcq";
        let op = TombstoneOperation::new(prev.to_string());
        let unsigned = op.signing_bytes().unwrap();
        let signed = op.into_signed("theSig".to_string());
        let signed_cbor = serde_ipld_dagcbor::to_vec(&signed.view()).unwrap();
        assert_ne!(
            unsigned, signed_cbor,
            "signed and unsigned tombstone CBOR must differ (sig included vs excluded)"
        );

        let json = signed.to_json().unwrap();
        assert_eq!(json["type"], "plc_tombstone");
        assert_eq!(json["prev"], prev);
        assert_eq!(json["sig"], "theSig");
        for absent in [
            "rotationKeys",
            "verificationMethods",
            "alsoKnownAs",
            "services",
        ] {
            assert!(
                json.get(absent).is_none(),
                "a tombstone must carry no `{absent}` field"
            );
        }
    }
}
