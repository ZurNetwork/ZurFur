use super::*;

// The full actor -> target matrix for `can_grant` — where the rule is
// pinned, since the e2e suite can only sign in the Owner.
#[test]
fn can_grant_matrix_matches_the_design() {
    let roles = || [Role::Owner, Role::Admin, Role::Manager, Role::Member];
    for actor in roles() {
        for target in roles() {
            let expected = match (&actor, &target) {
                (Role::Owner, Role::Owner) => false,
                (Role::Owner, _) => true,
                (Role::Admin, Role::Owner | Role::Admin) => false,
                (Role::Admin, _) => true,
                _ => false,
            };
            assert_eq!(
                actor.can_grant(&target),
                expected,
                "{actor:?} granting {target:?}"
            );
        }
    }
}

// The sharpest edges: no one grants Owner through this seam, an Admin
// cannot mint a peer Admin, and the lower roles grant nothing.
#[test]
fn no_owner_no_peer_admin_no_lower_role_grants() {
    for actor in [Role::Owner, Role::Admin, Role::Manager, Role::Member] {
        assert!(!actor.can_grant(&Role::Owner), "{actor:?} → Owner");
    }
    assert!(
        !Role::Admin.can_grant(&Role::Admin),
        "an Admin cannot grant Admin"
    );
    for actor in [Role::Manager, Role::Member] {
        assert!(!actor.can_grant(&Role::Member), "{actor:?} grants");
    }
}

#[test]
fn every_role_round_trips_through_as_str_and_parse() {
    for role in [Role::Owner, Role::Admin, Role::Manager, Role::Member] {
        let parsed: Role = role.as_str().parse().expect("as_str is always parseable");
        assert_eq!(parsed, role);
    }
}
