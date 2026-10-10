use domain::elements::account::ListingScope;
use domain::elements::role::Role;

use super::*;

async fn seeded() -> MemBackend {
    let backend = MemBackend::new();
    seed_identity(&backend).await.unwrap();
    backend
}

fn names(characters: &[Character]) -> Vec<String> {
    characters
        .iter()
        .map(|character| character.attributes.name.to_string())
        .collect()
}

#[tokio::test]
async fn alice_keeps_ember_and_kael_sona() {
    let backend = seeded().await;

    let kept = backend
        .character_store()
        .list_kept_by(&alice())
        .await
        .unwrap();

    let expected_names = ["Ember", "Kael-sona"];
    assert_eq!(names(&kept), expected_names);
}

#[tokio::test]
async fn bob_keeps_abco() {
    let backend = seeded().await;

    let kept = backend
        .character_store()
        .list_kept_by(&bob())
        .await
        .unwrap();

    let expected_names = ["Abco"];
    assert_eq!(names(&kept), expected_names);
}

#[tokio::test]
async fn every_fixture_character_is_private() {
    let backend = seeded().await;
    let characters = backend.character_store();

    let mut kept = characters.list_kept_by(&alice()).await.unwrap();
    kept.extend(characters.list_kept_by(&bob()).await.unwrap());

    assert_eq!(kept.len(), 3);
    assert!(
        kept.iter()
            .all(|character| character.presence == Presence::Private)
    );
}

#[tokio::test]
async fn alice_owns_supreme_arts_and_bob_belongs_to_no_account() {
    let backend = seeded().await;
    let accounts = backend.account_store();

    let alices = accounts
        .list_for_user(&alice(), ListingScope::SelfView)
        .await
        .unwrap();
    let bobs = accounts
        .list_for_user(&bob(), ListingScope::SelfView)
        .await
        .unwrap();

    let [membership] = alices.as_slice() else {
        panic!("alice holds exactly one membership, got {}", alices.len());
    };
    assert_eq!(membership.account.id, supreme_arts());
    assert_eq!(membership.account.name.as_str(), "Supreme Arts");
    assert_eq!(membership.role, Role::Owner);
    assert!(bobs.is_empty());
}

#[tokio::test]
async fn the_fixture_holds_no_deactivated_account() {
    let backend = seeded().await;

    let deactivated = backend
        .account_store()
        .list_deactivated_for_owner(&alice())
        .await
        .unwrap();

    assert!(deactivated.is_empty());
}

#[tokio::test]
async fn both_users_are_recognized() {
    let backend = seeded().await;
    let users = backend.user_store();

    assert!(users.find(&alice()).await.unwrap().is_some());
    assert!(users.find(&bob()).await.unwrap().is_some());
}

#[tokio::test]
async fn alice_is_named_by_her_display_name_and_bob_by_his_handle() {
    let backend = seeded().await;
    let cache = backend.profile_cache();

    let alice_profile = cache
        .get(alice().did())
        .await
        .unwrap()
        .expect("alice's profile");
    let bob_profile = cache
        .get(bob().did())
        .await
        .unwrap()
        .expect("bob's profile");

    assert_eq!(alice_profile.name_candidates().next(), Some("Alice"));
    assert_eq!(bob_profile.name_candidates().next(), Some("bob.test"));
}

#[tokio::test]
async fn the_fixture_actors_have_distinct_dids() {
    let dids = [
        alice().to_string(),
        bob().to_string(),
        supreme_arts().to_string(),
        abco().to_string(),
        ember().to_string(),
        kael_sona().to_string(),
    ];

    let distinct: std::collections::HashSet<&String> = dids.iter().collect();

    assert_eq!(distinct.len(), dids.len());
}

#[tokio::test]
async fn every_fixture_did_parses_as_a_did() {
    let dids = [
        ALICE_DID,
        BOB_DID,
        SUPREME_ARTS_DID,
        ABCO_DID,
        EMBER_DID,
        KAEL_SONA_DID,
    ];

    for did in dids {
        assert!(did.parse::<Did>().is_ok(), "{did} is not a DID");
    }
}

#[tokio::test]
async fn seeding_twice_fails() {
    let backend = seeded().await;

    let second = seed_identity(&backend).await;

    assert!(second.is_err());
}

#[tokio::test]
async fn a_world_seeded_twice_holds_the_same_timestamps() {
    let first = seeded().await;
    let second = seeded().await;

    let first_kept = first
        .character_store()
        .list_kept_by(&alice())
        .await
        .unwrap();
    let second_kept = second
        .character_store()
        .list_kept_by(&alice())
        .await
        .unwrap();
    let first_account = first
        .find(&supreme_arts())
        .await
        .unwrap()
        .expect("Supreme Arts");
    let second_account = second
        .find(&supreme_arts())
        .await
        .unwrap()
        .expect("Supreme Arts");

    assert_eq!(first_kept, second_kept);
    assert_eq!(first_account.created_at, second_account.created_at);
}
