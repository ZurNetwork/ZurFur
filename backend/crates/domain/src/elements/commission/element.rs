//! The commission's **flat composition** (ZMVP-166; Flat Composition DD
//! `45514754`, amended 2026-08-04): typed [`Element`](ElementRow)s contributed
//! into **core-declared Surfaces**, grouped by **Tabs**, with no parent pointers
//! anywhere.
//!
//! This replaces the recursive Surface/Component tree (ZMVP-71/72/73; Tree
//! Storage DD `28409880`) wholesale. Depth is now fixed by the *model* rather
//! than by data:
//!
//! ```text
//! commission (visibility — the formal root)
//! └─ tab            per-commission row, carries a mode
//!    └─ surface     CODE-declared, global and invariant; its per-commission
//!                   mode is data (absent = Total)
//!       └─ element  a typed leaf: envelope + opaque payload
//! ```
//!
//! Effective visibility is therefore three lookups of **fixed arity** —
//! [`effective_visibility`] = `min(tab.mode, surface_mode, element.mode)`, under
//! the commission's own [`Visibility`](super::Visibility) — instead of a
//! min-of-ancestors walk up an unbounded chain. Orphans, cycles, depth caps and
//! detached subtrees are not guarded against here; they are unrepresentable.
//!
//! **Structure is code, modes are data.** [`SKELETON`] declares which tabs exist
//! and which surfaces live in each. It is global and invariant: no commission
//! has a different set, no write creates or removes a surface, and an element
//! naming a **(tab, surface) pair** the skeleton does not declare is refused
//! ([`UnknownSurface`](crate::ports::UnknownSurface)). Only the *modes* — the
//! tab's row, the surface's optional override — are per-commission data.
//!
//! **The raw composition never serializes.** [`CommissionComposition`],
//! [`ElementRow`] and [`ElementPayload`] deliberately do **not** implement
//! `serde::Serialize`, so "serialize what was loaded" is a compile error, not a
//! review catch. The payload — the only *content* an element carries — is
//! wrapped in [`ElementPayload`] the moment it enters the domain and is
//! unwrapped only at a store's SQL boundary, so content leaves the server only
//! through a viewer projection that has applied [`effective_visibility`]
//! server-side (ZMVP-170). Err closed, by construction — and pinned by
//! `serialization_is_unrepresentable_for_the_raw_composition` below.

use std::collections::HashMap;
use std::ops::Deref;

use crate::{
    datetime::DateTimeUtc,
    elements::{commission::CommissionId, user::UserId},
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The app-private, stable handle for one **element** of a commission's
/// composition.
///
/// A UUIDv7 wrapped for type safety, mirroring [`CommissionId`]: the app mints
/// the key, the domain only names it. `Deref` exposes the inner UUID for foreign
/// keys and lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(uuid::Uuid);

impl ElementId {
    /// Wraps an already-minted UUID — e.g. a row read back from the store, or a
    /// client-supplied id being resolved.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh UUIDv7 element key. Shared with the satellite shapes that
    /// ride an element ([`NewSlot`](super::NewSlot), [`NewSeat`](super::NewSeat)),
    /// which is why the seed lives here rather than in each constructor.
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

/// The app-private, stable handle for one **tab** of a commission.
///
/// Tabs are the only composition level with a row of their own — their mode is
/// per-commission data, and elements must be able to cite one by key. UUIDv7,
/// minted with the commission (or by the ZMVP-166 backfill, which mints v4 for
/// commissions that predate the model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(uuid::Uuid);

impl TabId {
    /// Wraps an already-minted UUID (a row read back, or a client-supplied id
    /// being resolved).
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh UUIDv7 tab key — used when a commission's skeleton tabs are
    /// minted alongside it
    /// ([`CommissionWrites::create`](crate::ports::CommissionWrites::create)).
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

/// Why a string was rejected as one of the composition's **labels** — a
/// [`TabName`], [`SurfaceName`], [`ElementType`], or [`Band`].
///
/// Deliberately **one** error type across the four rather than four identical
/// copies: they share a single validation contract (trimmed, non-empty, capped,
/// no control characters) because they are all the same kind of value — a stable
/// vocabulary token stored in a `text` column — and a divergence between them
/// would be a bug, not a feature. Sibling value objects with genuinely different
/// rules (`SeatKind`, `SeatPrompt`, …) keep their own errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionLabelError {
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than the label's cap ([`LABEL_MAX_CHARS`]) after trimming.
    TooLong,
    /// Contains a control character (newline, tab, NUL, …) — a label is a token,
    /// not a message.
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

/// The shared length cap of every composition label, in characters. Generous for
/// any vocabulary token the type catalog (ZMVP-171) might mint, tight enough that
/// a label stays a label.
pub const LABEL_MAX_CHARS: usize = 64;

/// The one validation body every composition label shares: trim, refuse empty,
/// cap the length, refuse control characters. Named once so the four newtypes
/// cannot drift apart on what a label is.
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

/// One declared **tab**'s stable id — the `commission_tab.tab` token, e.g.
/// `"main"`.
///
/// A *name*, not a key: the row's key is [`TabId`]. Which names are legal is the
/// [`SKELETON`]'s to say; this type only pins the shape a label must have.
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

    /// Validate and wrap a tab name under the shared label rules — trimmed,
    /// non-empty, at most [`LABEL_MAX_CHARS`], no control characters.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for TabName {
    type Err = CompositionLabelError;

    /// The std parsing door: `"…".parse::<TabName>()?` (ruling R6).
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

/// One declared **surface**'s stable id — the `commission_element.surface` token,
/// e.g. `"content"`.
///
/// This is how an element addresses the surface it is contributed into: **by id,
/// never by a parent pointer**. Surfaces have no rows, so there is nothing to
/// point at — the [`SKELETON`] is the authority on which ids exist *and on which
/// tab each lives in*, and a (tab, surface) pair outside it is refused at the
/// store ([`UnknownSurface`](crate::ports::UnknownSurface)). Validating the
/// *shape* here and the *vocabulary* there keeps a malformed label from ever
/// reaching a query.
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

    /// Validate and wrap a surface name under the shared label rules — trimmed,
    /// non-empty, at most [`LABEL_MAX_CHARS`], no control characters. Whether
    /// the [`SKELETON`] declares this surface *in a given tab* is a **separate**
    /// question, settled by [`declares_surface`].
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for SurfaceName {
    type Err = CompositionLabelError;

    /// The std parsing door: `"…".parse::<SurfaceName>()?` (ruling R6).
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

/// An element's **type tag** — what the element *is*, from the type catalog.
///
/// The catalog itself is deliberately deferred (ZMVP-171), so v1 is an **open**
/// vocabulary: the core stores and returns the tag and never interprets it, the
/// way it never interprets the payload. Two tags are already spoken for by the
/// satellites this ticket carries — [`slot`](Self::slot) and [`seat`](Self::seat)
/// — because those elements have interpreted data hanging off their id.
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
    /// The tag borne by the element that carries a declared **Slot** (ZMVP-77):
    /// its `commission_slot` satellite shares the element's id.
    pub const SLOT_TAG: &'static str = "slot";
    /// The tag borne by the element that carries a declared **Seat** (ZMVP-76):
    /// its `commission_seat` satellite shares the element's id.
    pub const SEAT_TAG: &'static str = "seat";

    /// The type tag of a Slot-carrying element — one place the token lives, so
    /// the domain, both adapters, and the routes cannot spell it differently.
    pub fn slot() -> Self {
        Self(Self::SLOT_TAG.to_owned())
    }

    /// The type tag of a Seat-carrying element — see [`slot`](Self::slot).
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

    /// Validate and wrap a type tag under the shared label rules — trimmed,
    /// non-empty, at most [`LABEL_MAX_CHARS`], no control characters. No
    /// vocabulary check: the catalog is ZMVP-171's.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for ElementType {
    type Err = CompositionLabelError;

    /// The std parsing door: `"…".parse::<ElementType>()?` (ruling R6).
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

/// An element's **ordering band** within a surface: the run its `position` is
/// counted in, so one surface can hold several independently-ordered sequences.
///
/// ⚠️ **PLACEHOLDER VOCABULARY.** The band vocabulary is *undecided*, pending the
/// type-catalog DD (ZMVP-171) — Engineer ruling 2026-08-04. The column is
/// reserved now so that decision costs no migration; until it lands, everything
/// is born in [`Band::default`] (`"body"`) and no surface anywhere offers a way
/// to choose another. Do not grow a vocabulary check here ahead of the DD.
///
/// ```
/// use domain::elements::commission::Band;
///
/// assert_eq!(Band::default().as_str(), "body"); // the placeholder default
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Band(String);

impl Band {
    /// The one band that exists today — the placeholder every element is born
    /// into. See the type's warning: this is scaffolding, not a decision.
    pub const BODY: &'static str = "body";

    /// The validated, trimmed band as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Band {
    /// The placeholder band ([`BODY`](Self::BODY)) — matching the
    /// `commission_element.band` column default, so the two cannot disagree
    /// about what "unspecified" means.
    fn default() -> Self {
        Self(Self::BODY.to_owned())
    }
}

impl TryFrom<String> for Band {
    type Error = CompositionLabelError;

    /// Validate and wrap a band under the shared label rules — trimmed,
    /// non-empty, at most [`LABEL_MAX_CHARS`], no control characters.
    /// Deliberately no vocabulary check — see the type's warning.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        validate_label(raw).map(Self)
    }
}

impl std::str::FromStr for Band {
    type Err = CompositionLabelError;

    /// The std parsing door: `"…".parse::<Band>()?` (ruling R6).
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

/// How much of what sits behind it a viewer class may see — the successor of the
/// tree's `SurfaceMode`, now borne by all three composition terms (a tab, a
/// surface, an element) rather than by surfaces alone.
///
/// **Declaration order IS the openness ladder**, and the derived [`Ord`] is what
/// makes that load-bearing: `Total` (nobody outside) < `Presentation` (a
/// status-only card) < `Description` (composed content). So the min of the three
/// terms — [`effective_visibility`] — is the closed-door clamp *by construction*:
/// any `Total` anywhere in the chain closes the door, and an element written
/// wider than its surface or tab is inert rather than a leak. **Do not reorder
/// these variants**; the ordering is the invariant, not a formatting choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VisibilityMode {
    /// Everything — participants only. **The default of every term** (DD:
    /// closed-door), so a commission nobody has widened shows nothing outside.
    Total,
    /// Title + existence only — the status-only card tier.
    Presentation,
    /// Description-designated content — the widest tier.
    Description,
}

impl VisibilityMode {
    /// Every mode, from most closed to most open — the ladder the derived
    /// [`Ord`] follows. Lets tests pin the round-trip and the ordering without
    /// re-listing tokens.
    pub const ALL: &[VisibilityMode] = &[Self::Total, Self::Presentation, Self::Description];

    /// The stable, lowercase storage token — what the adapters write to the
    /// `mode` columns of `commission_tab`, `commission_element`, and
    /// `commission_surface_mode`. Persisted, so renaming a token is a migration,
    /// not a free edit.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Presentation => "presentation",
            Self::Description => "description",
            Self::Total => "total",
        }
    }

    /// Resolve a stored token back to its mode, or `None` for one outside the
    /// vocabulary — on a read path that means row tampering or a missed
    /// migration and surfaces as an error, never a silent default (the contract
    /// every persisted enum here keeps).
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|mode| mode.as_str() == token)
    }
}

impl Default for VisibilityMode {
    /// [`Total`](Self::Total) — the closed door. Matches the `DEFAULT 'total'` on
    /// every `mode` column *and* the "absent `commission_surface_mode` row means
    /// Total" rule, so every way of saying nothing means the same thing.
    fn default() -> Self {
        Self::Total
    }
}

/// The **effective visibility** of an element: `min(tab, surface, element)` —
/// three terms, fixed arity, no walk (Flat Composition DD).
///
/// Because [`VisibilityMode`]'s [`Ord`] is the openness ladder, this is the
/// closed-door clamp by construction: an element that claims `Description` under
/// a `Total` surface projects `Total`, so over-claiming is **inert**, not a leak,
/// and there is no chain length at which the clamp can be skipped. It composes
/// *under* the commission's own [`Visibility`](super::Visibility), which the
/// caller gates on first — the commission is the formal root.
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

/// One tab declared by the [`SKELETON`]: its stable id and the surfaces that
/// live in it.
///
/// A `&'static` structure, not data: this is the *code* half of "structure is
/// code, modes are data".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredTab {
    /// The tab's stable id — the token minted into `commission_tab.tab`.
    pub tab: &'static str,
    /// The surfaces declared inside this tab, in render order.
    pub surfaces: &'static [&'static str],
}

/// The **code-declared composition skeleton**: which tabs exist, and which
/// surfaces live in each. Global and invariant — every commission has exactly
/// this shape, and no write anywhere creates, renames, or removes a surface.
///
/// ⚠️ **PLACEHOLDER — NON-BINDING SCAFFOLDING.** One tab holding one permissive
/// surface is deliberately the *minimum* that makes the model real; it is **not**
/// a claim about what a commission's composition should look like. The type
/// catalog (ZMVP-171) owns the real skeleton — which tabs exist, which surfaces
/// they hold, and which element types each surface admits (the composition rules
/// ZMVP-167 will enforce). Nothing here should be read as a decision, and the
/// names below carry no meaning worth preserving.
pub const SKELETON: &[DeclaredTab] = &[DeclaredTab {
    tab: "main",
    surfaces: &["content"],
}];

/// Whether the skeleton declares `surface` **inside `tab`** — the
/// **fail-closed** vocabulary check both adapters run before writing an element
/// ([`UnknownSurface`](crate::ports::UnknownSurface)).
///
/// **The pair is the unit, not the surface alone.** A surface belongs to exactly
/// one tab in the skeleton, so answering "is this name declared *anywhere*"
/// would let an element addressing tab A carry a surface that only tab B
/// declares — a contribution into a place the skeleton never described, whose
/// tab-term clamp would then be a tab that has nothing to do with it. Taking the
/// tab's declared name (never its per-commission [`TabId`], which says nothing
/// about vocabulary) makes the wrongly paired address refusable by the same const
/// both adapters consult, so they cannot disagree about which addresses are
/// real.
///
/// Fail-closed in the literal sense: an unrecognized pair is refused, never
/// created. Because surfaces have no rows, "the surface does not exist here" and
/// "it exists here but is empty" are answered by the same authority — this
/// const.
pub fn declares_surface(tab: &TabName, surface: &SurfaceName) -> bool {
    SKELETON
        .iter()
        .find(|declared| declared.tab == tab.as_str())
        .is_some_and(|declared| declared.surfaces.contains(&surface.as_str()))
}

/// Every tab the skeleton declares, as validated [`TabName`]s — what
/// [`CommissionWrites::create`](crate::ports::CommissionWrites::create) mints a
/// row for, so a commission's tab state exists explicitly from birth (the
/// withheld-at-birth discipline: absence never has to mean anything).
///
/// Panics if the skeleton holds a label that is not a valid [`TabName`], which is
/// a programming error in the const above, not a runtime condition — and one the
/// unit tests below catch before it can ship.
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

/// **Where** an element sits: the tab (by [`TabId`]) and the declared surface
/// (by [`SurfaceName`]) it is contributed into.
///
/// The whole addressing model, named — because in the flat composition an
/// address is exactly this pair and *nothing else*. There is no parent, no path,
/// no chain to walk: an element that names a tab and a surface is fully placed.
/// Naming it keeps the pair from being carried as two loose parameters through
/// every constructor, port, and adapter, where they could be transposed or one
/// forgotten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceAddress {
    /// The tab the element sits in, **by id** — the row an element's composite
    /// foreign key targets, which is what binds it to one commission.
    pub tab: TabId,
    /// The declared surface within that tab, **by id**. Refused with
    /// [`UnknownSurface`](crate::ports::UnknownSurface) if the [`SKELETON`] does
    /// not declare it **in that tab** — the pair is checked together
    /// ([`declares_surface`]), never the name alone.
    pub surface: SurfaceName,
}

impl SurfaceAddress {
    /// The address naming `surface` inside `tab`.
    pub fn new(tab: TabId, surface: SurfaceName) -> Self {
        Self { tab, surface }
    }
}

/// The **type-owned half** of an element: the opaque JSON the core stores and
/// returns without ever interpreting it (the type catalog interprets it —
/// ZMVP-171).
///
/// **A newtype whose whole job is the derive it does not have.** The payload is
/// the only *content* an element carries, so it is the only part of the
/// composition whose escape would be a leak. As a bare `serde_json::Value` it
/// was serializable everywhere: [`CommissionComposition`] and [`ElementRow`]
/// could refuse `Serialize` all they liked while `element.payload` stayed one
/// `Json(…)` away from the wire. Wrapping it narrows that guard to the thing
/// that matters — `ElementPayload` implements **no** `serde::Serialize`, so
/// putting element content on a response is a compile error wherever it is
/// reached from, not only through the loaded aggregate.
///
/// Construction is `From<serde_json::Value>`: untrusted JSON crosses the
/// boundary once, at the route, and travels wrapped from there. Unwrapping ([`as_value`](Self::as_value) /
/// [`into_value`](Self::into_value)) exists for the **store adapters**, which
/// must hand the value to a `jsonb` bind — a deliberate, greppable act at a SQL
/// boundary, never on a response path.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementPayload(serde_json::Value);

impl ElementPayload {
    /// The wrapped value, borrowed — the adapters' bind door (a `jsonb`
    /// parameter). Not a serialization door: see the type's docs.
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    /// The wrapped value, owned — the same door as [`as_value`](Self::as_value)
    /// for a caller that owns the payload.
    pub fn into_value(self) -> serde_json::Value {
        self.0
    }
}

impl Default for ElementPayload {
    /// The **empty object**, `{}` — matching the `commission_element.payload`
    /// column's `DEFAULT '{}'::jsonb` and the request body's omitted-payload
    /// default, so every way of saying "no payload" means the same value. (Not
    /// `serde_json::Value`'s own default, which is `null`.)
    fn default() -> Self {
        Self(serde_json::Value::Object(serde_json::Map::new()))
    }
}

impl From<serde_json::Value> for ElementPayload {
    /// Wrap opaque JSON as an element payload — the one construction door
    /// (ruling R6: std traits are the vocabulary). Nothing is validated: the
    /// core does not interpret payloads, so there is no shape to check until
    /// ZMVP-171's catalog gives the type tag meaning.
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
    /// The unwrap, as the std door — see [`into_value`](ElementPayload::into_value).
    fn from(payload: ElementPayload) -> Self {
        payload.into_value()
    }
}

/// A freshly contributed element, ready to persist
/// ([`CommissionWrites::add_element`](crate::ports::CommissionWrites::add_element)).
///
/// Built with [`NewElement::contributed`], or with [`NewElement::carrying`] for
/// the element a Slot/Seat satellite hangs off. There is **no mode parameter**:
/// every element is born [`VisibilityMode::Total`] (the closed door), so widening
/// is always a separate, explicit act — the same posture the tree's `NewSurface`
/// kept. `position` is absent for the same reason it always was: the store
/// assigns append order in-transaction, inside the band.
#[derive(Debug)]
pub struct NewElement {
    /// The element key (UUIDv7) — minted here, or handed in by the satellite
    /// whose identity this element shares.
    pub id: ElementId,
    /// The commission this element is contributed to. The store verifies the
    /// address's tab belongs to this same commission — and the composite foreign
    /// key makes a cross-commission tab unrepresentable regardless.
    pub commission_id: CommissionId,
    /// Where it sits: the (tab, surface) pair.
    pub address: SurfaceAddress,
    /// What the element is — the open type tag (ZMVP-171 types it).
    pub element_type: ElementType,
    /// The ordering band the element's position is counted in. Placeholder
    /// vocabulary — see [`Band`].
    pub band: Band,
    /// The type-owned payload, opaque to the core; stored and returned
    /// unmodified — and non-serializable by construction ([`ElementPayload`]).
    pub payload: ElementPayload,
    /// The acting User (the owner; the route's authority gate settles that
    /// before this is built).
    pub created_by: UserId,
    /// When the element was contributed.
    pub created_at: DateTimeUtc,
}

impl NewElement {
    /// A new element contributed at `address`, carrying `payload` verbatim and
    /// born in the placeholder [`Band`]. Mints the element id; authority
    /// (owner-only in v1), the tab's existence, and the surface's declaration
    /// are the route's/store's concerns, settled when this is persisted.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{
    ///         Band, CommissionId, ElementPayload, ElementType, NewElement, SurfaceAddress,
    ///         SurfaceName, TabId,
    ///     },
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let address = SurfaceAddress::new(
    ///     TabId::new(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let element_type = "note".parse::<ElementType>().unwrap();
    /// let owner = UserId::new(uuid::Uuid::now_v7());
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

    /// The element that **carries** an identity-sharing satellite — a declared
    /// Slot (ZMVP-77) or Seat (ZMVP-76). Takes the satellite's already-minted
    /// `id` (one identity, two rows) and gives it the empty payload: the
    /// satellite's substance lives in its own table, which is why the generic
    /// [`contributed`](Self::contributed) add cannot declare one.
    ///
    /// Both adapters build the carrier through *this* constructor, so a Slot's
    /// element and a Seat's cannot drift from an ordinary element — or from each
    /// other — in anything but their type tag.
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
/// [`CommissionComposition::elements`].
///
/// **Deliberately not `Serialize`** — see [`CommissionComposition`].
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
    /// The type-owned payload, opaque to the core — the element's only
    /// *content*, and non-serializable by construction ([`ElementPayload`]).
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

/// A commission's **whole loaded composition** — every tab, every widened
/// surface mode, and every element
/// ([`CommissionStore::load_composition`](crate::ports::CommissionStore::load_composition)).
///
/// All three parts travel together deliberately: [`effective_visibility`] needs
/// all three terms, so a load that returned only elements would put every caller
/// one forgotten join away from projecting content it never clamped.
///
/// **This type must never serialize.** It holds everything — `Total`-tier content
/// included — so an impl of `serde::Serialize` here (or on [`ElementRow`]) would
/// put "the whole composition leaves the server" one `Json(loaded)` away. Neither
/// type implements it, so serializing the raw composition is a **compile error**
/// today; serialization exists only on a viewer projection that has already
/// applied [`effective_visibility_of`](Self::effective_visibility_of)
/// server-side (ZMVP-170). Do not add a `Serialize` derive here — project first,
/// always.
///
/// The guard does not stop at this aggregate. Refusing `Serialize` *here* only
/// protects "serialize what was loaded"; the element **content** itself is
/// protected one level down, by [`ElementPayload`] carrying no `Serialize`
/// either — so a caller that reaches past the composition and picks up a single
/// payload still cannot put it on the wire. Backed by construction, not by the
/// paragraph above.
#[derive(Debug)]
pub struct CommissionComposition {
    /// The commission's tabs (its skeleton rows), ordered by declared name.
    pub tabs: Vec<TabRow>,
    /// The per-commission surface-mode overrides. **An absent entry means
    /// [`VisibilityMode::Total`]** — the closed door said by saying nothing —
    /// which is why this is a sparse map rather than one entry per declared
    /// surface.
    pub surface_modes: HashMap<SurfaceName, VisibilityMode>,
    /// Every element, ordered by `(tab, surface, band, position)`.
    pub elements: Vec<ElementRow>,
}

impl CommissionComposition {
    /// The mode of the tab `id` names, or `None` if this composition holds no
    /// such tab. The composite foreign key makes a `None` here unreachable for an
    /// element's own tab — an element cannot cite a tab that isn't its
    /// commission's — so callers treat it as corruption, not as a case.
    pub fn tab_mode(&self, id: TabId) -> Option<VisibilityMode> {
        self.tabs
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.mode)
    }

    /// The mode of `surface` for this commission — the override if one was
    /// written, else [`VisibilityMode::Total`]. **Absence is the closed door**,
    /// so a surface nobody widened needs no row to be safe.
    pub fn surface_mode(&self, surface: &SurfaceName) -> VisibilityMode {
        self.surface_modes
            .get(surface)
            .copied()
            .unwrap_or(VisibilityMode::Total)
    }

    /// The effective visibility of one of this composition's elements —
    /// [`effective_visibility`] with the three terms resolved from here.
    ///
    /// **Fail-closed on a missing tab**: an element whose tab is absent projects
    /// [`VisibilityMode::Total`] rather than being treated as unconstrained. That
    /// state is unreachable through the write ports (the composite FK), so this
    /// is the answer to corruption, not a supported case — and the answer is the
    /// closed door.
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

    fn element(surface: &str, mode: VisibilityMode, tab: TabId) -> ElementRow {
        ElementRow {
            id: ElementId::mint(),
            address: SurfaceAddress::new(tab, surface.parse().expect("valid surface name")),
            element_type: "note".parse().expect("valid type"),
            mode,
            band: Band::default(),
            position: 0,
            created_by: UserId::new(uuid::Uuid::now_v7()),
            created_at: Utc::now(),
            payload: ElementPayload::default(),
        }
    }

    /// The skeleton's first declared tab name — the placeholder `"main"`,
    /// reached through the const so these tests survive ZMVP-171 renaming it.
    fn declared_tab() -> TabName {
        SKELETON[0]
            .tab
            .parse::<TabName>()
            .expect("the skeleton declares valid labels")
    }

    // THE projection rule (DD D4): effective visibility is the min of the three
    // terms, so an element claiming more than its surface or tab allows is
    // INERT — never a leak, at any combination.
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

    // Everything defaults to the closed door: the mode default, the composition's
    // absent-surface-mode rule, and the placeholder band all agree with the
    // column defaults in the migration.
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

    // Fail-closed corruption handling: an element whose tab is missing projects
    // Total rather than being treated as unconstrained (the composite FK makes
    // this unreachable through the write ports).
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

    // The three terms resolve from the composition itself: tab row, surface
    // override, element column.
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

    // Fail-closed skeleton lookup: an undeclared surface is rejected, never
    // created. Surfaces have no rows, so this const is the ONLY authority.
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

    // The check is on the PAIR, not the surface alone: a surface the skeleton
    // declares, addressed under a tab that does not declare it, is refused
    // exactly like an invented name. This is what keeps an element from landing
    // in a place the skeleton never described — whose tab-term clamp would then
    // be a tab with nothing to do with it.
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

        // A tab the skeleton knows nothing about declares NOTHING — not even a
        // surface that is perfectly real under its own tab.
        let wrong_tab = "not-a-declared-tab"
            .parse::<TabName>()
            .expect("valid label");
        assert!(
            !declares_surface(&wrong_tab, &real_surface),
            "a real surface under a tab that does not declare it must be refused"
        );

        // Cross-check every pair the skeleton does NOT declare, so this holds
        // once ZMVP-171 grows the skeleton past one tab.
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

    // Surface NAMES are globally unique across tabs — and that is a storage
    // invariant, not tidiness. `commission_surface_mode`'s primary key is
    // (commission_id, surface) with NO tab column: one widening row per surface
    // per commission. If two tabs ever declared the same surface name, widening
    // it under one tab would widen it under the other as well — a silent
    // cross-tab leak through the second term of the min, with nothing in the
    // schema to catch it. Keep names unique, or that PK has to grow a tab
    // column first (a migration, and a ZMVP-171 decision).
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
    // commission is born with — a malformed const would otherwise only surface
    // at commission creation.
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

    // The mode tokens are a closed, collision-free vocabulary that round-trips,
    // and the ordering the projection depends on is the openness ladder.
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

    // A new element's envelope: fresh id, the (tab, surface) address it names,
    // the acting user, its payload verbatim — and the placeholder band. There is
    // no mode field to set: every element is born Total.
    #[test]
    fn a_new_element_carries_its_address_and_payload() {
        let commission = CommissionId::new(uuid::Uuid::now_v7());
        let tab = TabId::mint();
        let surface = "content".parse::<SurfaceName>().expect("valid");
        let address = SurfaceAddress::new(tab, surface.clone());
        let element_type = "note".parse::<ElementType>().expect("valid");
        let owner = UserId::new(uuid::Uuid::now_v7());
        let body = json!({ "body": "Reference: 三毛猫 🐾", "revision": 3 });
        let payload = ElementPayload::from(body.clone());

        let contributed = NewElement::contributed(
            commission,
            address.clone(),
            element_type.clone(),
            payload.clone(),
            owner,
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

        // The satellite carrier shares an already-minted identity and is
        // otherwise an ordinary element with the empty payload.
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

    // The payload's own doors round-trip, and the empty default is the SAME
    // empty object the column DEFAULT and the omitted-body default mean — so
    // "no payload" has exactly one value however it is said.
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

    /// Compile-time probe for "does `T` implement [`serde::Serialize`]?",
    /// answered as a runtime `bool` so a test can assert the **negative** —
    /// which no ordinary `assert!` can express, because the thing being pinned
    /// is the *absence* of an impl.
    ///
    /// The trick is method-resolution priority: the inherent `probe` below
    /// exists only for `T: Serialize` and wins whenever it applies; everything
    /// else falls through one autoref step to the blanket trait impl, which
    /// answers `false`. Adding a `Serialize` derive to a pinned type therefore
    /// flips its probe and fails the test **at the derive**, instead of being
    /// caught (or not) in a review of whatever route serializes it later.
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

    // THE GUARD, pinned rather than documented: the raw composition, its rows,
    // and — the one that actually holds content — the element payload carry no
    // `Serialize`, so putting any of them on a response is a compile error.
    // Serialization exists only on the ZMVP-170 viewer projection, downstream
    // of `effective_visibility`.
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

        // The probe itself is honest: a type that DOES implement Serialize
        // answers true, so a false above means "no impl", not "probe broken".
        assert!(
            SerializeProbe::<serde_json::Value>::new().probe(),
            "control: serde_json::Value does implement Serialize"
        );
    }

    // The composition labels share one validation contract: trimmed, non-empty,
    // capped, control-character-free.
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

    // The satellite type tags live in one place, so the domain, both adapters,
    // and the routes cannot spell them differently.
    #[test]
    fn the_satellite_type_tags_are_stable() {
        assert_eq!(ElementType::slot().as_str(), ElementType::SLOT_TAG);
        assert_eq!(ElementType::seat().as_str(), ElementType::SEAT_TAG);
        assert_eq!(ElementType::SLOT_TAG, "slot");
        assert_eq!(ElementType::SEAT_TAG, "seat");
    }
}
