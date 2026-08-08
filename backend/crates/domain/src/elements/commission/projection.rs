//! The **viewer projection** — the one way a commission's composition leaves
//! the process (ZMVP-163; Flat Composition DD `45514754` D4, Boundary doctrine
//! `29622283`).
//!
//! [`CommissionComposition`](super::CommissionComposition) holds everything a
//! commission has, `Total`-tier content included, and deliberately implements no
//! `serde::Serialize` — so "serialize what was loaded" is a compile error. This
//! module supplies the alternative: [`CommissionComposition::project`], which
//! takes the tier the viewer stands at and returns a [`ProjectedComposition`]
//! holding **only what that tier admits**.
//!
//! Three properties are structural rather than remembered:
//!
//! 1. **The filter runs before the payload is ever readable as text.** A
//!    [`ProjectedElement`] carries its payload already rendered to a
//!    [`String`], and rendering happens *inside* the filter loop — so a caller
//!    that wants element content has no route to it except through a projection
//!    that has already applied [`effective_visibility`](super::effective_visibility).
//!    The unwrap doors ([`as_value`](super::ElementPayload::as_value) /
//!    [`into_value`](super::ElementPayload::into_value)) stay where they belong:
//!    the store adapters' `jsonb` binds, and — the one call in the api crate,
//!    verified 2026-08-08 — the *request*-side empty default in
//!    `routes::commissions::elements`, which unwraps a payload the caller just
//!    supplied rather than one that was ever stored. No response path unwraps
//!    anything; that is a claim about the code as it stands, not a rule the
//!    compiler enforces, so a reviewer adding a second call on a read path is
//!    the thing to watch for.
//! 2. **The viewer's tier cannot be confused with a thing's mode.**
//!    [`ViewerTier`] is a newtype, so passing an element's own
//!    [`VisibilityMode`] where the viewer's tier belongs — the transposition
//!    that would silently show everything to everyone — does not typecheck.
//! 3. **The skeleton is the authority on shape, and it is fail-closed.** Only
//!    tabs and surfaces the [`SKELETON`](super::SKELETON) declares are
//!    projected, and an element addressing a pair the skeleton does not declare
//!    is dropped rather than emitted unplaced. Rows that survive a skeleton
//!    change therefore go dark, never loose.
//!
//! **Nothing here renumbers or exposes an ordinal.** `VERSIONING.md` R3 forbids
//! a stored sequential key on the wire, and a *sparse* one would be worse than
//! forbidden: the gaps would count what the filter removed. The projection
//! emits elements in order and says nothing about where they sat.

use super::{
    CommissionComposition, ElementId, ElementType, SKELETON, SurfaceName, TabId, TabName,
    VisibilityMode,
};

/// The tier a viewer stands at on the openness ladder — the question
/// "how open must a thing be before this viewer may see it?".
///
/// A newtype over [`VisibilityMode`] on purpose. Both sides of the comparison
/// are modes, so a bare `VisibilityMode` parameter would accept an element's
/// *own* mode where the *viewer's* tier belongs, and that transposition fails
/// open — `effective >= effective` is always true, which is every element to
/// every caller. Wrapping the viewer's half makes the mistake a type error.
///
/// The ladder runs `Total` (participants only) < `Presentation` < `Description`
/// (the widest audience), so a *lower* tier admits *more*: a participant stands
/// at [`PARTICIPANT`](Self::PARTICIPANT) and is admitted to everything.
///
/// ```
/// use domain::elements::commission::{ViewerTier, VisibilityMode};
///
/// // A participant is admitted to every mode, including Total-tier content.
/// assert!(ViewerTier::PARTICIPANT.admits(VisibilityMode::Total));
///
/// // The widest audience is admitted only to what is Description-open.
/// let outsider = ViewerTier::at(VisibilityMode::Description);
/// assert!(outsider.admits(VisibilityMode::Description));
/// assert!(!outsider.admits(VisibilityMode::Presentation));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViewerTier(VisibilityMode);

impl ViewerTier {
    /// The tier of someone who holds a position in the commission: the bottom
    /// of the ladder, admitted to everything.
    ///
    /// Named rather than spelled `ViewerTier::at(VisibilityMode::Total)` at each
    /// call site, because "a Participant always receives the Total projection"
    /// is a rule worth being greppable — and because v1 serves no other tier
    /// (the outsider mapping from a commission's `visibility` is ZMVP-75's).
    pub const PARTICIPANT: Self = Self(VisibilityMode::Total);

    /// The tier standing at `mode` — the general constructor ZMVP-75 uses once
    /// it maps a commission's [`Visibility`](super::Visibility) and a seat's
    /// ceiling onto the ladder.
    pub const fn at(mode: VisibilityMode) -> Self {
        Self(mode)
    }

    /// Whether this viewer is admitted to something whose **effective**
    /// visibility is `effective` — i.e. whether that thing is at least as open
    /// as the tier the viewer stands at.
    ///
    /// The argument must be the effective mode
    /// ([`effective_visibility`](super::effective_visibility)), never a single
    /// term: comparing against an element's own mode would ignore the tab and
    /// surface clamps above it.
    pub fn admits(self, effective: VisibilityMode) -> bool {
        effective >= self.0
    }

    /// The mode this tier stands at.
    pub fn mode(self) -> VisibilityMode {
        self.0
    }
}

/// One tab in a viewer's projection.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProjectedTab {
    /// The tab's key — what a projected surface or element cites.
    pub id: TabId,
    /// The declared tab name.
    pub tab: TabName,
    /// The tab's own mode (the first term of the min), served so a client can
    /// explain what it sees. The filtering already happened.
    pub mode: VisibilityMode,
}

/// One declared surface in a viewer's projection.
///
/// Surfaces have no stored identity — their structure is the
/// [`SKELETON`](super::SKELETON)'s — so this is the skeleton's declaration
/// paired with the commission's mode for it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProjectedSurface {
    /// The declared surface id.
    pub surface: SurfaceName,
    /// The tab it lives in, **by id** — never a parent pointer.
    pub tab: TabId,
    /// The surface's per-commission mode (the second term of the min);
    /// [`VisibilityMode::Total`] when this commission never widened it.
    pub mode: VisibilityMode,
}

/// One element in a viewer's projection — the envelope plus its payload
/// **already rendered to canonical JSON text**.
///
/// The rendering is the point, not a convenience: it happens inside the filter,
/// so there is no ordering of operations in which a caller holds readable
/// element content that has not been cleared by
/// [`effective_visibility`](super::effective_visibility).
///
/// `#[non_exhaustive]` closes the back way in. Without it the fields are public,
/// so an api-side caller could assemble one from an unfiltered
/// [`ElementRow`](super::ElementRow) — rendering the payload itself and skipping
/// the clamp entirely — and the resulting value would be indistinguishable from
/// a projected one at the wire mapper. Outside this crate the struct now has no
/// literal form at all, so [`project`](CommissionComposition::project) is the
/// **only** source of one: not the intended path, the only path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProjectedElement {
    /// The element's key.
    pub id: ElementId,
    /// The tab it sits in, by id.
    pub tab: TabId,
    /// The declared surface it was contributed into.
    pub surface: SurfaceName,
    /// What it is — the type tag a renderer switches on, fail-closed on
    /// unknowns (ZMVP-170).
    pub kind: ElementType,
    /// The element's own mode (the third term of the min).
    pub mode: VisibilityMode,
    /// The type-owned payload as canonical JSON text. `"{}"` for an element
    /// that carries none, so "no payload" has one spelling here too.
    pub payload_json: String,
}

/// A commission's composition **as one viewer may see it** — the serializable
/// side of the boundary.
///
/// Ordered by the skeleton, not by storage: tabs in declaration order, surfaces
/// in declaration order within their tab, elements in the store's order within
/// their surface. Nothing carries a position, and nothing reveals that anything
/// was removed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ProjectedComposition {
    /// The tabs this viewer may see.
    pub tabs: Vec<ProjectedTab>,
    /// The surfaces this viewer may see, each inside an admitted tab.
    pub surfaces: Vec<ProjectedSurface>,
    /// The elements this viewer may see, in served order.
    pub elements: Vec<ProjectedElement>,
}

impl CommissionComposition {
    /// This composition as `viewer` may see it — **the only way its content
    /// leaves the domain**.
    ///
    /// For every element, `min(tab, surface, element)` is computed first
    /// ([`effective_visibility_of`](Self::effective_visibility_of), which itself
    /// answers a missing tab with the closed door) and the element is kept only
    /// if the viewer's tier admits that result. Tabs and surfaces are filtered
    /// on the same test against their own mode, so a tier that reaches no
    /// element in a tab is not told the tab is there either.
    ///
    /// Walking the [`SKELETON`](super::SKELETON) rather than the stored rows is
    /// what makes the shape fail-closed: a tab row or an element whose address
    /// the skeleton no longer declares is simply never reached, so a skeleton
    /// change retires content instead of leaking it into an undeclared place.
    ///
    /// ```
    /// use domain::elements::commission::{CommissionComposition, ViewerTier};
    ///
    /// // An empty composition projects to nothing for anybody.
    /// let empty = CommissionComposition {
    ///     tabs: Vec::new(),
    ///     surface_modes: Default::default(),
    ///     elements: Vec::new(),
    /// };
    /// assert!(empty.project(ViewerTier::PARTICIPANT).elements.is_empty());
    /// ```
    pub fn project(&self, viewer: ViewerTier) -> ProjectedComposition {
        let mut projected = ProjectedComposition::default();

        for declared in SKELETON {
            let Ok(name) = declared.tab.parse::<TabName>() else {
                continue;
            };
            let Some(row) = self.tabs.iter().find(|tab| tab.tab == name) else {
                continue;
            };
            if !viewer.admits(row.mode) {
                continue;
            }
            let tab = ProjectedTab {
                id: row.id,
                tab: name,
                mode: row.mode,
            };
            projected.tabs.push(tab);

            for declared_surface in declared.surfaces {
                let Ok(surface) = declared_surface.parse::<SurfaceName>() else {
                    continue;
                };
                let mode = self.surface_mode(&surface);
                if !viewer.admits(mode) {
                    continue;
                }
                let projected_surface = ProjectedSurface {
                    surface: surface.clone(),
                    tab: row.id,
                    mode,
                };
                projected.surfaces.push(projected_surface);

                let admitted = self.elements.iter().filter(|element| {
                    element.address.tab == row.id
                        && element.address.surface == surface
                        && viewer.admits(self.effective_visibility_of(element))
                });
                for element in admitted {
                    let projected_element = ProjectedElement {
                        id: element.id,
                        tab: element.address.tab,
                        surface: element.address.surface.clone(),
                        kind: element.element_type.clone(),
                        mode: element.mode,
                        payload_json: render_payload(&element.payload),
                    };
                    projected.elements.push(projected_element);
                }
            }
        }

        projected
    }
}

/// An element's payload as canonical JSON text.
///
/// `serde_json::to_string` of a [`Value`](serde_json::Value) is total in
/// practice — the failure arms (non-string map keys, non-finite floats) are
/// unrepresentable in the type — so the fallback is unreachable. It is the
/// **empty object** rather than a panic or an error because this runs on a read
/// path: a payload that somehow could not be rendered is served as "nothing",
/// never as a partial or a 500 that says something about the row.
fn render_payload(payload: &super::ElementPayload) -> String {
    serde_json::to_string(payload.as_value()).unwrap_or_else(|_| "{}".to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::elements::{
        commission::{Band, ElementPayload, ElementRow, SurfaceAddress, TabRow},
        user::UserId,
    };

    /// The skeleton's one declared pair today, reached through the const so
    /// these tests survive ZMVP-171 renaming it.
    fn declared_pair() -> (TabName, SurfaceName) {
        let tab = SKELETON[0].tab.parse().expect("valid label");
        let surface = SKELETON[0].surfaces[0].parse().expect("valid label");
        (tab, surface)
    }

    fn element(
        tab: TabId,
        surface: &SurfaceName,
        mode: VisibilityMode,
        payload: serde_json::Value,
    ) -> ElementRow {
        ElementRow {
            id: ElementId::new(uuid::Uuid::now_v7()),
            address: SurfaceAddress::new(tab, surface.clone()),
            element_type: "note".parse().expect("valid type"),
            mode,
            band: Band::default(),
            position: 0,
            created_by: UserId::new(uuid::Uuid::now_v7()),
            created_at: Utc::now(),
            payload: ElementPayload::from(payload),
        }
    }

    /// A composition with one skeleton tab at `tab_mode`, the declared surface
    /// at `surface_mode`, and the given elements.
    fn composition(
        tab_id: TabId,
        tab_mode: VisibilityMode,
        surface_mode: VisibilityMode,
        elements: Vec<ElementRow>,
    ) -> CommissionComposition {
        let (tab, surface) = declared_pair();
        CommissionComposition {
            tabs: vec![TabRow {
                id: tab_id,
                tab,
                mode: tab_mode,
            }],
            surface_modes: HashMap::from([(surface, surface_mode)]),
            elements,
        }
    }

    // THE closed-door proof at the projection level: an element the viewer's
    // tier does not admit is ABSENT — not emptied, not flagged, absent — and
    // its payload text never exists.
    #[test]
    fn an_element_below_the_viewers_tier_is_absent_entirely() {
        let tab_id = TabId::mint();
        let (_, surface) = declared_pair();
        let secret = json!({ "brief": "the private brief" });
        let public = json!({ "blurb": "the public blurb" });
        let composition = composition(
            tab_id,
            VisibilityMode::Description,
            VisibilityMode::Description,
            vec![
                element(tab_id, &surface, VisibilityMode::Total, secret),
                element(tab_id, &surface, VisibilityMode::Description, public),
            ],
        );

        let outsider = composition.project(ViewerTier::at(VisibilityMode::Description));
        assert_eq!(outsider.elements.len(), 1, "only the Description element");
        assert!(
            outsider.elements[0].payload_json.contains("public blurb"),
            "the admitted element keeps its payload"
        );
        assert!(
            !outsider
                .elements
                .iter()
                .any(|element| element.payload_json.contains("private brief")),
            "the withheld element's content is nowhere in the projection"
        );

        // The participant, standing at the bottom of the ladder, sees both.
        let participant = composition.project(ViewerTier::PARTICIPANT);
        assert_eq!(participant.elements.len(), 2);
    }

    // The min is what the filter tests, not the element's own mode: an element
    // claiming Description under a Total tab or surface is INERT, and an
    // outsider sees nothing at all.
    #[test]
    fn an_over_claiming_element_is_clamped_by_its_tab_and_surface() {
        let tab_id = TabId::mint();
        let (_, surface) = declared_pair();
        let over_claiming = || element(tab_id, &surface, VisibilityMode::Description, json!({}));
        let outsider = ViewerTier::at(VisibilityMode::Description);

        let closed_tab = composition(
            tab_id,
            VisibilityMode::Total,
            VisibilityMode::Description,
            vec![over_claiming()],
        )
        .project(outsider);
        assert!(
            closed_tab.elements.is_empty(),
            "a Total tab closes the door"
        );
        assert!(
            closed_tab.tabs.is_empty() && closed_tab.surfaces.is_empty(),
            "a tab the viewer cannot reach is not announced either"
        );

        let closed_surface = composition(
            tab_id,
            VisibilityMode::Description,
            VisibilityMode::Total,
            vec![over_claiming()],
        )
        .project(outsider);
        assert!(
            closed_surface.elements.is_empty(),
            "a Total surface closes the door"
        );
        assert!(
            closed_surface.surfaces.is_empty(),
            "and the surface itself is not announced"
        );
        assert_eq!(
            closed_surface.tabs.len(),
            1,
            "its tab is open, so the tab still shows"
        );
    }

    // Fail-closed on shape: an element addressing a pair the skeleton does not
    // declare is never projected, so a skeleton change retires content instead
    // of leaking it into a place nothing describes.
    #[test]
    fn an_element_outside_the_skeleton_is_never_projected() {
        let tab_id = TabId::mint();
        let (_, declared) = declared_pair();
        let undeclared = "not-a-declared-surface"
            .parse::<SurfaceName>()
            .expect("valid label");
        assert_ne!(undeclared, declared, "the fixture must be undeclared");

        let stray = element(
            tab_id,
            &undeclared,
            VisibilityMode::Description,
            json!({ "leak": true }),
        );
        let projected = composition(
            tab_id,
            VisibilityMode::Description,
            VisibilityMode::Description,
            vec![stray],
        )
        .project(ViewerTier::PARTICIPANT);

        assert!(
            projected.elements.is_empty(),
            "an undeclared address is dropped even for a participant"
        );

        // A tab row the skeleton does not declare is dropped the same way.
        let (_, surface) = declared_pair();
        let orphan_tab = TabId::mint();
        let orphaned = CommissionComposition {
            tabs: vec![TabRow {
                id: orphan_tab,
                tab: "retired-tab".parse().expect("valid label"),
                mode: VisibilityMode::Description,
            }],
            surface_modes: HashMap::from([(surface.clone(), VisibilityMode::Description)]),
            elements: vec![element(
                orphan_tab,
                &surface,
                VisibilityMode::Description,
                json!({}),
            )],
        };
        let projected = orphaned.project(ViewerTier::PARTICIPANT);
        assert!(projected.tabs.is_empty() && projected.elements.is_empty());
    }

    // An absent surface-mode row means Total — so a commission nobody widened
    // shows an outsider nothing, by having said nothing.
    #[test]
    fn a_never_widened_surface_is_closed_to_outsiders() {
        let tab_id = TabId::mint();
        let (tab, surface) = declared_pair();
        let never_widened = CommissionComposition {
            tabs: vec![TabRow {
                id: tab_id,
                tab,
                mode: VisibilityMode::Description,
            }],
            surface_modes: HashMap::new(),
            elements: vec![element(
                tab_id,
                &surface,
                VisibilityMode::Description,
                json!({}),
            )],
        };

        let outsider = never_widened.project(ViewerTier::at(VisibilityMode::Description));
        assert!(outsider.surfaces.is_empty() && outsider.elements.is_empty());

        let participant = never_widened.project(ViewerTier::PARTICIPANT);
        assert_eq!(
            participant.surfaces.len(),
            1,
            "the participant still sees it"
        );
        assert_eq!(participant.elements.len(), 1);
    }

    // The payload crosses verbatim, including an integer above 2^53 — the
    // regression the opaque-JSON-string form exists to prevent (DD 42762241
    // D4: Struct floats it on one tier and truncates it on the other).
    #[test]
    fn the_payload_survives_beyond_two_to_the_fifty_third() {
        let tab_id = TabId::mint();
        let (_, surface) = declared_pair();
        let huge = "9007199254740993"; // 2^53 + 1
        let payload = serde_json::from_str::<serde_json::Value>(&format!(
            "{{\"big\":{huge},\"text\":\"三毛猫 🐾\"}}"
        ))
        .expect("valid json");
        let composition = composition(
            tab_id,
            VisibilityMode::Description,
            VisibilityMode::Description,
            vec![element(tab_id, &surface, VisibilityMode::Total, payload)],
        );

        let projected = composition.project(ViewerTier::PARTICIPANT);
        let rendered = &projected.elements[0].payload_json;
        assert!(
            rendered.contains(huge),
            "the integer must survive as digits, not as a float: {rendered}"
        );
        assert!(rendered.contains("三毛猫 🐾"), "and so must the text");

        // An element with no payload renders the empty object, one spelling.
        let empty = element(tab_id, &surface, VisibilityMode::Total, json!({}));
        assert_eq!(render_payload(&empty.payload), "{}");
    }

    // Order is the skeleton's, and no ordinal escapes: the projected element
    // carries nothing a client could count gaps in (R3).
    #[test]
    fn elements_are_served_in_skeleton_order_carrying_no_ordinal() {
        let tab_id = TabId::mint();
        let (_, surface) = declared_pair();
        let mut first = element(tab_id, &surface, VisibilityMode::Total, json!({ "n": 1 }));
        first.position = 7; // a sparse stored ordinal, as a removal would leave
        let mut second = element(tab_id, &surface, VisibilityMode::Total, json!({ "n": 2 }));
        second.position = 9;
        let ids = [first.id, second.id];

        let projected = composition(
            tab_id,
            VisibilityMode::Description,
            VisibilityMode::Description,
            vec![first, second],
        )
        .project(ViewerTier::PARTICIPANT);

        assert_eq!(
            projected
                .elements
                .iter()
                .map(|element| element.id)
                .collect::<Vec<_>>(),
            ids,
            "served order is the store's order within the surface"
        );
        assert_eq!(projected.surfaces.len(), 1);
        assert_eq!(
            projected.surfaces[0].tab, tab_id,
            "surfaces cite tabs by id"
        );
    }

    // The tier newtype: a participant is admitted to everything, and the
    // comparison is against the EFFECTIVE mode.
    #[test]
    fn the_viewer_tier_ladder_admits_downward() {
        for mode in VisibilityMode::ALL {
            assert!(
                ViewerTier::PARTICIPANT.admits(*mode),
                "a participant is admitted to {mode:?}"
            );
        }
        assert_eq!(ViewerTier::PARTICIPANT.mode(), VisibilityMode::Total);

        let presentation = ViewerTier::at(VisibilityMode::Presentation);
        assert!(!presentation.admits(VisibilityMode::Total));
        assert!(presentation.admits(VisibilityMode::Presentation));
        assert!(presentation.admits(VisibilityMode::Description));
    }
}
