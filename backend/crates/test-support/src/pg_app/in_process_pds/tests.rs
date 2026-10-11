use super::*;

fn alice() -> Profile {
    Profile::new(Did::from("did:plc:alice".to_string()), "alice.test")
}

#[tokio::test]
async fn a_registered_handle_signs_in_as_its_did() {
    let pds = InProcessPds::default();
    pds.register(alice());

    let callback = pds.start("alice.test").await.expect("start");
    let code = callback
        .strip_prefix("/signin-callback?code=")
        .expect("the callback carries the code")
        .to_string();
    let did = pds.complete(code, None, None).await.expect("complete");

    assert_eq!(did, alice().did);
}

#[tokio::test]
async fn an_unregistered_handle_is_refused_at_both_steps() {
    let pds = InProcessPds::default();
    pds.register(alice());

    let started = pds.start("bob.test").await;
    let completed = pds.complete("bob.test".to_string(), None, None).await;

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

    let callback = pds.start("alice.test").await.expect("start");
    let code = callback
        .strip_prefix("/signin-callback?code=")
        .expect("the callback carries the code")
        .to_string();
    let did = pds.complete(code, None, None).await.expect("complete");

    assert_eq!(did, moved.did);
}

#[tokio::test]
async fn registering_a_did_under_a_new_handle_replaces_it() {
    let pds = InProcessPds::default();
    pds.register(alice());
    let renamed = Profile::new(alice().did, "alice-new.test");
    pds.register(renamed.clone());

    let old_handle = pds.start("alice.test").await;
    let profile = pds.fetch(&alice().did).await.expect("alice's profile");

    assert!(old_handle.is_err(), "the old handle no longer signs in");
    assert_eq!(profile, renamed, "the DID answers its newest profile");
}
