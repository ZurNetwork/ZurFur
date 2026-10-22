use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use domain::elements::custody::CustodySubject;

use super::*;

/// Runs `future` to its end on this thread. The derived custodian never waits on I/O,
/// so a ready answer arrives on the first poll; a pending one fails the test.
fn finished<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(output) = future.as_mut().poll(&mut context) else {
        panic!("the derived custodian answers without waiting");
    };
    output
}

/// A subject for the round-trip tests.
fn subject() -> CustodySubject {
    let parts: [&[u8]; 1] = [b"a subject"];
    CustodySubject::try_from(parts.as_slice()).expect("one short part encodes")
}

#[test]
fn a_root_that_is_not_32_bytes_is_refused_by_its_length_only() {
    let short_root = [0x7Au8; 16];

    let refusal =
        seal_handles(&short_root, &Environment::DEV, false).expect_err("a 16-byte root is refused");

    let message = format!("{refusal:#}");
    assert_eq!(
        message,
        "the custody root key must be exactly 32 bytes, got 16"
    );
}

#[test]
fn the_guard_refuses_before_the_length_is_read() {
    let short_root = [0x7Au8; 16];

    let refusal = seal_handles(&short_root, &Environment::PROD, false)
        .expect_err("production refuses a config root");

    let message = format!("{refusal:#}");
    assert!(message.starts_with("refusing to boot in PROD"), "{message}");
}

#[test]
fn a_32_byte_dev_root_gives_both_seal_handles() {
    let root = [0x7Au8; 32];

    let handles = seal_handles(&root, &Environment::DEV, false).expect("a dev root");

    let shown = format!("{handles:?}");
    assert_eq!(
        shown,
        "SealHandles { did_custody: ScopedSealer { purpose: DidCustody }, \
         signin_secrets: ScopedSealer { purpose: SigninSecrets } }"
    );
}

#[test]
fn each_handle_seals_under_a_key_its_custodian_holds() {
    let root = [0x7Au8; 32];
    let handles = seal_handles(&root, &Environment::DEV, false).expect("a dev root");
    let subject = subject();

    let did_custody_envelope = finished(handles.did_custody.seal(&subject, b"a did key"))
        .expect("the did-custody key is derived");
    let signin_envelope = finished(handles.signin_secrets.seal(&subject, b"a token"))
        .expect("the sign-in-secrets key is derived");

    let opened = finished(handles.signin_secrets.open(&subject, &signin_envelope))
        .expect("the sign-in handle opens its own envelope");
    assert_eq!(opened.expose(), b"a token");
    let refused = finished(handles.signin_secrets.open(&subject, &did_custody_envelope));
    assert!(
        refused.is_err(),
        "the sign-in handle doesn't open a did-custody envelope"
    );
}
