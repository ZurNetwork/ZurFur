//! The commission's flat composition: typed elements contributed into
//! code-declared surfaces, grouped by tabs, with no parent pointers.
//!
//! Structure is code ([`SKELETON`]), modes are data. Effective visibility is
//! `min(tab, surface, element)` — [`effective_visibility`]. The raw composition
//! and its payloads implement no `serde::Serialize`, so content can only leave
//! through a projection that clamped it server-side.

use std::{collections::HashMap, str::FromStr};

use serde::Deserialize;

use crate::{
    datetime::DateTimeUtc,
    elements::{commission::CommissionId, user::UserId},
    string_builder::{StringBuilder, StringBuilderViolation},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct CompositionLabel(String);
impl TryFrom<String> for CompositionLabel {
    type Error = CompositionLabelError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let label = StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(LABEL_MAX_CHARS)
            .no_control()
            .build()
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => CompositionLabelError::Empty,
                StringBuilderViolation::TooLong { .. } => CompositionLabelError::TooLong,
                StringBuilderViolation::ControlCharacter => CompositionLabelError::ControlCharacter,
            })?;

        Ok(Self(label))
    }
}
impl FromStr for CompositionLabel {
    type Err = CompositionLabelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The app-private key of one element of a commission's composition (UUIDv7).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
    derive_more::Into,
)]
#[serde(transparent)]
pub struct ElementId(uuid::Uuid);
pub type SeatId = ElementId;

impl ElementId {
    /// Mint a fresh UUIDv7 element key; also used by the satellite shapes that
    /// ride an element.
    pub(super) fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

/// The app-private key of one tab of a commission (UUIDv7). Tabs are the only
/// composition level with a row of their own.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    derive_more::From,
    derive_more::Display,
    derive_more::FromStr,
    derive_more::AsRef,
    derive_more::Into,
)]
pub struct TabId(uuid::Uuid);

impl TabId {
    /// Mint a fresh UUIDv7 tab key, as done when a commission's skeleton tabs
    /// are created alongside it.
    pub fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

/// Why a string was rejected as a composition label — a [`TabName`],
/// [`SurfaceName`], [`ElementType`], or [`Band`]. One error for all four: they
/// share one validation contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompositionLabelError {
    /// Empty once trimmed.
    #[error("composition label must not be empty")]
    Empty,
    /// Longer than [`LABEL_MAX_CHARS`] after trimming.
    #[error("composition label must be at most {LABEL_MAX_CHARS} characters")]
    TooLong,
    /// Contains a control character.
    #[error("composition label must not contain control characters")]
    ControlCharacter,
}

/// The shared length cap of every composition label, in characters.
pub const LABEL_MAX_CHARS: usize = 64;

/// One declared tab's stable name — the `commission_tab.tab` token, e.g.
/// `"main"`. A name, not a key: the row's key is [`TabId`], and which names are
/// legal is the [`SKELETON`]'s to say.
///
/// ```
/// use domain::elements::commission::TabName;
///
/// let tab = "  main  ".parse::<TabName>().unwrap();
/// assert_eq!(tab.as_ref(), "main"); // trimmed
///
/// assert!("   ".parse::<TabName>().is_err()); // empty after trim
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct TabName(CompositionLabel);

/// One declared surface's stable name — the `commission_element.surface` token,
/// e.g. `"content"`. Surfaces have no rows; this type validates the label's
/// shape, while [`SKELETON`] owns the vocabulary.
///
/// ```
/// use domain::elements::commission::SurfaceName;
///
/// let surface = "  content  ".parse::<SurfaceName>().unwrap();
/// assert_eq!(surface.as_ref(), "content"); // trimmed
///
/// assert!("a\nb".parse::<SurfaceName>().is_err()); // control character
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct SurfaceName(CompositionLabel);

/// An element's type tag — what the element is. An open vocabulary in v1: the
/// core stores and returns the tag and never interprets it. [`slot`](Self::slot)
/// and [`seat`](Self::seat) are the two tags already spoken for.
///
/// ```
/// use domain::elements::commission::ElementType;
///
/// assert_eq!("  note ".parse::<ElementType>().unwrap().as_ref(), "note");
/// assert_eq!(ElementType::seat().as_ref(), "seat");
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct ElementType(CompositionLabel);

impl ElementType {
    /// The tag of an element carrying a declared Slot; the `commission_slot`
    /// satellite shares the element's id.
    pub const SLOT_TAG: &'static str = "slot";
    /// The tag of an element carrying a declared Seat; the `commission_seat`
    /// satellite shares the element's id.
    pub const SEAT_TAG: &'static str = "seat";

    /// The type tag of a Slot-carrying element.
    pub fn slot() -> Self {
        Self::SLOT_TAG.parse().expect("a valid label")
    }

    /// The type tag of a Seat-carrying element.
    pub fn seat() -> Self {
        Self::SEAT_TAG.parse().expect("a valid label")
    }
}

/// An element's ordering band within a surface: the run its `position` is
/// counted in, so one surface can hold several independently-ordered sequences.
///
/// ⚠️ Placeholder vocabulary — everything is born [`Band::default`] (`"body"`).
/// Do not add a vocabulary check here ahead of the type-catalog decision.
///
/// ```
/// use domain::elements::commission::Band;
///
/// assert_eq!(Band::default().as_ref(), "body"); // the placeholder default
/// ```
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    derive_more::Display,
    derive_more::AsRef,
    derive_more::FromStr,
)]
#[as_ref(str)]
pub struct Band(CompositionLabel);

impl Band {
    /// The one band that exists today — the placeholder every element is born into.
    pub const BODY: &'static str = "body";
}

impl Default for Band {
    /// The placeholder band ([`BODY`](Self::BODY)), matching the
    /// `commission_element.band` column default.
    fn default() -> Self {
        Self::BODY.parse().expect("a valid label")
    }
}

/// How much of what sits behind it a viewer class may see; borne by all three
/// composition terms (tab, surface, element).
///
/// Declaration order IS the openness ladder and the derived [`Ord`] is
/// load-bearing for [`effective_visibility`] — **do not reorder the variants**.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum VisibilityMode {
    /// Participants only; the default of every term.
    Total,
    /// Title + existence only — the status-only card tier.
    Presentation,
    /// Description-designated content — the widest tier.
    Description,
}

impl Default for VisibilityMode {
    /// [`Total`](Self::Total) — the closed door, matching every `mode` column's
    /// `DEFAULT 'total'` and the absent-surface-mode rule.
    fn default() -> Self {
        Self::Total
    }
}

impl VisibilityMode {
    ///
    /// The effective visibility of an element: `min(tab, surface, element)`.
    /// Over-claiming is inert, never a leak. Composes under the commission's own
    /// [`Visibility`](super::Visibility), which the caller gates on first.
    ///
    /// ```
    /// use domain::elements::commission::VisibilityMode;
    ///
    /// // An element over-claiming under a closed surface stays closed.
    /// let effective = VisibilityMode::effective_visibility(
    ///     VisibilityMode::Description,
    ///     VisibilityMode::Total,
    ///     VisibilityMode::Description,
    /// );
    /// assert_eq!(effective, VisibilityMode::Total);
    /// ```
    pub fn effective_visibility(
        tab: VisibilityMode,
        surface: VisibilityMode,
        element: VisibilityMode,
    ) -> VisibilityMode {
        tab.min(surface).min(element)
    }
}

/// One tab declared by the [`SKELETON`]: its stable name and the surfaces that
/// live in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredTab {
    /// The tab's stable id — the token minted into `commission_tab.tab`.
    pub tab: &'static str,
    /// The surfaces declared inside this tab, in render order.
    pub surfaces: &'static [&'static str],
}

/// The code-declared composition skeleton: which tabs exist, and which surfaces
/// live in each. Global and invariant — no write creates, renames, or removes a
/// surface.
///
/// ⚠️ Placeholder scaffolding, not a decision: the real skeleton is the type
/// catalog's. The names below carry no meaning worth preserving.
pub const SKELETON: &[DeclaredTab] = &[DeclaredTab {
    tab: "main",
    surfaces: &["content"],
}];

/// Whether the skeleton declares `surface` inside `tab` — the fail-closed
/// vocabulary check both adapters run before writing an element
/// ([`UnknownSurface`](crate::ports::UnknownSurface)).
///
/// The (tab, surface) pair is the unit: a surface declared under another tab is
/// refused exactly like an invented name.
pub fn declares_surface(tab: &TabName, surface: &SurfaceName) -> bool {
    SKELETON
        .iter()
        .find(|declared| declared.tab == tab.as_ref())
        .is_some_and(|declared| declared.surfaces.contains(&surface.as_ref()))
}

/// Every tab the skeleton declares, as validated [`TabName`]s — what
/// [`CommissionWrites::create`](crate::ports::CommissionWrites::create) mints a
/// row for, so a commission's tab state exists explicitly from birth.
///
/// Panics if the skeleton holds a malformed label (a programming error in the
/// const above).
pub fn declared_tabs() -> Vec<TabName> {
    SKELETON
        .iter()
        .map(|declared| {
            declared
                .tab
                .parse::<TabName>()
                .expect("SKELETON declares a malformed tab name")
        })
        .collect()
}

/// Where an element sits: the tab (by [`TabId`]) and the declared surface (by
/// [`SurfaceName`]) it is contributed into. The whole addressing model — there
/// is no parent, path or chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceAddress {
    /// The tab the element sits in, by id — the composite foreign key that
    /// binds it to one commission.
    pub tab: TabId,
    /// The declared surface within that tab. Refused with
    /// [`UnknownSurface`](crate::ports::UnknownSurface) unless
    /// [`declares_surface`] admits the pair.
    pub surface: SurfaceName,
}

impl SurfaceAddress {
    /// The address naming `surface` inside `tab`.
    pub fn new(tab: TabId, surface: SurfaceName) -> Self {
        Self { tab, surface }
    }
}

/// The type-owned half of an element: opaque JSON the core stores and returns
/// without interpreting it.
///
/// Implements no `serde::Serialize`, so putting element content on a response is
/// a compile error. Unwrapping exists for the adapters' `jsonb` binds, never for
/// a response path: borrow it via [`AsRef`], or take it by value via [`Into`].
#[derive(
    Debug,
    Clone,
    PartialEq,
    derive_more::From,
    derive_more::Into,
    derive_more::Display,
    derive_more::AsRef,
)]
pub struct ElementPayload(serde_json::Value);

impl Default for ElementPayload {
    /// The empty object `{}`, matching the `commission_element.payload` column
    /// default — not `serde_json::Value`'s own `null` default.
    fn default() -> Self {
        Self(serde_json::Value::Object(serde_json::Map::new()))
    }
}

/// A freshly contributed element, ready to persist
/// ([`CommissionWrites::add_element`](crate::ports::CommissionWrites::add_element)).
///
/// Every element is born [`VisibilityMode::Total`] — there is no mode parameter,
/// so widening is always a separate act. `position` is assigned by the store
/// in-transaction, inside the band.
#[derive(Debug)]
pub struct NewElement {
    /// The element key (UUIDv7).
    pub id: ElementId,
    /// The commission this element is contributed to.
    pub commission_id: CommissionId,
    /// Where it sits: the (tab, surface) pair.
    pub address: SurfaceAddress,
    /// What the element is — the open type tag.
    pub element_type: ElementType,
    /// The ordering band the element's position is counted in.
    pub band: Band,
    /// The type-owned payload, opaque to the core.
    pub payload: ElementPayload,
    /// The acting User.
    pub created_by: UserId,
    /// When the element was contributed.
    pub created_at: DateTimeUtc,
}

impl NewElement {
    /// A new element contributed at `address`, carrying `payload` verbatim and
    /// born in the placeholder [`Band`]. Mints the element id; authority, the
    /// tab's existence and the surface's declaration are settled on persist.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{
    ///         Band, CommissionId, ElementPayload, ElementType, NewElement, SurfaceAddress,
    ///         SurfaceName, TabId,
    ///     },
    ///     did::Did,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let address = SurfaceAddress::new(
    ///     TabId::from(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let element_type = "note".parse::<ElementType>().unwrap();
    /// let owner = UserId::from(Did::from("did:plc:alice".to_string()));
    /// let body = serde_json::json!({ "body": "hi" });
    /// let payload = ElementPayload::from(body.clone());
    ///
    /// let element =
    ///     NewElement::contributed(commission, address, element_type, payload, owner, Utc::now());
    /// assert_eq!(element.payload.as_ref(), &body); // opaque, verbatim
    /// assert_eq!(element.band, Band::default()); // the placeholder band
    /// ```
    pub fn contributed(
        commission: CommissionId,
        address: SurfaceAddress,
        element_type: ElementType,
        payload: ElementPayload,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: ElementId::mint(),
            commission_id: commission,
            address,
            element_type,
            band: Band::default(),
            payload,
            created_by,
            created_at: now,
        }
    }

    /// The element that carries an identity-sharing satellite (a declared Slot
    /// or Seat): takes the satellite's already-minted `id` — one identity, two
    /// rows — and gives it the empty payload.
    pub fn carrying(
        id: ElementId,
        commission: CommissionId,
        address: SurfaceAddress,
        element_type: ElementType,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id,
            commission_id: commission,
            address,
            element_type,
            band: Band::default(),
            payload: ElementPayload::default(),
            created_by,
            created_at: now,
        }
    }
}

/// One stored element as read back — the adapter-neutral row shape of
/// [`CommissionComposition::elements`]. Deliberately not `Serialize`.
#[derive(Debug)]
pub struct ElementRow {
    /// The element's key.
    pub id: ElementId,
    /// Where it sits: the (tab, surface) pair it was contributed at.
    pub address: SurfaceAddress,
    /// What it is — the open type tag.
    pub element_type: ElementType,
    /// Its own visibility mode: the third term of [`effective_visibility`].
    pub mode: VisibilityMode,
    /// The ordering band its `position` is counted in.
    pub band: Band,
    /// Order within `(tab, surface, band)`, ascending and contiguous from 0.
    pub position: i32,
    /// Who contributed it.
    pub created_by: UserId,
    /// When it was contributed.
    pub created_at: DateTimeUtc,
    /// The type-owned payload, opaque to the core.
    pub payload: ElementPayload,
}

/// One stored tab as read back: its key, its declared name, and its mode — the
/// first term of [`effective_visibility`].
#[derive(Debug)]
pub struct TabRow {
    /// The tab's key — what an element's `tab` cites.
    pub id: TabId,
    /// The declared tab id this row realizes (a [`SKELETON`] name).
    pub tab: TabName,
    /// The tab's visibility mode.
    pub mode: VisibilityMode,
}

/// A commission's whole loaded composition — every tab, every widened surface
/// mode, and every element
/// ([`CommissionStore::load_composition`](crate::ports::CommissionStore::load_composition)).
/// All three travel together because [`effective_visibility`] needs all three
/// terms.
///
/// **Never add a `Serialize` derive here.** It holds `Total`-tier content;
/// serialization exists only on a viewer projection that has already applied
/// [`effective_visibility_of`](Self::effective_visibility_of).
#[derive(Debug)]
pub struct CommissionComposition {
    /// The commission's tabs (its skeleton rows), ordered by declared name.
    pub tabs: Vec<TabRow>,
    /// The per-commission surface-mode overrides; sparse — an absent entry
    /// means [`VisibilityMode::Total`].
    pub surface_modes: HashMap<SurfaceName, VisibilityMode>,
    /// Every element, ordered by `(tab, surface, band, position)`.
    pub elements: Vec<ElementRow>,
}

impl CommissionComposition {
    /// The mode of the tab `id` names, or `None` if this composition holds no
    /// such tab (corruption, not a supported case).
    pub fn tab_mode(&self, id: TabId) -> Option<VisibilityMode> {
        self.tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.mode)
    }

    /// The mode of `surface` for this commission — the override if one was
    /// written, else [`VisibilityMode::Total`].
    pub fn surface_mode(&self, surface: &SurfaceName) -> VisibilityMode {
        self.surface_modes
            .get(surface)
            .copied()
            .unwrap_or(VisibilityMode::Total)
    }

    /// The effective visibility of one of this composition's elements —
    /// [`effective_visibility`] with the three terms resolved from here. An
    /// element whose tab is absent projects [`VisibilityMode::Total`].
    pub fn effective_visibility_of(&self, element: &ElementRow) -> VisibilityMode {
        let tab = self
            .tab_mode(element.address.tab)
            .unwrap_or(VisibilityMode::Total);
        let surface = self.surface_mode(&element.address.surface);
        VisibilityMode::effective_visibility(tab, surface, element.mode)
    }
}

#[cfg(test)]
mod tests;
