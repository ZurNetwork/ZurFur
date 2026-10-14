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
