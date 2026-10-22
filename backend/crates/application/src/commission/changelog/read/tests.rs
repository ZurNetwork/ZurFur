use adapter_mem::fixture::{alice, bob, old_sketch, untitled};
use domain::elements::level::Level;
use test_support::recording::{CallLog, NEVER_FOR_A_STRANGER, assert_reads_none_of};

use super::*;
use crate::{
    commission::Commissions,
    test_world::{
        self, World, absent_commission, recorded, seed_alice_s, stranger, stranger_targets,
    },
};

/// Reads `commission`'s changelog as `viewer`.
async fn read_changelog(
    world: &World,
    viewer: &UserId,
    commission: CommissionId,
) -> CommissionResult<Output> {
    let query = Query {
        actor_id: viewer.clone(),
        commission_id: commission,
    };
    Commissions::new(&world.ports).changelog().read(query).await
}

/// The changelog read's answer and the reads it made, alone.
async fn recorded_read(
    world: &World,
    log: &CallLog,
    viewer: &UserId,
    commission: CommissionId,
) -> (CommissionResult<Output>, Vec<&'static str>) {
    let before = log.events().len();
    let answer = read_changelog(world, viewer, commission).await;
    let reads = log.events().split_off(before);
    (answer, reads)
}

#[tokio::test]
async fn a_stranger_s_absent_and_not_theirs_commission_are_one_refusal_with_the_same_reads() {
    let (world, log) = recorded().await;
    let stranger = stranger();

    for target in stranger_targets(&world).await {
        let (absent, absent_reads) =
            recorded_read(&world, &log, &stranger, absent_commission()).await;
        let (not_theirs, not_theirs_reads) =
            recorded_read(&world, &log, &stranger, target.id).await;

        let label = target.label;
        assert!(
            matches!(absent, Err(CommissionError::NotAMember)),
            "{label}: absent"
        );
        assert!(
            matches!(not_theirs, Err(CommissionError::NotAMember)),
            "{label}: not theirs"
        );
        assert_eq!(absent_reads, ["find_summary", "is_participant"], "{label}");
        assert_eq!(not_theirs_reads, absent_reads, "{label}");
        assert_reads_none_of(&absent_reads, &NEVER_FOR_A_STRANGER, label);
    }
}

#[tokio::test]
async fn a_stranger_reads_no_changelog_of_a_public_or_listed_live_commission() {
    let (world, log) = recorded().await;
    let stranger = stranger();
    let public = seed_alice_s(&world, "Public, live", Level::Public, false).await;
    let listed = seed_alice_s(&world, "Listed, live", Level::Listed, false).await;

    for target in [public, listed] {
        let (answer, reads) = recorded_read(&world, &log, &stranger, target.id).await;

        let label = target.label;
        assert!(
            matches!(answer, Err(CommissionError::NotAMember)),
            "{label}"
        );
        assert_eq!(reads, ["find_summary", "is_participant"], "{label}");
        assert_reads_none_of(&reads, &NEVER_FOR_A_STRANGER, label);
    }
}

#[tokio::test]
async fn bob_a_participant_reads_untitled_s_changelog() {
    let world = test_world::world().await;

    let read = read_changelog(&world, &bob(), untitled()).await;

    let read = read.expect("bob reads Untitled's changelog");
    assert!(!read.entries.is_empty(), "Untitled's changelog has entries");
}

#[tokio::test]
async fn alice_reads_the_archived_old_sketch_s_changelog_by_its_typed_path() {
    let world = test_world::world().await;

    let read = read_changelog(&world, &alice(), old_sketch()).await;

    let read = read.expect("alice reads Old sketch's changelog");
    let kinds: Vec<ChangelogEntryKind> = read.entries.into_iter().map(|entry| entry.kind).collect();
    assert_eq!(
        kinds,
        [ChangelogEntryKind::Created, ChangelogEntryKind::Archived]
    );
}
