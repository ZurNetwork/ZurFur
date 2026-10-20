//! The den Query over the in-memory runtime: every malformed path is the one not-found, and
//! the identity and commission arms serve a viewer's own root, their Accounts, their
//! Characters and their commissions on the fixture world, and nothing of anyone else's.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::sync::Arc;

use adapter_mem::{
    MemBackend, MemProfileSource,
    fixture::{
        abco, alice, bob, ember, kael_sona, old_sketch, seed_commission, seed_identity,
        supreme_arts, untitled, untitled_slots,
    },
};
use application::den::{
    DenAccess, DenContent, DenError, DenListing, DenNode, NodeKind, NodeLevel, SoftDelete,
    node::{self, NodeQuery},
    root::{self, RootQuery},
};
use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        handle::Handle,
        profile::Profile,
        role::Role,
        user::UserId,
    },
    ports::{ProfileCache, UnitOfWork},
};
use test_support::recording::{CallLog, RecordingProfileCache, RecordingProfileSource, SubjectLog};

/// A deactivated Account owned by alice, with bob a Member.
const CLOSED_STUDIO_DID: &str = "did:plc:c1osedstudiofixtureaaaaa";

/// A runtime over an empty in-memory world, and a viewer with no user row.
fn bare() -> (composition::Runtime, UserId) {
    let did = Did::from("did:plc:alice-den".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    (runtime, UserId::from(did))
}

/// Reads the node at `path` as a viewer with no user row.
async fn read_bare(path: Vec<String>) -> Result<node::Output, DenError> {
    let (runtime, viewer) = bare();
    let query = NodeQuery {
        viewer,
        path,
        include_deleted: false,
        page: None,
    };
    runtime.app().den().node(query).await
}

/// The fixture world: identity's actors and commission's rows, seeded into one backend.
async fn world() -> (composition::Runtime, MemBackend) {
    let mem = test_support::runtime::mem(alice().did()).build();
    seed_identity(&mem.backend)
        .await
        .expect("seeds identity's fixtures");
    seed_commission(&mem.backend, &alice(), &bob(), &supreme_arts())
        .await
        .expect("seeds commission's fixtures");
    (mem.runtime, mem.backend)
}

/// The deactivated Account "Closed Studio".
fn closed_studio() -> AccountId {
    AccountId::from(Did::from(CLOSED_STUDIO_DID.to_owned()))
}

/// Seeds "Closed Studio", alice its Owner and bob a Member, and deactivates it.
async fn seed_closed_studio(runtime: &composition::Runtime) {
    let handle = "closed-studio.zurfur.app"
        .parse::<Handle>()
        .expect("a valid handle");
    let name = "Closed Studio"
        .parse::<AccountName>()
        .expect("a valid name");
    let (account, ownership) = Account::open(
        alice(),
        closed_studio().did().clone(),
        handle,
        name,
        Utc::now(),
    );
    let target = closed_studio();
    application::transaction(&*runtime.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create(&account, &ownership).await?;
        test_support::membership::seat_member(
            uow,
            &target,
            &bob(),
            Role::Member,
            &alice(),
            Utc::now(),
        )
        .await?;
        uow.accounts().soft_delete(&target).await
    })
    .await
    .expect("seeds a deactivated Account");
}

/// `viewer`'s own root.
async fn root(
    runtime: &composition::Runtime,
    viewer: UserId,
    include_deleted: bool,
) -> Result<root::Output, DenError> {
    let query = RootQuery {
        viewer,
        include_deleted,
        page: None,
    };
    runtime.app().den().root(query).await
}

/// The node at `path` below `viewer`'s root.
async fn read(
    runtime: &composition::Runtime,
    viewer: UserId,
    path: &[&str],
    include_deleted: bool,
) -> Result<node::Output, DenError> {
    let query = NodeQuery {
        viewer,
        path: path.iter().map(|piece| (*piece).to_owned()).collect(),
        include_deleted,
        page: None,
    };
    runtime.app().den().node(query).await
}

/// The listing a node read answered with.
fn listing(output: node::Output) -> DenListing {
    let Some(DenContent::Listing(listing)) = output.content else {
        panic!("expected a listing, got {:?}", output.content);
    };
    listing
}

/// The names of a listing's entries, in order.
fn names(listing: &DenListing) -> Vec<String> {
    listing
        .entries
        .iter()
        .map(|entry| entry.name.to_string())
        .collect()
}

/// A node's Den path, as text.
fn path_of(node: &DenNode) -> Vec<String> {
    node.segments.iter().map(ToString::to_string).collect()
}

/// What an open node shows: its kind, its own level, whether its content is shown, and its
/// soft-deleted word.
fn open(node: &DenNode) -> (NodeKind, NodeLevel, bool, Option<SoftDelete>) {
    let DenAccess::Open {
        kind,
        own_level,
        content_not_shown,
        soft_delete,
    } = node.access
    else {
        panic!("expected {} to be open", node.name);
    };
    (kind, own_level, content_not_shown, soft_delete)
}

/// An open, live, Private folder whose content is shown.
const OPEN_PRIVATE_FOLDER: (NodeKind, NodeLevel, bool, Option<SoftDelete>) =
    (NodeKind::Directory, NodeLevel::Private, false, None);

#[tokio::test]
async fn an_empty_path_is_not_found() {
    let answer = read_bare(Vec::new()).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}

#[tokio::test]
async fn seventeen_pieces_are_not_found() {
    let path = vec!["accounts".to_owned(); 17];

    let answer = read_bare(path).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}

#[tokio::test]
async fn each_malformed_piece_is_not_found() {
    let too_long = "a".repeat(2049);
    let malformed = ["", ".", "..", "a/b", "a b", "\u{e9}", too_long.as_str()];

    for piece in malformed {
        let path = vec!["accounts".to_owned(), piece.to_owned()];

        let answer = read_bare(path).await;

        assert!(matches!(answer, Err(DenError::NotFound)), "{piece:?}");
    }
}

#[tokio::test]
async fn a_viewer_with_no_user_row_has_no_root() {
    let (runtime, viewer) = bare();

    let answer = root(&runtime, viewer, false).await;

    assert!(matches!(answer, Err(DenError::NotFound)));
}

#[tokio::test]
async fn alice_s_root_is_named_alice_and_holds_her_three_open_private_folders_and_posts() {
    let (runtime, _backend) = world().await;

    let output = root(&runtime, alice(), false).await.expect("alice's root");

    assert_eq!(output.node.name.to_string(), "Alice");
    assert_eq!(output.node.segments, Vec::new());
    assert_eq!(open(&output.node), OPEN_PRIVATE_FOLDER);
    assert_eq!(
        names(&output.listing),
        ["accounts", "characters", "commissions", "posts"]
    );
    let (folders, posts) = output.listing.entries.split_at(3);
    for entry in folders {
        assert_eq!(open(entry), OPEN_PRIVATE_FOLDER, "{}", entry.name);
        assert!(!entry.mount, "{}", entry.name);
    }
    assert!(posts[0].mount);
    assert_eq!(output.listing.next_page, None);
}

/// `posts` is a mount of `/post/<did>`: open, Public, its content not shown, and empty.
#[tokio::test]
async fn the_posts_mount_opens_public_and_empty_with_its_content_not_shown() {
    let (runtime, _backend) = world().await;

    let output = root(&runtime, alice(), false).await.expect("alice's root");
    let posts = read(&runtime, alice(), &["posts"], false)
        .await
        .expect("alice's posts");
    let below = read(&runtime, alice(), &["posts", "anything"], false).await;

    let public_not_shown = (NodeKind::Directory, NodeLevel::Public, true, None);
    let entry = output
        .listing
        .entries
        .iter()
        .find(|entry| entry.name.to_string() == "posts")
        .expect("the root lists posts");
    assert!(entry.mount);
    assert_eq!(path_of(entry), ["posts"]);
    assert_eq!(open(entry), public_not_shown);
    assert_eq!(posts.node.name.to_string(), "posts");
    assert!(posts.node.mount);
    assert_eq!(open(&posts.node), public_not_shown);
    assert_eq!(listing(posts), DenListing::default());
    assert!(matches!(below, Err(DenError::NotFound)));
}

#[tokio::test]
async fn bob_s_root_is_named_by_his_handle() {
    let (runtime, _backend) = world().await;

    let output = root(&runtime, bob(), false).await.expect("bob's root");

    assert_eq!(output.node.name.to_string(), "bob.test");
}

#[tokio::test]
async fn alice_s_accounts_mounts_supreme_arts_and_bob_s_mounts_nothing() {
    let (runtime, _backend) = world().await;
    let supreme_arts_did = supreme_arts().to_string();

    let alice_s = listing(
        read(&runtime, alice(), &["accounts"], false)
            .await
            .expect("alice's accounts"),
    );
    let bob_s = listing(
        read(&runtime, bob(), &["accounts"], false)
            .await
            .expect("bob's accounts"),
    );

    assert_eq!(names(&alice_s), ["Supreme Arts"]);
    let mount = &alice_s.entries[0];
    assert!(mount.mount);
    assert_eq!(path_of(mount), ["accounts", supreme_arts_did.as_str()]);
    assert_eq!(open(mount), OPEN_PRIVATE_FOLDER);
    assert_eq!(bob_s, DenListing::default());
}

#[tokio::test]
async fn supreme_arts_opens_to_its_characters_commissions_and_workflows() {
    let (runtime, _backend) = world().await;
    let supreme_arts_did = supreme_arts().to_string();
    let account = ["accounts", supreme_arts_did.as_str()];
    let folder = |word| [account[0], account[1], word];

    let folders = listing(
        read(&runtime, alice(), &account, false)
            .await
            .expect("Supreme Arts"),
    );
    let characters = read(&runtime, alice(), &folder("characters"), false)
        .await
        .expect("Supreme Arts' characters");
    let commissions = read(&runtime, alice(), &folder("commissions"), false)
        .await
        .expect("Supreme Arts' commissions");
    let workflows = read(&runtime, alice(), &folder("workflows"), false)
        .await
        .expect("Supreme Arts' workflows");

    let content_not_shown = (NodeKind::Directory, NodeLevel::Private, true, None);
    assert_eq!(names(&folders), ["characters", "commissions", "workflows"]);
    assert_eq!(open(&characters.node), OPEN_PRIVATE_FOLDER);
    assert_eq!(listing(characters), DenListing::default());
    assert_eq!(open(&commissions.node), content_not_shown);
    assert_eq!(listing(commissions), DenListing::default());
    assert_eq!(open(&workflows.node), OPEN_PRIVATE_FOLDER);
}

#[tokio::test]
async fn each_viewer_s_characters_mounts_the_characters_they_keep() {
    let (runtime, _backend) = world().await;

    let alice_s = listing(
        read(&runtime, alice(), &["characters"], false)
            .await
            .expect("alice's characters"),
    );
    let bob_s = listing(
        read(&runtime, bob(), &["characters"], false)
            .await
            .expect("bob's characters"),
    );

    let private_character = (NodeKind::Directory, NodeLevel::Private, true, None);
    assert_eq!(names(&alice_s), ["Ember", "Kael-sona"]);
    assert_eq!(names(&bob_s), ["Abco"]);
    for entry in alice_s.entries.iter().chain(&bob_s.entries) {
        assert!(entry.mount, "{}", entry.name);
        assert_eq!(open(entry), private_character, "{}", entry.name);
    }
}

#[tokio::test]
async fn a_kept_character_opens_by_its_typed_path_and_shows_no_content_yet() {
    let (runtime, _backend) = world().await;
    let ember_did = ember().to_string();
    let kael_sona_did = kael_sona().to_string();

    let ember = read(&runtime, alice(), &["characters", &ember_did], false)
        .await
        .expect("Ember");
    let kael_sona = read(&runtime, alice(), &["characters", &kael_sona_did], false)
        .await
        .expect("Kael-sona");

    let empty_listing = Some(DenContent::Listing(DenListing::default()));
    assert_eq!(ember.node.name.to_string(), "Ember");
    assert_eq!(kael_sona.node.name.to_string(), "Kael-sona");
    assert_eq!(ember.content, empty_listing);
}

#[tokio::test]
async fn another_s_character_and_a_missing_one_are_the_same_not_found() {
    let (runtime, _backend) = world().await;
    let abco_did = abco().to_string();

    let abco = read(&runtime, alice(), &["characters", &abco_did], false).await;
    let missing = read(&runtime, alice(), &["characters", "did:plc:nope"], false).await;

    assert!(matches!(abco, Err(DenError::NotFound)));
    assert!(matches!(missing, Err(DenError::NotFound)));
}

#[tokio::test]
async fn another_s_account_and_a_missing_one_are_the_same_not_found() {
    let (runtime, _backend) = world().await;
    let supreme_arts_did = supreme_arts().to_string();

    let supreme_arts = read(&runtime, bob(), &["accounts", &supreme_arts_did], false).await;
    let missing = read(&runtime, bob(), &["accounts", "did:plc:nope"], false).await;

    assert!(matches!(supreme_arts, Err(DenError::NotFound)));
    assert!(matches!(missing, Err(DenError::NotFound)));
}

#[tokio::test]
async fn a_deactivated_account_is_listed_to_its_owner_only_when_asked_for() {
    let (runtime, _backend) = world().await;
    seed_closed_studio(&runtime).await;

    let without = listing(
        read(&runtime, alice(), &["accounts"], false)
            .await
            .expect("alice's accounts"),
    );
    let with = listing(
        read(&runtime, alice(), &["accounts"], true)
            .await
            .expect("alice's accounts, deleted included"),
    );

    let deactivated = (
        NodeKind::Directory,
        NodeLevel::Private,
        false,
        Some(SoftDelete::Deactivated),
    );
    assert_eq!(names(&without), ["Supreme Arts"]);
    assert_eq!(names(&with), ["Closed Studio", "Supreme Arts"]);
    assert_eq!(open(&with.entries[0]), deactivated);
    assert_eq!(open(&with.entries[1]), OPEN_PRIVATE_FOLDER);
}

#[tokio::test]
async fn a_deactivated_account_s_typed_path_opens_to_its_owner_marked() {
    let (runtime, _backend) = world().await;
    seed_closed_studio(&runtime).await;
    let closed_did = closed_studio().to_string();

    let closed = read(&runtime, alice(), &["accounts", &closed_did], false)
        .await
        .expect("Closed Studio, by its owner");

    let deactivated = (
        NodeKind::Directory,
        NodeLevel::Private,
        false,
        Some(SoftDelete::Deactivated),
    );
    assert_eq!(closed.node.name.to_string(), "Closed Studio");
    assert_eq!(open(&closed.node), deactivated);
}

#[tokio::test]
async fn a_member_who_is_not_the_owner_never_sees_a_deactivated_account() {
    let (runtime, _backend) = world().await;
    seed_closed_studio(&runtime).await;
    let closed_did = closed_studio().to_string();

    let listed = listing(
        read(&runtime, bob(), &["accounts"], true)
            .await
            .expect("bob's accounts, deleted included"),
    );
    let typed = read(&runtime, bob(), &["accounts", &closed_did], true).await;
    let missing = read(&runtime, bob(), &["accounts", "did:plc:nope"], true).await;

    assert_eq!(listed, DenListing::default());
    assert!(matches!(typed, Err(DenError::NotFound)));
    assert!(matches!(missing, Err(DenError::NotFound)));
}

#[tokio::test]
async fn a_display_name_the_rule_refuses_falls_back_to_the_handle() {
    let (runtime, backend) = world().await;
    let refused = Profile::new(alice().did().clone(), "alice.test").with_display_name("\u{202E}");
    backend.put_profile(&refused).await.expect("caches");

    let output = root(&runtime, alice(), false).await.expect("alice's root");

    assert_eq!(output.node.name.to_string(), "alice.test");
}

#[tokio::test]
async fn a_handle_that_did_not_check_out_falls_back_to_the_did() {
    let (runtime, backend) = world().await;
    let unverified = Profile::new(alice().did().clone(), "handle.invalid");
    backend.put_profile(&unverified).await.expect("caches");

    let output = root(&runtime, alice(), false).await.expect("alice's root");

    assert_eq!(output.node.name.to_string(), alice().to_string());
}

#[tokio::test]
async fn a_cache_miss_with_the_pds_down_names_the_root_by_its_did() {
    let (mut runtime, backend) = world().await;
    runtime.profile_source = Arc::new(MemProfileSource::unreachable());
    let carol = Did::from("did:plc:carol-den".to_owned());
    backend.provision(&carol).await.expect("recognizes carol");

    let output = root(&runtime, UserId::from(carol.clone()), false)
        .await
        .expect("carol's root");

    assert_eq!(output.node.name.to_string(), carol.to_string());
}

/// A profile cache whose every call fails.
struct FailingProfileCache;

#[async_trait]
impl ProfileCache for FailingProfileCache {
    async fn get(&self, _did: &Did) -> anyhow::Result<Option<Profile>> {
        Err(anyhow::anyhow!("the profile cache is down (test)"))
    }

    async fn put(&self, _profile: &Profile) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("the profile cache is down (test)"))
    }
}

#[tokio::test]
async fn a_failing_cache_names_the_root_by_its_did_and_never_fails_the_read() {
    let (mut runtime, _backend) = world().await;
    runtime.profile_cache = Arc::new(FailingProfileCache);
    runtime.profile_source = Arc::new(MemProfileSource::unreachable());

    let output = root(&runtime, alice(), false).await.expect("alice's root");

    assert_eq!(output.node.name.to_string(), alice().to_string());
}

/// The Den paths of a listing's entries `viewer` can open.
fn open_paths(listing: &DenListing) -> Vec<Vec<String>> {
    listing
        .entries
        .iter()
        .filter(|entry| matches!(entry.access, DenAccess::Open { .. }))
        .map(path_of)
        .filter(|path| !path.is_empty())
        .collect()
}

/// Every answer for every node `viewer` can reach from their root, deleted ones included,
/// as text.
async fn walk(runtime: &composition::Runtime, viewer: &UserId) -> Vec<String> {
    let root = root(runtime, viewer.clone(), true)
        .await
        .expect("the viewer's root");
    let mut pending = open_paths(&root.listing);
    let mut answers = vec![format!("{root:?}")];
    while let Some(path) = pending.pop() {
        let query = NodeQuery {
            viewer: viewer.clone(),
            path,
            include_deleted: true,
            page: None,
        };
        let output = runtime
            .app()
            .den()
            .node(query)
            .await
            .expect("an entry the viewer was shown opens");
        if let Some(DenContent::Listing(listing)) = &output.content {
            pending.extend(open_paths(listing));
        }
        answers.push(format!("{output:?}"));
    }
    answers
}

#[tokio::test]
async fn no_answer_alice_can_reach_names_bob() {
    let (runtime, _backend) = world().await;
    seed_closed_studio(&runtime).await;
    let bob_s = [bob().to_string(), "bob.test".to_owned()];

    let answers = walk(&runtime, &alice()).await;

    assert!(
        answers.len() > 8,
        "the walk reached alice's nodes: {answers:#?}"
    );
    for inside_a_commission in ["Old sketch", "sketch-01.png", "Open slot", "changelog"] {
        assert!(
            answers
                .iter()
                .any(|answer| answer.contains(inside_a_commission)),
            "the walk reached alice's commissions: {inside_a_commission}"
        );
    }
    for answer in &answers {
        for word in &bob_s {
            assert!(!answer.contains(word.as_str()), "{word} in {answer}");
        }
    }
}

#[tokio::test]
async fn the_den_reads_only_the_viewer_s_cached_profile_and_never_the_profile_source() {
    let (mut runtime, _backend) = world().await;
    seed_closed_studio(&runtime).await;
    let cache_log = CallLog::default();
    let cache_subjects = SubjectLog::default();
    let source_log = CallLog::default();
    runtime.profile_cache = Arc::new(RecordingProfileCache::new(
        runtime.profile_cache.clone(),
        cache_log.clone(),
        cache_subjects.clone(),
    ));
    runtime.profile_source = Arc::new(RecordingProfileSource::new(
        runtime.profile_source.clone(),
        source_log.clone(),
        SubjectLog::default(),
    ));

    walk(&runtime, &alice()).await;

    let alice_did = alice().to_string();
    let subjects = cache_subjects.subjects();
    assert!(!subjects.is_empty(), "the root is named from the cache");
    assert!(subjects.iter().all(|did| *did == alice_did), "{subjects:?}");
    assert!(
        cache_log
            .events()
            .iter()
            .all(|event| *event == "get_profile"),
        "{:?}",
        cache_log.events()
    );
    assert_eq!(source_log.events(), Vec::<&str>::new());
}

/// An open, live, Private folder whose content isn't shown yet.
const NOT_SHOWN_PRIVATE_FOLDER: (NodeKind, NodeLevel, bool, Option<SoftDelete>) =
    (NodeKind::Directory, NodeLevel::Private, true, None);

#[tokio::test]
async fn untitled_opens_from_alice_s_commissions_to_its_four_entries() {
    let (runtime, _backend) = world().await;
    let untitled_id = untitled().to_string();

    let output = read(&runtime, alice(), &["commissions", &untitled_id], false)
        .await
        .expect("Untitled");

    assert_eq!(output.node.name.to_string(), "Untitled");
    assert!(output.node.mount);
    assert_eq!(open(&output.node), OPEN_PRIVATE_FOLDER);
    assert_eq!(
        names(&listing(output)),
        ["attachments", "changelog", "products", "slots"]
    );
}

#[tokio::test]
async fn untitled_s_attachments_list_its_two_files() {
    let (runtime, _backend) = world().await;
    let untitled_id = untitled().to_string();

    let output = read(
        &runtime,
        alice(),
        &["commissions", &untitled_id, "attachments"],
        false,
    )
    .await
    .expect("Untitled's attachments");

    assert_eq!(names(&listing(output)), ["ref-abco.png", "sketch-01.png"]);
}

#[tokio::test]
async fn a_slot_of_untitled_opens_flagged_and_empty() {
    let (runtime, _backend) = world().await;
    let untitled_id = untitled().to_string();
    let [first_slot, ..] = untitled_slots();
    let slot_key = first_slot.to_string();

    let output = read(
        &runtime,
        alice(),
        &["commissions", &untitled_id, "slots", &slot_key],
        false,
    )
    .await
    .expect("a slot of Untitled");

    assert_eq!(open(&output.node), NOT_SHOWN_PRIVATE_FOLDER);
    assert_eq!(listing(output), DenListing::default());
}

#[tokio::test]
async fn old_sketch_is_listed_in_alice_s_commissions_only_when_asked_for_marked_archived() {
    let (runtime, _backend) = world().await;

    let hidden = listing(
        read(&runtime, alice(), &["commissions"], false)
            .await
            .expect("alice's commissions"),
    );
    let shown = listing(
        read(&runtime, alice(), &["commissions"], true)
            .await
            .expect("alice's commissions, deleted ones included"),
    );

    assert_eq!(names(&hidden), ["Untitled"]);
    assert_eq!(names(&shown), ["Old sketch", "Untitled"]);
    let archived = (
        NodeKind::Directory,
        NodeLevel::Private,
        false,
        Some(SoftDelete::Archived),
    );
    assert_eq!(open(&shown.entries[0]), archived);
    assert!(shown.entries.iter().all(|entry| entry.mount));
}

#[tokio::test]
async fn old_sketch_s_typed_path_opens_marked_archived_without_asking_for_deleted_ones() {
    let (runtime, _backend) = world().await;
    let old_sketch_id = old_sketch().to_string();

    let output = read(&runtime, alice(), &["commissions", &old_sketch_id], false)
        .await
        .expect("Old sketch by its typed path");

    assert_eq!(output.node.name.to_string(), "Old sketch");
    let archived = (
        NodeKind::Directory,
        NodeLevel::Private,
        false,
        Some(SoftDelete::Archived),
    );
    assert_eq!(open(&output.node), archived);
}

#[tokio::test]
async fn bob_s_commissions_list_only_untitled() {
    let (runtime, _backend) = world().await;

    let bob_s = listing(
        read(&runtime, bob(), &["commissions"], true)
            .await
            .expect("bob's commissions"),
    );

    assert_eq!(names(&bob_s), ["Untitled"]);
}
