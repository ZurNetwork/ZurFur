#[cfg(test)]
mod tests;

/// The kind of act a changelog entry records — the frozen entry taxonomy.
/// Variants whose emitter has not shipped yet are inert, never stored.
///
/// Each variant persists as its derived [`Display`](std::fmt::Display) token in
/// `commission_changelog.kind` and resolves back through the derived
/// [`FromStr`](std::str::FromStr), so the enum owns the vocabulary. Renaming a
/// token is a migration.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum ChangelogEntryKind {
    /// The commission was created — the stream's genesis entry.
    Created,
    /// The commission moved to another lifecycle step (Draft/Batched/Active/…).
    LifecycleMoved,
    /// A direction-status transition (the per-direction status between the
    /// creator and the commissioner).
    StatusChanged,
    /// A deadline was set (or cleared — the payload says which).
    DeadlineSet,
    /// An existing deadline was extended.
    DeadlineExtended,
    /// A Participant set or cleared the manual Delayed flag (the payload says
    /// which). Always an explicit act with an actor.
    Delayed,
    /// System entry: the commission became Late — its deadline passed. No actor.
    Late,
    /// A seat was declared on the commission.
    SeatDeclared,
    /// Someone was invited to a seat.
    SeatInvited,
    /// Someone applied for a seat.
    SeatApplied,
    /// A seat application/invitation was accepted — the seat fills.
    SeatAccepted,
    /// A seat application/invitation was declined.
    SeatDeclined,
    /// A seated participant left their seat.
    SeatLeft,
    /// A seated participant was evicted from their seat.
    SeatEvicted,
    /// A seat's visibility ceiling changed.
    CeilingChanged,
    /// A view grant was issued.
    ViewGrantIssued,
    /// A view grant was revoked.
    ViewGrantRevoked,
    /// Commission-Admin authority was granted.
    AdminGranted,
    /// Commission-Admin authority was revoked.
    AdminRevoked,
    /// Ownership of the commission was transferred.
    OwnershipTransferred,
    /// The commission was attached into a commission tree.
    TreeAttached,
    /// The commission was detached from a commission tree.
    TreeDetached,
    /// A phase was checked off.
    PhaseCheckedOff,
    /// A phase was approved by the client.
    PhaseApproved,
    /// A file entered the commission record.
    FileAdded,
    /// Markup was added over a file entry.
    MarkupAdded,
    /// An invoice was issued.
    InvoiceIssued,
    /// An invoice was voided.
    InvoiceVoided,
    /// An invoice was marked paid by the provider.
    InvoiceMarkedPaid,
    /// A payment was reported sent by the payer.
    InvoicePaymentSent,
    /// A gallery snapshot of the commission was published.
    SnapshotPublished,
    /// The owner archived the commission — soft-removed from active views, the
    /// record and its facts surviving intact.
    Archived,
    /// The owner un-archived the commission, returning it to active views.
    Unarchived,
    /// A standalone free-text note; the text rides the entry's `note` field.
    Note,
    /// The external linked channel was declared or replaced.
    ChannelLinked,
    /// The external linked channel was cleared.
    ChannelUnlinked,
    /// The commission was renamed. The entry carries neither the old title nor
    /// the new one.
    Renamed,
    /// The owner rated the commission or changed its rating: the rating and the
    /// Graphic flag it now carries.
    MaturityRated,
    /// A Slot was declared on the commission. The entry carries its element id, never its
    /// title or notes.
    SlotDeclared,
}

/// The kinds that say whether a Late is still owed: the kind a Late mark records, and the kinds
/// that re-arm it. A re-arming entry recorded after the last mark makes a Late owed again, so each
/// fresh miss is its own event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateKinds {
    mark: ChangelogEntryKind,
    rearming: Vec<ChangelogEntryKind>,
}

impl Default for LateKinds {
    /// The domain's: a [`Late`](ChangelogEntryKind::Late) entry marks, and a deadline set (a
    /// clear records as one) or extended re-arms it.
    fn default() -> Self {
        use strum::VariantArray;
        let rearming = ChangelogEntryKind::VARIANTS
            .iter()
            .copied()
            .filter(rearms_late)
            .collect();
        Self {
            mark: ChangelogEntryKind::Late,
            rearming,
        }
    }
}

impl LateKinds {
    /// The kind a Late mark records.
    pub fn mark(&self) -> ChangelogEntryKind {
        self.mark
    }

    /// The kinds that re-arm a Late, in declaration order.
    pub fn rearming(&self) -> &[ChangelogEntryKind] {
        &self.rearming
    }
}

/// Whether an entry of `kind`, recorded after a Late mark, makes a Late owed again.
fn rearms_late(kind: &ChangelogEntryKind) -> bool {
    matches!(
        kind,
        ChangelogEntryKind::DeadlineSet | ChangelogEntryKind::DeadlineExtended
    )
}

/// The kinds still recorded outside the den's write gate, each with a payload its caller
/// builds. [`NewChangelogEntry::event`](super::NewChangelogEntry::event) takes only these, so
/// a gated kind is built only by its own domain constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::IntoStaticStr, strum::VariantArray)]
#[strum(serialize_all = "snake_case")]
pub enum OpenKind {
    /// A seat was declared on the commission.
    SeatDeclared,
    /// A view grant was issued.
    ViewGrantIssued,
    /// A view grant was revoked.
    ViewGrantRevoked,
    /// A file entered the commission record.
    FileAdded,
    /// Markup was added over a file entry.
    MarkupAdded,
    /// The owner archived the commission.
    Archived,
    /// The owner un-archived the commission.
    Unarchived,
    /// The external linked channel was declared or replaced.
    ChannelLinked,
    /// The external linked channel was cleared.
    ChannelUnlinked,
}

impl From<OpenKind> for ChangelogEntryKind {
    /// The stored kind an open kind records as.
    fn from(open: OpenKind) -> Self {
        match open {
            OpenKind::SeatDeclared => Self::SeatDeclared,
            OpenKind::ViewGrantIssued => Self::ViewGrantIssued,
            OpenKind::ViewGrantRevoked => Self::ViewGrantRevoked,
            OpenKind::FileAdded => Self::FileAdded,
            OpenKind::MarkupAdded => Self::MarkupAdded,
            OpenKind::Archived => Self::Archived,
            OpenKind::Unarchived => Self::Unarchived,
            OpenKind::ChannelLinked => Self::ChannelLinked,
            OpenKind::ChannelUnlinked => Self::ChannelUnlinked,
        }
    }
}
