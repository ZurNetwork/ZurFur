use std::collections::BTreeSet;

use super::*;

/// The skeleton's first declared tab name, read through the const.
fn declared_tab() -> TabName {
    SKELETON[0]
        .tab
        .parse::<TabName>()
        .expect("the skeleton declares valid labels")
}

// An undeclared surface is rejected; the const is the only authority.
#[test]
fn the_skeleton_refuses_an_undeclared_surface() {
    let tab = declared_tab();
    let declared = "content".parse::<SurfaceName>().expect("valid");
    assert!(
        declares_surface(&tab, &declared),
        "the placeholder surface exists"
    );

    for unknown in ["nope", "Content", "content ", "main", "body"] {
        let Ok(name) = unknown.parse::<SurfaceName>() else {
            continue;
        };
        if name.as_ref() == declared.as_ref() {
            continue;
        }
        assert!(
            !declares_surface(&tab, &name),
            "{unknown:?} is not declared and must be refused"
        );
    }
}

// The check is on the pair: a real surface under the wrong tab is refused
// exactly like an invented name.
#[test]
fn a_surface_under_the_wrong_tab_is_refused() {
    let real_tab = declared_tab();
    let real_surface = SKELETON[0].surfaces[0]
        .parse::<SurfaceName>()
        .expect("the skeleton declares valid labels");
    assert!(
        declares_surface(&real_tab, &real_surface),
        "the pair the skeleton actually declares"
    );

    let wrong_tab = "not-a-declared-tab"
        .parse::<TabName>()
        .expect("valid label");
    assert!(
        !declares_surface(&wrong_tab, &real_surface),
        "a real surface under a tab that does not declare it must be refused"
    );

    // Cross-check every pair, so this holds once the skeleton grows.
    for tab in SKELETON {
        let name = tab.tab.parse::<TabName>().expect("valid label");
        for other in SKELETON {
            for surface in other.surfaces {
                let surface = surface.parse::<SurfaceName>().expect("valid label");
                let declared = tab.surfaces.contains(&surface.as_ref());
                assert_eq!(
                    declares_surface(&name, &surface),
                    declared,
                    "({:?}, {surface:?}) must be declared iff {:?} lists it",
                    tab.tab,
                    tab.tab
                );
            }
        }
    }
}

// Surface names must be globally unique across tabs:
// commission_surface_mode's PK is (commission_id, surface) with no tab
// column, so a duplicate name would make widening one widen both.
#[test]
fn the_skeleton_declares_globally_unique_surface_names() {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for tab in SKELETON {
        for surface in tab.surfaces {
            assert!(
                seen.insert(surface),
                "surface {surface:?} is declared in more than one tab — \
                 commission_surface_mode (commission_id, surface) cannot tell them \
                 apart, so widening one would widen both"
            );
        }
    }
    assert!(
        !seen.is_empty(),
        "the skeleton declares at least one surface"
    );
}

// The skeleton's own labels are well-formed and its tabs are the ones a
// commission is born with.
#[test]
fn the_skeleton_declares_well_formed_labels() {
    let tabs = declared_tabs();
    assert_eq!(tabs.len(), SKELETON.len(), "one row per declared tab");
    assert_eq!(
        tabs.iter().map(|tab| tab.as_ref()).collect::<Vec<_>>(),
        vec!["main"],
        "the placeholder skeleton (ZMVP-171 owns the real one)"
    );

    let mut seen = BTreeSet::new();
    for declared in SKELETON {
        assert!(
            seen.insert(declared.tab),
            "duplicate tab {:?}",
            declared.tab
        );
        assert!(!declared.surfaces.is_empty(), "a tab with no surfaces");
        let name = declared
            .tab
            .parse::<TabName>()
            .expect("a declared tab is a valid label");
        for surface in declared.surfaces {
            let parsed = surface
                .parse::<SurfaceName>()
                .expect("a declared surface is a valid label");
            assert!(declares_surface(&name, &parsed));
        }
    }
}
