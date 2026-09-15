use chrono::Utc;

use super::super::{Band, ElementId, ElementPayload, SurfaceAddress};
use super::*;
use crate::elements::{did::Did, user::UserId};

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
