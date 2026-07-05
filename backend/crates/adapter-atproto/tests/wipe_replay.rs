//! ZMVP-106 capstone: an image-bearing `app.zurfur.feed.post` written through the
//! [`PublicRecords`] port survives a **wipe and replay** — booted against one
//! throwaway PDS, then against a second, provably-clean one — and lands
//! byte-for-byte deterministic, with a mechanical guard that the stored record
//! carries no field outside the final ZMVP-104 lexicon.
//!
//! This adds **no** boundary-crossing machinery: it composes the already-shipped,
//! already-security-reviewed helpers (ZMVP-102/103/104/105) into the epic's exit
//! proof — "the full write path proven deterministic against a rig that holds no
//! hidden state" (epic ZMVP-101). Needs a container runtime socket (podman/docker),
//! like every other testcontainers suite in the workspace.

use std::collections::BTreeSet;

use adapter_atproto::AtprotoPublicRecords;
use domain::elements::did::Did;
use domain::elements::public_record::{
    AspectRatio, AtUri, Embed, FeedPost, PublicRecord, RecordRef, SelfLabels,
};
use domain::ports::PublicRecords;
use test_support::contract::{TINY_PNG, fixed_created_at};
use test_support::{ActingCredential, ThrowawayPds};

// --- AC1/AC2 (F4/F5): the write path survives a wipe and replays deterministically ---

/// Everything one capstone run observes against one PDS — the read-back record,
/// the downloaded blob bytes, and the two content-address CIDs plus the AT-URI the
/// cross-run assertions compare.
struct FlowWitness {
    /// The record read back through the port (field-identical to what was written).
    feed_post: FeedPost,
    /// The blob bytes downloaded from the PDS via `getBlob`.
    blob_bytes: Vec<u8>,
    /// The record CID — the content hash of the DAG-CBOR-encoded record value.
    record_cid: cid::Cid,
    /// The blob CID — the content address of the uploaded image bytes.
    blob_cid: cid::Cid,
    /// Where the record landed (`at://<did>/app.zurfur.feed.post/<rkey>`).
    uri: AtUri,
}

/// AC2 — the epic's exit criterion: the full image-bearing write path, run against
/// a throwaway PDS and then against a **second, provably-clean** one, lands
/// byte-for-byte deterministic with **no manual intervention** (the wipe is a
/// `drop` + a fresh `boot`, all inside this one test body — a brand-new container
/// with a RAM-backed `/pds` and a fresh per-instance stub PLC shares nothing with
/// the first).
#[tokio::test]
async fn post_with_blob_survives_wipe_and_replay() {
    // Run 1 on a fresh PDS; dropping it at the end of the block wipes it (the
    // container, its tmpfs repo, and its per-instance PLC are all gone).
    let run1 = {
        let pds = ThrowawayPds::boot().await.expect("boot throwaway PDS #1");
        run_capstone_flow(&pds).await
    };
    // Run 2 on a second PDS that provably shares nothing with the first.
    let run2 = {
        let pds = ThrowawayPds::boot()
            .await
            .expect("boot throwaway PDS #2 (post-wipe)");
        run_capstone_flow(&pds).await
    };

    // AC1/AC2 — the literal criteria: the identical flow reads back field-identical
    // and byte-identical on both runs.
    assert_eq!(
        run1.feed_post, run2.feed_post,
        "the record reads back field-identical across the wipe"
    );
    assert_eq!(
        run1.blob_bytes.as_slice(),
        TINY_PNG,
        "run 1: the blob downloads byte-identical to what was uploaded"
    );
    assert_eq!(
        run2.blob_bytes.as_slice(),
        TINY_PNG,
        "run 2 (post-wipe): the blob downloads byte-identical to what was uploaded"
    );

    // Determinism — the "deterministic" the exit criterion asks for, encoded as the
    // pass condition (not a hopeful claim): if either equality ever fails, the write
    // path is NOT deterministic and that is the ticket's own signal, not a flake.
    //
    // blob CID = the content address of the image bytes (CIDv1 raw/sha-256):
    // identical bytes address identically on any PDS (Gallery Posts DD 29949954 §2,
    // "byte-identical blobs across PDSes render as one gallery item").
    assert_eq!(
        run1.blob_cid, run2.blob_cid,
        "the content-addressed blob CID is stable across the wipe"
    );
    // record CID = the content hash of the record VALUE encoded as canonical
    // DAG-CBOR. It is deterministic because DAG-CBOR is atproto's canonical encoding
    // AND `created_at` is pinned, so both runs encode byte-identical record bytes.
    // This is DISTINCT from the commit CID / `rev`, which fold in the repo's
    // per-instance signing key and a monotonically increasing TID and therefore
    // DIFFER on every commit — we never assert those (the port does not even expose
    // them). See https://atproto.com/specs/repository.
    assert_eq!(
        run1.record_cid, run2.record_cid,
        "the DAG-CBOR record CID is stable across the wipe (createdAt pinned)"
    );

    // The rkey is a fresh TID minted per `createRecord`, and each fresh PDS mints a
    // fresh DID for `capstone.test`, so the AT-URI necessarily differs across runs.
    // Asserting the difference documents the boundary; we never assert URI equality.
    assert_ne!(
        run1.uri, run2.uri,
        "a fresh repo + a fresh-TID rkey means the AT-URI differs across runs"
    );
}

/// One capstone run against `pds`: provision an account, upload the image blob,
/// build a valid ZMVP-104-shaped image-bearing post with a pinned `createdAt`,
/// write it **through the domain port**, then verify — for this run — that it
/// reads back field-identical, the blob downloads byte-identical, and the stored
/// record carries no field outside the final lexicon. Returns the witness the
/// cross-run determinism assertions compare.
async fn run_capstone_flow(pds: &ThrowawayPds) -> FlowWitness {
    let (store, did) = bearer_adapter(pds, "capstone.test").await;

    // Upload the image blob (AC1, blob half).
    let blob = store
        .upload_blob(TINY_PNG.to_vec(), "image/png")
        .await
        .expect("upload_blob");
    assert_eq!(
        blob.size,
        TINY_PNG.len() as u64,
        "the blob ref records the uploaded byte length"
    );

    // A valid, ZMVP-104-shaped, image-bearing post. `labels` is empty (Safe): no
    // rating context exists here to derive a maturity label from, and deriving one
    // is the compose/publish layer's job, not the capstone's (F1 → ZMVP-108).
    // `created_at` is pinned so the record's DAG-CBOR bytes are identical run to run.
    let written = FeedPost {
        text: None,
        embed: Some(Embed {
            blob: blob.clone(),
            alt: "capstone probe".to_string(),
            aspect_ratio: Some(AspectRatio {
                width: 1,
                height: 1,
            }),
        }),
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: fixed_created_at(),
    };

    // Write THROUGH THE PORT (not raw XRPC): this is the boundary the capstone proves.
    let record_ref: RecordRef = store
        .create_record(&did, &PublicRecord::FeedPost(written.clone()))
        .await
        .expect("create_record through the PublicRecords port");

    // AC1: read back field-identical through the port.
    let read = expect_feed_post(
        store
            .get_record(&record_ref.uri)
            .await
            .expect("get_record reads back the just-written record"),
    );
    assert_eq!(
        read, written,
        "the record reads back field-identical to what was written"
    );

    // AC1: the blob downloads byte-for-byte identical from the PDS.
    let blob_bytes = raw_get_blob(pds, &did, &blob.cid).await;
    assert_eq!(
        blob_bytes.as_slice(),
        TINY_PNG,
        "the uploaded blob downloads byte-identical from the PDS"
    );

    // AC4: the record the PDS actually stored carries no field outside the lexicon.
    let stored_value = raw_get_record_value(pds, &record_ref.uri).await;
    assert_stored_value_within_final_lexicon(&stored_value);

    FlowWitness {
        feed_post: read,
        blob_bytes,
        record_cid: record_ref.cid,
        blob_cid: blob.cid,
        uri: record_ref.uri,
    }
}

/// Provision a fixture account on `pds` and build the Bearer-authed adapter for it,
/// returning the adapter and the acting DID.
///
/// The `access_jwt` is handed straight to the adapter constructor and never
/// surfaces again — the credential stays quarantined behind the port, exactly as
/// ZMVP-105 established.
async fn bearer_adapter(pds: &ThrowawayPds, handle: &str) -> (AtprotoPublicRecords, Did) {
    let account = pds
        .provision_account(handle)
        .await
        .expect("provision fixture account");
    let access_jwt = match &account.credential {
        ActingCredential::PdsSession { access_jwt, .. } => access_jwt.clone(),
        _ => unreachable!("new credential variants opt in explicitly"),
    };
    let store = AtprotoPublicRecords::new(&account.endpoint, access_jwt)
        .expect("build the atproto public-records adapter");
    (store, Did::new(account.did))
}

/// Unwrap the single-variant public-record envelope to its [`FeedPost`].
fn expect_feed_post(record: PublicRecord) -> FeedPost {
    match record {
        PublicRecord::FeedPost(post) => post,
    }
}

/// Download a blob's bytes straight from the PDS via `com.atproto.sync.getBlob`
/// (an unauthenticated read) — the raw byte-fidelity witness, off-port on purpose.
async fn raw_get_blob(pds: &ThrowawayPds, did: &Did, blob_cid: &cid::Cid) -> Vec<u8> {
    let url = format!(
        "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
        pds.endpoint(),
        did.as_str(),
        blob_cid,
    );
    reqwest::get(&url)
        .await
        .expect("getBlob request reaches the PDS")
        .error_for_status()
        .expect("getBlob succeeds")
        .bytes()
        .await
        .expect("getBlob body")
        .to_vec()
}

/// Fetch the raw stored record `value` object via `com.atproto.repo.getRecord` —
/// the exact wire shape the PDS holds, so AC4 checks what actually crossed the
/// boundary rather than a re-encode of the domain value.
async fn raw_get_record_value(pds: &ThrowawayPds, uri: &AtUri) -> serde_json::Value {
    let url = format!(
        "{}/xrpc/com.atproto.repo.getRecord?repo={}&collection={}&rkey={}",
        pds.endpoint(),
        uri.did.as_str(),
        uri.collection.as_str(),
        uri.rkey.as_str(),
    );
    let body = reqwest::get(&url)
        .await
        .expect("getRecord request reaches the PDS")
        .error_for_status()
        .expect("getRecord succeeds")
        .bytes()
        .await
        .expect("getRecord body");
    let parsed: serde_json::Value =
        serde_json::from_slice(&body).expect("getRecord response is JSON");
    parsed
        .get("value")
        .cloned()
        .expect("getRecord response carries a value object")
}

// --- AC4 (F6): the final lexicon bounds the stored record shape ---

/// The on-disk **final** `app.zurfur.feed.post` lexicon (ZMVP-104), reached from
/// this crate at the workspace root. Loading it from disk pins AC4 to the real
/// file: a record that grew a field the lexicon does not declare — or a lexicon
/// edit that dropped a field the record still writes — fails the guard below.
const FEED_POST_LEXICON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../lexicons/app.zurfur.feed.post.json"
);

/// The `(record type, allowed, required)` shape of the final lexicon's `main`
/// record, read straight off disk: its NSID (the record `$type`) plus the
/// top-level property names.
fn final_lexicon_record_shape() -> (String, BTreeSet<String>, BTreeSet<String>) {
    let raw = std::fs::read_to_string(FEED_POST_LEXICON).expect("read the final feed.post lexicon");
    let lexicon: serde_json::Value = serde_json::from_str(&raw).expect("parse the lexicon JSON");
    let record_type = lexicon["id"]
        .as_str()
        .expect("lexicon id is a string")
        .to_string();
    let record = &lexicon["defs"]["main"]["record"];

    let allowed = record["properties"]
        .as_object()
        .expect("lexicon defs.main.record.properties is an object")
        .keys()
        .cloned()
        .collect();
    let required = record["required"]
        .as_array()
        .expect("lexicon defs.main.record.required is an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("a required entry is a string")
                .to_string()
        })
        .collect();
    (record_type, allowed, required)
}

/// Pure, I/O-free check that a stored record `value` conforms to the lexicon's
/// **shape**: `$type` names exactly the final record type, every other top-level
/// key is one the lexicon declares, and every required property is present.
/// Returns a describing `Err` on the first violation so a failure names the
/// offending field.
///
/// This checks the *actual wire shape the PDS stored*, so it needs neither the
/// crate-private wire type nor a re-encode — a stray or draft field the adapter
/// somehow emitted, or an Index-side private fact that leaked across the boundary
/// (`medium`, `commissionRef`, tags — Class B, must never cross), breaks it.
fn stored_value_within(
    value: &serde_json::Value,
    record_type: &str,
    allowed: &BTreeSet<String>,
    required: &BTreeSet<String>,
) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "stored record value is not a JSON object".to_string())?;

    // `$type` is the atproto record-type discriminator, not a lexicon property —
    // but it must name exactly the final record type: a same-shaped record under
    // a different `$type` (a draft variant, say) is outside the lexicon too.
    match object.get("$type").and_then(|v| v.as_str()) {
        Some(found) if found == record_type => {}
        found => {
            return Err(format!(
                "record $type is {found:?}, expected {record_type:?}"
            ));
        }
    }
    let keys: BTreeSet<String> = object
        .keys()
        .filter(|key| key.as_str() != "$type")
        .cloned()
        .collect();

    let stray: Vec<&String> = keys.difference(allowed).collect();
    if !stray.is_empty() {
        return Err(format!(
            "field(s) outside the final lexicon: {stray:?} (declared: {allowed:?})"
        ));
    }
    let missing: Vec<&String> = required.difference(&keys).collect();
    if !missing.is_empty() {
        return Err(format!("required lexicon field(s) absent: {missing:?}"));
    }
    Ok(())
}

/// Assert (panicking) that a stored record `value` conforms to the final lexicon.
/// The capstone flow runs this on the record the PDS actually stored, so a
/// boundary leak is caught on the real wire shape, on every run.
fn assert_stored_value_within_final_lexicon(value: &serde_json::Value) {
    let (record_type, allowed, required) = final_lexicon_record_shape();
    if let Err(reason) = stored_value_within(value, &record_type, &allowed, &required) {
        panic!("the stored record violates the final lexicon: {reason}");
    }
}

/// AC4: the guard rejects any field outside the final lexicon and demands the
/// mandatory ones. Fast (no container): it exercises the pure check against the
/// on-disk lexicon, so a drifted record shape or a leaked private fact is caught
/// deterministically. The capstone flow runs the *same* check on the real record
/// the PDS stored (see `run_capstone_flow`).
#[test]
fn stored_record_has_no_field_outside_the_final_lexicon() {
    let (record_type, allowed, required) = final_lexicon_record_shape();
    assert_eq!(
        record_type, "app.zurfur.feed.post",
        "the on-disk lexicon names the final record type"
    );

    // The frozen ZMVP-104 field set (one-way door: fields may only be ADDED, and a
    // deliberate additive change must update this pin). If this fails, the on-disk
    // lexicon drifted from the field list the capstone was written against.
    let expected_allowed: BTreeSet<String> =
        ["text", "embed", "reply", "credits", "labels", "createdAt"]
            .into_iter()
            .map(String::from)
            .collect();
    let expected_required: BTreeSet<String> = ["createdAt", "labels"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(
        allowed, expected_allowed,
        "the on-disk lexicon is the final ZMVP-104 field set"
    );
    assert_eq!(
        required, expected_required,
        "the on-disk lexicon requires exactly createdAt + labels"
    );

    // A real image-bearing post's stored shape (what the adapter emits, plus the
    // atproto `$type`) conforms.
    let conforming = serde_json::json!({
        "$type": "app.zurfur.feed.post",
        "embed": { "blob": {}, "alt": "capstone probe" },
        "labels": { "$type": "com.atproto.label.defs#selfLabels", "values": [] },
        "createdAt": "2026-07-05T12:00:00.000Z",
    });
    assert!(
        stored_value_within(&conforming, &record_type, &allowed, &required).is_ok(),
        "a record whose fields are all declared by the lexicon must pass"
    );

    // A stray Index-side private fact (`medium` is Class B — it lives in The Index,
    // never on the public record) must be rejected.
    let leaky = serde_json::json!({
        "$type": "app.zurfur.feed.post",
        "labels": { "values": [] },
        "createdAt": "2026-07-05T12:00:00.000Z",
        "medium": "digital",
    });
    assert!(
        stored_value_within(&leaky, &record_type, &allowed, &required).is_err(),
        "a field outside the final lexicon (a leaked private fact) must be rejected"
    );

    // A record missing a required field must be rejected too.
    let missing_required = serde_json::json!({
        "$type": "app.zurfur.feed.post",
        "createdAt": "2026-07-05T12:00:00.000Z",
    });
    assert!(
        stored_value_within(&missing_required, &record_type, &allowed, &required).is_err(),
        "a record missing a required field (labels) must be rejected"
    );

    // The same shape under a different `$type` (a draft variant, say) must be
    // rejected: AC4 is "the final lexicon", record type included.
    let wrong_type = serde_json::json!({
        "$type": "app.zurfur.feed.post.draft",
        "labels": { "values": [] },
        "createdAt": "2026-07-05T12:00:00.000Z",
    });
    assert!(
        stored_value_within(&wrong_type, &record_type, &allowed, &required).is_err(),
        "a record under a non-final $type must be rejected"
    );
}
