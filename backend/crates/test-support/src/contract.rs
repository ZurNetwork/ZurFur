//! The shared [`PublicRecords`] conformance suite (ZMVP-105, AC5).
//!
//! One generic body, run against **every** adapter that claims to implement the
//! public-boundary write port — the in-memory fake ([`adapter-mem`]'s
//! `MemPublicRecords`, fast unit test) and the real atproto adapter driving a
//! [`ThrowawayPds`](crate::ThrowawayPds) fixture account (integration test,
//! container). The suite *existing and passing on both* is the acceptance
//! criterion: it is what proves the fake and the real boundary honor the same
//! contract, so core development against the fake is trustworthy.
//!
//! Scope is the **port surface only** — create / put / delete / get / upload — so
//! the single body can run anywhere. Behaviors that need something off-port (an
//! unreachable endpoint, an authorization refusal, a raw blob download for a
//! byte-for-byte compare) live in the adapter's own test module, not here.

use domain::elements::did::Did;
use domain::elements::public_record::{
    AspectRatio, Credit, Embed, FeedPost, PublicRecord, ReplyRef, ReplySubject, SelfLabels,
};
use domain::ports::{PublicRecords, PublicRecordsError};

/// A minimal, valid 1×1 PNG — real image bytes so the PDS's own mime inference
/// agrees with the `image/png` hint (the reference PDS sniffs blob content).
pub const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// A fixed, millisecond-precision timestamp so field-identical round-trips do not
/// depend on sub-millisecond clock precision surviving an RFC-3339 encode.
///
/// Public so a determinism capstone (ZMVP-106) can pin the **same** `createdAt`
/// this suite uses: with the timestamp fixed, the record's DAG-CBOR bytes — and
/// therefore its content-address record CID — are identical across two runs on
/// two freshly-booted PDSes, which is exactly what "the write path is
/// deterministic against a wipeable rig" asserts. Reuse this rather than
/// duplicating the literal.
pub fn fixed_created_at() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-07-05T12:00:00.000Z")
        .expect("valid RFC-3339")
        .with_timezone(&chrono::Utc)
}

fn expect_feed_post(record: PublicRecord) -> FeedPost {
    match record {
        PublicRecord::FeedPost(post) => post,
    }
}

/// Run the full shared conformance suite against `store`, acting as `actor`.
///
/// `actor` is the repo every write targets; the real adapter can only write to
/// the identity it is authenticated as, so callers pass the fixture account's DID
/// (the fake accepts any DID). Panics with a descriptive message on the first
/// divergence, so a failure names the exact behavior that broke.
pub async fn public_records_contract<T: PublicRecords + ?Sized>(store: &T, actor: &Did) {
    create_reads_back_field_identical(store, actor).await;
    put_overwrites_in_place(store, actor).await;
    delete_then_get_is_not_found(store, actor).await;
    blob_upload_is_content_addressed_and_referenced(store, actor).await;
    rich_record_round_trips(store, actor).await;
}

/// AC1: a created record reads back field-for-field identical.
async fn create_reads_back_field_identical<T: PublicRecords + ?Sized>(store: &T, actor: &Did) {
    let post = FeedPost {
        text: Some("a plain text post".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: fixed_created_at(),
    };
    let written = PublicRecord::FeedPost(post.clone());

    let record_ref = store
        .create_record(actor, &written)
        .await
        .expect("create_record should succeed");
    assert_eq!(
        &record_ref.uri.did, actor,
        "the returned URI must be in the acting repo"
    );
    assert_eq!(
        record_ref.uri.collection.as_str(),
        "app.zurfur.feed.post",
        "the collection is fixed by the record variant"
    );

    let read = store
        .get_record(&record_ref.uri)
        .await
        .expect("get_record should read back the just-created record");
    assert_eq!(
        read, written,
        "the record read back must be field-identical to the one written"
    );
}

/// AC1 (update): put overwrites the record at its key in place.
async fn put_overwrites_in_place<T: PublicRecords + ?Sized>(store: &T, actor: &Did) {
    let original = PublicRecord::FeedPost(FeedPost {
        text: Some("first".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: fixed_created_at(),
    });
    let record_ref = store
        .create_record(actor, &original)
        .await
        .expect("create for put test");

    let updated = PublicRecord::FeedPost(FeedPost {
        text: Some("second — overwritten".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels(vec!["suggestive".to_string()]),
        created_at: fixed_created_at(),
    });
    let put_ref = store
        .put_record(&record_ref.uri, &updated)
        .await
        .expect("put_record should overwrite");
    assert_eq!(
        put_ref.uri, record_ref.uri,
        "put keeps the same address (same rkey)"
    );

    let read = store
        .get_record(&record_ref.uri)
        .await
        .expect("get after put");
    assert_eq!(read, updated, "get reflects the put, not the original");
}

/// AC3: delete removes the record; a subsequent get is a clean NotFound.
async fn delete_then_get_is_not_found<T: PublicRecords + ?Sized>(store: &T, actor: &Did) {
    let record = PublicRecord::FeedPost(FeedPost {
        text: Some("doomed".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: fixed_created_at(),
    });
    let record_ref = store
        .create_record(actor, &record)
        .await
        .expect("create for delete test");

    store
        .delete_record(&record_ref.uri)
        .await
        .expect("delete_record should succeed");

    match store.get_record(&record_ref.uri).await {
        Err(PublicRecordsError::NotFound) => {}
        other => panic!("get after delete must be Err(NotFound), got {other:?}"),
    }
}

/// AC2: a blob upload is content-addressed (stable CID for identical bytes) and a
/// record can reference it, round-tripping the reference field-identically.
async fn blob_upload_is_content_addressed_and_referenced<T: PublicRecords + ?Sized>(
    store: &T,
    actor: &Did,
) {
    let blob_ref = store
        .upload_blob(TINY_PNG.to_vec(), "image/png")
        .await
        .expect("upload_blob should succeed");
    assert_eq!(
        blob_ref.size,
        TINY_PNG.len() as u64,
        "the blob ref records the uploaded byte length"
    );

    // Content-addressing: uploading the identical bytes yields the identical CID.
    // (A byte-different blob would therefore address differently — the guarantee
    // behind "reads back byte-identical".)
    let again = store
        .upload_blob(TINY_PNG.to_vec(), "image/png")
        .await
        .expect("second upload_blob");
    assert_eq!(
        blob_ref.cid, again.cid,
        "identical bytes must content-address to the same CID"
    );

    let embed = Embed {
        blob: blob_ref.clone(),
        alt: "a one-pixel test image".to_string(),
        aspect_ratio: Some(AspectRatio {
            width: 1,
            height: 1,
        }),
    };
    let record = PublicRecord::FeedPost(FeedPost {
        text: None,
        embed: Some(embed.clone()),
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: fixed_created_at(),
    });
    let record_ref = store
        .create_record(actor, &record)
        .await
        .expect("create image-bearing record");

    let read = expect_feed_post(
        store
            .get_record(&record_ref.uri)
            .await
            .expect("get image record"),
    );
    let read_embed = read.embed.expect("the record must carry the embed");
    assert_eq!(
        read_embed, embed,
        "the embedded blob reference must round-trip field-identically \
         (cid, mime, size, alt, aspect ratio)"
    );
}

/// AC1 over the correlation-bearing fields: credits, non-empty labels, and a
/// reply anchor all round-trip field-identically.
async fn rich_record_round_trips<T: PublicRecords + ?Sized>(store: &T, actor: &Did) {
    // A record to reply to, so the reply's strong-ref points at a real revision.
    let root = PublicRecord::FeedPost(FeedPost {
        text: Some("the root post".to_string()),
        embed: None,
        reply: None,
        credits: Vec::new(),
        labels: SelfLabels::safe(),
        created_at: fixed_created_at(),
    });
    let root_ref = store
        .create_record(actor, &root)
        .await
        .expect("create root for reply test");
    let root_subject = ReplySubject::Record(domain::elements::public_record::StrongRef {
        uri: root_ref.uri.clone(),
        cid: root_ref.cid,
    });

    let reply = FeedPost {
        text: Some("a reply crediting collaborators".to_string()),
        embed: None,
        reply: Some(ReplyRef {
            root: root_subject.clone(),
            parent: root_subject,
        }),
        credits: vec![
            Credit {
                role: "artist".to_string(),
                did: Did::new("did:plc:collaborator1".to_string()),
            },
            Credit {
                role: "some-unknown-open-role".to_string(),
                did: Did::new("did:plc:collaborator2".to_string()),
            },
        ],
        labels: SelfLabels(vec!["nudity".to_string(), "adult".to_string()]),
        created_at: fixed_created_at(),
    };
    let written = PublicRecord::FeedPost(reply.clone());
    let record_ref = store
        .create_record(actor, &written)
        .await
        .expect("create rich record");

    let read = store
        .get_record(&record_ref.uri)
        .await
        .expect("get rich record");
    assert_eq!(
        read, written,
        "credits, labels, and the reply anchor must all round-trip field-identically"
    );
}
