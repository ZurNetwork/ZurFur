use strum::VariantArray;

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

// The persisted discriminant round-trips (case-insensitively, since Role's
// old parser lowercased its input), and an unknown token is a typed error
// that keeps the offending text verbatim. The derived Ord must still pin
// the rank ladder `can_grant` leans on.
#[test]
fn every_role_round_trips_through_its_token_and_parses_case_insensitively() {
    let tokens: Vec<&'static str> = Role::VARIANTS.iter().map(<&'static str>::from).collect();
    assert_eq!(tokens, ["owner", "admin", "manager", "member"]);

    for role in Role::VARIANTS {
        let token = <&'static str>::from(role);
        assert_eq!(role.to_string(), token);
        assert_eq!(token.parse::<Role>(), Ok(role.clone()));
        assert_eq!(Role::try_from(token), Ok(role.clone()));
    }

    assert_eq!("OWNER".parse::<Role>(), Ok(Role::Owner));
    assert_eq!(
        "god".parse::<Role>(),
        Err(UnknownRole("god".into())),
        "an unknown token keeps its original text in the error"
    );

    assert!(
        Role::VARIANTS.windows(2).all(|pair| pair[0] < pair[1]),
        "declaration order IS the rank ladder — can_grant depends on it"
    );
}
