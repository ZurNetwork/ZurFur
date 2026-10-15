use super::*;

fn alice() -> Profile {
    Profile::new(Did::from("did:plc:alice".to_string()), "alice.test")
}

/// Whether `completed` is a refusal for its browser, as the port names it.
fn refused_for_its_browser(completed: anyhow::Result<Did>) -> bool {
    completed.is_err_and(|refused| refused.is::<BrowserMismatch>())
}

/// `raw` as the handle a visitor typed.
fn handle(raw: &str) -> AtHandle {
    raw.parse().expect("a valid handle")
}

/// The callback's code: the authorization URL's `code` parameter.
fn code_of(started: &SigninStarted) -> String {
    started
        .authorization_url
        .strip_prefix("/signin-callback?code=")
        .expect("the callback carries the code")
        .to_string()
}

#[tokio::test]
async fn a_registered_handle_signs_in_as_its_did() {
    let pds = InProcessPds::default();
    pds.register(alice());

    let started = pds.start(&handle("alice.test")).await.expect("start");
    let code = code_of(&started);
    let did = pds
        .complete(code, None, None, Some(started.browser_binding))
        .await
        .expect("complete");

    assert_eq!(did, alice().did);
}

#[tokio::test]
async fn an_unregistered_handle_is_refused_at_both_steps() {
    let pds = InProcessPds::default();
    pds.register(alice());

    let started = pds.start(&handle("bob.test")).await;
    let binding = BrowserBinding::from("in-process-binding:bob.test".to_string());
    let completed = pds
        .complete("bob.test".to_string(), None, None, Some(binding))
        .await;

    assert!(started.is_err(), "start refuses an unknown handle");
    assert!(
        completed.is_err(),
        "complete refuses a handle no one registered"
    );
}

#[tokio::test]
async fn a_registered_did_fetches_its_own_profile_and_no_other() {
    let pds = InProcessPds::default();
    pds.register(alice());

    let profile = pds.fetch(&alice().did).await.expect("alice's profile");
    let stranger = pds.fetch(&Did::from("did:plc:bob".to_string())).await;

    assert_eq!(profile, alice());
    assert!(stranger.is_err(), "an unknown DID's PDS is unreachable");
}

#[tokio::test]
async fn registering_a_handle_again_replaces_it() {
    let pds = InProcessPds::default();
    pds.register(alice());
    let moved = Profile::new(Did::from("did:plc:alice2".to_string()), "alice.test");
    pds.register(moved.clone());

    let started = pds.start(&handle("alice.test")).await.expect("start");
    let code = code_of(&started);
    let did = pds
        .complete(code, None, None, Some(started.browser_binding))
        .await
        .expect("complete");

    assert_eq!(did, moved.did);
}

#[tokio::test]
async fn registering_a_did_under_a_new_handle_replaces_it() {
    let pds = InProcessPds::default();
    pds.register(alice());
    let renamed = Profile::new(alice().did, "alice-new.test");
    pds.register(renamed.clone());

    let old_handle = pds.start(&handle("alice.test")).await;
    let profile = pds.fetch(&alice().did).await.expect("alice's profile");

    assert!(old_handle.is_err(), "the old handle no longer signs in");
    assert_eq!(profile, renamed, "the DID answers its newest profile");
}

#[tokio::test]
async fn a_callback_without_its_browser_binding_is_refused() {
    let pds = InProcessPds::default();
    pds.register(alice());
    let started = pds.start(&handle("alice.test")).await.expect("start");
    let code = code_of(&started);
    let foreign = BrowserBinding::from("in-process-binding:bob.test".to_string());

    let bare = pds.complete(code.clone(), None, None, None).await;
    let crossed = pds.complete(code.clone(), None, None, Some(foreign)).await;
    let own = pds
        .complete(code, None, None, Some(started.browser_binding))
        .await;

    assert!(refused_for_its_browser(bare), "no binding, no sign-in");
    assert!(
        refused_for_its_browser(crossed),
        "another sign-in's binding is refused"
    );
    assert!(own.is_ok(), "its own binding completes the sign-in");
}
