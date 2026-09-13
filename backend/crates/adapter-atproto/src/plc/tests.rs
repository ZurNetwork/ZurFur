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
    let sig =
        "lza4at_jCtGo_TYgL5PC1ZNP7lhF4DV8H50LWHhvdHcB143x1wEwqZ43xvV36Pws6OOnJLJrkibEUFDFqkhIhg";

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

// A handleless genesis operation carries an EMPTY `alsoKnownAs` — no alias
// until a later `update_handle` claims one — and still signs successfully.
#[test]
fn identity_only_handleless_op_has_no_also_known_as() {
    let op = PlcOperation::identity_only_handleless(
        vec!["did:key:cold".to_string(), "did:key:hot".to_string()],
        "did:key:sign".to_string(),
    );
    assert!(
        op.also_known_as.is_empty(),
        "handleless op must have no alsoKnownAs"
    );

    let signing_bytes = op.signing_bytes();
    assert!(
        signing_bytes.is_ok(),
        "a handleless op's signing_bytes must succeed"
    );
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
    let sig =
        "lza4at_jCtGo_TYgL5PC1ZNP7lhF4DV8H50LWHhvdHcB143x1wEwqZ43xvV36Pws6OOnJLJrkibEUFDFqkhIhg";

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
