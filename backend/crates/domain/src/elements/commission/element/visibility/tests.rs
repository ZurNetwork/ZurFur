use std::collections::BTreeSet;

use strum::VariantArray;

use super::*;

// Effective visibility is the min of the three terms, so over-claiming is
// inert at any combination.
#[test]
fn effective_visibility_clamps_an_over_claiming_element() {
    // The headline case: a wide-open element under a closed surface.
    assert_eq!(
        VisibilityMode::effective_visibility(
            VisibilityMode::Description,
            VisibilityMode::Total,
            VisibilityMode::Description,
        ),
        VisibilityMode::Total,
        "a Total surface closes the door however wide the element claims"
    );
    // A closed TAB clamps just as hard, one level further out.
    assert_eq!(
        VisibilityMode::effective_visibility(
            VisibilityMode::Total,
            VisibilityMode::Description,
            VisibilityMode::Description,
        ),
        VisibilityMode::Total,
    );
    // The element itself is the narrowest term: it can always close further.
    assert_eq!(
        VisibilityMode::effective_visibility(
            VisibilityMode::Description,
            VisibilityMode::Description,
            VisibilityMode::Presentation,
        ),
        VisibilityMode::Presentation,
    );
    // All three wide open is the only way anything reaches Description.
    assert_eq!(
        VisibilityMode::effective_visibility(
            VisibilityMode::Description,
            VisibilityMode::Description,
            VisibilityMode::Description,
        ),
        VisibilityMode::Description,
    );

    // Exhaustive: the result is never wider than ANY of its terms.
    for tab in VisibilityMode::VARIANTS {
        for surface in VisibilityMode::VARIANTS {
            for own in VisibilityMode::VARIANTS {
                let effective = VisibilityMode::effective_visibility(*tab, *surface, *own);
                assert!(
                    effective <= *tab && effective <= *surface && effective <= *own,
                    "min({tab:?}, {surface:?}, {own:?}) = {effective:?} exceeded a term",
                );
            }
        }
    }
}

// The mode tokens round-trip and order by openness.
#[test]
fn visibility_mode_tokens_round_trip_and_order_by_openness() {
    let mut seen = BTreeSet::new();
    for mode in VisibilityMode::VARIANTS {
        let token = <&'static str>::from(*mode);
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(token.parse::<VisibilityMode>().ok(), Some(*mode));
    }
    assert_eq!(VisibilityMode::VARIANTS.len(), 3, "exactly the three modes");
    assert_eq!(
        "wide-open".parse::<VisibilityMode>().ok(),
        None,
        "tampering"
    );

    assert!(
        VisibilityMode::Total < VisibilityMode::Presentation
            && VisibilityMode::Presentation < VisibilityMode::Description,
        "declaration order IS the openness ladder — the min clamp depends on it"
    );
}
