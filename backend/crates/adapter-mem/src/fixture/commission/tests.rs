use domain::elements::did::Did;

use super::*;

/// alice, as identity's fixture names her.
fn alice() -> UserId {
    UserId::from(Did::from("did:plc:c5vzalicefixtureaaaaaaaa".to_owned()))
}

/// bob, as identity's fixture names him.
fn bob() -> UserId {
    UserId::from(Did::from("did:plc:m2xfbobfixtureaaaaaaaaaa".to_owned()))
}

/// The Account Supreme Arts, as identity's fixture names it.
fn supreme_arts() -> AccountId {
    AccountId::from(Did::from("did:plc:t7nesupremeartsfixtureaa".to_owned()))
}

/// A backend holding commission's fixture rows.
async fn seeded() -> MemBackend {
    let backend = MemBackend::new();
    seed_commission(&backend, &alice(), &bob(), &supreme_arts())
        .await
        .expect("the commission fixtures seed");
    backend
}

/// The stored titles of a listing, in order.
fn titles(listing: &[domain::elements::commission::CommissionSummary]) -> Vec<&str> {
    listing
        .iter()
        .map(|summary| summary.title.as_str())
        .collect()
}

// alice takes part in both fixture commissions; the listing keeps the archived
// one and marks it, ordered by id.
#[tokio::test]
async fn alice_takes_part_in_untitled_and_the_archived_old_sketch() {
    let backend = seeded().await;

    let listing = backend
        .commission_store()
        .list_participating(&alice())
        .await
        .unwrap();

    assert_eq!(titles(&listing), ["Old sketch", "Untitled"]);
    let ids: Vec<CommissionId> = listing.iter().map(|summary| summary.id).collect();
    assert_eq!(ids, [old_sketch(), untitled()]);
    assert!(listing[0].is_archived(), "Old sketch is archived");
    assert!(!listing[1].is_archived(), "Untitled is live");
    assert!(
        listing
            .iter()
            .all(|summary| summary.visibility == Visibility::Private),
        "every fixture commission is Private"
    );
}

// bob takes part only in "Untitled": "Old sketch" has alice as its only
// Participant.
#[tokio::test]
async fn bob_takes_part_only_in_untitled() {
    let backend = seeded().await;
    let store = backend.commission_store();

    let listing = store.list_participating(&bob()).await.unwrap();

    assert_eq!(titles(&listing), ["Untitled"]);
    assert!(store.is_participant(&untitled(), &bob()).await.unwrap());
    assert!(!store.is_participant(&old_sketch(), &bob()).await.unwrap());
    assert!(store.is_participant(&old_sketch(), &alice()).await.unwrap());
}

// "Untitled" holds its two files, named and keyed as fixed, and the name read
// answers without the bytes.
#[tokio::test]
async fn untitled_holds_its_two_files() {
    let backend = seeded().await;

    let files = backend.commission_store().files(&untitled()).await.unwrap();
    let keys: Vec<FileKey> = files.iter().map(|file| file.id).collect();
    assert_eq!(keys, untitled_files());
    let upload_times: Vec<DateTimeUtc> = files.iter().map(|file| file.created_at).collect();
    assert_eq!(upload_times, [time_of(REF_ABCO), time_of(SKETCH_01)]);

    let file_store = backend.file_store();
    let mut names = Vec::new();
    for key in untitled_files() {
        let name = file_store.filename(key).await.unwrap().expect("stored");
        names.push(name.as_str().to_owned());
    }
    assert_eq!(names, ["ref-abco.png", "sketch-01.png"]);
}

// "Untitled" declares four open Slots, in order, with the fixed ids.
#[tokio::test]
async fn untitled_declares_four_open_slots() {
    let backend = seeded().await;

    let slots = backend.commission_store().slots(&untitled()).await.unwrap();

    let slot_titles: Vec<&str> = slots.iter().map(|slot| slot.title.as_str()).collect();
    assert_eq!(slot_titles, ["Abco", "Ember", "Kael", "Open slot"]);
    let ids: Vec<ElementId> = slots.iter().map(|slot| slot.element_id).collect();
    assert_eq!(ids, untitled_slots());
}

// "Old sketch" has no files and no Slots.
#[tokio::test]
async fn old_sketch_holds_no_files_and_no_slots() {
    let backend = seeded().await;
    let store = backend.commission_store();

    assert!(store.files(&old_sketch()).await.unwrap().is_empty());
    assert!(store.slots(&old_sketch()).await.unwrap().is_empty());
}

// The Account's board holds one Workflow, "Queue", Private and with no columns.
#[tokio::test]
async fn the_account_holds_the_workflow_queue() {
    let backend = seeded().await;
    let workflows = backend.workflow_store();

    let listing = workflows.list_for_account(&supreme_arts()).await.unwrap();

    let names: Vec<&str> = listing
        .iter()
        .map(|summary| summary.name.as_str())
        .collect();
    assert_eq!(names, ["Queue"]);
    assert_eq!(listing[0].id, queue());
    let board = workflows
        .find(&queue())
        .await
        .unwrap()
        .expect("Queue exists");
    assert_eq!(board.visibility, Visibility::Private);
    assert!(workflows.columns(&queue()).await.unwrap().is_empty());
}

// Each fixture commission carries the changelog its use cases would write.
#[tokio::test]
async fn the_fixture_commissions_carry_their_changelog() {
    let backend = seeded().await;

    let untitled_kinds: Vec<ChangelogEntryKind> = backend
        .changelog_entries(untitled())
        .await
        .unwrap()
        .iter()
        .map(|entry| entry.kind)
        .collect();
    let old_sketch_kinds: Vec<ChangelogEntryKind> = backend
        .changelog_entries(old_sketch())
        .await
        .unwrap()
        .iter()
        .map(|entry| entry.kind)
        .collect();

    let expected_untitled = [
        ChangelogEntryKind::Created,
        ChangelogEntryKind::FileAdded,
        ChangelogEntryKind::FileAdded,
    ];
    let expected_old_sketch = [ChangelogEntryKind::Created, ChangelogEntryKind::Archived];
    assert_eq!(untitled_kinds, expected_untitled);
    assert_eq!(old_sketch_kinds, expected_old_sketch);
}

/// Everything the Den reads from a seeded backend, in the order it reads it:
/// both people's listings, "Untitled"'s files with their names and its Slots,
/// each commission's changelog, and the Account's Workflows.
async fn what_the_den_reads(backend: &MemBackend) -> String {
    let store = backend.commission_store();
    let file_store = backend.file_store();
    let mut lines = Vec::new();
    for person in [alice(), bob()] {
        let listing = store.list_participating(&person).await.unwrap();
        lines.push(format!("{listing:?}"));
    }
    for file in store.files(&untitled()).await.unwrap() {
        let name = file_store.filename(file.id).await.unwrap();
        lines.push(format!("{file:?} {name:?}"));
    }
    let slots = store.slots(&untitled()).await.unwrap();
    lines.push(format!("{slots:?}"));
    for commission in [untitled(), old_sketch()] {
        for entry in backend.changelog_entries(commission).await.unwrap() {
            let read = (entry.kind, entry.actor_id, entry.payload, entry.created_at);
            lines.push(format!("{read:?}"));
        }
    }
    let workflows = backend
        .workflow_store()
        .list_for_account(&supreme_arts())
        .await
        .unwrap();
    lines.push(format!("{workflows:?}"));
    lines.join("\n")
}

// Seeding two backends gives the same world wherever the Den reads it: ids,
// names and times are all fixed.
#[tokio::test]
async fn seeding_is_deterministic() {
    let first = seeded().await;
    let second = seeded().await;

    let first_reads = what_the_den_reads(&first).await;
    let second_reads = what_the_den_reads(&second).await;

    assert_eq!(first_reads, second_reads);
    let commission = first.find_commission(untitled()).await.unwrap().unwrap();
    assert_eq!(commission.created_at, time_of(UNTITLED));
}

// "Old sketch" was created before "Untitled" and archived in the past.
#[tokio::test]
async fn old_sketch_was_archived_in_the_past() {
    let backend = seeded().await;

    let old = backend
        .find_commission(old_sketch())
        .await
        .unwrap()
        .unwrap();
    let archived_at = old.archived_at.expect("Old sketch is archived");

    assert!(old.created_at < time_of(UNTITLED));
    assert!(
        archived_at < time_of(UNTITLED),
        "archived before Untitled began"
    );
    assert!(archived_at < chrono::Utc::now());
}
