//! `MemIdentityResolver` conformance + mem-specific seeding behaviour.
//!
//! The shared suite ([`test_support::identity_resolver_contract`]) is the same
//! body the real atproto resolver runs; the extra tests pin how the fake's
//! seeding helpers stage the world.

use adapter_mem::MemIdentityResolver;
use domain::elements::did::Did;
use domain::elements::handle::AtHandle;
use domain::ports::{IdentityResolver, ResolveError};
use test_support::identity_resolver_contract::{StagedIdentities, identity_resolver_contract};

fn handle(raw: &str) -> AtHandle {
    raw.parse().expect("a valid handle")
}

fn did(raw: &str) -> Did {
    raw.parse().expect("a valid DID")
}

/// Stage every case the shared suite needs on a fresh fake.
fn staged_world() -> (MemIdentityResolver, StagedIdentities) {
    let resolver = MemIdentityResolver::new();
    let staged = StagedIdentities {
        confirmed_handle: handle("alice.example.com"),
        confirmed_did: did("did:plc:aliceaaaaaaaaaaaaaaaaaaa"),
        unconfirmed_handle: handle("mallory.example.com"),
        handle_whose_did_claims_nothing: handle("nameless.example.com"),
        disowned_did: did("did:plc:disownedaaaaaaaaaaaaaaaa"),
        handleless_did: did("did:plc:handlelessaaaaaaaaaaaaa"),
        did_claiming_unknown_handle: did("did:plc:claimsnobodyaaaaaaaaaaaa"),
        unknown_handle: handle("nobody.example.com"),
        unknown_did: did("did:plc:unknownaaaaaaaaaaaaaaaaa"),
        unavailable_handle: handle("down.example.com"),
        refused_handle: handle("internal.example.com"),
        unavailable_did: did("did:web:down.example.org"),
        refused_did: did("did:web:internal.example.org"),
        handle_naming_unavailable_document: handle("slowdoc.example.com"),
        handle_naming_refused_document: handle("baddoc.example.com"),
        handle_naming_missing_document: handle("nodoc.example.com"),
        did_claiming_unavailable_handle: did("did:plc:claimsdownaaaaaaaaaaaaaa"),
        did_claiming_refused_handle: did("did:plc:claimsrefusedaaaaaaaaaaa"),
    };

    resolver.seed_pair(&staged.confirmed_handle, &staged.confirmed_did);

    // mallory.example.com names alice's DID, which claims alice, not mallory.
    resolver.seed_handle_claim(&staged.unconfirmed_handle, &staged.confirmed_did);

    // The disowned DID claims alice's handle, which names alice's DID instead.
    resolver.seed_document(&staged.disowned_did, Some(&staged.confirmed_handle));
    resolver.seed_document(&staged.handleless_did, None);

    // nameless.example.com names the DID whose document claims no handle.
    resolver.seed_handle_record(
        &staged.handle_whose_did_claims_nothing,
        &staged.handleless_did,
    );

    resolver.make_handle_unavailable(&staged.unavailable_handle);
    resolver.refuse_handle(&staged.refused_handle);
    resolver.make_did_unavailable(&staged.unavailable_did);
    resolver.refuse_did(&staged.refused_did);

    // Handles naming a DID whose document fails at the second step: it cannot
    // be fetched, its host breaks a fetch rule, or the DID does not resolve.
    resolver.seed_handle_record(
        &staged.handle_naming_unavailable_document,
        &staged.unavailable_did,
    );
    resolver.seed_handle_record(&staged.handle_naming_refused_document, &staged.refused_did);
    resolver.seed_handle_record(&staged.handle_naming_missing_document, &staged.unknown_did);

    // This DID claims the handle that names nothing.
    resolver.seed_document(
        &staged.did_claiming_unknown_handle,
        Some(&staged.unknown_handle),
    );

    resolver.seed_document(
        &staged.did_claiming_unavailable_handle,
        Some(&staged.unavailable_handle),
    );
    resolver.seed_document(
        &staged.did_claiming_refused_handle,
        Some(&staged.refused_handle),
    );

    (resolver, staged)
}

#[tokio::test]
async fn mem_satisfies_identity_resolver_contract() {
    let (resolver, staged) = staged_world();
    identity_resolver_contract(&resolver, &staged).await;
}

#[tokio::test]
async fn an_empty_fake_finds_nothing() {
    let resolver = MemIdentityResolver::new();
    let by_handle = resolver.resolve_handle(&handle("alice.example.com")).await;
    assert!(matches!(by_handle, Err(ResolveError::NotFound)));
    let by_did = resolver
        .resolve_did(&did("did:plc:aliceaaaaaaaaaaaaaaaaaaa"))
        .await;
    assert!(matches!(by_did, Err(ResolveError::NotFound)));
}

#[tokio::test]
async fn a_one_way_claim_to_an_unstaged_did_is_not_confirmed() {
    let resolver = MemIdentityResolver::new();
    let one_way = handle("bob.example.com");
    resolver.seed_handle_claim(&one_way, &did("did:plc:bobaaaaaaaaaaaaaaaaaaaaa"));
    let result = resolver.resolve_handle(&one_way).await;
    assert!(
        matches!(result, Err(ResolveError::NotConfirmed)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_one_way_claim_keeps_an_already_staged_document() {
    let (resolver, staged) = staged_world();
    // The confirmed DID already claims alice; a second handle naming it must not
    // wipe that claim.
    resolver.seed_handle_claim(&handle("alias.example.com"), &staged.confirmed_did);
    let claimed = resolver.resolve_did(&staged.confirmed_did).await;
    let expected_claim = Some(staged.confirmed_handle.clone());
    assert_eq!(claimed.expect("still resolves"), expected_claim);
}

#[tokio::test]
async fn a_handle_record_alone_leaves_the_did_unresolved() {
    let resolver = MemIdentityResolver::new();
    let erin = handle("erin.example.com");
    resolver.seed_handle_record(&erin, &did("did:plc:erinaaaaaaaaaaaaaaaaaaaa"));
    let result = resolver.resolve_handle(&erin).await;
    assert!(
        matches!(result, Err(ResolveError::NotFound)),
        "got {result:?}"
    );
}

#[tokio::test]
async fn a_handle_naming_a_refused_document_is_refused() {
    let resolver = MemIdentityResolver::new();
    let carol = handle("carol.example.com");
    let carol_did = did("did:web:carol.example.com");
    resolver.seed_pair(&carol, &carol_did);
    resolver.refuse_did(&carol_did);
    let result = resolver.resolve_handle(&carol).await;
    assert!(
        matches!(result, Err(ResolveError::Refused(_))),
        "got {result:?}"
    );
}

#[tokio::test]
async fn an_unreachable_fake_is_unavailable_for_everything() {
    let (resolver, staged) = staged_world();
    resolver.make_unreachable();
    let by_handle = resolver.resolve_handle(&staged.confirmed_handle).await;
    assert!(matches!(by_handle, Err(ResolveError::Unavailable(_))));
    let by_did = resolver.resolve_did(&staged.confirmed_did).await;
    assert!(matches!(by_did, Err(ResolveError::Unavailable(_))));
}

#[tokio::test]
async fn clones_share_the_staged_world() {
    let resolver = MemIdentityResolver::new();
    let shared = resolver.clone();
    let dave = handle("dave.example.com");
    let dave_did = did("did:plc:daveaaaaaaaaaaaaaaaaaaaa");
    resolver.seed_pair(&dave, &dave_did);
    let resolved = shared.resolve_handle(&dave).await;
    assert_eq!(resolved.expect("seen through the clone"), dave_did);
}
