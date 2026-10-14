use super::*;

const CALLBACK: &str = "http://127.0.0.1:8180/signin-callback";

fn fixed_did() -> Did {
    Did::from("did:plc:fixedsigninaaaaaaaaaaaaa".to_string())
}

fn sign_in() -> FixedSignIn {
    FixedSignIn::new(fixed_did(), CALLBACK)
}

fn handle(raw: &str) -> AtHandle {
    raw.parse::<AtHandle>().expect("a handle")
}

#[tokio::test]
async fn start_redirects_straight_back_to_the_callback() {
    let started = sign_in().start(&handle("bob.test")).await.unwrap();

    let expected_url = format!("{CALLBACK}?code={CALLBACK_CODE}");
    assert_eq!(started.authorization_url, expected_url);
    assert_eq!(started.lifetime, SIGN_IN_LIFETIME);
}

#[tokio::test]
async fn whatever_handle_was_typed_the_sign_in_is_the_fixed_did() {
    let fixed = sign_in();
    let started = fixed.start(&handle("someone.else.test")).await.unwrap();

    let did = fixed
        .complete(
            CALLBACK_CODE.to_string(),
            None,
            None,
            Some(started.browser_binding),
        )
        .await
        .unwrap();

    assert_eq!(did, fixed_did());
}

#[tokio::test]
async fn each_start_binds_a_different_browser() {
    let fixed = sign_in();

    let first = fixed.start(&handle("alice.test")).await.unwrap();
    let second = fixed.start(&handle("alice.test")).await.unwrap();

    let first_binding: &str = first.browser_binding.as_ref();
    let second_binding: &str = second.browser_binding.as_ref();
    assert_ne!(first_binding, second_binding);
}

#[tokio::test]
async fn a_callback_without_a_binding_is_refused() {
    let fixed = sign_in();
    fixed.start(&handle("alice.test")).await.unwrap();

    let completed = fixed
        .complete(CALLBACK_CODE.to_string(), None, None, None)
        .await;

    assert!(completed.is_err(), "no binding, no sign-in");
}

#[tokio::test]
async fn a_binding_start_never_issued_is_refused() {
    let fixed = sign_in();
    fixed.start(&handle("alice.test")).await.unwrap();

    let forged = BrowserBinding::from("forged".to_string());
    let completed = fixed
        .complete(CALLBACK_CODE.to_string(), None, None, Some(forged))
        .await;

    assert!(completed.is_err(), "a forged binding signs no one in");
}

#[tokio::test]
async fn a_binding_completes_one_sign_in_only() {
    let fixed = sign_in();
    let started = fixed.start(&handle("alice.test")).await.unwrap();
    let replayed = started.browser_binding.clone();

    fixed
        .complete(
            CALLBACK_CODE.to_string(),
            None,
            None,
            Some(started.browser_binding),
        )
        .await
        .unwrap();
    let second = fixed
        .complete(CALLBACK_CODE.to_string(), None, None, Some(replayed))
        .await;

    assert!(second.is_err(), "a used binding signs no one in again");
}

/// A sign-in whose bindings expire at once.
fn expiring_sign_in() -> FixedSignIn {
    FixedSignIn {
        lifetime: Duration::ZERO,
        ..sign_in()
    }
}

#[tokio::test]
async fn an_expired_binding_is_refused() {
    let fixed = expiring_sign_in();
    let started = fixed.start(&handle("alice.test")).await.unwrap();

    let completed = fixed
        .complete(
            CALLBACK_CODE.to_string(),
            None,
            None,
            Some(started.browser_binding),
        )
        .await;

    assert!(completed.is_err(), "an expired binding signs no one in");
}

#[tokio::test]
async fn a_start_prunes_the_bindings_that_expired_unused() {
    let fixed = expiring_sign_in();
    fixed.start(&handle("alice.test")).await.unwrap();
    fixed.start(&handle("alice.test")).await.unwrap();

    let kept = fixed.issued.lock().unwrap().len();

    assert_eq!(kept, 1, "only the binding just issued is kept");
}

#[tokio::test]
async fn a_live_binding_survives_another_start() {
    let fixed = sign_in();
    fixed.start(&handle("alice.test")).await.unwrap();
    fixed.start(&handle("alice.test")).await.unwrap();

    let kept = fixed.issued.lock().unwrap().len();

    assert_eq!(kept, 2, "unexpired bindings stay");
}
