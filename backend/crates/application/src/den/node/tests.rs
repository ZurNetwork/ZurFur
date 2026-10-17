use domain::elements::commission::CommissionId;
use uuid::Uuid;

use super::*;
use crate::{
    den::{DenNode, NameOrdinal},
    test_world::{self, seed_titled_at},
};

/// The older of the two commissions titled "Test 1": a fixed UUIDv7 a millisecond-scale age
/// before the newer one.
const OLDER: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0d01);

/// The newer of the two commissions titled "Test 1".
const NEWER: Uuid = Uuid::from_u128(0x0194_0000_0000_7000_8000_0000_0000_0d02);

/// The title both twins carry.
const TWIN_TITLE: &str = "Test 1";

/// A number the tests expect.
fn ordinal(number: u32) -> Option<NameOrdinal> {
    let ordinal = NameOrdinal::try_from(number).expect("a valid test ordinal");
    Some(ordinal)
}

/// The den Query for `viewer` at `pieces`.
async fn read(
    ports: &Ports,
    viewer: &UserId,
    include_deleted: bool,
    pieces: &[String],
) -> Result<Output, DenError> {
    let query = NodeQuery {
        viewer: viewer.clone(),
        path: pieces.to_vec(),
        include_deleted,
        page: None,
    };
    Den::from(ports).node(query).await
}

/// The Den path to `commission` in the viewer's `commissions` folder, then `below` it.
fn commission_pieces(commission: CommissionId, below: &[&str]) -> Vec<String> {
    let folder = ["commissions".to_owned(), commission.to_string()];
    let below = below.iter().map(|piece| (*piece).to_owned());
    folder.into_iter().chain(below).collect()
}

/// The last segment an entry shows, or nothing on a card.
fn last_segment(node: &DenNode) -> String {
    node.segments
        .last()
        .map(|segment| segment.as_ref().to_owned())
        .unwrap_or_default()
}

/// The twins in the viewer's `commissions` listing, each as its segment and its number.
async fn twins_listed(
    ports: &Ports,
    viewer: &UserId,
    include_deleted: bool,
) -> Vec<(String, Option<NameOrdinal>)> {
    let folder = ["commissions".to_owned()];
    let output = read(ports, viewer, include_deleted, &folder)
        .await
        .expect("the commissions folder opens");
    let Some(DenContent::Listing(listing)) = output.content else {
        panic!("the commissions folder lists");
    };
    listing
        .entries
        .iter()
        .filter(|entry| entry.name.as_ref() == TWIN_TITLE)
        .map(|entry| (last_segment(entry), entry.name_ordinal))
        .collect()
}

/// `(commission, number)` pairs as [`twins_listed`] answers them.
fn expected_twins(
    pairs: &[(CommissionId, Option<NameOrdinal>)],
) -> Vec<(String, Option<NameOrdinal>)> {
    pairs
        .iter()
        .map(|(commission, number)| (commission.to_string(), *number))
        .collect()
}

/// The number the node read gives the node at `pieces`.
async fn node_number(
    ports: &Ports,
    viewer: &UserId,
    include_deleted: bool,
    pieces: &[String],
) -> Option<NameOrdinal> {
    let output = read(ports, viewer, include_deleted, pieces)
        .await
        .expect("the node opens");
    output.node.name_ordinal
}

#[tokio::test]
async fn bob_sees_his_one_twin_plain_while_alice_sees_both_told_apart() {
    let (world, backend) = test_world::world_and_backend().await;
    let alice = adapter_mem::fixture::alice();
    let bob = adapter_mem::fixture::bob();
    let older = seed_titled_at(&world, &backend, OLDER, TWIN_TITLE, false, &[]).await;
    let newer = seed_titled_at(
        &world,
        &backend,
        NEWER,
        TWIN_TITLE,
        false,
        std::slice::from_ref(&bob),
    )
    .await;

    let alice_s_listing = twins_listed(&world.ports, &alice, false).await;
    let bob_s_listing = twins_listed(&world.ports, &bob, false).await;
    let alice_s_newer = read(&world.ports, &alice, false, &commission_pieces(newer, &[]))
        .await
        .expect("alice opens the newer twin");
    let bob_s_newer = node_number(&world.ports, &bob, false, &commission_pieces(newer, &[])).await;

    let expected_alice_s = expected_twins(&[(older, None), (newer, ordinal(2))]);
    let expected_bob_s = expected_twins(&[(newer, None)]);
    assert_eq!(alice_s_listing, expected_alice_s);
    assert_eq!(bob_s_listing, expected_bob_s);
    assert_eq!(alice_s_newer.node.name.as_ref(), TWIN_TITLE);
    assert_eq!(alice_s_newer.node.name_ordinal, ordinal(2));
    assert_eq!(bob_s_newer, None);
}

#[tokio::test]
async fn a_twin_s_crumb_carries_its_number_below_it() {
    let (world, backend) = test_world::world_and_backend().await;
    let alice = adapter_mem::fixture::alice();
    seed_titled_at(&world, &backend, OLDER, TWIN_TITLE, false, &[]).await;
    let newer = seed_titled_at(&world, &backend, NEWER, TWIN_TITLE, false, &[]).await;

    let attachments = read(
        &world.ports,
        &alice,
        false,
        &commission_pieces(newer, &["attachments"]),
    )
    .await
    .expect("the newer twin's attachments open");

    let crumb_numbers: Vec<_> = attachments
        .crumbs
        .iter()
        .map(|crumb| crumb.name_ordinal)
        .collect();
    assert_eq!(crumb_numbers, [None, None, ordinal(2)]);
    assert_eq!(attachments.crumbs[2].name.as_ref(), TWIN_TITLE);
    assert_eq!(attachments.node.name_ordinal, None);
}

#[tokio::test]
async fn a_path_with_no_twins_carries_no_number() {
    let world = test_world::world().await;
    let alice = adapter_mem::fixture::alice();
    let untitled = adapter_mem::fixture::untitled();

    let attachments = read(
        &world.ports,
        &alice,
        false,
        &commission_pieces(untitled, &["attachments"]),
    )
    .await
    .expect("Untitled's attachments open");

    let Some(DenContent::Listing(listing)) = &attachments.content else {
        panic!("the attachments list");
    };
    let numbers: Vec<_> = attachments
        .crumbs
        .iter()
        .map(|crumb| crumb.name_ordinal)
        .chain([attachments.node.name_ordinal])
        .chain(listing.entries.iter().map(|entry| entry.name_ordinal))
        .collect();
    assert!(numbers.iter().all(Option::is_none), "{numbers:?}");
    assert_eq!(numbers.len(), 6);
}

#[tokio::test]
async fn an_archived_twin_numbers_only_with_include_deleted() {
    let (world, backend) = test_world::world_and_backend().await;
    let alice = adapter_mem::fixture::alice();
    let older = seed_titled_at(&world, &backend, OLDER, TWIN_TITLE, true, &[]).await;
    let newer = seed_titled_at(&world, &backend, NEWER, TWIN_TITLE, false, &[]).await;
    let newer_pieces = commission_pieces(newer, &[]);
    let older_pieces = commission_pieces(older, &[]);

    let hidden_listing = twins_listed(&world.ports, &alice, false).await;
    let hidden_newer = node_number(&world.ports, &alice, false, &newer_pieces).await;
    let hidden_older = node_number(&world.ports, &alice, false, &older_pieces).await;
    let shown_listing = twins_listed(&world.ports, &alice, true).await;
    let shown_newer = node_number(&world.ports, &alice, true, &newer_pieces).await;
    let shown_older = node_number(&world.ports, &alice, true, &older_pieces).await;

    let expected_hidden = expected_twins(&[(newer, None)]);
    let expected_shown = expected_twins(&[(older, None), (newer, ordinal(2))]);
    assert_eq!(hidden_listing, expected_hidden);
    assert_eq!(hidden_newer, None);
    assert_eq!(hidden_older, None);
    assert_eq!(shown_listing, expected_shown);
    assert_eq!(shown_newer, ordinal(2));
    assert_eq!(shown_older, None);
}

#[tokio::test]
async fn telling_twins_apart_stores_nothing() {
    let (world, backend) = test_world::world_and_backend().await;
    let alice = adapter_mem::fixture::alice();
    let older = seed_titled_at(&world, &backend, OLDER, TWIN_TITLE, false, &[]).await;
    let newer = seed_titled_at(&world, &backend, NEWER, TWIN_TITLE, false, &[]).await;

    let listed = twins_listed(&world.ports, &alice, false).await;
    let numbered = node_number(&world.ports, &alice, false, &commission_pieces(newer, &[])).await;

    assert_eq!(numbered, ordinal(2));
    assert_eq!(listed.len(), 2);
    for commission in [older, newer] {
        let stored = world
            .ports
            .commissions
            .find_summary(&commission)
            .await
            .expect("the store answers")
            .expect("the twin is stored");
        assert_eq!(stored.title.as_str(), TWIN_TITLE);
    }
}
