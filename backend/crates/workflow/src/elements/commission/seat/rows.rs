use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId,
            element::{ElementId, SurfaceAddress},
        },
        user::UserId,
    },
};

use super::{SeatKind, SeatLink, SeatPrompt};

/// A freshly declared Seat, ready to persist into a declared surface
/// ([`CommissionWrites::declare_seat`](crate::ports::CommissionWrites::declare_seat)).
/// One id, two rows: the store writes the carrying element and the seat
/// satellite atomically. Every Seat is born vacant — there is no occupant field
/// — and `position` is assigned by the store in-transaction.
#[derive(Debug)]
pub struct NewSeat {
    /// The element key (UUIDv7) — the seat's identity; the element and the
    /// satellite row share it.
    pub id: ElementId,
    /// The commission this Seat is declared on.
    pub commission_id: CommissionId,
    /// Where the carrying element sits: the (tab, surface) pair. An absent or
    /// foreign tab refuses with [`UnknownTab`](crate::ports::UnknownTab); an
    /// undeclared pair with [`UnknownSurface`](crate::ports::UnknownSurface).
    pub address: SurfaceAddress,
    /// The seat's semantic kind — open vocabulary, kinds repeat freely.
    pub kind: SeatKind,
    /// The optional free-text requirement prompt riding the vacant seat.
    pub prompt: Option<SeatPrompt>,
    /// The optional external requirements link riding the vacant seat.
    pub link: Option<SeatLink>,
    /// The acting User.
    pub created_by: UserId,
    /// When the seat was declared.
    pub created_at: DateTimeUtc,
}

impl NewSeat {
    /// A new Seat contributed at `address`, born vacant, carrying its kind and
    /// whatever requirements ride it. Mints the element id; authority, the tab's
    /// existence and the surface's declaration are settled on persist.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSeat, SeatKind, SurfaceAddress, SurfaceName, TabId},
    ///     did::Did,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::from(uuid::Uuid::now_v7());
    /// let address = SurfaceAddress::new(
    ///     TabId::from(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let owner = UserId::from(Did::from("did:plc:alice".to_string()));
    /// let kind = "Creator".parse::<SeatKind>().unwrap();
    /// let seat = NewSeat::contributed_at(commission, address.clone(), kind, None, None, owner, Utc::now());
    /// assert_eq!(seat.address, address);
    /// assert_eq!(seat.kind.as_str(), "Creator");
    /// ```
    pub fn contributed_at(
        commission: CommissionId,
        address: SurfaceAddress,
        kind: SeatKind,
        prompt: Option<SeatPrompt>,
        link: Option<SeatLink>,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: ElementId::mint(),
            commission_id: commission,
            address,
            kind,
            prompt,
            link,
            created_by,
            created_at: now,
        }
    }
}
