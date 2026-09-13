use std::collections::BTreeSet;

use chrono::Utc;
use serde_json::json;
use strum::VariantArray;

use super::*;
use crate::elements::did::Did;

fn element(surface: &str, mode: VisibilityMode, tab: TabId) -> ElementRow {
    ElementRow {
        id: ElementId::mint(),
        address: SurfaceAddress::new(tab, surface.parse().expect("valid surface name")),
        element_type: "note".parse().expect("valid type"),
        mode,
        band: Band::default(),
        position: 0,
        created_by: UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7()))),
        created_at: Utc::now(),
        payload: ElementPayload::default(),
    }
}

/// The skeleton's first declared tab name, read through the const.
fn declared_tab() -> TabName {
    SKELETON[0]
        .tab
        .parse::<TabName>()
        .expect("the skeleton declares valid labels")
}

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

// Every default agrees with the migration's column defaults.
#[test]
fn every_default_is_the_closed_door() {
    assert_eq!(VisibilityMode::default(), VisibilityMode::Total);
    assert_eq!(Band::default().as_ref(), Band::BODY);

    let composition = CommissionComposition {
        tabs: Vec::new(),
        surface_modes: HashMap::new(),
        elements: Vec::new(),
    };
    let never_widened = "content".parse::<SurfaceName>().expect("valid");
    assert_eq!(
        composition.surface_mode(&never_widened),
        VisibilityMode::Total,
        "an absent surface-mode row means Total, not unconstrained"
    );
}

// An element whose tab is missing projects Total, not unconstrained.
#[test]
fn an_element_whose_tab_is_missing_projects_closed() {
    let orphan_tab = TabId::mint();
    let widened = "content".parse::<SurfaceName>().expect("valid");
    let composition = CommissionComposition {
        tabs: Vec::new(),
        surface_modes: HashMap::from([(widened.clone(), VisibilityMode::Description)]),
        elements: Vec::new(),
    };
    let stray = element("content", VisibilityMode::Description, orphan_tab);

    assert_eq!(
        composition.effective_visibility_of(&stray),
        VisibilityMode::Total,
        "a missing tab is corruption, and corruption answers closed"
    );
}

// The three terms resolve from the composition itself.
#[test]
fn effective_visibility_of_resolves_all_three_terms() {
    let tab_id = TabId::mint();
    let surface = "content".parse::<SurfaceName>().expect("valid");
    let composition = CommissionComposition {
        tabs: vec![TabRow {
            id: tab_id,
            tab: "main".parse().expect("valid"),
            mode: VisibilityMode::Description,
        }],
        surface_modes: HashMap::from([(surface.clone(), VisibilityMode::Presentation)]),
        elements: Vec::new(),
    };

    let wide = element("content", VisibilityMode::Description, tab_id);
    assert_eq!(
        composition.effective_visibility_of(&wide),
        VisibilityMode::Presentation,
        "the surface override is the narrowest term here"
    );
    assert_eq!(
        composition.tab_mode(tab_id),
        Some(VisibilityMode::Description)
    );
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

// A new element's envelope: fresh id, address, acting user, payload
// verbatim, placeholder band — and no mode field to set.
#[test]
fn a_new_element_carries_its_address_and_payload() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let tab = TabId::mint();
    let surface = "content".parse::<SurfaceName>().expect("valid");
    let address = SurfaceAddress::new(tab, surface.clone());
    let element_type = "note".parse::<ElementType>().expect("valid");
    let owner = UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())));
    let body = json!({ "body": "Reference: 三毛猫 🐾", "revision": 3 });
    let payload = ElementPayload::from(body.clone());

    let contributed = NewElement::contributed(
        commission,
        address.clone(),
        element_type.clone(),
        payload.clone(),
        owner.clone(),
        Utc::now(),
    );

    assert_eq!(contributed.commission_id, commission);
    assert_eq!(contributed.address.tab, tab, "the tab is addressed BY ID");
    assert_eq!(contributed.address.surface, surface, "the surface too");
    assert_eq!(contributed.element_type, element_type);
    assert_eq!(
        contributed.payload, payload,
        "the payload is carried opaque"
    );
    assert_eq!(contributed.band, Band::default());
    assert_eq!(contributed.created_by, owner);

    // The satellite carrier shares an already-minted identity.
    let seat_id = ElementId::mint();
    let carrier = NewElement::carrying(
        seat_id,
        commission,
        address,
        ElementType::seat(),
        owner,
        Utc::now(),
    );
    assert_eq!(carrier.id, seat_id, "one identity, two rows");
    assert_eq!(
        carrier.payload.as_ref(),
        &json!({}),
        "substance lives in the satellite"
    );
    assert_eq!(carrier.band, Band::default());
}

// The payload's doors round-trip and the empty default is `{}`.
#[test]
fn the_payload_wraps_opaque_json_and_defaults_to_the_empty_object() {
    let raw = json!({ "list": [1, 2, 3], "nothing": null, "flag": true });
    let payload = ElementPayload::from(raw.clone());

    assert_eq!(
        payload.as_ref(),
        &raw,
        "carried verbatim, via the std borrow door"
    );
    assert_eq!(
        serde_json::Value::from(payload.clone()),
        raw,
        "and the owned one, via the std Into door"
    );
    assert_eq!(serde_json::Value::from(payload), raw, "as does From");

    assert_eq!(
        ElementPayload::default().as_ref(),
        &json!({}),
        "the empty default is `{{}}`, matching the jsonb column DEFAULT — \
         deliberately NOT serde_json's own `null` default"
    );
}

/// Compile-time probe for "does `T` implement `serde::Serialize`?",
/// answered as a runtime `bool` so a test can assert the negative. Relies on
/// method-resolution priority: the inherent `probe` exists only for
/// `T: Serialize`; everything else falls through one autoref step to the
/// blanket trait impl, which answers `false`.
struct SerializeProbe<T>(std::marker::PhantomData<T>);

impl<T> SerializeProbe<T> {
    const fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: serde::Serialize> SerializeProbe<T> {
    /// The high-priority arm: only exists when `T: Serialize`.
    fn probe(&self) -> bool {
        true
    }
}

/// The fallback arm, reached by one extra autoref when the inherent `probe`
/// does not apply.
trait NotSerialize {
    fn probe(self) -> bool;
}

impl<T> NotSerialize for &SerializeProbe<T> {
    fn probe(self) -> bool {
        false
    }
}

// The raw composition, its rows, and the element payload carry no
// `Serialize`, so putting any on a response is a compile error.
#[test]
fn serialization_is_unrepresentable_for_the_raw_composition() {
    assert!(
        !SerializeProbe::<ElementPayload>::new().probe(),
        "ElementPayload must NOT implement Serialize: it is the element's content, \
         and a derive here would put Total-tier content one Json(…) from the wire"
    );
    assert!(
        !SerializeProbe::<ElementRow>::new().probe(),
        "ElementRow must NOT implement Serialize — project first, always"
    );
    assert!(
        !SerializeProbe::<CommissionComposition>::new().probe(),
        "CommissionComposition must NOT implement Serialize — project first, always"
    );

    // The probe is honest: a type that does implement Serialize answers
    // true, so a false above means "no impl", not "probe broken".
    assert!(
        SerializeProbe::<serde_json::Value>::new().probe(),
        "control: serde_json::Value does implement Serialize"
    );
}

// The composition labels share one validation contract.
#[test]
fn composition_labels_share_one_validation_contract() {
    assert_eq!(" main ".parse::<TabName>().unwrap().as_ref(), "main");
    assert_eq!(
        " content ".parse::<SurfaceName>().unwrap().as_ref(),
        "content"
    );
    assert_eq!(" note ".parse::<ElementType>().unwrap().as_ref(), "note");
    assert_eq!(" body ".parse::<Band>().unwrap().as_ref(), "body");

    assert_eq!(
        "  ".parse::<SurfaceName>(),
        Err(CompositionLabelError::Empty)
    );
    assert_eq!(
        "a\nb".parse::<ElementType>(),
        Err(CompositionLabelError::ControlCharacter)
    );
    assert_eq!(
        "x".repeat(LABEL_MAX_CHARS + 1).parse::<TabName>(),
        Err(CompositionLabelError::TooLong)
    );
    assert!("x".repeat(LABEL_MAX_CHARS).parse::<Band>().is_ok());
}

// The satellite type tags live in one place.
#[test]
fn the_satellite_type_tags_are_stable() {
    assert_eq!(ElementType::slot().as_ref(), ElementType::SLOT_TAG);
    assert_eq!(ElementType::seat().as_ref(), ElementType::SEAT_TAG);
    assert_eq!(ElementType::SLOT_TAG, "slot");
    assert_eq!(ElementType::SEAT_TAG, "seat");
}
