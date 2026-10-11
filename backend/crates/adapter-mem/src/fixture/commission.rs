//! Commission's part of the fixture world: the commission "Untitled" (two
//! Participants, two files, four open Slots), the archived "Old sketch" (its
//! owner its only Participant) and the Workflow "Queue" on an Account's board.
//!
//! Every id the Den reads is fixed, and each row's time is the one its UUIDv7
//! id carries, so seeding twice gives the same listings, files, Slots,
//! changelog and Workflows. The skeleton tab ids are the exception: the write
//! port mints them, they differ per seed, and the Den never shows them.

use chrono::{DateTime, TimeDelta};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        commission::{
            ChangelogEntryKind, Commission, CommissionFile, CommissionId, CommissionTitle,
            ElementId, FileKey, FileName, NewChangelogEntry, NewSlot, SlotTitle, SurfaceAddress,
            SurfaceName, Visibility,
        },
        user::UserId,
        workflow::{WorkflowId, WorkflowName},
    },
};
use serde_json::json;
use uuid::Uuid;

use crate::{MemBackend, workflow::StoredWorkflow};

/// "Untitled", the commission alice owns and bob takes part in.
const UNTITLED: Uuid = Uuid::from_u128(0x01a0ef9c_4e00_7a00_8000_000000000001);
/// "Old sketch", alice's archived commission, created months before
/// "Untitled" (2026-06-01).
const OLD_SKETCH: Uuid = Uuid::from_u128(0x019e830e_1a00_7a00_8000_000000000002);
/// The file "ref-abco.png" in "Untitled".
const REF_ABCO: Uuid = Uuid::from_u128(0x01a0ef9c_5000_7a00_8000_000000000011);
/// The file "sketch-01.png" in "Untitled".
const SKETCH_01: Uuid = Uuid::from_u128(0x01a0ef9c_5100_7a00_8000_000000000012);
/// The four Slots of "Untitled", each with its title, in declaration order.
const SLOTS: [(Uuid, &str); 4] = [
    (
        Uuid::from_u128(0x01a0ef9c_6000_7a00_8000_000000000021),
        "Abco",
    ),
    (
        Uuid::from_u128(0x01a0ef9c_6100_7a00_8000_000000000022),
        "Ember",
    ),
    (
        Uuid::from_u128(0x01a0ef9c_6200_7a00_8000_000000000023),
        "Kael",
    ),
    (
        Uuid::from_u128(0x01a0ef9c_6300_7a00_8000_000000000024),
        "Open slot",
    ),
];
/// The Workflow "Queue" on the Account's board.
const QUEUE: Uuid = Uuid::from_u128(0x01a0ef9c_7000_7a00_8000_000000000031);

/// How long after its creation "Old sketch" was archived.
const OLD_SKETCH_ARCHIVED_AFTER: TimeDelta = TimeDelta::days(30);

/// The fixed id of the commission "Untitled".
pub fn untitled() -> CommissionId {
    CommissionId::from(UNTITLED)
}

/// The fixed id of the archived commission "Old sketch".
pub fn old_sketch() -> CommissionId {
    CommissionId::from(OLD_SKETCH)
}

/// The fixed keys of "Untitled"'s two files, in upload order.
pub fn untitled_files() -> [FileKey; 2] {
    [FileKey::from(REF_ABCO), FileKey::from(SKETCH_01)]
}

/// The fixed element ids of "Untitled"'s four Slots, in declaration order.
pub fn untitled_slots() -> [ElementId; 4] {
    SLOTS.map(|(id, _)| ElementId::from(id))
}

/// The fixed id of the Workflow "Queue".
pub fn queue() -> WorkflowId {
    WorkflowId::from(QUEUE)
}

/// Seed commission's fixture rows into `backend`: "Untitled" owned by `alice`
/// with `bob` a Participant, "Old sketch" owned by `alice` and archived, and the
/// Workflow "Queue" on `account`'s board. The people and the Account are
/// identity's fixtures, passed in so this module owns none of their values.
pub async fn seed_commission(
    backend: &MemBackend,
    alice: &UserId,
    bob: &UserId,
    account: &AccountId,
) -> anyhow::Result<()> {
    seed_untitled(backend, alice, bob).await?;
    seed_old_sketch(backend, alice).await?;
    seed_queue(backend, account);
    Ok(())
}

/// "Untitled": created by `alice` through the commission write port, `bob`
/// added as a Participant, two files uploaded by `alice`, four open Slots.
async fn seed_untitled(backend: &MemBackend, alice: &UserId, bob: &UserId) -> anyhow::Result<()> {
    let created_at = time_of(UNTITLED);
    create_commission(backend, UNTITLED, "Untitled", alice).await?;

    // No route admits a second Participant yet, so bob's row is seeded.
    backend.seed_participant(untitled(), bob.clone(), created_at);

    for (key, filename) in [(REF_ABCO, "ref-abco.png"), (SKETCH_01, "sketch-01.png")] {
        upload_file(backend, key, filename, alice).await?;
    }

    let address = only_address(backend, untitled()).await?;
    let slots = SLOTS
        .iter()
        .map(|(id, title)| {
            let title = title.parse::<SlotTitle>()?;
            let mut slot = NewSlot::contributed_at(
                untitled(),
                address.clone(),
                title,
                None,
                alice.clone(),
                time_of(*id),
            );
            slot.id = ElementId::from(*id);
            Ok(slot)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let database = backend.database();
    let mut uow = database.begin().await?;
    uow.commissions().declare_slots(&slots).await?;
    uow.commit().await?;
    Ok(())
}

/// "Old sketch": created by `alice` and archived by her a while later.
async fn seed_old_sketch(backend: &MemBackend, alice: &UserId) -> anyhow::Result<()> {
    create_commission(backend, OLD_SKETCH, "Old sketch", alice).await?;

    let archived_at = time_of(OLD_SKETCH) + OLD_SKETCH_ARCHIVED_AFTER;
    let archived_entry = NewChangelogEntry::event(
        old_sketch(),
        ChangelogEntryKind::Archived,
        alice.clone(),
        json!({ "title": "Old sketch" }),
        archived_at,
    );
    let database = backend.database();
    let mut uow = database.begin().await?;
    uow.commissions()
        .set_archived(&old_sketch(), Some(archived_at))
        .await?;
    uow.changelog().append(&archived_entry).await?;
    uow.commit().await?;
    Ok(())
}

/// "Queue": a Private Workflow on `account`'s board, with no columns. Inserted
/// directly, since the Workflow write port mints its own id and the fixture's
/// is fixed.
fn seed_queue(backend: &MemBackend, account: &AccountId) {
    let name = "Queue"
        .parse::<WorkflowName>()
        .expect("the fixture's Workflow name is valid");
    let stored = StoredWorkflow {
        account_id: account.clone(),
        name,
        visibility: Visibility::Private,
    };
    backend
        .workflows
        .lock()
        .expect("MemBackend workflows mutex poisoned")
        .insert(queue(), stored);
}

/// Create the commission `id` titled `title` and owned by `owner`, with its
/// `created` changelog entry, as the create use case writes them.
async fn create_commission(
    backend: &MemBackend,
    id: Uuid,
    title: &str,
    owner: &UserId,
) -> anyhow::Result<()> {
    let created_at = time_of(id);
    let mut commission = Commission::create(
        title.parse::<CommissionTitle>()?,
        owner.clone(),
        created_at,
        None,
    );
    commission.id = CommissionId::from(id);
    let created_entry = NewChangelogEntry::event(
        commission.id,
        ChangelogEntryKind::Created,
        owner.clone(),
        json!({ "title": title }),
        created_at,
    );
    let database = backend.database();
    let mut uow = database.begin().await?;
    uow.commissions().create(&commission).await?;
    uow.changelog().append(&created_entry).await?;
    uow.commit().await?;
    Ok(())
}

/// Upload a small placeholder file into "Untitled" under `key`, as the upload
/// use case writes it: bytes first, then the link and its `file_added` entry.
async fn upload_file(
    backend: &MemBackend,
    key: Uuid,
    filename: &str,
    uploader: &UserId,
) -> anyhow::Result<()> {
    const CONTENT_TYPE: &str = "image/png";
    let file_key = FileKey::from(key);
    let uploaded_at = time_of(key);
    let checked_name = FileName::try_new(filename)?;
    let mut content: &[u8] = b"fixture image bytes";
    let byte_size = backend
        .file_store()
        .put(file_key, &checked_name, CONTENT_TYPE, &mut content)
        .await?;

    let file_added_entry = NewChangelogEntry::event(
        untitled(),
        ChangelogEntryKind::FileAdded,
        uploader.clone(),
        json!({
            "file_id": key,
            "filename": filename,
            "content_type": CONTENT_TYPE,
            "byte_size": byte_size,
        }),
        uploaded_at,
    );
    let file = CommissionFile {
        id: file_key,
        commission_id: untitled(),
        uploaded_by: uploader.clone(),
        created_at: uploaded_at,
    };
    let database = backend.database();
    let mut uow = database.begin().await?;
    uow.commissions().add_file(&file).await?;
    uow.changelog().append(&file_added_entry).await?;
    uow.commit().await?;
    Ok(())
}

/// The commission's one skeleton address: its only tab's only surface.
async fn only_address(
    backend: &MemBackend,
    commission: CommissionId,
) -> anyhow::Result<SurfaceAddress> {
    let tabs = backend.tabs_of(commission).await?;
    let tab = tabs
        .first()
        .ok_or_else(|| anyhow::anyhow!("a created commission has its skeleton tab"))?;
    let surface = "content".parse::<SurfaceName>()?;
    Ok(SurfaceAddress::new(tab.id, surface))
}

/// The instant a UUIDv7 id carries, so a row's time matches its id.
fn time_of(id: Uuid) -> DateTimeUtc {
    let (seconds, nanos) = id
        .get_timestamp()
        .expect("every fixture id is a UUIDv7")
        .to_unix();
    let seconds = i64::try_from(seconds).expect("a fixture time fits in i64 seconds");
    DateTime::from_timestamp(seconds, nanos).expect("a fixture time is in range")
}

#[cfg(test)]
mod tests;
