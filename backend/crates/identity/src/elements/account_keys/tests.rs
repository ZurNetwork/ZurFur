use super::*;

// A SecretKey's Debug must never reveal its bytes.
#[test]
fn secret_key_debug_is_redacted() {
    let key = SecretKey::new(vec![0xAB; 32]);
    let shown = format!("{key:?}");
    assert_eq!(shown, "SecretKey(<redacted>)");
    assert!(!shown.contains("ab"));
    assert!(!shown.contains("171"));
}

// Every AccountKeys field is a redacted SecretKey, so the bundle is safe.
#[test]
fn account_keys_debug_redacts_every_field() {
    let keys = AccountKeys {
        cold_recovery: SecretKey::new(vec![1; 32]),
        operational: SecretKey::new(vec![2; 32]),
        signing: SecretKey::new(vec![3; 32]),
    };
    let shown = format!("{keys:?}");
    assert_eq!(shown.matches("<redacted>").count(), 3);
    assert!(!shown.contains("[1, 1, 1"));
}
