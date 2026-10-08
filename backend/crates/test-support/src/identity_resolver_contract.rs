//! The shared [`IdentityResolver`] conformance suite.
//!
//! One generic body, run against every adapter that implements the port: the
//! in-memory fake (`adapter-mem`'s `MemIdentityResolver`) and the real atproto
//! resolver over its scripted DNS and mock hosts. Each runner stages the world
//! its own way and describes it in a [`StagedIdentities`]; the suite then checks
//! that every staged case answers as the port promises.
//!
//! Scope is the port surface only. Off-port behaviour (which fetch rule a host
//! broke, how a document is parsed) lives in the adapter's own tests.

use domain::elements::did::Did;
use domain::elements::handle::AtHandle;
use domain::ports::{IdentityResolver, ResolveError};

/// The identities a runner staged before calling [`identity_resolver_contract`],
/// each set up to answer one way. All handles and DIDs must be distinct.
pub struct StagedIdentities {
    /// A handle naming [`Self::confirmed_did`], whose document claims the handle back.
    pub confirmed_handle: AtHandle,
    /// The DID [`Self::confirmed_handle`] names.
    pub confirmed_did: Did,
    /// A handle naming a DID whose document claims another handle.
    pub unconfirmed_handle: AtHandle,
    /// A handle naming a DID whose document claims no handle at all.
    pub handle_whose_did_claims_nothing: AtHandle,
    /// A DID whose document claims a handle that names a different DID.
    pub disowned_did: Did,
    /// A DID whose document claims no handle at all.
    pub handleless_did: Did,
    /// A DID whose document claims a handle that names nothing.
    pub did_claiming_unknown_handle: Did,
    /// A handle that names nothing.
    pub unknown_handle: AtHandle,
    /// A DID that does not resolve.
    pub unknown_did: Did,
    /// A handle whose lookup cannot finish.
    pub unavailable_handle: AtHandle,
    /// A handle whose host breaks a fetch rule.
    pub refused_handle: AtHandle,
    /// A DID whose document cannot be fetched in time.
    pub unavailable_did: Did,
    /// A DID whose document's host breaks a fetch rule.
    pub refused_did: Did,
    /// A handle naming a DID whose document cannot be fetched in time.
    pub handle_naming_unavailable_document: AtHandle,
    /// A handle naming a DID whose document's host breaks a fetch rule.
    pub handle_naming_refused_document: AtHandle,
    /// A handle naming a DID that does not resolve.
    pub handle_naming_missing_document: AtHandle,
    /// A DID whose claimed handle's lookup cannot finish.
    pub did_claiming_unavailable_handle: Did,
    /// A DID whose claimed handle's host breaks a fetch rule.
    pub did_claiming_refused_handle: Did,
}

/// Run the whole suite against `resolver` over the `staged` world. Panics with
/// a message naming the broken promise on the first divergence.
pub async fn identity_resolver_contract<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    a_confirmed_pair_resolves_both_ways(resolver, staged).await;
    an_unconfirmed_handle_is_not_confirmed(resolver, staged).await;
    a_did_without_a_confirmed_handle_has_none(resolver, staged).await;
    unknown_identities_are_not_found(resolver, staged).await;
    lookup_failures_keep_their_class(resolver, staged).await;
    a_back_check_outage_is_an_error_but_a_refusal_is_none(resolver, staged).await;
    no_error_message_echoes_the_input(resolver, staged).await;
}

/// A handle and DID that claim each other resolve to each other.
async fn a_confirmed_pair_resolves_both_ways<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let did = resolver
        .resolve_handle(&staged.confirmed_handle)
        .await
        .expect("a confirmed handle resolves");
    assert_eq!(
        did, staged.confirmed_did,
        "a confirmed handle resolves to the DID it names"
    );

    let handle = resolver
        .resolve_did(&staged.confirmed_did)
        .await
        .expect("a confirmed DID resolves");
    let expected_handle = Some(staged.confirmed_handle.clone());
    assert_eq!(
        handle, expected_handle,
        "a confirmed DID resolves to the handle it claims"
    );
}

/// A handle whose DID does not claim it back is `NotConfirmed`, whether the
/// DID's document claims another handle or none at all.
async fn an_unconfirmed_handle_is_not_confirmed<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let claims_another = resolver.resolve_handle(&staged.unconfirmed_handle).await;
    assert!(
        matches!(claims_another, Err(ResolveError::NotConfirmed)),
        "a handle whose DID claims another handle is NotConfirmed, got {claims_another:?}"
    );

    let claims_nothing = resolver
        .resolve_handle(&staged.handle_whose_did_claims_nothing)
        .await;
    assert!(
        matches!(claims_nothing, Err(ResolveError::NotConfirmed)),
        "a handle whose DID claims no handle is NotConfirmed, got {claims_nothing:?}"
    );
}

/// A DID whose claim fails the back-check, claims nothing, or claims a handle
/// that names nothing has no handle.
async fn a_did_without_a_confirmed_handle_has_none<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let disowned = resolver.resolve_did(&staged.disowned_did).await;
    assert!(
        matches!(disowned, Ok(None)),
        "a DID claiming a handle that names another DID has no handle, got {disowned:?}"
    );

    let handleless = resolver.resolve_did(&staged.handleless_did).await;
    assert!(
        matches!(handleless, Ok(None)),
        "a DID claiming no handle has no handle, got {handleless:?}"
    );

    let claiming_unknown = resolver
        .resolve_did(&staged.did_claiming_unknown_handle)
        .await;
    assert!(
        matches!(claiming_unknown, Ok(None)),
        "a DID claiming a handle that names nothing has no handle (not an error), \
         got {claiming_unknown:?}"
    );
}

/// A handle that names nothing and a DID that does not resolve are `NotFound`.
async fn unknown_identities_are_not_found<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let handle_result = resolver.resolve_handle(&staged.unknown_handle).await;
    assert!(
        matches!(handle_result, Err(ResolveError::NotFound)),
        "a handle naming nothing is NotFound, got {handle_result:?}"
    );

    let did_result = resolver.resolve_did(&staged.unknown_did).await;
    assert!(
        matches!(did_result, Err(ResolveError::NotFound)),
        "a DID that does not resolve is NotFound (an error, not None), got {did_result:?}"
    );
}

/// An outage stays `Unavailable` and a broken fetch rule stays `Refused`, in
/// either direction and at either step of a handle lookup.
async fn lookup_failures_keep_their_class<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let unavailable_handle = resolver.resolve_handle(&staged.unavailable_handle).await;
    assert!(
        matches!(unavailable_handle, Err(ResolveError::Unavailable(_))),
        "a handle lookup that cannot finish is Unavailable, got {unavailable_handle:?}"
    );

    let refused_handle = resolver.resolve_handle(&staged.refused_handle).await;
    assert!(
        matches!(refused_handle, Err(ResolveError::Refused(_))),
        "a handle host breaking a fetch rule is Refused, got {refused_handle:?}"
    );

    let unavailable_back_check = resolver
        .resolve_handle(&staged.handle_naming_unavailable_document)
        .await;
    assert!(
        matches!(unavailable_back_check, Err(ResolveError::Unavailable(_))),
        "a handle whose DID document cannot be fetched is Unavailable, not NotConfirmed, \
         got {unavailable_back_check:?}"
    );

    let refused_back_check = resolver
        .resolve_handle(&staged.handle_naming_refused_document)
        .await;
    assert!(
        matches!(refused_back_check, Err(ResolveError::Refused(_))),
        "a handle whose DID document is refused is Refused, not NotConfirmed, \
         got {refused_back_check:?}"
    );

    let missing_back_check = resolver
        .resolve_handle(&staged.handle_naming_missing_document)
        .await;
    assert!(
        matches!(missing_back_check, Err(ResolveError::NotFound)),
        "a handle naming a DID that does not resolve is NotFound, not NotConfirmed, \
         got {missing_back_check:?}"
    );

    let unavailable_did = resolver.resolve_did(&staged.unavailable_did).await;
    assert!(
        matches!(unavailable_did, Err(ResolveError::Unavailable(_))),
        "a DID document that cannot be fetched is Unavailable, got {unavailable_did:?}"
    );

    let refused_did = resolver.resolve_did(&staged.refused_did).await;
    assert!(
        matches!(refused_did, Err(ResolveError::Refused(_))),
        "a DID whose own document is refused is Err(Refused), not None, got {refused_did:?}"
    );
}

/// When a DID's claimed handle cannot be checked, an outage is an error (so a
/// caller can retry) while a refusal means the claim is simply not confirmed.
async fn a_back_check_outage_is_an_error_but_a_refusal_is_none<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let outage = resolver
        .resolve_did(&staged.did_claiming_unavailable_handle)
        .await;
    assert!(
        matches!(outage, Err(ResolveError::Unavailable(_))),
        "a back-check that cannot finish is Err(Unavailable), not None, got {outage:?}"
    );

    let refusal = resolver
        .resolve_did(&staged.did_claiming_refused_handle)
        .await;
    assert!(
        matches!(refusal, Ok(None)),
        "a refused back-check means no confirmed handle, got {refusal:?}"
    );
}

/// No failure's message or debug form contains the handle or DID it was asked
/// about. Both are fixed by `ResolveError` itself, so this is a regression guard
/// for the shared type rather than a test of the adapter.
async fn no_error_message_echoes_the_input<R: IdentityResolver + ?Sized>(
    resolver: &R,
    staged: &StagedIdentities,
) {
    let failing_handles = [
        &staged.unconfirmed_handle,
        &staged.unknown_handle,
        &staged.unavailable_handle,
        &staged.refused_handle,
        &staged.handle_whose_did_claims_nothing,
        &staged.handle_naming_unavailable_document,
        &staged.handle_naming_refused_document,
        &staged.handle_naming_missing_document,
    ];
    for handle in failing_handles {
        let Err(error) = resolver.resolve_handle(handle).await else {
            panic!("{handle} was staged to fail");
        };
        let rendered = format!("{error} {error:#} {error:?} {error:#?}");
        assert!(
            !rendered.contains(handle.as_ref()),
            "the error for {handle} echoed it: {rendered}"
        );
    }

    let failing_dids = [
        &staged.unknown_did,
        &staged.unavailable_did,
        &staged.refused_did,
        &staged.did_claiming_unavailable_handle,
    ];
    for did in failing_dids {
        let Err(error) = resolver.resolve_did(did).await else {
            panic!("{did} was staged to fail");
        };
        let rendered = format!("{error} {error:#} {error:?} {error:#?}");
        assert!(
            !rendered.contains(did.as_ref()),
            "the error for {did} echoed it: {rendered}"
        );
    }
}
