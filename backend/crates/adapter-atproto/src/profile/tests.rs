use super::presented_handle;

const DID: &str = "did:plc:actor";

// Finding 2: a handle is only the actor's when it resolves BACK to this DID.
#[test]
fn a_handle_that_resolves_back_to_this_did_is_presented() {
    assert_eq!(
        presented_handle(DID, "alice.zurfur.app", Some(DID)),
        "alice.zurfur.app",
        "a bidirectionally-verified handle is trusted"
    );
}

#[test]
fn a_handle_resolving_to_another_did_is_never_presented() {
    // A spoofed / stale `alsoKnownAs`: the claimed handle belongs to someone else.
    assert_eq!(
        presented_handle(DID, "victim.zurfur.app", Some("did:plc:someoneelse")),
        DID,
        "a handle owned by a different DID falls back to the DID, never impersonates"
    );
}

#[test]
fn an_unresolvable_handle_falls_back_to_the_did() {
    // Reverse resolution could not be completed (malformed handle, resolver
    // failure, …) — the claim is unconfirmed, so it must not be presented.
    assert_eq!(
        presented_handle(DID, "alice.zurfur.app", None),
        DID,
        "an unconfirmable handle is never presented as trusted"
    );
}
