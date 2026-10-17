use super::*;

#[test]
fn role_alias_trims_surrounding_whitespace() {
    let alias = RoleAlias::new("  Studio Head  ").expect("non-empty after trimming");
    assert_eq!(alias.as_str(), "Studio Head");
}

#[test]
fn role_alias_rejects_empty_and_whitespace_only() {
    assert_eq!(RoleAlias::new(""), Err(InvalidRoleAlias));
    assert_eq!(RoleAlias::new("   "), Err(InvalidRoleAlias));
}

#[test]
fn role_alias_round_trips_through_display_and_from_str() {
    let alias = RoleAlias::new("Studio Head").expect("non-empty");
    let round_tripped: RoleAlias = alias.to_string().parse().expect("re-parses");
    assert_eq!(alias, round_tripped);
}
