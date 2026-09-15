use super::*;

// Display-only vocabulary, not persisted or parsed: pin the rendered tokens.
#[test]
fn account_entity_tokens_are_pinned() {
    assert_eq!(AccountEntity::Account.to_string(), "account");
    assert_eq!(AccountEntity::User.to_string(), "user");
    assert_eq!(AccountEntity::Workflow.to_string(), "workflow");
    assert_eq!(AccountEntity::Column.to_string(), "column");
    assert_eq!(AccountEntity::Commission.to_string(), "commission");
}
