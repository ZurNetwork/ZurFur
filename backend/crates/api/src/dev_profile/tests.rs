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
