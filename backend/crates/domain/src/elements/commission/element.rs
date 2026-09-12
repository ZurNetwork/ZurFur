//! The commission's flat composition: typed elements contributed into
//! code-declared surfaces, grouped by tabs, with no parent pointers.
//!
//! Structure is code ([`SKELETON`]), modes are data. Effective visibility is
//! `min(tab, surface, element)` — [`effective_visibility`]. The raw composition
//! and its payloads implement no `serde::Serialize`, so content can only leave
//! through a projection that clamped it server-side.

use std::collections::HashMap;
use std::ops::Deref;

use serde::Deserialize;
use uuid::Uuid;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::CommissionId,
        id::{IdError, parse_uuid},
        user::UserId,
    },
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The app-private key of one element of a commission's composition (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct ElementId(uuid::Uuid);
pub type SeatId = ElementId;

impl ElementId {
    /// Wraps an already-minted UUID.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh UUIDv7 element key; also used by the satellite shapes that
    /// ride an element.
    pub(super) fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl Deref for ElementId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for ElementId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

impl From<Uuid> for ElementId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

/// The app-private key of one tab of a commission (UUIDv7). Tabs are the only
/// composition level with a row of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(uuid::Uuid);

impl TabId {
    /// Wraps an already-minted UUID.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh UUIDv7 tab key, as done when a commission's skeleton tabs
    /// are created alongside it.
    pub fn mint() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl Deref for TabId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for TabId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

impl TryFrom<Uuid> for TabId {
    type Error = IdError;

    fn try_from(value: Uuid) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}

/// Why a string was rejected as a composition label — a [`TabName`],
/// [`SurfaceName`], [`ElementType`], or [`Band`]. One error for all four: they
/// share one validation contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionLabelError {
    /// Empty once trimmed.
    Empty,
    /// Longer than [`LABEL_MAX_CHARS`] after trimming.
    TooLong,
    /// Contains a control character.
    ControlCharacter,
}

impl std::fmt::Display for CompositionLabelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "composition label must not be empty"),
            Self::TooLong => write!(
                f,
                "composition label must be at most {LABEL_MAX_CHARS} characters"
            ),
            Self::ControlCharacter => {
                write!(f, "composition label must not contain control characters")
            }
        }
    }
}

impl std::error::Error for CompositionLabelError {}

/// The shared length cap of every composition label, in characters.
pub const LABEL_MAX_CHARS: usize = 64;

/// The validation every composition label shares: trim, refuse empty, cap the
/// length, refuse control characters.
fn validate_label(raw: String) -> Result<String, CompositionLabelError> {
    StringBuilder::new(raw)
        .trimmed()
        .non_empty()
        .max_chars(LABEL_MAX_CHARS)
        .no_control()
        .build()
        .map_err(|violation| match violation {
            StringBuilderViolation::Empty => CompositionLabelError::Empty,
            StringBuilderViolation::TooLong { .. } => CompositionLabelError::TooLong,
            StringBuilderViolation::ControlCharacter => CompositionLabelError::ControlCharacter,
        })
}

/// One declared tab's stable name — the `commission_tab.tab` token, e.g.
/// `"main"`. A name, not a key: the row's key is [`TabId`], and which names are
/// legal is the [`SKELETON`]'s to say.
///
/// ```
/// use domain::elements::commission::TabName;
///
/// let tab = "  main  ".parse::<TabName>().unwrap();
/// assert_eq!(tab.as_str(), "main"); // trimmed
///
/// assert!("   ".parse::<TabName>().is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TabName(String);

impl TabName {
    /// The validated, trimmed name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TabName {
    type Error = CompositionLabelError;

    /// Validate and wrap a tab name under the shared label rules.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for TabName {
    type Err = CompositionLabelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for TabName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for TabName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One declared surface's stable name — the `commission_element.surface` token,
/// e.g. `"content"`. Surfaces have no rows; this type validates the label's
/// shape, while [`SKELETON`] owns the vocabulary.
///
/// ```
/// use domain::elements::commission::SurfaceName;
///
/// let surface = "  content  ".parse::<SurfaceName>().unwrap();
/// assert_eq!(surface.as_str(), "content"); // trimmed
///
/// assert!("a\nb".parse::<SurfaceName>().is_err()); // control character
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceName(String);

impl SurfaceName {
    /// The validated, trimmed name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SurfaceName {
    type Error = CompositionLabelError;

    /// Validate and wrap a surface name under the shared label rules. Whether
    /// the [`SKELETON`] declares it in a given tab is [`declares_surface`]'s
    /// separate question.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for SurfaceName {
    type Err = CompositionLabelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for SurfaceName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for SurfaceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An element's type tag — what the element is. An open vocabulary in v1: the
/// core stores and returns the tag and never interprets it. [`slot`](Self::slot)
/// and [`seat`](Self::seat) are the two tags already spoken for.
///
/// ```
/// use domain::elements::commission::ElementType;
///
/// assert_eq!("  note ".parse::<ElementType>().unwrap().as_str(), "note");
/// assert_eq!(ElementType::seat().as_str(), "seat");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementType(String);

impl ElementType {
    /// The tag of an element carrying a declared Slot; the `commission_slot`
    /// satellite shares the element's id.
    pub const SLOT_TAG: &'static str = "slot";
    /// The tag of an element carrying a declared Seat; the `commission_seat`
    /// satellite shares the element's id.
    pub const SEAT_TAG: &'static str = "seat";

    /// The type tag of a Slot-carrying element.
    pub fn slot() -> Self {
        Self(Self::SLOT_TAG.to_owned())
    }

    /// The type tag of a Seat-carrying element.
    pub fn seat() -> Self {
        Self(Self::SEAT_TAG.to_owned())
    }

    /// The validated, trimmed tag as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ElementType {
    type Error = CompositionLabelError;

    /// Validate and wrap a type tag under the shared label rules. No vocabulary
    /// check — the tag is open.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for ElementType {
    type Err = CompositionLabelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for ElementType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
/// assert_eq!(Band::default().as_str(), "body"); // the placeholder default
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Band(String);

impl Band {
    /// The one band that exists today — the placeholder every element is born into.
    pub const BODY: &'static str = "body";

    /// The validated, trimmed band as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Band {
    /// The placeholder band ([`BODY`](Self::BODY)), matching the
    /// `commission_element.band` column default.
    fn default() -> Self {
        Self(Self::BODY.to_owned())
    }
}

impl TryFrom<String> for Band {
    type Error = CompositionLabelError;

    /// Validate and wrap a band under the shared label rules. No vocabulary
    /// check — see the type's warning.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for Band {
    type Err = CompositionLabelError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for Band {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for Band {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How much of what sits behind it a viewer class may see; borne by all three
/// composition terms (tab, surface, element).
///
/// Declaration order IS the openness ladder and the derived [`Ord`] is
/// load-bearing for [`effective_visibility`] — **do not reorder the variants**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VisibilityMode {
    /// Participants only; the default of every term.
    Total,
    /// Title + existence only — the status-only card tier.
    Presentation,
    /// Description-designated content — the widest tier.
    Description,
}

impl VisibilityMode {
    /// Every mode, from most closed to most open.
    pub const ALL: &[VisibilityMode] = &[Self::Total, Self::Presentation, Self::Description];

    /// The stable, lowercase storage token written to the `mode` columns.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Presentation => "presentation",
            Self::Description => "description",
            Self::Total => "total",
        }
    }

    /// Resolve a stored token back to its mode, or `None` for one outside the
    /// vocabulary. Callers surface `None` as an error, never a silent default.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|mode| mode.as_str() == token)
    }
}

impl Default for VisibilityMode {
    /// [`Total`](Self::Total) — the closed door, matching every `mode` column's
    /// `DEFAULT 'total'` and the absent-surface-mode rule.
    fn default() -> Self {
        Self::Total
    }
}

/// The effective visibility of an element: `min(tab, surface, element)`.
/// Over-claiming is inert, never a leak. Composes under the commission's own
/// [`Visibility`](super::Visibility), which the caller gates on first.
///
/// ```
/// use domain::elements::commission::{VisibilityMode, effective_visibility};
///
/// // An element over-claiming under a closed surface stays closed.
/// let effective = effective_visibility(
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
        .find(|declared| declared.tab == tab.as_str())
        .is_some_and(|declared| declared.surfaces.contains(&surface.as_str()))
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
/// a compile error. Unwrapping ([`as_value`](Self::as_value) /
/// [`into_value`](Self::into_value)) exists for the adapters' `jsonb` binds,
/// never for a response path.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementPayload(serde_json::Value);

impl ElementPayload {
    /// The wrapped value, borrowed — the adapters' `jsonb` bind door.
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    /// The wrapped value, owned.
    pub fn into_value(self) -> serde_json::Value {
        self.0
    }
}

impl Default for ElementPayload {
    /// The empty object `{}`, matching the `commission_element.payload` column
    /// default — not `serde_json::Value`'s own `null` default.
    fn default() -> Self {
        Self(serde_json::Value::Object(serde_json::Map::new()))
    }
}

impl From<serde_json::Value> for ElementPayload {
    /// Wrap opaque JSON as an element payload — the one construction door.
    /// Nothing is validated; the core does not interpret payloads.
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

impl AsRef<serde_json::Value> for ElementPayload {
    fn as_ref(&self) -> &serde_json::Value {
        self.as_value()
    }
}

impl From<ElementPayload> for serde_json::Value {
    fn from(payload: ElementPayload) -> Self {
        payload.into_value()
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
    ///     TabId::new(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let element_type = "note".parse::<ElementType>().unwrap();
    /// let owner = UserId::new(Did::new("did:plc:alice".to_string()));
    /// let body = serde_json::json!({ "body": "hi" });
    /// let payload = ElementPayload::from(body.clone());
    ///
    /// let element =
    ///     NewElement::contributed(commission, address, element_type, payload, owner, Utc::now());
    /// assert_eq!(element.payload.as_value(), &body); // opaque, verbatim
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
        effective_visibility(tab, surface, element.mode)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::Utc;
    use serde_json::json;

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
            created_by: UserId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7()))),
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
            effective_visibility(
                VisibilityMode::Description,
                VisibilityMode::Total,
                VisibilityMode::Description,
            ),
            VisibilityMode::Total,
            "a Total surface closes the door however wide the element claims"
        );
        // A closed TAB clamps just as hard, one level further out.
        assert_eq!(
            effective_visibility(
                VisibilityMode::Total,
                VisibilityMode::Description,
                VisibilityMode::Description,
            ),
            VisibilityMode::Total,
        );
        // The element itself is the narrowest term: it can always close further.
        assert_eq!(
            effective_visibility(
                VisibilityMode::Description,
                VisibilityMode::Description,
                VisibilityMode::Presentation,
            ),
            VisibilityMode::Presentation,
        );
        // All three wide open is the only way anything reaches Description.
        assert_eq!(
            effective_visibility(
                VisibilityMode::Description,
                VisibilityMode::Description,
                VisibilityMode::Description,
            ),
            VisibilityMode::Description,
        );

        // Exhaustive: the result is never wider than ANY of its terms.
        for tab in VisibilityMode::ALL {
            for surface in VisibilityMode::ALL {
                for own in VisibilityMode::ALL {
                    let effective = effective_visibility(*tab, *surface, *own);
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
        assert_eq!(Band::default().as_str(), Band::BODY);

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
            if name.as_str() == declared.as_str() {
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
                    let declared = tab.surfaces.contains(&surface.as_str());
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
            tabs.iter().map(TabName::as_str).collect::<Vec<_>>(),
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
        for mode in VisibilityMode::ALL {
            let token = mode.as_str();
            assert!(seen.insert(token), "duplicate token {token:?}");
            assert_eq!(VisibilityMode::parse(token), Some(*mode));
        }
        assert_eq!(VisibilityMode::ALL.len(), 3, "exactly the three modes");
        assert_eq!(VisibilityMode::parse("wide-open"), None, "tampering");

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
        let owner = UserId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7())));
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
            carrier.payload.as_value(),
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

        assert_eq!(payload.as_value(), &raw, "carried verbatim");
        assert_eq!(payload.as_ref(), &raw, "the std borrow door agrees");
        assert_eq!(payload.clone().into_value(), raw, "and the owned one");
        assert_eq!(serde_json::Value::from(payload), raw, "as does From");

        assert_eq!(
            ElementPayload::default().as_value(),
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
        assert_eq!(" main ".parse::<TabName>().unwrap().as_str(), "main");
        assert_eq!(
            " content ".parse::<SurfaceName>().unwrap().as_str(),
            "content"
        );
        assert_eq!(" note ".parse::<ElementType>().unwrap().as_str(), "note");
        assert_eq!(" body ".parse::<Band>().unwrap().as_str(), "body");

        assert_eq!(
            "  ".parse::<SurfaceName>(),
            Err(CompositionLabelError::Empty)
        );
        assert_eq!(
            "a\nb".parse::<ElementType>(),
            Err(CompositionLabelError::ControlCharacter)
        );
        assert_eq!(
            TabName::try_from("x".repeat(LABEL_MAX_CHARS + 1)),
            Err(CompositionLabelError::TooLong)
        );
        assert!(Band::try_from("x".repeat(LABEL_MAX_CHARS)).is_ok());
    }

    // The satellite type tags live in one place.
    #[test]
    fn the_satellite_type_tags_are_stable() {
        assert_eq!(ElementType::slot().as_str(), ElementType::SLOT_TAG);
        assert_eq!(ElementType::seat().as_str(), ElementType::SEAT_TAG);
        assert_eq!(ElementType::SLOT_TAG, "slot");
        assert_eq!(ElementType::SEAT_TAG, "seat");
    }
}
