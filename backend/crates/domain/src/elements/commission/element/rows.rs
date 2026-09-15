use crate::{
    datetime::DateTimeUtc,
    elements::{commission::CommissionId, user::UserId},
};

use super::{Band, ElementId, ElementType, SurfaceName, TabId, TabName, VisibilityMode};

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

#[cfg(test)]
mod tests;
