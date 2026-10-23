use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use strum::VariantArray as _;

use crate::EXAMPLE_DEV_ROOT_KEY;

use super::*;

/// Runs `future` to its end on this thread. A custodian here never waits on I/O, so a
/// ready answer arrives on the first poll; a pending one fails the test.
fn finished<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(output) = future.as_mut().poll(&mut context) else {
        panic!("the custodian answers without waiting");
    };
    output
}

/// A subject for the round-trip tests.
fn subject() -> CustodySubject {
    let parts: [&[u8]; 1] = [b"a subject"];
    CustodySubject::try_from(parts.as_slice()).expect("one short part encodes")
}

/// A message for the MAC tests.
fn message() -> MacMessage {
    let parts: [&[u8]; 1] = [b"a node"];
    MacMessage::try_from(parts.as_slice()).expect("one short part encodes")
}

/// A dev root of 32 repeated `byte`s.
fn root(byte: u8) -> RootSecret {
    RootSecret::from(Zeroizing::new([byte; 32]))
}

/// The example dev root shipped in `.env.example`.
fn example_root() -> RootSecret {
    let bytes: [u8; 32] = EXAMPLE_DEV_ROOT_KEY
        .try_into()
        .expect("the example root is 32 bytes");
    RootSecret::from(Zeroizing::new(bytes))
}

/// Whether `result` is the refusal for `purpose` not being held.
fn not_held<T>(result: Result<T, CustodyError>, purpose: Purpose) -> bool {
    matches!(result, Err(CustodyError::PurposeNotHeld { purpose: refused }) if refused == purpose)
}

#[test]
fn a_custody_without_a_purpose_answers_purpose_not_held() {
    let custody = Custody::default();
    let subject = subject();

    let sealed_did_key = finished(custody.did_custody.seal(&subject, b"a did key"));
    let sealed_token = finished(custody.signin_secrets.seal(&subject, b"a token"));
    let tagged = finished(custody.den_mac.mac(MacUse::DenNode, &message()));

    assert!(not_held(sealed_did_key, Purpose::DidCustody));
    assert!(not_held(sealed_token, Purpose::SigninSecrets));
    assert!(not_held(tagged, Purpose::DenMac));
}

#[test]
fn the_guard_checks_the_raw_root_before_derivation() {
    let refusal = Custody::derive(example_root(), &Environment::DEV, true, Purpose::VARIANTS)
        .expect_err("submitting under the example root is refused");

    let message = format!("{refusal:#}");
    assert!(
        message.starts_with("refusing PLC directory submission"),
        "{message}"
    );
}

#[test]
fn the_guard_runs_inside_the_one_custody_function() {
    for env in [Environment::PROD, Environment::STG] {
        let derived = Custody::derive(root(0x7A), &env, false, &[Purpose::DidCustody]);
        let kept = derive_keeping_legacy_root(root(0x7A), &env, false, &[Purpose::DidCustody]);

        let derive_refusal = format!("{:#}", derived.expect_err("a config root is refused"));
        let kept_refusal = format!("{:#}", kept.expect_err("a config root is refused"));
        assert!(
            derive_refusal.starts_with("refusing to boot in"),
            "{derive_refusal}"
        );
        assert!(
            kept_refusal.starts_with("refusing to boot in"),
            "{kept_refusal}"
        );
    }
}

#[test]
fn a_dev_root_gives_every_handle() {
    let custody = Custody::derive(root(0x7A), &Environment::DEV, false, Purpose::VARIANTS)
        .expect("a dev root");

    let shown = format!("{custody:?}");
    assert_eq!(
        shown,
        "Custody { did_custody: ScopedSealer { purpose: DidCustody }, \
         signin_secrets: ScopedSealer { purpose: SigninSecrets }, \
         den_mac: ScopedMac { purpose: DenMac } }"
    );
}

#[test]
fn only_the_purposes_asked_for_are_held() {
    let custody = Custody::derive(root(0x7A), &Environment::DEV, false, &[Purpose::DidCustody])
        .expect("a dev root");
    let subject = subject();

    let sealed_did_key = finished(custody.did_custody.seal(&subject, b"a did key"));
    let sealed_token = finished(custody.signin_secrets.seal(&subject, b"a token"));
    let tagged = finished(custody.den_mac.mac(MacUse::DenNode, &message()));

    assert!(sealed_did_key.is_ok(), "the did-custody key is derived");
    assert!(not_held(sealed_token, Purpose::SigninSecrets));
    assert!(not_held(tagged, Purpose::DenMac));
}

#[test]
fn each_handle_seals_under_a_key_its_custodian_holds() {
    let custody = Custody::derive(root(0x7A), &Environment::DEV, false, Purpose::VARIANTS)
        .expect("a dev root");
    let subject = subject();

    let did_custody_envelope = finished(custody.did_custody.seal(&subject, b"a did key"))
        .expect("the did-custody key is derived");
    let signin_envelope = finished(custody.signin_secrets.seal(&subject, b"a token"))
        .expect("the sign-in-secrets key is derived");

    let opened = finished(custody.signin_secrets.open(&subject, &signin_envelope))
        .expect("the sign-in handle opens its own envelope");
    assert_eq!(opened.expose(), b"a token");
    let refused = finished(custody.signin_secrets.open(&subject, &did_custody_envelope));
    assert!(
        refused.is_err(),
        "the sign-in handle doesn't open a did-custody envelope"
    );
}

#[test]
fn the_legacy_copy_is_the_root_itself() {
    let (_custody, legacy_root) =
        derive_keeping_legacy_root(root(0x7A), &Environment::DEV, false, &[Purpose::DidCustody])
            .expect("a dev root");

    assert_eq!(*legacy_root, [0x7Au8; 32]);
}

#[test]
fn debug_prints_no_root_bytes() {
    let custody = Custody::derive(root(0xAB), &Environment::DEV, false, Purpose::VARIANTS)
        .expect("a dev root");

    let shown = format!("{custody:?}");

    let root_forms = ["ab", "AB", "q6ur", "171"];
    for form in root_forms {
        assert!(!shown.contains(form), "{shown} holds {form}");
    }
}
