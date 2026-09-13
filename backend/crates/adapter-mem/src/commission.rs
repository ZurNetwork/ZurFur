//! In-process fakes of the commission seam: the stored shapes, the
//! [`CommissionWrites`]/[`ChangelogWrites`] write views staged by the
//! [`MemUnitOfWork`](crate::MemUnitOfWork), the
//! [`MemCommissionStore`]/[`MemChangelogStore`] read stores, and the commission
//! seed/inspect helpers on [`MemBackend`].

use std::collections::HashMap;

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    commission::{
        Band, ChangelogEntry, ChangelogEntryKind, ChannelPointer, Commission,
        CommissionComposition, CommissionFile, CommissionId, CommissionMarkup, CommissionTitle,
        DeadlineStatus, DirectionStatus, ElementId, ElementPayload, ElementRow, ElementType,
        FileKey, GrantLevel, LapsedDeadline, LifecycleStep, NewChangelogEntry, NewElement, NewSeat,
        NewSlot, Seat, SeatInvitation, SeatInvitationId, SeatKind, SeatLink, SeatPrompt, Slot,
        SlotTitle, SurfaceAddress, SurfaceName, TabId, TabName, TabRow, Visibility, VisibilityMode,
        declared_tabs, declares_surface, derive_deadline_status,
    },
    did::Did,
    invitation::InvitationState,
    maturity::Maturity,
    user::UserId,
    workflow::{Column, ColumnId, LexOrdering, WorkflowId},
};
use domain::ports::{
    ChangelogStore, ChangelogWrites, ColumnStore, CommissionReads, CommissionStore,
    CommissionWrites, ElementNotFound, UnknownSurface, UnknownTab,
};
use serde_json::Value;

use crate::{MemBackend, workflow::MemColumnStore};

/// Resolve a tab within `commission`, handing back its declared name so the
/// caller can consult the skeleton. An absent id and a foreign tab both refuse
/// with [`UnknownTab`], indistinguishably. Every element write path goes through
/// here first, where pg also takes the tab's row lock.
fn require_tab(
    tabs: &HashMap<TabId, StoredTab>,
    commission: CommissionId,
    tab: TabId,
) -> anyhow::Result<TabName> {
    match tabs.get(&tab) {
        Some(stored) if stored.commission_id == commission => Ok(stored.tab.clone()),
        _ => Err(UnknownTab.into()),
    }
}

/// The shared address gate of every element write: resolve the tab
/// ([`require_tab`]), then require the skeleton to declare this surface inside
/// it, else [`UnknownSurface`]. The order is part of the contract — an address
/// wrong in both ways refuses as [`UnknownTab`].
fn require_address(
    tabs: &HashMap<TabId, StoredTab>,
    commission: CommissionId,
    address: &SurfaceAddress,
) -> anyhow::Result<()> {
    let tab = require_tab(tabs, commission, address.tab)?;
    if !declares_surface(&tab, &address.surface) {
        return Err(UnknownSurface.into());
    }
    Ok(())
}

/// The next append `position`, counted over the element's
/// `(commission, tab, surface, band)` ordering group.
fn next_position(
    elements: &HashMap<ElementId, StoredElement>,
    commission: CommissionId,
    address: &SurfaceAddress,
    band: &Band,
) -> i32 {
    elements
        .values()
        .filter(|element| {
            element.commission_id == commission
                && element.address == *address
                && element.band == *band
        })
        .map(|element| element.position + 1)
        .max()
        .unwrap_or(0)
}

/// Insert one element on the unit's staged snapshot behind the shared address
/// gate — the single write path every element takes.
fn insert_element(backend: &MemBackend, element: &NewElement) -> anyhow::Result<()> {
    let tabs = backend.tabs.lock().expect("MemBackend tabs mutex poisoned");
    let mut elements = backend
        .elements
        .lock()
        .expect("MemBackend elements mutex poisoned");
    require_address(&tabs, element.commission_id, &element.address)?;
    let position = next_position(
        &elements,
        element.commission_id,
        &element.address,
        &element.band,
    );
    let stored = StoredElement {
        commission_id: element.commission_id,
        address: element.address.clone(),
        element_type: element.element_type.clone(),
        mode: VisibilityMode::default(),
        band: element.band.clone(),
        position,
        created_by: element.created_by.clone(),
        created_at: element.created_at,
        payload: element.payload.clone(),
    };
    elements.insert(element.id, stored);
    Ok(())
}

/// The fields of a [`Commission`] we keep behind the lock; a read rebuilds the
/// aggregate, which is not `Clone`. `Clone` lets a unit stage the map,
/// `PartialEq` lets [`crate::merge_map`] tell an untouched row from a written one.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredCommission {
    /// The commission's fixed, always-present Title, validated non-empty.
    pub(crate) title: CommissionTitle,
    /// The User who created it — the permanent owner.
    pub(crate) owner_id: UserId,
    /// Its single [`LifecycleStep`]; a freshly created commission is `Draft`.
    pub(crate) lifecycle_step: LifecycleStep,
    /// Who may see it; a freshly created commission is [`Visibility::Private`].
    pub(crate) visibility: Visibility,
    /// The nullable-but-fixed deadline envelope field.
    pub(crate) deadline: Option<domain::datetime::DateTimeUtc>,
    /// The maturity posture, or `None` while unrated; one field, so pg's
    /// both-or-neither CHECK holds by construction.
    pub(crate) maturity: Option<Maturity>,
    /// The direction-axis Status, or `None`; one cell, so a set replaces.
    pub(crate) direction_status: Option<DirectionStatus>,
    /// The deadline-axis Status, or `None`; the same one-cell shape.
    pub(crate) deadline_status: Option<DeadlineStatus>,
    /// The external linked-channel pointer, or `None` while none is declared.
    pub(crate) linked_channel: Option<ChannelPointer>,
    /// When the commission was archived, or `None` while active.
    pub(crate) archived_at: Option<domain::datetime::DateTimeUtc>,
    /// When the commission was created.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
}

impl StoredCommission {
    /// Rebuild the aggregate from its stored parts.
    fn rebuild(&self, id: CommissionId) -> Commission {
        // Late is derived fresh at lookup, never persisted; the stored
        // `deadline_status` is the manual `Delayed` flag only.
        let deadline_status = derive_deadline_status(
            self.deadline,
            &self.lifecycle_step,
            self.deadline_status,
            chrono::Utc::now(),
        );
        Commission {
            id,
            title: self.title.clone(),
            owner_id: self.owner_id.clone(),
            lifecycle_step: self.lifecycle_step.clone(),
            visibility: self.visibility.clone(),
            deadline: self.deadline,
            maturity: self.maturity,
            direction_status: self.direction_status,
            deadline_status,
            linked_channel: self.linked_channel.clone(),
            archived_at: self.archived_at,
            created_at: self.created_at,
        }
    }
}

/// One commission element as the mem backend keeps it, keyed by [`ElementId`],
/// so the row's own id lives in the key.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredElement {
    /// The commission this element belongs to.
    pub(crate) commission_id: CommissionId,
    /// Where it sits: the (tab, surface) pair — the whole addressing model.
    /// There is no parent field, here or in pg.
    pub(crate) address: SurfaceAddress,
    /// What it is — the open type tag.
    pub(crate) element_type: ElementType,
    /// Its own visibility mode: the third term of the effective-visibility min.
    pub(crate) mode: VisibilityMode,
    /// The ordering band its position is counted in.
    pub(crate) band: Band,
    /// Order within `(tab, surface, band)` (append = max + 1).
    pub(crate) position: i32,
    /// Who contributed the element.
    pub(crate) created_by: UserId,
    /// When it was contributed.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
    /// The type-owned payload, opaque here as in pg, carried in its
    /// non-serializable wrapper.
    pub(crate) payload: ElementPayload,
}

/// One commission tab as the mem backend keeps it, keyed by [`TabId`]. Minted
/// with the commission and never removed.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredTab {
    /// The commission this tab belongs to; pg binds this with a composite
    /// foreign key.
    pub(crate) commission_id: CommissionId,
    /// The declared tab id this row realizes (a skeleton name).
    pub(crate) tab: TabName,
    /// The tab's visibility mode: the first term of the min.
    pub(crate) mode: VisibilityMode,
}

/// One declared Slot's satellite, keyed by the [`ElementId`] of the element
/// carrying it. Deliberately occupant-less: fill is unrepresentable until the
/// Character epic adds it.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredSlot {
    /// The commission the Slot belongs to (the pg row's own commission FK).
    pub(crate) commission_id: CommissionId,
    /// The Slot's required title, validated at the boundary.
    pub(crate) title: SlotTitle,
    /// The optional freeform notes, exactly as declared.
    pub(crate) notes: Option<String>,
}

impl StoredSlot {
    /// Rebuild the read shape for the carrying element `id` that keys this
    /// satellite.
    fn rebuild(&self, id: ElementId) -> Slot {
        Slot {
            element_id: id,
            commission_id: self.commission_id,
            title: self.title.clone(),
            notes: self.notes.clone(),
        }
    }
}

/// One declared Seat's interpreted half, keyed by the seat's [`ElementId`] —
/// one identity, split between [`StoredElement`] and this satellite.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredSeat {
    /// The owning commission, denormalized to back the seats() read.
    pub(crate) commission_id: CommissionId,
    /// The seat's semantic kind (open vocabulary; kinds repeat freely).
    pub(crate) kind: SeatKind,
    /// The optional free-text requirement prompt riding the vacant seat.
    pub(crate) prompt: Option<SeatPrompt>,
    /// The optional external requirements link riding the vacant seat.
    pub(crate) link: Option<SeatLink>,
    /// The single occupant slot; at most one is unrepresentable to violate.
    pub(crate) occupant: Option<UserId>,
}

/// One pending (or once-pending) seat invitation, keyed by
/// [`SeatInvitationId`]; a read rebuilds the entity, which is not `Clone`.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredSeatInvitation {
    /// The commission whose Seat is offered.
    pub(crate) commission: CommissionId,
    /// The Seat being offered (its carrying element id).
    pub(crate) seat: ElementId,
    /// The User being invited.
    pub(crate) invited_user: UserId,
    /// The commission owner who issued the offer.
    pub(crate) inviter: UserId,
    /// Where the offer sits in its lifecycle.
    pub(crate) state: InvitationState,
    /// When the invitation was issued.
    pub(crate) created_at: DateTimeUtc,
    /// When the invitation last changed state.
    pub(crate) updated_at: DateTimeUtc,
}

impl StoredSeatInvitation {
    /// Rebuild the domain [`SeatInvitation`] from the stored parts.
    fn rebuild(&self, id: SeatInvitationId) -> SeatInvitation {
        SeatInvitation {
            id,
            commission: self.commission,
            seat: self.seat,
            invited_user: self.invited_user.clone(),
            inviter: self.inviter.clone(),
            state: self.state,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// One appended changelog entry as the mem backend keeps it. A push never
/// rewrites an entry; the only way one disappears is `delete`'s cascade.
/// `PartialEq` lets the commit-time merge diff by value, there being no id.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredChangelogEntry {
    /// The store-assigned ordering key; global and monotonic, not
    /// per-commission.
    pub(crate) seq: i64,
    /// The stream the entry belongs to.
    pub(crate) commission_id: CommissionId,
    /// What act the entry records.
    pub(crate) kind: ChangelogEntryKind,
    /// Who did it — `None` for a system entry.
    pub(crate) actor_id: Option<UserId>,
    /// Kind-specific parameters (JSON), enough to render a sentence.
    pub(crate) payload: Value,
    /// Free text riding the entry, if any.
    pub(crate) note: Option<String>,
    /// When the act happened; carried for display, while `seq` is the order.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
}

impl StoredChangelogEntry {
    /// Rebuild the read shape from the stored parts.
    fn rebuild(&self) -> ChangelogEntry {
        ChangelogEntry {
            seq: self.seq,
            commission_id: self.commission_id,
            kind: self.kind,
            actor_id: self.actor_id.clone(),
            payload: self.payload.clone(),
            note: self.note.clone(),
            created_at: self.created_at,
        }
    }
}

/// In-memory [`CommissionWrites`] view, vended by
/// [`MemUnitOfWork::commissions`](crate::MemUnitOfWork) over the unit's staging
/// snapshot, so a write reaches the shared store only on commit.
pub struct MemCommissionWrites(pub(crate) MemBackend);

#[async_trait]
impl CommissionWrites for MemCommissionWrites {
    /// Insert the freshly created commission with one tab row per tab the code
    /// skeleton declares and its owner's participant row, all on this unit's
    /// staging snapshot — so a tabless or owner-less commission is
    /// unrepresentable. Every tab is born [`VisibilityMode::Total`]; the
    /// commission's own visibility gates over the composition, never seeds it.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        {
            let mut commissions = self
                .0
                .commissions
                .lock()
                .expect("MemBackend commissions mutex poisoned");
            commissions.insert(
                commission.id,
                StoredCommission {
                    title: commission.title.clone(),
                    owner_id: commission.owner_id.clone(),
                    lifecycle_step: commission.lifecycle_step.clone(),
                    visibility: commission.visibility.clone(),
                    deadline: commission.deadline,
                    maturity: commission.maturity,
                    direction_status: commission.direction_status,
                    deadline_status: commission.deadline_status,
                    linked_channel: commission.linked_channel.clone(),
                    archived_at: commission.archived_at,
                    created_at: commission.created_at,
                },
            );
        }
        {
            let mut tabs = self.0.tabs.lock().expect("MemBackend tabs mutex poisoned");
            for tab in declared_tabs() {
                let stored = StoredTab {
                    commission_id: commission.id,
                    tab,
                    mode: VisibilityMode::default(),
                };
                tabs.insert(TabId::mint(), stored);
            }
        }
        let mut participants = self
            .0
            .participants
            .lock()
            .expect("MemBackend participants mutex poisoned");
        // A duplicate add is a no-op preserving the ORIGINAL created_at —
        // seat acceptance re-adds whoever it seats.
        participants
            .entry((commission.id, commission.owner_id.clone()))
            .or_insert(commission.created_at);
        Ok(())
    }

    /// Contribute one element into a declared surface at `position = max + 1`
    /// within its `(tab, surface, band)` group, behind the shared address gate.
    /// The element is born [`VisibilityMode::Total`] and its opaque payload is
    /// held verbatim.
    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()> {
        insert_element(&self.0, element)
    }

    /// Remove one element, its identity-sharing satellites and a seat's pending
    /// offers, then renumber the remaining `(tab, surface, band)` group to
    /// contiguous positions — all on the staging snapshot, so they commit
    /// together. An absent id and a foreign element both refuse with
    /// [`ElementNotFound`], indistinguishably. No element is protected.
    async fn remove_element(
        &mut self,
        commission: &CommissionId,
        element: &ElementId,
    ) -> anyhow::Result<()> {
        let (commission, element) = (*commission, *element);
        // `tabs` before `elements`, the SAME order `insert_element` takes, so
        // the two maps are never acquired in opposite orders.
        let tabs = self.0.tabs.lock().expect("MemBackend tabs mutex poisoned");
        let mut elements = self
            .0
            .elements
            .lock()
            .expect("MemBackend elements mutex poisoned");
        let Some(removed) = elements
            .get(&element)
            .filter(|stored| stored.commission_id == commission)
            .cloned()
        else {
            return Err(ElementNotFound.into());
        };
        // Resolved through the same gate the add path uses; a miss is
        // corruption, answering with the same UnknownTab pg would.
        require_tab(&tabs, commission, removed.address.tab)?;
        drop(tabs);
        elements.remove(&element);

        // Renumber the vacated ordering group to contiguous positions.
        let mut group: Vec<(ElementId, i32)> = elements
            .iter()
            .filter(|(_, stored)| {
                stored.commission_id == commission
                    && stored.address == removed.address
                    && stored.band == removed.band
            })
            .map(|(id, stored)| (*id, stored.position))
            .collect();
        group.sort_by_key(|(_, position)| *position);
        for (index, (id, _)) in group.into_iter().enumerate() {
            elements
                .get_mut(&id)
                .expect("group member was just enumerated")
                .position = index as i32;
        }
        drop(elements);

        // The identity-sharing satellites, and a seat's pending offers.
        self.0
            .slots
            .lock()
            .expect("MemBackend slots mutex poisoned")
            .remove(&element);
        let had_seat = self
            .0
            .seats
            .lock()
            .expect("MemBackend seats mutex poisoned")
            .remove(&element)
            .is_some();
        if had_seat {
            self.0
                .seat_invitations
                .lock()
                .expect("MemBackend seat_invitations mutex poisoned")
                .retain(|_, invitation| invitation.seat != element);
        }
        Ok(())
    }

    /// Record a file entry's link on the unit's staged snapshot, so it commits
    /// atomically with the caller's `file_added` changelog entry. The bytes were
    /// stored separately, before this unit.
    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()> {
        let mut files = self
            .0
            .files
            .lock()
            .expect("MemBackend files mutex poisoned");
        files.insert(file.id, file.clone());
        Ok(())
    }

    /// Record one annotation on the unit's staged snapshot, so it commits
    /// atomically with the caller's `markup_added` changelog entry. The file
    /// entry's existence stays the caller's check.
    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {
        let mut markups = self
            .0
            .markups
            .lock()
            .expect("MemBackend markups mutex poisoned");
        markups.insert(markup.id, markup.clone());
        Ok(())
    }

    /// Declare a batch of Slots: per Slot, the shared address gate plants an
    /// [`ElementType::slot`]-typed element and the Slot lands as its
    /// `StoredSlot` satellite. The batch commits or vanishes together, so a
    /// refusal mid-batch applies nothing.
    async fn declare_slots(&mut self, new_slots: &[NewSlot]) -> anyhow::Result<()> {
        for slot in new_slots {
            let carrier = NewElement::carrying(
                slot.id,
                slot.commission_id,
                slot.address.clone(),
                ElementType::slot(),
                slot.created_by.clone(),
                slot.created_at,
            );
            insert_element(&self.0, &carrier)?;

            let mut slots = self
                .0
                .slots
                .lock()
                .expect("MemBackend slots mutex poisoned");
            slots.insert(
                slot.id,
                StoredSlot {
                    commission_id: slot.commission_id,
                    title: slot.title.clone(),
                    notes: slot.notes.clone(),
                },
            );
        }
        Ok(())
    }

    /// Whether the commission bears any fact, answered on the unit's staged
    /// snapshot. Constant `false`: no fact-minter exists yet, so there is no fact
    /// map to scan. Whoever registers the first fact table in the pg adapter MUST
    /// add the matching map here too.
    async fn commission_has_facts(&mut self, _id: &CommissionId) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Remove the commission with its changelog entries and its whole
    /// composition — tabs, elements, surface modes, and the Slot/Seat satellites
    /// with a seat's pending offers. Lands on the unit's staged snapshot; an
    /// absent commission is a no-op. Participants, files and positioning do NOT
    /// cascade here, a known divergence; any future child map must.
    async fn delete(&mut self, id: &CommissionId) -> anyhow::Result<()> {
        let id = *id;
        {
            let mut commissions = self
                .0
                .commissions
                .lock()
                .expect("MemBackend commissions mutex poisoned");
            commissions.remove(&id);
        }
        {
            let mut changelog = self
                .0
                .changelog
                .lock()
                .expect("MemBackend changelog mutex poisoned");
            changelog.retain(|entry| entry.commission_id != id);
        }

        // In the order pg's cascade reaches it: a seat's offers, the
        // satellites, their elements, then the tabs and surface modes.
        let doomed_seats: Vec<ElementId> = {
            let mut seats = self
                .0
                .seats
                .lock()
                .expect("MemBackend seats mutex poisoned");
            let doomed: Vec<ElementId> = seats
                .iter()
                .filter(|(_, seat)| seat.commission_id == id)
                .map(|(seat_id, _)| *seat_id)
                .collect();
            seats.retain(|_, seat| seat.commission_id != id);
            doomed
        };
        self.0
            .seat_invitations
            .lock()
            .expect("MemBackend seat_invitations mutex poisoned")
            .retain(|_, invitation| !doomed_seats.contains(&invitation.seat));
        self.0
            .slots
            .lock()
            .expect("MemBackend slots mutex poisoned")
            .retain(|_, slot| slot.commission_id != id);
        self.0
            .elements
            .lock()
            .expect("MemBackend elements mutex poisoned")
            .retain(|_, element| element.commission_id != id);
        self.0
            .tabs
            .lock()
            .expect("MemBackend tabs mutex poisoned")
            .retain(|_, tab| tab.commission_id != id);
        self.0
            .surface_modes
            .lock()
            .expect("MemBackend surface_modes mutex poisoned")
            .retain(|(commission, _), _| *commission != id);
        Ok(())
    }

    /// Flip the stored archive stamp — the mem mirror of the pg
    /// conditional `UPDATE`: the write applies only on a **real transition**
    /// (the `is_none`/`is_some` arms differ between stored and requested), so a
    /// repeat in the same direction changes nothing, answers `false`, and keeps
    /// the original stamp. An absent commission answers `false` (existence is
    /// the caller's check). Staged like every write here: shared state moves
    /// only on commit.
    async fn set_archived(
        &mut self,
        id: &CommissionId,
        archived_at: Option<domain::datetime::DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        let id = *id;
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let Some(stored) = commissions.get_mut(&id) else {
            return Ok(false);
        };
        if stored.archived_at.is_none() == archived_at.is_none() {
            return Ok(false);
        }
        stored.archived_at = archived_at;
        Ok(true)
    }

    /// Write the maturity posture — the mem mirror of the pg
    /// `UPDATE commission SET maturity, graphic`. Replace-only by
    /// signature (no clear arm exists); an absent commission is a no-op, per
    /// the port contract (existence is the caller's check).
    async fn set_maturity(&mut self, id: &CommissionId, maturity: Maturity) -> anyhow::Result<()> {
        let id = *id;
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        if let Some(stored) = commissions.get_mut(&id) {
            stored.maturity = Some(maturity);
        }
        Ok(())
    }
    /// Declare a seat: behind the shared address gate, one `StoredElement`
    /// and one `StoredSeat` land under the same [`ElementId`], so both halves
    /// commit or vanish together. Every seat is born vacant.
    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()> {
        let carrier = NewElement::carrying(
            seat.id,
            seat.commission_id,
            seat.address.clone(),
            ElementType::seat(),
            seat.created_by.clone(),
            seat.created_at,
        );
        insert_element(&self.0, &carrier)?;

        let mut seats = self
            .0
            .seats
            .lock()
            .expect("MemBackend seats mutex poisoned");
        seats.insert(
            seat.id,
            StoredSeat {
                commission_id: seat.commission_id,
                kind: seat.kind.clone(),
                prompt: seat.prompt.clone(),
                link: seat.link.clone(),
                occupant: None,
            },
        );
        Ok(())
    }

    /// Insert the pending seat invitation unless one is already pending for the
    /// same `(seat, invited_user)`, in which case this is a no-op — different
    /// Users may each hold one to the same Seat. Returns the offer that now
    /// stands: the fresh one, or the pending one already on file.
    async fn create_seat_invitation(
        &mut self,
        invitation: &SeatInvitation,
    ) -> anyhow::Result<SeatInvitation> {
        let mut invitations = self
            .0
            .seat_invitations
            .lock()
            .expect("MemBackend seat_invitations mutex poisoned");
        let already_pending = invitations.iter().find_map(|(id, stored)| {
            (stored.seat == invitation.seat
                && stored.invited_user == invitation.invited_user
                && stored.state == InvitationState::Pending)
                .then(|| stored.rebuild(*id))
        });
        if let Some(standing) = already_pending {
            // At most one pending offer per (seat, user).
            return Ok(standing);
        }
        let issued = StoredSeatInvitation {
            commission: invitation.commission,
            seat: invitation.seat,
            invited_user: invitation.invited_user.clone(),
            inviter: invitation.inviter.clone(),
            state: invitation.state,
            created_at: invitation.created_at,
            updated_at: invitation.updated_at,
        };
        let standing = issued.rebuild(invitation.id);
        invitations.insert(invitation.id, issued);
        Ok(standing)
    }

    /// Flip a pending seat invitation to revoked and stamp `updated_at`. A
    /// non-pending or absent invitation is a no-op, not an error.
    async fn revoke_seat_invitation(&mut self, id: &SeatInvitationId) -> anyhow::Result<()> {
        let id = *id;
        let mut invitations = self
            .0
            .seat_invitations
            .lock()
            .expect("MemBackend seat_invitations mutex poisoned");
        if let Some(stored) = invitations.get_mut(&id)
            && stored.state == InvitationState::Pending
        {
            stored.state = InvitationState::Revoked;
            stored.updated_at = chrono::Utc::now();
        }
        Ok(())
    }

    /// Repoint (or clear) the linked-channel pointer, applying only when the
    /// stored value differs — so a repeat, or an absent commission, answers
    /// `false`, which the caller's changelog append keys on.
    async fn set_linked_channel(
        &mut self,
        id: &CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        let id = *id;
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let Some(stored) = commissions.get_mut(&id) else {
            return Ok(false);
        };
        if stored.linked_channel.as_ref() == channel {
            return Ok(false);
        }
        stored.linked_channel = channel.cloned();
        Ok(true)
    }

    /// Upsert the grantee's key: one per (commission, grantee), keyed by the
    /// grantee's DID, so re-granting replaces the level.
    async fn grant_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        self.0
            .view_grants
            .lock()
            .expect("MemBackend view_grants mutex poisoned")
            .insert((*commission, Did::from(to_user.to_string())), level);
        Ok(())
    }

    /// Remove the grantee's key (a hard delete), returning whether one existed —
    /// the bool the caller keys its `view_grant_revoked` append on.
    async fn revoke_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
    ) -> anyhow::Result<bool> {
        Ok(self
            .0
            .view_grants
            .lock()
            .expect("MemBackend view_grants mutex poisoned")
            .remove(&(*commission, Did::from(to_user.to_string())))
            .is_some())
    }

    /// Repoint (or clear) the direction-axis Status; one slot, so a set replaces
    /// whole. An absent commission is a no-op.
    async fn set_direction_status(
        &mut self,
        id: &CommissionId,
        status: Option<DirectionStatus>,
    ) -> anyhow::Result<bool> {
        let id = *id;
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let Some(stored) = commissions.get_mut(&id) else {
            return Ok(false);
        };
        if stored.direction_status == status {
            return Ok(false);
        }
        stored.direction_status = status;
        Ok(true)
    }

    /// Repoint (or clear) the stored deadline; an absent commission is a no-op.
    async fn set_deadline(
        &mut self,
        id: &CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        let id = *id;
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let Some(stored) = commissions.get_mut(&id) else {
            return Ok(false);
        };
        if stored.deadline == deadline {
            return Ok(false);
        }
        stored.deadline = deadline;
        Ok(true)
    }

    /// Repoint (or clear) the deadline-axis Status; one slot, so a set replaces
    /// whole. An absent commission is a no-op.
    async fn set_deadline_status(
        &mut self,
        id: &CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<bool> {
        let id = *id;
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let Some(stored) = commissions.get_mut(&id) else {
            return Ok(false);
        };
        if stored.deadline_status == status {
            return Ok(false);
        }
        stored.deadline_status = status;
        Ok(true)
    }

    /// The sweeper's candidate scan, answered on the unit's staged snapshot:
    /// deadline strictly before `now`, not already Late, lifecycle not terminal;
    /// ordered by deadline, id as tiebreak.
    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>> {
        // Late is never persisted, so dedup on the changelog itself: skip only
        // if the latest `late` entry is AFTER the latest deadline change, since a
        // deadline set or extension re-arms the log.
        let logged_since_change: std::collections::HashSet<CommissionId> = {
            let changelog = self
                .0
                .changelog
                .lock()
                .expect("MemBackend changelog mutex poisoned");
            let mut latest_late: std::collections::HashMap<CommissionId, i64> =
                std::collections::HashMap::new();
            let mut latest_change: std::collections::HashMap<CommissionId, i64> =
                std::collections::HashMap::new();
            for entry in changelog.iter() {
                let target = match entry.kind {
                    ChangelogEntryKind::Late => &mut latest_late,
                    ChangelogEntryKind::DeadlineSet | ChangelogEntryKind::DeadlineExtended => {
                        &mut latest_change
                    }
                    _ => continue,
                };
                target
                    .entry(entry.commission_id)
                    .and_modify(|seq| *seq = (*seq).max(entry.seq))
                    .or_insert(entry.seq);
            }
            latest_late
                .into_iter()
                .filter(|(id, late_seq)| *late_seq > latest_change.get(id).copied().unwrap_or(0))
                .map(|(id, _)| id)
                .collect()
        };
        let commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let mut lapsed: Vec<LapsedDeadline> = commissions
            .iter()
            .filter_map(|(id, stored)| {
                let deadline = stored.deadline?;
                if deadline >= now
                    || stored.lifecycle_step.is_terminal()
                    || logged_since_change.contains(id)
                {
                    return None;
                }
                Some(LapsedDeadline {
                    id: *id,
                    deadline,
                    status: stored.deadline_status,
                })
            })
            .collect();
        lapsed.sort_by_key(|lapse| (lapse.deadline, *lapse.id));
        Ok(lapsed)
    }
}

/// In-memory [`ChangelogWrites`] view: appends land on the unit's staged
/// snapshot, so an entry commits atomically with the domain writes beside it.
pub struct MemChangelogWrites(pub(crate) MemBackend);

#[async_trait]
impl ChangelogWrites for MemChangelogWrites {
    /// Push one entry, assigning the next `seq`, monotonic over the whole log.
    async fn append(&mut self, entry: &NewChangelogEntry) -> anyhow::Result<()> {
        let mut changelog = self
            .0
            .changelog
            .lock()
            .expect("MemBackend changelog mutex poisoned");
        let seq = changelog.last().map(|e| e.seq + 1).unwrap_or(1);
        changelog.push(StoredChangelogEntry {
            seq,
            commission_id: entry.commission_id,
            kind: entry.kind,
            actor_id: entry.actor_id.clone(),
            payload: entry.payload.clone(),
            note: entry.note.clone(),
            created_at: entry.created_at,
        });
        Ok(())
    }
}

/// The read half of a commission unit of work over the unit's staged snapshot,
/// so reads see writes issued through the same handle. There is no lock to take
/// in process, so `find_for_update`/`tab_for_update` are their unlocked twins.
#[async_trait]
impl CommissionReads for MemCommissionWrites {
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        MemCommissionStore(self.0.clone()).find(id).await
    }

    async fn find_for_update(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        MemCommissionStore(self.0.clone()).find(id).await
    }

    async fn is_participant(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        MemCommissionStore(self.0.clone())
            .is_participant(commission, user)
            .await
    }

    /// The tab row, scoped to `commission`; an absent id and a foreign tab both
    /// answer `None`.
    async fn tab_for_update(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<Option<TabRow>> {
        let tabs = self.0.tabs.lock().expect("MemBackend tabs mutex poisoned");
        let located = tabs
            .get(tab)
            .filter(|stored| &stored.commission_id == commission);
        Ok(located.map(|stored| TabRow {
            id: *tab,
            tab: stored.tab.clone(),
            mode: stored.mode,
        }))
    }
}

/// In-memory [`CommissionStore`] read surface over the shared [`MemBackend`].
pub struct MemCommissionStore(pub(crate) MemBackend);

#[async_trait]
impl CommissionStore for MemCommissionStore {
    /// Rebuild a [`Commission`] from its stored parts, or `None`.
    async fn find(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        let id = *id;
        let commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        Ok(commissions.get(&id).map(|stored| stored.rebuild(id)))
    }

    /// The [`GrantLevel`] `user` holds on `commission`, or `None`. A key is
    /// issued to a User, never an Account, so account membership confers nothing.
    async fn view_grant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<GrantLevel>> {
        Ok(self
            .0
            .view_grants
            .lock()
            .expect("MemBackend view_grants mutex poisoned")
            .get(&(*commission, Did::from(user.to_string())))
            .copied())
    }

    /// Which column of `workflow_id` holds this commission, or `None`. Scoped by
    /// board: a commission sits in at most one column per board.
    async fn current_column_of_workflow(
        &self,
        commission: &CommissionId,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<Column>> {
        MemColumnStore(self.0.clone())
            .find_column(workflow_id, commission)
            .await
    }

    /// This commission's index within `column_id`, or `None` if that column does
    /// not hold it.
    async fn current_position_in_column(
        &self,
        commission: &CommissionId,
        column_id: &ColumnId,
    ) -> anyhow::Result<Option<u8>> {
        let Some(column) = MemColumnStore(self.0.clone()).find(column_id).await? else {
            return Ok(None);
        };

        let Some(index) = column.iter().position(|card| card == commission) else {
            return Ok(None);
        };

        u8::try_from(index)
            .map_err(|_| anyhow::anyhow!("card position {index} is out of range"))
            .map(Some)
    }

    /// Load the whole composition: the tab, surface-mode and element maps
    /// filtered by commission and ordered as pg orders them. `None` when the
    /// commission has NO tabs, which means no such commission — an empty
    /// `elements` list is the ordinary state of a fresh one.
    async fn load_composition(
        &self,
        id: &CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>> {
        let id = *id;
        let mut tabs: Vec<TabRow> = {
            let stored = self.0.tabs.lock().expect("MemBackend tabs mutex poisoned");
            stored
                .iter()
                .filter(|(_, tab)| tab.commission_id == id)
                .map(|(tab_id, tab)| TabRow {
                    id: *tab_id,
                    tab: tab.tab.clone(),
                    mode: tab.mode,
                })
                .collect()
        };
        if tabs.is_empty() {
            return Ok(None);
        }
        tabs.sort_by(|left, right| left.tab.as_ref().cmp(right.tab.as_ref()));

        let surface_modes = self
            .0
            .surface_modes
            .lock()
            .expect("MemBackend surface_modes mutex poisoned")
            .iter()
            .filter(|((commission, _), _)| *commission == id)
            .map(|((_, surface), mode)| (surface.clone(), *mode))
            .collect();

        let mut elements: Vec<ElementRow> = {
            let stored = self
                .0
                .elements
                .lock()
                .expect("MemBackend elements mutex poisoned");
            stored
                .iter()
                .filter(|(_, element)| element.commission_id == id)
                .map(|(element_id, element)| ElementRow {
                    id: *element_id,
                    address: element.address.clone(),
                    element_type: element.element_type.clone(),
                    mode: element.mode,
                    band: element.band.clone(),
                    position: element.position,
                    created_by: element.created_by.clone(),
                    created_at: element.created_at,
                    payload: element.payload.clone(),
                })
                .collect()
        };
        elements.sort_by(|left, right| {
            (
                uuid::Uuid::from(left.address.tab),
                left.address.surface.as_ref(),
                left.band.as_ref(),
                left.position,
            )
                .cmp(&(
                    uuid::Uuid::from(right.address.tab),
                    right.address.surface.as_ref(),
                    right.band.as_ref(),
                    right.position,
                ))
        });

        let composition = CommissionComposition {
            tabs,
            surface_modes,
            elements,
        };
        Ok(Some(composition))
    }

    /// Answered from the persisted membership map, never a computed
    /// owner-∪-seated union; an unknown commission answers `false`. Unaffected by
    /// placement or view grants — neither makes an account's members
    /// Participants.
    async fn is_participant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        let participants = self
            .0
            .participants
            .lock()
            .expect("MemBackend participants mutex poisoned");
        Ok(participants.contains_key(&(*commission, user.clone())))
    }

    /// The commission's seat satellites in declaration order (seat ids are
    /// UUIDv7, so id order is declaration order). No seats is the empty list.
    async fn seats(&self, commission: &CommissionId) -> anyhow::Result<Vec<Seat>> {
        let commission = *commission;
        let seats = self
            .0
            .seats
            .lock()
            .expect("MemBackend seats mutex poisoned");
        let mut found: Vec<Seat> = seats
            .iter()
            .filter(|(_, stored)| stored.commission_id == commission)
            .map(|(id, stored)| Seat {
                id: *id,
                kind: stored.kind.clone(),
                prompt: stored.prompt.clone(),
                link: stored.link.clone(),
                occupant: stored.occupant.clone(),
            })
            .collect();
        found.sort_by_key(|seat| uuid::Uuid::from(seat.id));
        Ok(found)
    }

    /// The lone pending seat invitation for `(commission, seat, user)`, or
    /// `None`. Accepted or revoked offers never match, and neither does another
    /// seat's or commission's — the binding lives in the lookup.
    async fn find_pending_seat_invitation(
        &self,
        commission: &CommissionId,
        seat: &ElementId,
        user: &UserId,
    ) -> anyhow::Result<Option<SeatInvitation>> {
        let (commission, seat) = (*commission, *seat);
        let invitations = self
            .0
            .seat_invitations
            .lock()
            .expect("MemBackend seat_invitations mutex poisoned");
        Ok(invitations.iter().find_map(|(id, stored)| {
            (stored.commission == commission
                && stored.seat == seat
                && &stored.invited_user == user
                && stored.state == InvitationState::Pending)
                .then(|| stored.rebuild(*id))
        }))
    }

    /// The file-entry link `key` names within `commission`; a key belonging to a
    /// different commission answers `None`, never an existence oracle.
    async fn find_file(
        &self,
        commission: &CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>> {
        let files = self
            .0
            .files
            .lock()
            .expect("MemBackend files mutex poisoned");
        Ok(files
            .get(&key)
            .filter(|file| &file.commission_id == commission)
            .cloned())
    }

    /// The annotations on one file entry, sorted by `MarkupKey` (UUIDv7, so
    /// creation order). Filtered on the commission too, so a foreign file key
    /// yields an empty vector rather than a signal.
    async fn markups_for_file(
        &self,
        commission: &CommissionId,
        file: FileKey,
    ) -> anyhow::Result<Vec<CommissionMarkup>> {
        let commission = *commission;
        let markups = self
            .0
            .markups
            .lock()
            .expect("MemBackend markups mutex poisoned");

        let mut found: Vec<CommissionMarkup> = markups
            .values()
            .filter(|markup| markup.commission_id == commission && markup.file_id == file)
            .cloned()
            .collect();
        found.sort_by_key(|markup| markup.id);

        Ok(found)
    }

    /// Scan `commissions` for `owner`'s rows, drop archived ones, and rebuild
    /// each. Sorted by [`CommissionId`] (UUIDv7, so creation order), since the
    /// `HashMap` scan has no natural order.
    async fn list_owned_by(&self, owner: &UserId) -> anyhow::Result<Vec<Commission>> {
        let commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let mut owned: Vec<Commission> = commissions
            .iter()
            .filter(|(_, stored)| &stored.owner_id == owner && stored.archived_at.is_none())
            .map(|(id, stored)| stored.rebuild(*id))
            .collect();
        owned.sort_by_key(|commission| *commission.id);
        Ok(owned)
    }
}

/// In-memory [`ChangelogStore`] read surface over the shared [`MemBackend`].
pub struct MemChangelogStore(pub(crate) MemBackend);

#[async_trait]
impl ChangelogStore for MemChangelogStore {
    /// The commission's stream in ascending `seq`; entries are pushed in seq
    /// order, so a filter preserves it.
    async fn entries(&self, commission: &CommissionId) -> anyhow::Result<Vec<ChangelogEntry>> {
        let commission = *commission;
        let changelog = self
            .0
            .changelog
            .lock()
            .expect("MemBackend changelog mutex poisoned");
        Ok(changelog
            .iter()
            .filter(|entry| entry.commission_id == commission)
            .map(StoredChangelogEntry::rebuild)
            .collect())
    }
}

/// Commission seed/inspect helpers: they write straight to the shared state,
/// skipping the begin()/accessor/commit() ceremony.
impl MemBackend {
    /// Persist a commission directly onto the shared store (test seed).
    pub async fn create_commission(&self, commission: &Commission) -> anyhow::Result<()> {
        MemCommissionWrites(self.clone()).create(commission).await
    }

    /// Resolve a commission by id (inspect helper).
    pub async fn find_commission(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        MemCommissionStore(self.clone()).find(&id).await
    }

    /// Every stored commission, rebuilt from its parts, in unspecified order
    /// (inspect helper), so a test can introspect what a bare `201` persisted.
    pub async fn all_commissions(&self) -> anyhow::Result<Vec<Commission>> {
        let commissions = self
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        Ok(commissions
            .iter()
            .map(|(id, stored)| stored.rebuild(*id))
            .collect())
    }

    /// A commission's changelog entries in stream order (inspect helper).
    pub async fn changelog_entries(
        &self,
        commission: CommissionId,
    ) -> anyhow::Result<Vec<ChangelogEntry>> {
        MemChangelogStore(self.clone()).entries(&commission).await
    }

    /// The declared Slot whose carrying element is `element` (inspect helper);
    /// there is no read port for the satellite yet.
    pub async fn find_slot(&self, element: ElementId) -> anyhow::Result<Option<Slot>> {
        let slots = self.slots.lock().expect("MemBackend slots mutex poisoned");
        Ok(slots.get(&element).map(|stored| stored.rebuild(element)))
    }

    /// Every Slot declared on `commission`, in declaration order (carrying
    /// element ids are UUIDv7, so id order is creation order).
    pub async fn slots_of(&self, commission: CommissionId) -> anyhow::Result<Vec<Slot>> {
        let slots = self.slots.lock().expect("MemBackend slots mutex poisoned");
        let mut found: Vec<Slot> = slots
            .iter()
            .filter(|(_, stored)| stored.commission_id == commission)
            .map(|(id, stored)| stored.rebuild(*id))
            .collect();
        found.sort_by_key(|slot| uuid::Uuid::from(slot.element_id));
        Ok(found)
    }

    /// The commission's tabs, in declared order (inspect helper) — how a test
    /// learns the tab id its element writes must address.
    pub async fn tabs_of(&self, commission: CommissionId) -> anyhow::Result<Vec<TabRow>> {
        let composition = MemCommissionStore(self.clone())
            .load_composition(&commission)
            .await?;
        Ok(composition.map(|loaded| loaded.tabs).unwrap_or_default())
    }

    /// The commission's elements, in `(tab, surface, band, position)` order
    /// (inspect helper).
    pub async fn elements_of(&self, commission: CommissionId) -> anyhow::Result<Vec<ElementRow>> {
        let composition = MemCommissionStore(self.clone())
            .load_composition(&commission)
            .await?;
        Ok(composition
            .map(|loaded| loaded.elements)
            .unwrap_or_default())
    }

    /// Plant one extra tab row under an arbitrary declared name (test-only
    /// seeder), for the one case no ordinary path reaches while the skeleton has
    /// a single tab: an address whose tab is real but whose `(tab, surface)` pair
    /// the skeleton does not declare.
    pub fn seed_tab(&self, commission: CommissionId, tab: TabName) -> TabId {
        let id = TabId::mint();
        let stored = StoredTab {
            commission_id: commission,
            tab,
            mode: VisibilityMode::default(),
        };
        self.tabs
            .lock()
            .expect("MemBackend tabs mutex poisoned")
            .insert(id, stored);
        id
    }

    /// Widen (or narrow) a tab's mode directly (test-only seeder); there is no
    /// widening port yet. Panics if `tab` is not a tab of this store.
    pub fn set_tab_mode(&self, tab: TabId, mode: VisibilityMode) {
        self.tabs
            .lock()
            .expect("MemBackend tabs mutex poisoned")
            .get_mut(&tab)
            .expect("set_tab_mode: no such tab")
            .mode = mode;
    }

    /// Widen (or narrow) a surface's mode for one commission (test-only seeder);
    /// writing the entry takes it off the absent-row-means-Total default.
    pub fn set_surface_mode(
        &self,
        commission: CommissionId,
        surface: SurfaceName,
        mode: VisibilityMode,
    ) {
        self.surface_modes
            .lock()
            .expect("MemBackend surface_modes mutex poisoned")
            .insert((commission, surface), mode);
    }

    /// Fill a declared Seat's occupant slot directly (test-only seeder); there
    /// is no seat-fill port yet. Panics if `seat` is not a declared seat.
    pub fn occupy_seat(&self, seat: ElementId, occupant: UserId) {
        let mut seats = self.seats.lock().expect("MemBackend seats mutex poisoned");
        seats
            .get_mut(&seat)
            .expect("occupy_seat: no such declared seat")
            .occupant = Some(occupant);
    }

    /// Seed a non-owner participant membership row (test-only), standing in for
    /// a seated member until the seat-accept path exists.
    pub fn seed_participant(&self, commission: CommissionId, user: UserId) {
        // A re-seed of an already-seated pair is a no-op, preserving the
        // original created_at.
        self.participants
            .lock()
            .expect("MemBackend participants mutex poisoned")
            .entry((commission, user))
            .or_insert_with(chrono::Utc::now);
    }
}

#[cfg(test)]
mod tests;
