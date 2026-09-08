//! Commission file entries (ZMVP-88, streaming seam ZMVP-205) as a use case:
//! upload then download, against the in-memory fakes. The HTTP-surface
//! behaviors (headers, closed-door 404s, cross-commission invisibility) stay
//! pinned at `api/tests/commission_files.rs`; this file is the use case's own
//! contract — streaming, authorization order, and the reject-then-delete
//! cleanup a buffered test can't see through the HTTP layer.

use std::io::Cursor;

use application::{
    commission::{
        CommissionError,
        files::{download, upload},
    },
    transaction,
};
use chrono::Utc;
use domain::{
    elements::{
        commission::{Commission, CommissionId, CommissionTitle, FileKey},
        did::Did,
    },
    ports::{Database, UnitOfWork},
};
use tokio::io::{AsyncRead, AsyncReadExt};

/// Seeds a committed commission owned by a freshly provisioned `owner_did`.
async fn seed_commission(database: &dyn Database, owner_did: &Did, title: &str) -> CommissionId {
    let title: CommissionTitle = title.parse().expect("valid title");
    let owner_did = owner_did.clone();
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let owner = uow.users().provision(&owner_did).await?;
        let commission = Commission::create(title, owner.id, Utc::now(), None);
        let id = commission.id;
        uow.commissions().create(&commission).await?;
        Ok(id)
    })
    .await
    .expect("seed a commission")
}

/// Drains a downloaded reader into a `Vec<u8>` for a byte-equality assertion.
async fn read_all(mut content: Box<dyn AsyncRead + Send + Unpin>) -> Vec<u8> {
    let mut buf = Vec::new();
    content
        .read_to_end(&mut buf)
        .await
        .expect("read the downloaded content");
    buf
}

// Happy roundtrip — upload then download resolve to the exact bytes, with
// metadata (filename, content type, byte size) carried through.
#[tokio::test]
async fn a_participant_uploads_then_downloads_the_exact_bytes() {
    let owner_did = Did::new("did:plc:artist".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Ref sheet").await;
    let owner = runtime
        .users
        .find_by_did(&owner_did)
        .await
        .expect("find owner")
        .expect("owner provisioned");

    let bytes = b"the file contents".to_vec();
    let command = upload::Command {
        actor_id: owner.id.clone(),
        commission_id,
        filename: Some("ref.png".to_string()),
        content_type: Some("image/png".to_string()),
    };
    let app = runtime.app();
    let uploaded = app
        .commissions()
        .files()
        .upload(command, Cursor::new(bytes.clone()), 1024, Utc::now())
        .await
        .expect("upload succeeds");

    let query = download::Query {
        actor_id: owner.id,
        commission_id,
        file_id: uploaded.id,
    };
    let download::Output { result: download } = app
        .commissions()
        .files()
        .download(query)
        .await
        .expect("download succeeds");

    assert_eq!(download.metadata.filename.as_str(), "ref.png");
    assert_eq!(download.metadata.content_type, "image/png");
    assert_eq!(download.metadata.byte_size, bytes.len() as i64);
    assert_eq!(
        read_all(download.content).await,
        bytes,
        "the exact bytes round-trip"
    );
}

// The `file_added` changelog entry records the counted byte_size (not a
// caller-declared one) alongside the filename/content_type — the payload
// renders a sentence without joins (Changelog DD's core-renderable rule).
#[tokio::test]
async fn upload_records_a_file_added_changelog_entry_with_byte_size() {
    let owner_did = Did::new("did:plc:changelog-artist".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Ref").await;
    let owner = runtime
        .users
        .find_by_did(&owner_did)
        .await
        .unwrap()
        .unwrap();

    let bytes = b"12345".to_vec();
    let command = upload::Command {
        actor_id: owner.id.clone(),
        commission_id,
        filename: Some("five.bin".to_string()),
        content_type: None,
    };
    let uploaded = runtime
        .app()
        .commissions()
        .files()
        .upload(command, Cursor::new(bytes.clone()), 1024, Utc::now())
        .await
        .expect("upload succeeds");

    let entries = runtime
        .changelog
        .entries(&commission_id)
        .await
        .expect("read changelog");
    assert_eq!(entries.len(), 1, "the file_added entry, and only it");
    let entry = &entries[0];
    assert_eq!(entry.kind.as_str(), "file_added");
    assert_eq!(entry.actor_id, Some(owner.id));
    assert_eq!(entry.payload["file_id"], uploaded.id.to_string());
    assert_eq!(entry.payload["filename"], "five.bin");
    assert_eq!(entry.payload["byte_size"], bytes.len() as u64);
}

// A non-participant is turned away before a byte is read: `NotAMember`, no
// blob ever written, and nothing appended.
#[tokio::test]
async fn a_non_participant_upload_is_rejected_before_any_store_write() {
    let owner_did = Did::new("did:plc:owner".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Private").await;

    let outsider = fixture
        .backend
        .provision(&Did::new("did:plc:outsider".to_string()))
        .await
        .expect("provision outsider");

    let command = upload::Command {
        actor_id: outsider.id,
        commission_id,
        filename: Some("sneaky.png".to_string()),
        content_type: Some("image/png".to_string()),
    };
    let result = runtime
        .app()
        .commissions()
        .files()
        .upload(command, Cursor::new(b"x".to_vec()), 1024, Utc::now())
        .await;

    assert!(
        matches!(result, Err(CommissionError::NotAMember)),
        "got {result:?}"
    );
    assert_eq!(fixture.backend.blob_count(), 0, "no blob was ever written");
    assert!(
        runtime
            .changelog
            .entries(&commission_id)
            .await
            .unwrap()
            .is_empty(),
        "nothing was appended for the rejected upload",
    );
}

// Over the cap: the use case answers `FileTooLarge`, and the blob it wrote to
// count the overage is deleted, not left orphaned.
#[tokio::test]
async fn an_over_cap_upload_is_rejected_and_its_blob_is_deleted() {
    let owner_did = Did::new("did:plc:cap-artist".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Ref").await;
    let owner = runtime
        .users
        .find_by_did(&owner_did)
        .await
        .unwrap()
        .unwrap();

    let command = upload::Command {
        actor_id: owner.id,
        commission_id,
        filename: Some("big.bin".to_string()),
        content_type: None,
    };
    let result = runtime
        .app()
        .commissions()
        .files()
        .upload(command, Cursor::new(vec![b'x'; 10]), 4, Utc::now())
        .await;

    assert!(
        matches!(result, Err(CommissionError::FileTooLarge)),
        "got {result:?}"
    );
    assert_eq!(
        fixture.backend.blob_count(),
        0,
        "the over-cap blob was deleted, not orphaned"
    );
}

// Empty: the use case answers `FileEmpty`, and its blob is deleted too.
#[tokio::test]
async fn an_empty_upload_is_rejected_and_its_blob_is_deleted() {
    let owner_did = Did::new("did:plc:empty-artist".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Ref").await;
    let owner = runtime
        .users
        .find_by_did(&owner_did)
        .await
        .unwrap()
        .unwrap();

    let command = upload::Command {
        actor_id: owner.id,
        commission_id,
        filename: Some("empty.bin".to_string()),
        content_type: None,
    };
    let result = runtime
        .app()
        .commissions()
        .files()
        .upload(command, Cursor::new(Vec::new()), 1024, Utc::now())
        .await;

    assert!(
        matches!(result, Err(CommissionError::FileEmpty)),
        "got {result:?}"
    );
    assert_eq!(
        fixture.backend.blob_count(),
        0,
        "the empty upload's blob was deleted"
    );
}

// A path-separator filename is refused before minting a key: `InvalidFileName`,
// and no blob is ever written for it.
#[tokio::test]
async fn an_invalid_filename_is_rejected() {
    let owner_did = Did::new("did:plc:filename-artist".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Ref").await;
    let owner = runtime
        .users
        .find_by_did(&owner_did)
        .await
        .unwrap()
        .unwrap();

    let command = upload::Command {
        actor_id: owner.id,
        commission_id,
        filename: Some("../../etc/passwd".to_string()),
        content_type: None,
    };
    let result = runtime
        .app()
        .commissions()
        .files()
        .upload(command, Cursor::new(b"x".to_vec()), 1024, Utc::now())
        .await;

    assert!(
        matches!(result, Err(CommissionError::InvalidFileName(_))),
        "got {result:?}"
    );
    assert_eq!(
        fixture.backend.blob_count(),
        0,
        "filename validation runs before any blob is written"
    );
}

// Downloading a key nothing was ever stored under is `FileNotFound`.
#[tokio::test]
async fn downloading_an_unknown_key_is_file_not_found() {
    let owner_did = Did::new("did:plc:downloader".to_string());
    let fixture = test_support::runtime::mem(&owner_did).build();
    let runtime = fixture.runtime;
    let commission_id = seed_commission(&*runtime.database, &owner_did, "Ref").await;
    let owner = runtime
        .users
        .find_by_did(&owner_did)
        .await
        .unwrap()
        .unwrap();

    let query = download::Query {
        actor_id: owner.id,
        commission_id,
        file_id: FileKey::generate(),
    };
    let result = runtime.app().commissions().files().download(query).await;

    // `download::Output` (the `Ok` side) holds a live reader and cannot
    // derive `Debug`, so the failure message names only the error side.
    match result {
        Err(CommissionError::FileNotFound) => {}
        Err(other) => panic!("expected FileNotFound, got Err({other:?})"),
        Ok(_) => panic!("expected FileNotFound, got Ok"),
    }
}
