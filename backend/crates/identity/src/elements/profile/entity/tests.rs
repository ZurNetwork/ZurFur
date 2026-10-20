use super::*;

fn alice_did() -> Did {
    Did::from("did:plc:alice".to_string())
}

#[test]
fn name_candidates_run_display_name_then_handle_then_did() {
    let profile = Profile::new(alice_did(), "alice.test").with_display_name("Alice");

    let candidates: Vec<&str> = profile.name_candidates().collect();

    let expected_chain = ["Alice", "alice.test", "did:plc:alice"];
    assert_eq!(candidates, expected_chain);
}

#[test]
fn name_candidates_without_a_display_name_start_at_the_handle() {
    let profile = Profile::new(alice_did(), "alice.test");

    let candidates: Vec<&str> = profile.name_candidates().collect();

    let expected_chain = ["alice.test", "did:plc:alice"];
    assert_eq!(candidates, expected_chain);
}

#[test]
fn name_candidates_skip_a_handle_that_did_not_verify() {
    let profile = Profile::new(alice_did(), DisplayHandle::INVALID).with_display_name("Alice");

    let candidates: Vec<&str> = profile.name_candidates().collect();

    let expected_chain = ["Alice", "did:plc:alice"];
    assert_eq!(candidates, expected_chain);
}

#[test]
fn name_candidates_always_end_at_the_did() {
    let profile = Profile::new(alice_did(), DisplayHandle::INVALID);

    let candidates: Vec<&str> = profile.name_candidates().collect();

    let expected_chain = ["did:plc:alice"];
    assert_eq!(candidates, expected_chain);
}
