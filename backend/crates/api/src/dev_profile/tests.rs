use domain::elements::custody::{MacMessage, MacUse};
use test_support::custody::{root_of, test_custody, test_root};

use super::*;

#[test]
fn dev_passes_the_lock() {
    assert!(ensure_dev(&Environment::DEV).is_ok());
}

#[test]
fn staging_and_production_are_refused_with_the_reason() {
    for env in [Environment::STG, Environment::PROD] {
        let refused = ensure_dev(&env).expect_err("only dev passes");

        let reason = refused.to_string();
        assert!(
            reason.contains("fixture user"),
            "names the danger: {reason}"
        );
        assert!(
            reason.contains("ZURFUR_ENV=dev"),
            "names the way out: {reason}"
        );
    }
}

#[test]
fn zurfur_env_set_to_dev_passes_the_variable_lock() {
    for spelling in ["dev", "DEV"] {
        assert!(ensure_dev_variable(Some(OsStr::new(spelling))).is_ok());
    }
}

#[test]
fn an_unset_zurfur_env_is_refused_as_unset() {
    let refused = ensure_dev_variable(None).expect_err("unset is not dev");

    assert!(matches!(refused, OutsideDev::Unset));
    let reason = refused.to_string();
    assert!(reason.contains("ZURFUR_ENV unset"), "says why: {reason}");
}

#[test]
fn any_other_zurfur_env_is_refused_by_name() {
    for other in ["prod", "stg", "", "Dev", "dev "] {
        let refused = ensure_dev_variable(Some(OsStr::new(other))).expect_err("not dev");

        let named = OutsideDev::Named(other.to_string());
        assert_eq!(refused.to_string(), named.to_string());
    }
}

/// A message for the Den MAC tests.
fn den_node() -> MacMessage {
    let parts: [&[u8]; 1] = [b"a den node"];
    MacMessage::try_from(parts.as_slice()).expect("one short part encodes")
}

#[tokio::test]
async fn dev_profile_derives_from_the_configured_root() {
    let message = den_node();
    let from_test_root = den_mac(test_root(), &Environment::DEV, false).expect("a dev root");
    let from_another_root =
        den_mac(root_of([0x11; 32]), &Environment::DEV, false).expect("a dev root");
    let test_custodian = test_custody().den_mac;

    let profile_tag = from_test_root
        .mac(MacUse::DenNode, &message)
        .await
        .expect("the profile holds the Den's key");
    let custodian_tag = test_custodian
        .mac(MacUse::DenNode, &message)
        .await
        .expect("the test custodian holds the Den's key");
    let other_tag = from_another_root
        .mac(MacUse::DenNode, &message)
        .await
        .expect("the profile holds the Den's key");

    assert_eq!(profile_tag.stored_bytes(), custodian_tag.stored_bytes());
    assert_eq!(profile_tag.key_id(), custodian_tag.key_id());
    assert_ne!(profile_tag.stored_bytes(), other_tag.stored_bytes());
}

#[test]
fn the_custody_guard_runs_before_the_dev_profile_derives() {
    let refusal = den_mac(test_root(), &Environment::PROD, false)
        .expect_err("the custody guard refuses production");

    let message = format!("{refusal:#}");
    assert!(message.starts_with("refusing to boot in PROD"), "{message}");
}
