use super::*;

// Display-only vocabulary, not persisted or parsed: pin the rendered tokens.
#[test]
fn not_found_entity_tokens_are_pinned() {
    assert_eq!(NotFoundEntity::Commission.to_string(), "commission");
    assert_eq!(NotFoundEntity::Character.to_string(), "character");
    assert_eq!(NotFoundEntity::User.to_string(), "user");
    assert_eq!(NotFoundEntity::Account.to_string(), "account");
    assert_eq!(NotFoundEntity::Element.to_string(), "element");
}
