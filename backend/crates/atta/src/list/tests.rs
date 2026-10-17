use super::card_order;
use crate::fake::{
    Call, FakeHost, GRANT_CARD, STANDING, den_path, kind, own_level, path, real_path, segment,
};
use crate::{
    Access, Caps, Entry, Error, Kind, Level, Listing, NameOrdinal, PageToken, Reach, Resolver, Via,
    View, Viewer,
};

/// The access a viewer holds at `level`, given by `via`.
fn expected_access(level: Level, via: Via) -> Access {
    Access { level, via }
}

/// alice's root holds a folder with three real children and three mounts.
fn world() -> FakeHost {
    FakeHost::default()
        .top_level()
        .file_type("file")
        .node("/user/alice", "user", Level::Private, &[STANDING])
        .node("/user/alice/stuff", "folder", Level::Private, &[])
        .node("/user/alice/stuff/b", "file", Level::Private, &[])
        .named("/user/alice/stuff/b", "Beta")
        .node("/user/alice/stuff/a", "folder", Level::Private, &[])
        .named("/user/alice/stuff/a", "Alpha")
        .node("/user/alice/stuff/z", "folder", Level::Private, &[])
        .named("/user/alice/stuff/z", "Alpha")
        .mount("/user/alice/stuff", "m1", "/commission/c1")
        .mount("/user/alice/stuff", "m2", "/commission/stranger")
        .mount("/user/alice/stuff", "m3", "/commission/card")
        .node("/commission/c1", "commission", Level::Private, &[STANDING])
        .named("/commission/c1", "Gamma")
        .node("/commission/stranger", "commission", Level::Private, &[])
        .node(
            "/commission/card",
            "commission",
            Level::Private,
            &[GRANT_CARD],
        )
        .named("/commission/card", "Delta")
}

fn resolver(host: &FakeHost, page_size: usize) -> Resolver<'_> {
    let caps = Caps {
        page_size,
        ..Caps::default()
    };
    Resolver {
        host,
        viewer: Viewer::from("alice".to_owned()),
        caps,
    }
}

async fn list(
    host: &FakeHost,
    folder: &[&str],
    page_size: usize,
    page: Option<PageToken>,
) -> Result<Option<Listing>, Error> {
    let resolver = resolver(host, page_size);
    let den_path: Vec<_> = folder.iter().map(|piece| segment(piece)).collect();
    let resolved = resolver
        .resolve_path(&path("/user/alice"), &den_path)
        .await?;
    resolver.list(&resolved, page).await
}

async fn listing(host: &FakeHost, page_size: usize, page: Option<PageToken>) -> Listing {
    list(host, &["stuff"], page_size, page)
        .await
        .expect("the folder resolves")
        .expect("the folder lists")
}

/// Each entry's last Den path segment, or `card:<name>` for a card, which shows no path.
fn segments(entries: &[Entry]) -> Vec<String> {
    let shown = |entry: &Entry| match den_path(entry).and_then(<[_]>::last) {
        Some(segment) => segment.to_string(),
        None => format!("card:{}", entry.name),
    };
    entries.iter().map(shown).collect()
}

#[tokio::test]
async fn a_folder_lists_its_children_and_mounts_by_name_then_segment() {
    let host = world();
    let listing = listing(&host, 50, None).await;
    assert_eq!(
        segments(&listing.entries),
        ["a", "z", "b", "card:Delta", "m1"]
    );
    assert_eq!(listing.next, None);
    let gamma = &listing.entries[4];
    assert!(gamma.mount);
    let expected_real_path = path("/commission/c1");
    let expected_den_path = [segment("stuff"), segment("m1")];
    assert_eq!(real_path(gamma), Some(&expected_real_path));
    assert_eq!(den_path(gamma), Some(&expected_den_path[..]));
    assert_eq!(own_level(gamma), Some(Level::Private));
    assert_eq!(kind(&listing.entries[2]), Some(Kind::File));
}

#[tokio::test]
async fn a_private_entry_is_left_out_and_a_listed_one_is_a_card() {
    let host = world();
    let listing = listing(&host, 50, None).await;
    assert!(!segments(&listing.entries).contains(&"m2".to_owned()));
    let delta = listing
        .entries
        .iter()
        .find(|entry| entry.name.as_ref() == "Delta")
        .expect("the card is listed");
    assert_eq!(delta.view, View::Card);
}

#[tokio::test]
async fn admit_sees_each_visible_entry_for_a_listing_with_what_gave_it() {
    let host = world();
    listing(&host, 50, None).await;
    let listed: Vec<_> = host
        .admits()
        .into_iter()
        .filter(|(_, _, reach)| *reach == Reach::Listing)
        .collect();
    assert_eq!(listed.len(), 5);
    let card = listed
        .iter()
        .find(|(path, _, _)| path.to_string() == "/commission/card")
        .expect("the card was asked about");
    assert_eq!(card.1, expected_access(Level::Listed, Via::Grant));
    let child = listed
        .iter()
        .find(|(path, _, _)| path.to_string() == "/user/alice/stuff/a")
        .expect("a child was asked about");
    assert_eq!(child.1, expected_access(Level::Public, Via::Standing));
}

#[tokio::test]
async fn refused_entries_are_left_out_before_the_page_is_cut() {
    let host = world().refuse("/user/alice/stuff/a", Reach::Listing);
    let first = listing(&host, 2, None).await;
    assert_eq!(segments(&first.entries), ["z", "b"]);
    let next = first.next.expect("more kept entries follow");
    assert_eq!(next.offset(), 2);
    let second = listing(&host, 2, Some(next)).await;
    assert_eq!(segments(&second.entries), ["card:Delta", "m1"]);
    assert_eq!(second.next, None);
}

#[tokio::test]
async fn no_token_follows_a_refused_tail() {
    let host = world()
        .refuse("/commission/c1", Reach::Listing)
        .refuse("/commission/card", Reach::Listing);
    let listing = listing(&host, 3, None).await;
    assert_eq!(segments(&listing.entries), ["a", "z", "b"]);
    assert_eq!(listing.next, None);
}

#[tokio::test]
async fn a_token_past_the_end_gives_an_empty_last_page() {
    let host = world();
    let listing = listing(&host, 2, Some(PageToken::at(40))).await;
    assert!(listing.entries.is_empty());
    assert_eq!(listing.next, None);
}

#[tokio::test]
async fn a_page_size_of_zero_still_serves_one_entry() {
    let host = world();
    let listing = listing(&host, 0, None).await;
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.next.map(|token| token.offset()), Some(1));
}

#[tokio::test]
async fn a_segment_naming_a_real_child_and_a_mount_drops_both() {
    let host = world().mount("/user/alice/stuff", "a", "/commission/c1");
    let listing = listing(&host, 50, None).await;
    assert_eq!(segments(&listing.entries), ["z", "b", "card:Delta", "m1"]);
}

#[tokio::test]
async fn a_mount_whose_target_is_not_directly_under_a_top_level_directory_is_left_out() {
    let host = world()
        .node("/commission/c1/attachments", "folder", Level::Private, &[])
        .mount("/user/alice/stuff", "deep", "/commission/c1/attachments");
    let listing = listing(&host, 50, None).await;
    assert!(!segments(&listing.entries).contains(&"deep".to_owned()));
}

#[tokio::test]
async fn a_symlink_is_absent_from_a_listing() {
    let host = world().linked("/user/alice/stuff/b", "/commission/c1");
    let listing = listing(&host, 50, None).await;
    assert_eq!(segments(&listing.entries), ["a", "z", "card:Delta", "m1"]);
}

#[tokio::test]
async fn an_open_entry_sorts_before_a_card_of_the_same_name() {
    let host = world().named("/commission/card", "Alpha");
    let listing = listing(&host, 50, None).await;
    assert_eq!(
        segments(&listing.entries),
        ["a", "z", "card:Alpha", "b", "m1"]
    );
}

#[tokio::test]
async fn only_an_open_directory_lists() {
    let file_host = world();
    let card_host = world();
    let file = list(&file_host, &["stuff", "b"], 50, None)
        .await
        .expect("the file resolves");
    let card = list(&card_host, &["stuff", "m3"], 50, None)
        .await
        .expect("the card resolves");
    assert_eq!(file, None);
    assert_eq!(card, None);
    let asked_for_children = |host: &FakeHost| {
        host.calls()
            .iter()
            .any(|call| matches!(call, Call::Children { .. }))
    };
    assert!(!asked_for_children(&file_host));
    assert!(!asked_for_children(&card_host));
}

#[tokio::test]
async fn cards_of_the_same_name_follow_the_keyed_hash_of_their_real_path_never_their_key() {
    let keys: Vec<String> = (0..8).map(|index| format!("k{index}")).collect();
    let host = keys.iter().fold(
        world().node("/user/alice/cards", "folder", Level::Private, &[]),
        |host, key| {
            let target = format!("/commission/{key}");
            host.mount("/user/alice/cards", key, &target)
                .node(
                    &target,
                    &format!("card-{key}"),
                    Level::Private,
                    &[GRANT_CARD],
                )
                .named(&target, "Same")
        },
    );
    let listing = list(&host, &["cards"], 50, None)
        .await
        .expect("the folder resolves")
        .expect("the folder lists");
    let listed_types: Vec<String> = listing
        .entries
        .iter()
        .map(|entry| entry.typ.to_string())
        .collect();
    let mut by_keyed_hash = keys.clone();
    by_keyed_hash.sort_by_key(|key| card_order(&path(&format!("/commission/{key}"))));
    let expected_types: Vec<String> = by_keyed_hash
        .iter()
        .map(|key| format!("card-{key}"))
        .collect();
    assert_eq!(listed_types, expected_types);
}

#[tokio::test]
async fn a_node_resolved_for_another_viewer_is_not_listed() {
    let host = world();
    let alice = resolver(&host, 50);
    let bob = Resolver {
        host: &host,
        viewer: Viewer::from("bob".to_owned()),
        caps: Caps::default(),
    };
    let resolved = alice
        .resolve_path(&path("/user/alice"), &[segment("stuff")])
        .await
        .expect("the folder resolves for alice");
    let listed_for_bob = bob.list(&resolved, None).await;
    assert!(
        matches!(listed_for_bob, Err(Error::NotFound)),
        "{listed_for_bob:?}"
    );
}

#[tokio::test]
async fn names_list_by_number_value_and_without_regard_to_case() {
    let names = [
        ("n1", "file10"),
        ("n2", "File2"),
        ("n3", "file2"),
        ("n4", "file02"),
        ("n5", "apple"),
    ];
    let host = names.iter().fold(
        world().node("/user/alice/files", "folder", Level::Private, &[]),
        |host, (key, name)| {
            let at = format!("/user/alice/files/{key}");
            host.node(&at, "file", Level::Private, &[]).named(&at, name)
        },
    );
    let listing = list(&host, &["files"], 50, None)
        .await
        .expect("the folder resolves")
        .expect("the folder lists");
    let listed_names: Vec<String> = listing
        .entries
        .iter()
        .map(|entry| entry.name.to_string())
        .collect();
    let expected_names = ["apple", "File2", "file02", "file2", "file10"];
    assert_eq!(listed_names, expected_names);
}

/// A number the tests expect.
fn ordinal(number: u32) -> Option<NameOrdinal> {
    let ordinal = NameOrdinal::try_from(number).expect("a valid test ordinal");
    Some(ordinal)
}

/// alice's folder `twins`, holding a real child at each key with its name.
fn twins(children: &[(&str, &str)]) -> FakeHost {
    let folder = world().node("/user/alice/twins", "folder", Level::Private, &[]);
    children.iter().fold(folder, |host, (key, name)| {
        let at = format!("/user/alice/twins/{key}");
        host.node(&at, "file", Level::Private, &[]).named(&at, name)
    })
}

/// One page of alice's folder `twins`.
async fn twins_page(host: &FakeHost, page_size: usize, page: Option<PageToken>) -> Listing {
    list(host, &["twins"], page_size, page)
        .await
        .expect("the folder resolves")
        .expect("the folder lists")
}

/// Each entry's last Den path segment (or card name) with its number.
fn numbered(entries: &[Entry]) -> Vec<(String, Option<NameOrdinal>)> {
    let shown = segments(entries);
    let numbers = entries.iter().map(|entry| entry.ordinal);
    shown.into_iter().zip(numbers).collect()
}

/// `(segment, number)` pairs from test text.
fn expected_numbers(pairs: &[(&str, Option<NameOrdinal>)]) -> Vec<(String, Option<NameOrdinal>)> {
    pairs
        .iter()
        .map(|(segment, number)| ((*segment).to_owned(), *number))
        .collect()
}

#[tokio::test]
async fn a_folder_with_no_equal_names_carries_no_number() {
    let host = twins(&[("a", "Alpha"), ("b", "Beta"), ("c", "Gamma")]);

    let listing = twins_page(&host, 50, None).await;

    let numbers: Vec<_> = listing.entries.iter().map(|entry| entry.ordinal).collect();
    assert_eq!(numbers, [None, None, None]);
}

#[tokio::test]
async fn a_twin_the_viewer_cannot_see_gives_no_number() {
    let host = twins(&[("a", "Test 1"), ("z", "Test 1")])
        .refuse("/user/alice/twins/a", Reach::Listing)
        .mount("/user/alice/twins", "m", "/commission/stranger")
        .named("/commission/stranger", "Test 1");

    let listing = twins_page(&host, 50, None).await;

    let expected = expected_numbers(&[("z", None)]);
    assert_eq!(numbered(&listing.entries), expected);
}

#[tokio::test]
async fn a_hidden_entry_named_like_a_number_does_not_shift_it() {
    let host = twins(&[("a", "Test 1"), ("b", "Test 1"), ("h", "Test 1 (2)")])
        .refuse("/user/alice/twins/h", Reach::Listing);

    let listing = twins_page(&host, 50, None).await;

    let expected = expected_numbers(&[("a", None), ("b", ordinal(2))]);
    assert_eq!(numbered(&listing.entries), expected);
}

#[tokio::test]
async fn twins_split_across_a_page_keep_their_numbers() {
    let host = twins(&[("a", "Test 1"), ("b", "Test 1"), ("c", "Test 1")]);

    let first = twins_page(&host, 2, None).await;
    let second = twins_page(&host, 2, first.next).await;

    let expected_first = expected_numbers(&[("a", None), ("b", ordinal(2))]);
    let expected_second = expected_numbers(&[("c", ordinal(3))]);
    assert_eq!(numbered(&first.entries), expected_first);
    assert_eq!(numbered(&second.entries), expected_second);
}

#[tokio::test]
async fn a_number_skips_a_name_another_entry_already_shows() {
    let host = twins(&[("a", "Test 1"), ("b", "Test 1"), ("c", "Test 1 (2)")]);

    let listing = twins_page(&host, 50, None).await;

    let expected = expected_numbers(&[("a", None), ("b", ordinal(3)), ("c", None)]);
    assert_eq!(numbered(&listing.entries), expected);
}

#[tokio::test]
async fn the_newer_commission_gets_the_number() {
    let older = "01930000-0000-7000-8000-000000000001";
    let newer = "01940000-0000-7000-8000-000000000002";
    let host = [newer, older].into_iter().fold(
        world().node("/user/alice/commissions", "folder", Level::Private, &[]),
        |host, id| {
            let target = format!("/commission/{id}");
            host.mount("/user/alice/commissions", id, &target)
                .node(&target, "commission", Level::Private, &[STANDING])
                .named(&target, "Test 1")
        },
    );

    let listing = list(&host, &["commissions"], 50, None)
        .await
        .expect("the folder resolves")
        .expect("the folder lists");

    let expected = expected_numbers(&[(older, None), (newer, ordinal(2))]);
    assert_eq!(numbered(&listing.entries), expected);
}

#[tokio::test]
async fn names_differing_only_in_case_get_no_number() {
    let host = twins(&[("a", "Test 1"), ("b", "test 1"), ("c", "TEST 1")]);

    let listing = twins_page(&host, 50, None).await;

    let numbers: Vec<_> = listing.entries.iter().map(|entry| entry.ordinal).collect();
    assert_eq!(numbers, [None, None, None]);
}

#[tokio::test]
async fn a_card_twin_is_numbered_after_the_open_one() {
    let host = twins(&[("a", "Test 1")])
        .mount("/user/alice/twins", "m", "/commission/card")
        .named("/commission/card", "Test 1");

    let listing = twins_page(&host, 50, None).await;

    let expected = expected_numbers(&[("a", None), ("card:Test 1", ordinal(2))]);
    assert_eq!(numbered(&listing.entries), expected);
}
