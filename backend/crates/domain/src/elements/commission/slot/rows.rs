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

/// A freshly declared Slot, ready to persist into a declared surface
/// ([`CommissionWrites::declare_slots`](crate::ports::CommissionWrites::declare_slots)).
/// The store writes an ordinary element and the Slot satellite beside it, keyed
/// by that element's id. No occupant field of any kind.
#[derive(Debug)]
pub struct NewSlot {
    /// The element key (UUIDv7) of the element carrying this Slot; it also keys
    /// the satellite row.
    pub id: ElementId,
    /// The commission this Slot is declared on.
    pub commission_id: CommissionId,
    /// Where the carrying element sits: the (tab, surface) pair. An absent or
    /// foreign tab refuses with [`UnknownTab`](crate::ports::UnknownTab); an
    /// undeclared pair with [`UnknownSurface`](crate::ports::UnknownSurface).
    pub address: SurfaceAddress,
    /// The Slot's required title, validated at the boundary.
    pub title: super::SlotTitle,
    /// Optional freeform notes, carried verbatim; the boundary trims and maps
    /// blank to `None`.
    pub notes: Option<String>,
    /// The acting User.
    pub created_by: UserId,
    /// When the Slot was declared.
    pub created_at: DateTimeUtc,
}

impl NewSlot {
    /// A new Slot contributed at `address`, titled `title`, with optional
    /// `notes`. Mints the element id; authority, the tab's existence and the
    /// surface's declaration are settled on persist.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NewSlot, SlotTitle, SurfaceAddress, SurfaceName, TabId},
    ///     did::Did,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let address = SurfaceAddress::new(
    ///     TabId::from(uuid::Uuid::now_v7()),
    ///     "content".parse::<SurfaceName>().unwrap(),
    /// );
    /// let owner = UserId::from(Did::from("did:plc:alice".to_string()));
    /// let title = "The knight".parse::<SlotTitle>().unwrap();
    /// let slot = NewSlot::contributed_at(commission, address.clone(), title, None, owner, Utc::now());
    /// assert_eq!(slot.address, address);
    /// assert_eq!(slot.title.as_str(), "The knight");
    /// assert!(slot.notes.is_none());
    /// ```
    pub fn contributed_at(
        commission: CommissionId,
        address: SurfaceAddress,
        title: super::SlotTitle,
        notes: Option<String>,
        created_by: UserId,
        now: DateTimeUtc,
    ) -> Self {
        Self {
            id: ElementId::mint(),
            commission_id: commission,
            address,
            title,
            notes,
            created_by,
            created_at: now,
        }
    }
}

#[cfg(test)]
mod tests;
