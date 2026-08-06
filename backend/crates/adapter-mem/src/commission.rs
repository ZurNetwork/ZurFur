//! In-process fakes of the commission seam (ZMVP-65/87): the stored shapes, the
//! [`CommissionWrites`]/[`ChangelogWrites`] write views (staged by the
//! [`MemUnitOfWork`](crate::MemUnitOfWork), so they commit-or-discard with the
//! unit), the pool-shaped [`MemCommissionStore`]/[`MemChangelogStore`] read
//! stores, and the commission seed/inspect helpers on [`MemBackend`]. Split out
//! of the backend file along the domain seam (the `public_records` precedent) so
//! later commission tickets extend this module instead of one shared hotspot.

use std::collections::HashMap;

use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    account::AccountId,
    commission::{
        Band, ChangelogEntry, ChangelogEntryKind, ChannelPointer, Commission,
        CommissionComposition, CommissionFile, CommissionId, CommissionTitle, DeadlineStatus,
        DirectionStatus, ElementId, ElementPayload, ElementRow, ElementType, FileKey, GrantLevel,
        LapsedDeadline, LifecycleStep, NewChangelogEntry, NewElement, NewSeat, NewSlot, Placement,
        Seat, SeatInvitation, SeatInvitationId, SeatKind, SeatLink, SeatPrompt, Slot, SlotTitle,
        SurfaceAddress, SurfaceName, TabId, TabName, TabRow, Visibility, VisibilityMode,
        declared_tabs, declares_surface, derive_deadline_status,
    },
    invitation::InvitationState,
    maturity::Maturity,
    user::UserId,
};
use domain::ports::{
    ChangelogStore, ChangelogWrites, CommissionStore, CommissionWrites, ElementNotFound,
    UnknownSurface, UnknownTab,
};
use serde_json::Value;

use crate::MemBackend;

/// Resolve a tab within `commission` — the mem mirror of
/// `PgCommissionWrites::require_tab` (ZMVP-166), handing back the tab's
/// **declared name** so the caller can consult the skeleton.
///
/// An absent id and a tab from another commission both refuse with
/// [`UnknownTab`], indistinguishably, before anything about either is revealed.
///
/// pg's version also takes the tab's row lock (`SELECT … FOR UPDATE`), which is
/// what serializes concurrent appends and removals into one tab. There is no
/// lock to mirror here — every mem write already runs under the backend's
/// mutexes — but the *call sites* mirror pg's exactly (both the add path and the
/// removal path go through here, before touching any element), so the two
/// adapters keep one discipline and one observable error order.
///
/// (There is no cross-commission *structural* backstop here the way pg has its
/// composite foreign key; the fake's gate is the whole enforcement, which is why
/// the mem parity tests exercise the foreign-tab case explicitly.)
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

/// The shared **address gate** of every element write — the mem mirror of
/// `PgCommissionWrites::require_address` (ZMVP-166): resolve the tab
/// ([`require_tab`]), then require the code skeleton to declare this surface
/// **inside that tab**, else [`UnknownSurface`]. One path, so the generic add
/// and the two satellite declarations can't drift apart on either rule — and the
/// surface half consults the very same const the pg adapter does, so the two
/// adapters cannot disagree about which addresses are real.
///
/// **The order mirrors pg's, deliberately**: the pair check needs the tab's
/// declared name, so the tab is resolved first and an address that is wrong in
/// *both* ways refuses as [`UnknownTab`], not [`UnknownSurface`]. Mem is
/// single-threaded behind a mutex and has no lock to take, but the observable
/// error order is part of the contract — a parity test pins it.
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

/// The next append `position` within an element's ordering group — the mem
/// mirror of the pg `COALESCE(MAX(position) + 1, 0)` subquery, counted over the
/// same `(commission, tab, surface, band)` tuple the pg statement filters on.
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

/// Insert one element on the unit's staged snapshot, behind the shared address
/// gate — the mem mirror of `PgCommissionWrites::insert_element`, and the single
/// write path every element takes (the generic add, a Slot's carrier, a Seat's).
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
        created_by: element.created_by,
        created_at: element.created_at,
        payload: element.payload.clone(),
    };
    elements.insert(element.id, stored);
    Ok(())
}

/// The fields of a [`Commission`] we keep behind the lock. Like `Account`,
/// `Commission` isn't `Clone` (an aggregate root, not a value), so we store its
/// parts and rebuild a fresh `Commission` on read. `Clone` so a unit of work can
/// deep-copy the commissions map into its staging snapshot (see
/// [`MemBackend::stage`]). `PartialEq` lets [`crate::merge_map`] diff a unit's
/// staged value against its pristine base snapshot to tell an untouched row
/// (rode along in the snapshot) apart from one this unit actually wrote.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredCommission {
    /// The commission's fixed, always-present Title (ZMVP-65), validated non-empty.
    pub(crate) title: CommissionTitle,
    /// The User who created it and owns it — the permanent owner (DESIGN/Commission).
    pub(crate) owner_id: UserId,
    /// Its single [`LifecycleStep`]; a freshly created commission is `Draft`.
    pub(crate) lifecycle_step: LifecycleStep,
    /// Who may see it; a freshly created commission is [`Visibility::Private`].
    pub(crate) visibility: Visibility,
    /// The nullable-but-fixed deadline envelope field.
    pub(crate) deadline: Option<domain::datetime::DateTimeUtc>,
    /// The maturity posture, or `None` while unrated (ZMVP-31) — the mem
    /// mirror of the pg `maturity` + `graphic` column pair (one field here:
    /// the both-or-neither CHECK is a struct by construction).
    pub(crate) maturity: Option<Maturity>,
    /// The direction-axis Status, or `None` while none is set (ZMVP-85) — the
    /// mem mirror of the pg `direction_status` column: one nullable cell, so a
    /// set replaces by construction.
    pub(crate) direction_status: Option<DirectionStatus>,
    /// The deadline-axis Status, or `None` while none is held (ZMVP-86) — the
    /// mem mirror of the pg `deadline_status` column: the same one-cell shape.
    pub(crate) deadline_status: Option<DeadlineStatus>,
    /// The external linked-channel pointer, or `None` while none is declared
    /// (ZMVP-87 AC3) — the mem mirror of the pg `linked_channel` column.
    pub(crate) linked_channel: Option<ChannelPointer>,
    /// When the commission was archived, or `None` while active (ZMVP-68) —
    /// the mem mirror of the pg `archived_at` column.
    pub(crate) archived_at: Option<domain::datetime::DateTimeUtc>,
    /// When the commission was created.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
}

impl StoredCommission {
    /// Rebuild the aggregate from its stored parts (the commission analogue of
    /// how `find` rebuilds an `Account`).
    fn rebuild(&self, id: CommissionId) -> Commission {
        // Late is derived fresh at lookup, never persisted — the pg `find`
        // mirror (Engineer ruling 2026-07-08). The stored `deadline_status` is
        // the manual `Delayed` flag only.
        let deadline_status = derive_deadline_status(
            self.deadline,
            &self.lifecycle_step,
            self.deadline_status,
            chrono::Utc::now(),
        );
        Commission {
            id,
            title: self.title.clone(),
            owner_id: self.owner_id,
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

/// One commission element as the mem backend keeps it — the in-memory mirror of
/// a pg `commission_element` row (ZMVP-166). Keyed by [`ElementId`] in the
/// backend map, so the row's own id lives in the key. `Clone` so a unit of work
/// can deep-copy the element map into its staging snapshot. `PartialEq` lets
/// [`crate::merge_map`] diff a unit's staged value against its pristine base
/// snapshot to tell an untouched row apart from one this unit actually wrote.
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
    /// The type-owned payload, opaque here exactly as in pg — and carried in
    /// its non-serializable wrapper, so the fake cannot become the easy route
    /// around the guard the real store keeps.
    pub(crate) payload: ElementPayload,
}

/// One commission tab as the mem backend keeps it — the in-memory mirror of a pg
/// `commission_tab` row (ZMVP-166), keyed by [`TabId`]. Minted with the
/// commission (the withheld-at-birth discipline), never removed. `Clone` and
/// `PartialEq` for the same staging/merge reasons as [`StoredElement`].
#[derive(Clone, PartialEq)]
pub(crate) struct StoredTab {
    /// The commission this tab belongs to — the mem stand-in for the composite
    /// foreign key that binds an element's tab to its own commission in pg.
    pub(crate) commission_id: CommissionId,
    /// The declared tab id this row realizes (a skeleton name).
    pub(crate) tab: TabName,
    /// The tab's visibility mode: the first term of the min.
    pub(crate) mode: VisibilityMode,
}

/// One declared Slot's **satellite** as the mem backend keeps it — the
/// in-memory mirror of a pg `commission_slot` row (ZMVP-77). Keyed in the
/// backend map by the [`ElementId`] of the element that carries the Slot (the
/// satellite's own key), exactly like the pg table. Deliberately occupant-less: fill is unrepresentable until the
/// Character epic adds it. `Clone` so a unit of work can deep-copy the map into
/// its staging snapshot. `PartialEq` lets [`crate::merge_map`] diff a unit's
/// staged value against its pristine base snapshot to tell an untouched row
/// apart from one this unit actually wrote.
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

/// One declared Seat's interpreted half as the mem backend keeps it — the
/// in-memory mirror of a pg `commission_seat` row (ZMVP-76), keyed by the
/// seat's [`ElementId`] in the backend map (one identity: the element in
/// [`StoredElement`], this satellite here). `Clone` so a unit of work can
/// deep-copy the seat map into its staging snapshot. `PartialEq` lets
/// [`crate::merge_map`] diff a unit's staged value against its pristine base
/// snapshot to tell an untouched row apart from one this unit actually wrote.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredSeat {
    /// The owning commission — the mem mirror of the denormalized
    /// `commission_seat.commission_id` column backing the seats() read.
    pub(crate) commission_id: CommissionId,
    /// The seat's semantic kind (open vocabulary; kinds repeat freely).
    pub(crate) kind: SeatKind,
    /// The optional free-text requirement prompt riding the vacant seat.
    pub(crate) prompt: Option<SeatPrompt>,
    /// The optional external requirements link riding the vacant seat.
    pub(crate) link: Option<SeatLink>,
    /// The single occupant slot — `None` from declaration until ZMVP-79 fills
    /// it; at most one occupant is unrepresentable to violate (AC3).
    pub(crate) occupant: Option<UserId>,
}

/// One pending (or once-pending) seat invitation as the mem backend keeps it —
/// the in-memory mirror of a pg `commission_invitation` row (ZMVP-78), keyed by
/// the [`SeatInvitationId`] in the backend map. Stored as parts because
/// [`SeatInvitation`] isn't `Clone` (an entity with a lifecycle, like
/// `Invitation`); a read rebuilds a fresh one. `Clone` so a unit of work can
/// deep-copy the map into its staging snapshot. `PartialEq` lets
/// [`crate::merge_map`] diff a unit's staged value against its pristine base
/// snapshot to tell an untouched row apart from one this unit actually wrote.
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
    /// Where the offer sits in its lifecycle. [`InvitationState`] is `Copy`.
    pub(crate) state: InvitationState,
    /// When the invitation was issued.
    pub(crate) created_at: DateTimeUtc,
    /// When the invitation last changed state.
    pub(crate) updated_at: DateTimeUtc,
}

impl StoredSeatInvitation {
    /// Rebuild the domain [`SeatInvitation`] from the stored parts (it isn't
    /// `Clone`).
    fn rebuild(&self, id: SeatInvitationId) -> SeatInvitation {
        SeatInvitation {
            id,
            commission: self.commission,
            seat: self.seat,
            invited_user: self.invited_user,
            inviter: self.inviter,
            state: self.state,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// One appended changelog entry as the mem backend keeps it — the in-memory
/// mirror of a pg `commission_changelog` row (ZMVP-87). `Clone` so a unit of
/// work can deep-copy the log into its staging snapshot. A push never rewrites
/// an existing entry; the only way one disappears is `delete`'s whole-commission
/// cascade. `PartialEq` lets the Unit-of-Work's commit-time merge diff a unit's
/// staged log against its pristine base snapshot by value — an entry carries no
/// id apart from its own data, so equality IS identity here.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredChangelogEntry {
    /// The store-assigned ordering key — the mem mirror of the pg `bigserial`
    /// (global, monotonic, not per-commission).
    pub(crate) seq: i64,
    /// The stream the entry belongs to.
    pub(crate) commission_id: CommissionId,
    /// What act the entry records.
    pub(crate) kind: ChangelogEntryKind,
    /// Who did it — `None` for a system entry.
    pub(crate) actor_id: Option<UserId>,
    /// Kind-specific parameters (JSON), self-sufficient to render a sentence.
    pub(crate) payload: Value,
    /// Free text riding the entry, if any.
    pub(crate) note: Option<String>,
    /// When the act happened — carried for display; `seq` is the order.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
}

impl StoredChangelogEntry {
    /// Rebuild the read shape from the stored parts.
    fn rebuild(&self) -> ChangelogEntry {
        ChangelogEntry {
            seq: self.seq,
            commission_id: self.commission_id,
            kind: self.kind,
            actor_id: self.actor_id,
            payload: self.payload.clone(),
            note: self.note.clone(),
            created_at: self.created_at,
        }
    }
}

/// One placement-log row as the mem backend keeps it — the in-memory mirror of a
/// pg `commission_placement` row (ZMVP-70), and (with the latest `seq` per
/// commission) of the `commission_current_placement` cache pointer. `Clone` so a
/// unit of work can deep-copy the log/cache into its staging snapshot. Append-only
/// like the pg log: nothing here mutates a pushed row (an account's hard-delete can
/// still remove one, ZMVP-57 AC1). `PartialEq` lets the Unit-of-Work's commit-time
/// merge diff a unit's staged log against its pristine base snapshot by value — a
/// row carries no id apart from its own data, so equality IS identity here.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredPlacement {
    /// The store-assigned ordering key — the mem mirror of the pg `bigserial`
    /// (global, monotonic): the greatest `seq` for a commission is its current
    /// placement, the least its origin.
    pub(crate) seq: i64,
    /// The commission being positioned.
    pub(crate) commission_id: CommissionId,
    /// The account into whose position the commission was placed.
    pub(crate) account_id: AccountId,
    /// The User who performed the placement (the owner in v1).
    pub(crate) placed_by: UserId,
    /// When the placement happened.
    pub(crate) placed_at: DateTimeUtc,
}

impl StoredPlacement {
    /// Rebuild the domain [`Placement`] from the stored parts.
    fn rebuild(&self) -> Placement {
        Placement {
            seq: self.seq,
            commission_id: self.commission_id,
            account_id: self.account_id,
            placed_by: self.placed_by,
            placed_at: self.placed_at,
        }
    }
}

/// In-memory [`CommissionWrites`] view: commission writes land on the shared
/// state. Vended by [`MemUnitOfWork::commissions`](crate::MemUnitOfWork), where
/// the [`MemBackend`] it wraps is the unit's *staging* snapshot — so a write
/// reaches the shared store only on commit (drop = rollback), exactly like
/// `MemAccountWrites`.
pub struct MemCommissionWrites(pub(crate) MemBackend);

#[async_trait]
impl CommissionWrites for MemCommissionWrites {
    /// Insert the freshly created commission, keyed by its id — **together with
    /// one tab row per tab the code skeleton declares** ([`declared_tabs`],
    /// ZMVP-166) **and its owner's participant row** (ZMVP-76: the owner is a
    /// permanent Participant from birth, stamped with the commission's creation
    /// instant), the mem mirror of the pg adapter's inserts in one transaction:
    /// all three maps belong to this unit's staging snapshot, so commission,
    /// tabs, and membership commit or vanish together — a tabless or owner-less
    /// commission is unrepresentable. Every tab is born
    /// [`VisibilityMode::Total`], and nothing here reads
    /// `commission.visibility` (the commission is the formal root; its
    /// visibility gates *over* the composition rather than seeding it). The pg
    /// `id` is a
    /// PRIMARY KEY, so a duplicate would raise a violation there; the fake does
    /// not model that (a plain `insert`, the same as `MemAccountWrites::create`
    /// does for its own account id), because commission ids are freshly-minted
    /// UUIDv7 — a collision is unreachable by construction, never a case a test
    /// can reach.
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
                    owner_id: commission.owner_id,
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
        // A duplicate add is a no-op that preserves the ORIGINAL created_at —
        // the mem mirror of the pg `ON CONFLICT (commission_id, user_id) DO
        // NOTHING` (ZMVP-140): a fresh commission's owner row can't collide
        // here, but ZMVP-79's seat acceptance re-adds whoever it seats, who
        // may already be a participant through another seat.
        participants
            .entry((commission.id, commission.owner_id))
            .or_insert(commission.created_at);
        Ok(())
    }

    /// Contribute one element into a declared surface — the mem mirror of the pg
    /// `INSERT … position = max + 1 within (tab, surface, band)` (ZMVP-166),
    /// behind the same shared address gate ([`require_address`]), in the same
    /// order: an absent/foreign tab refuses with [`UnknownTab`], and only then a
    /// surface the skeleton does not declare **in that tab** with
    /// [`UnknownSurface`]. The element is born [`VisibilityMode::Total`] and its
    /// opaque payload is held verbatim, so it reads back exactly as written.
    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()> {
        insert_element(&self.0, element)
    }

    /// Remove one element — the mem mirror of the pg gate + tab lock + `DELETE`
    /// + renumber (ZMVP-166): the target must exist in `commission` (an absent
    /// id and a foreign element both refuse with [`ElementNotFound`],
    /// indistinguishably, so removal probes reveal nothing), its tab must
    /// resolve through the **same gate the add path uses** ([`require_tab`] —
    /// where pg takes the row lock that serializes a removal against a
    /// concurrent append), whatever shares its identity leaves with it (the pg
    /// `ON DELETE CASCADE` on the Slot/Seat satellites and a seat's pending
    /// invitations, walked here explicitly), and the remaining
    /// `(tab, surface, band)` group renumbers to contiguous positions — all on
    /// the unit's staging snapshot, so removal and renumber commit or vanish
    /// together. There is no protected element: tabs and surfaces are skeleton,
    /// not elements.
    async fn remove_element(
        &mut self,
        commission: CommissionId,
        element: ElementId,
    ) -> anyhow::Result<()> {
        // `tabs` before `elements`, the SAME order `insert_element` takes — the
        // mem mirror of pg's "lock the tab row before touching an element row",
        // and the reason the two maps can never be acquired in opposite orders
        // by two paths.
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
        // The removal's tab, resolved through the same gate the add path uses —
        // pg takes this as a row lock between its element gate and its DELETE.
        // A miss is corruption (pg's composite foreign key makes it unwritable),
        // and corruption answers with the same UnknownTab pg would.
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

        // The identity-sharing satellites, and a seat's pending offers: the mem
        // mirror of the pg cascades off `commission_element (id)`.
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

    /// Record a file entry's link on the unit's staged snapshot (ZMVP-88) — the
    /// in-memory mirror of the pg `INSERT INTO commission_file`, so the link commits
    /// atomically with the `file_added` changelog entry the caller appends on the
    /// same unit (drop = rollback). The bytes were stored separately, before this
    /// unit, through [`FileStore`](domain::ports::FileStore).
    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()> {
        let mut files = self
            .0
            .files
            .lock()
            .expect("MemBackend files mutex poisoned");
        files.insert(file.id, file.clone());
        Ok(())
    }

    /// Declare a batch of Slots — the mem mirror of the pg per-Slot two-insert
    /// transaction (ZMVP-77; array operation per the PR #108 ruling): per
    /// Slot, the same shared address gate ([`require_address`]) and append
    /// order as [`insert_element`] plant an ordinary [`ElementType::slot`]-typed
    /// element with the empty payload, and the Slot itself lands as the
    /// [`StoredSlot`] satellite keyed by that element's id. All maps belong to
    /// this unit's staging snapshot, so the whole batch commits or vanishes
    /// together — a refusal mid-batch errors the unit and nothing is applied.
    /// No changelog entry (the frozen taxonomy has no Slot variant), and no
    /// occupant exists to store.
    async fn declare_slots(&mut self, new_slots: &[NewSlot]) -> anyhow::Result<()> {
        for slot in new_slots {
            let carrier = NewElement::carrying(
                slot.id,
                slot.commission_id,
                slot.address.clone(),
                ElementType::slot(),
                slot.created_by,
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

    /// Whether the commission bears any fact (ZMVP-67) — the in-memory mirror of
    /// the pg predicate, answered on the unit's staged snapshot so the fake keeps
    /// the same-transaction semantics the delete gate (ZMVP-66) relies on.
    ///
    /// Constant `false` for the same reason the pg body is: no fact-minter exists,
    /// so `MemBackend` holds no fact map any query could scan. The fact registry
    /// and its tripwires live in the pg adapter (`COMMISSION_FACT_TABLES` in
    /// `adapter-pg/src/commission.rs`, Deletion DD `3014657`); the change that
    /// registers the first fact table there MUST also give this fake the matching
    /// fact map and check it here, or mem-backed gate tests would pass against a
    /// predicate blind to the facts they stage.
    async fn commission_has_facts(&mut self, _id: CommissionId) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Remove the commission and, with it, its changelog entries **and its whole
    /// composition** — the mem mirror of the pg `DELETE FROM commission` plus
    /// every child table's `ON DELETE CASCADE` (ZMVP-66; ruling E35). Lands on
    /// the unit's staged snapshot, so it commits or rolls back with the caller's
    /// fact gate (ruling E17), like every write here. An absent commission is a
    /// no-op, per the port contract.
    ///
    /// The composition arm (tabs, elements, surface modes, and the Slot/Seat
    /// satellites with a seat's pending offers) is swept because without it
    /// [`load_composition`](CommissionStore::load_composition) would answer `Some` for
    /// a commission pg answers `None` for — the fake lying about "gone
    /// entirely". Maps this ticket does **not** own (participants, files,
    /// positioning) still don't cascade here; that divergence predates ZMVP-166
    /// and belongs to whoever owns them.
    ///
    /// A future commission-child map added to [`MemBackend`] must cascade here
    /// too, mirroring its pg table's cascade.
    async fn delete(&mut self, id: CommissionId) -> anyhow::Result<()> {
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

        // The composition, in the order pg's cascade reaches it: a seat's
        // offers, the satellites, the elements they rode, then the tabs and
        // surface modes.
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

    /// Flip the stored archive stamp (ZMVP-68) — the mem mirror of the pg
    /// conditional `UPDATE`: the write applies only on a **real transition**
    /// (the `is_none`/`is_some` arms differ between stored and requested), so a
    /// repeat in the same direction changes nothing, answers `false`, and keeps
    /// the original stamp. An absent commission answers `false` (existence is
    /// the caller's check). Staged like every write here: shared state moves
    /// only on commit.
    async fn set_archived(
        &mut self,
        id: CommissionId,
        archived_at: Option<domain::datetime::DateTimeUtc>,
    ) -> anyhow::Result<bool> {
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
    /// `UPDATE commission SET maturity, graphic` (ZMVP-31). Replace-only by
    /// signature (no clear arm exists); an absent commission is a no-op, per
    /// the port contract (existence is the caller's check).
    async fn set_maturity(&mut self, id: CommissionId, maturity: Maturity) -> anyhow::Result<()> {
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
    /// Declare a seat — the mem mirror of the pg adapter's element + satellite
    /// pair (ZMVP-76): behind the same shared address gate
    /// ([`require_address`]), one [`StoredElement`] (an ordinary
    /// [`ElementType::seat`]-typed element) and one [`StoredSeat`] land under
    /// the same [`ElementId`] in this unit's staging snapshot, so both halves
    /// commit or vanish together. The occupant is never written here: every
    /// seat is born vacant (AC3; ZMVP-79 fills it).
    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()> {
        let carrier = NewElement::carrying(
            seat.id,
            seat.commission_id,
            seat.address.clone(),
            ElementType::seat(),
            seat.created_by,
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

    /// Insert the pending seat invitation, unless one is already pending for the
    /// same `(seat, invited_user)` — in which case this is a no-op, the in-memory
    /// mirror of the pg partial unique index (`... WHERE state = 'pending'`,
    /// ZMVP-78). The handler also checks
    /// [`find_pending_seat_invitation`](CommissionStore::find_pending_seat_invitation)
    /// first, so this is the belt-and-suspenders backstop. Several *different*
    /// Users may hold pending invitations to one Seat — only a duplicate for the
    /// same pair is dropped. Staged like every write here.
    async fn create_seat_invitation(&mut self, invitation: &SeatInvitation) -> anyhow::Result<()> {
        let mut invitations = self
            .0
            .seat_invitations
            .lock()
            .expect("MemBackend seat_invitations mutex poisoned");
        let already_pending = invitations.values().any(|stored| {
            stored.seat == invitation.seat
                && stored.invited_user == invitation.invited_user
                && stored.state == InvitationState::Pending
        });
        if already_pending {
            // At most one pending offer per (seat, user): a second issue is a
            // no-op, not a second row.
            return Ok(());
        }
        invitations.insert(
            invitation.id,
            StoredSeatInvitation {
                commission: invitation.commission,
                seat: invitation.seat,
                invited_user: invitation.invited_user,
                inviter: invitation.inviter,
                state: invitation.state,
                created_at: invitation.created_at,
                updated_at: invitation.updated_at,
            },
        );
        Ok(())
    }

    /// Flip a pending seat invitation to revoked and stamp `updated_at`. A
    /// non-pending or absent invitation is left untouched — a no-op, not an error
    /// (the handler decides whether that's a 404/200), mirroring the pg guarded
    /// `UPDATE` (ZMVP-78). Staged like every write here.
    async fn revoke_seat_invitation(&mut self, id: SeatInvitationId) -> anyhow::Result<()> {
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

    /// Repoint (or clear) the stored linked-channel pointer — the mem mirror of
    /// the pg conditional `UPDATE`: the write applies only when the stored value
    /// differs from the requested one, so a repeat answers `false` and the
    /// caller's changelog append keys on the bool. An absent commission answers
    /// `false`, per the port contract (existence is the caller's check).
    async fn set_linked_channel(
        &mut self,
        id: CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
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

    /// Append a placement-log row and repoint the current-placement cache to it —
    /// the mem mirror of the pg append + `commission_current_placement` upsert,
    /// both on the unit's staged snapshot so they land atomically on commit. The
    /// `seq` is the next over the whole placement log (the mem mirror of the pg
    /// global `bigserial`), and the cache is overwritten with this row — so the
    /// cache always equals the latest log row. Re-placement always appends; the
    /// log is never rewritten.
    async fn place(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        placed_by: UserId,
        at: DateTimeUtc,
    ) -> anyhow::Result<()> {
        let mut placements = self
            .0
            .placements
            .lock()
            .expect("MemBackend placements mutex poisoned");
        let seq = placements.last().map(|p| p.seq + 1).unwrap_or(1);
        let row = StoredPlacement {
            seq,
            commission_id: commission,
            account_id: account,
            placed_by,
            placed_at: at,
        };
        placements.push(row.clone());
        drop(placements);

        self.0
            .current_placements
            .lock()
            .expect("MemBackend current_placements mutex poisoned")
            .insert(commission, row);
        Ok(())
    }

    /// Upsert the account's key on the unit's staged snapshot — the mem mirror of
    /// the pg `commission_view_grant` upsert: one key per (commission, account),
    /// re-granting replaces the level.
    async fn grant_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        self.0
            .view_grants
            .lock()
            .expect("MemBackend view_grants mutex poisoned")
            .insert((commission, account), level);
        Ok(())
    }

    /// Remove the account's key on the staged snapshot (hard-delete, DD `29130754`
    /// D5) — the mem mirror of the pg `DELETE`. Returns whether a key existed: a
    /// revoke of a non-existent key is an idempotent no-op answering `false`, the
    /// bool the caller keys its `view_grant_revoked` changelog append on.
    async fn revoke_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<bool> {
        Ok(self
            .0
            .view_grants
            .lock()
            .expect("MemBackend view_grants mutex poisoned")
            .remove(&(commission, account))
            .is_some())
    }

    /// Repoint (or clear) the stored direction-axis Status — the mem mirror of
    /// the pg `UPDATE commission SET direction_status` (ZMVP-85): one nullable
    /// slot, so a set replaces whole. An absent commission is a no-op, per the
    /// port contract (existence is the caller's check).
    async fn set_direction_status(
        &mut self,
        id: CommissionId,
        status: Option<DirectionStatus>,
    ) -> anyhow::Result<bool> {
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

    /// Repoint (or clear) the stored deadline — the mem mirror of the pg
    /// `UPDATE commission SET deadline` (ZMVP-86). An absent commission is a
    /// no-op, per the port contract (existence is the caller's check).
    async fn set_deadline(
        &mut self,
        id: CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
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

    /// Repoint (or clear) the stored deadline-axis Status — the mem mirror of
    /// the pg `UPDATE commission SET deadline_status` (ZMVP-86): one nullable
    /// slot, so a set replaces whole. An absent commission is a no-op, per the
    /// port contract.
    async fn set_deadline_status(
        &mut self,
        id: CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<bool> {
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

    /// The sweeper's candidate scan — the mem mirror of the pg query (ZMVP-86,
    /// ruling E12), answered on the unit's staged snapshot so the scan already
    /// sees this unit's writes (the same same-transaction semantics as
    /// [`commission_has_facts`](CommissionWrites::commission_has_facts)):
    /// deadline strictly before `now`, not already Late, lifecycle not
    /// terminal; ordered by deadline (id tiebreak) like the pg `ORDER BY`.
    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>> {
        // Late is never persisted, so dedup the log on the changelog itself (the
        // pg anti-join mirror). A commission is skipped only if its latest `late`
        // entry is *after* its latest deadline change — a `deadline_set` /
        // `deadline_extended` re-arms the log, so each fresh miss is its own
        // event. Its Late *state* is derived on lookup; this pass only decides
        // what still needs an entry.
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
/// snapshot and reach the shared store only on commit (drop = rollback) — the
/// mem mirror of the DD's entries-commit-atomically-with-domain-writes rule.
pub struct MemChangelogWrites(pub(crate) MemBackend);

#[async_trait]
impl ChangelogWrites for MemChangelogWrites {
    /// Push one entry, assigning the next `seq` — the mem mirror of the pg
    /// `bigserial` (monotonic over the whole log, like the single sequence).
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
            actor_id: entry.actor_id,
            payload: entry.payload.clone(),
            note: entry.note.clone(),
            created_at: entry.created_at,
        });
        Ok(())
    }
}

/// In-memory [`CommissionStore`] read surface over the shared [`MemBackend`] —
/// the canonical commission read port's fake (ZMVP-87).
pub struct MemCommissionStore(pub(crate) MemBackend);

#[async_trait]
impl CommissionStore for MemCommissionStore {
    /// Rebuilds a [`Commission`] from its stored parts (it isn't `Clone`), or
    /// `None` if never created.
    async fn find(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        let commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        Ok(commissions.get(&id).map(|stored| stored.rebuild(id)))
    }

    /// The current-placement pointer (ZMVP-70) from the cache map, or `None` if
    /// the commission was never placed — the mem mirror of a
    /// `commission_current_placement` read.
    async fn current_placement(
        &self,
        commission: CommissionId,
    ) -> anyhow::Result<Option<Placement>> {
        Ok(self
            .0
            .current_placements
            .lock()
            .expect("MemBackend current_placements mutex poisoned")
            .get(&commission)
            .map(StoredPlacement::rebuild))
    }

    /// The commission's placement log in append order (ascending `seq`) — the
    /// rows are pushed in seq order, so filtering preserves it (the mem mirror of
    /// `ORDER BY seq`). An unplaced commission has an empty log.
    async fn placement_log(&self, commission: CommissionId) -> anyhow::Result<Vec<Placement>> {
        Ok(self
            .0
            .placements
            .lock()
            .expect("MemBackend placements mutex poisoned")
            .iter()
            .filter(|p| p.commission_id == commission)
            .map(StoredPlacement::rebuild)
            .collect())
    }

    /// The [`GrantLevel`] `account` holds on `commission`, or `None` (ZMVP-70) —
    /// the mem mirror of a `commission_view_grant` lookup.
    async fn view_grant(
        &self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<Option<GrantLevel>> {
        Ok(self
            .0
            .view_grants
            .lock()
            .expect("MemBackend view_grants mutex poisoned")
            .get(&(commission, account))
            .copied())
    }

    /// Load the whole composition — the mem mirror of the pg three-query read
    /// (ZMVP-166): the tab, surface-mode, and element maps each filtered by
    /// commission and ordered the way the pg statements order them, so a caller
    /// observes the same shape from either adapter. `None` when the commission
    /// has **no tabs** (tabs are minted with the commission, so that means no
    /// such commission); an empty `elements` list is the ordinary state of a
    /// fresh one and is not absence.
    async fn load_composition(
        &self,
        id: CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>> {
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
        tabs.sort_by(|left, right| left.tab.as_str().cmp(right.tab.as_str()));

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
                    created_by: element.created_by,
                    created_at: element.created_at,
                    payload: element.payload.clone(),
                })
                .collect()
        };
        elements.sort_by(|left, right| {
            (
                *left.address.tab,
                left.address.surface.as_str(),
                left.band.as_str(),
                left.position,
            )
                .cmp(&(
                    *right.address.tab,
                    right.address.surface.as_str(),
                    right.band.as_str(),
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

    /// Answers from the **persisted membership map** (ZMVP-76, Engineer
    /// ruling: the mem mirror of `commission_participant`, never a computed
    /// owner-∪-seated union): the owner's entry is inserted with the
    /// commission; ZMVP-79's seated arm adds entries behind this same lookup.
    /// An unknown commission has no entries, so it answers `false`.
    /// **Unaffected by placement or view grants** (Ownership Separation DD
    /// Decision 8): positioning is environmental and a key is only a view, so
    /// neither makes an account's members Participants.
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool> {
        let participants = self
            .0
            .participants
            .lock()
            .expect("MemBackend participants mutex poisoned");
        Ok(participants.contains_key(&(commission, user)))
    }

    /// The commission's seat satellites in declaration order — the mem mirror
    /// of the pg `ORDER BY id` read (seat ids are UUIDv7, so id order is
    /// declaration order). No seats (or no commission) is the empty list.
    async fn seats(&self, commission: CommissionId) -> anyhow::Result<Vec<Seat>> {
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
                occupant: stored.occupant,
            })
            .collect();
        found.sort_by_key(|seat| *seat.id);
        Ok(found)
    }

    /// The lone pending seat invitation for `(commission, seat, user)`, or `None`
    /// (ZMVP-78) — the mem mirror of the pg query scoped to
    /// `commission_id`/`seat_id`/`invited_user`/pending. Accepted/revoked
    /// invitations are history, not live offers, so they never match; a
    /// *different* seat's — or another commission's — offer never matches either
    /// (the authorization binding lives in the lookup, not caller discipline).
    async fn find_pending_seat_invitation(
        &self,
        commission: CommissionId,
        seat: ElementId,
        user: UserId,
    ) -> anyhow::Result<Option<SeatInvitation>> {
        let invitations = self
            .0
            .seat_invitations
            .lock()
            .expect("MemBackend seat_invitations mutex poisoned");
        Ok(invitations.iter().find_map(|(id, stored)| {
            (stored.commission == commission
                && stored.seat == seat
                && stored.invited_user == user
                && stored.state == InvitationState::Pending)
                .then(|| stored.rebuild(*id))
        }))
    }

    /// The file-entry link `key` names **within `commission`** (ZMVP-88) — the mem
    /// mirror of the pg query filtered by both id and commission_id: a key that
    /// belongs to a *different* commission answers `None` (never a cross-commission
    /// existence oracle).
    async fn find_file(
        &self,
        commission: CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>> {
        let files = self
            .0
            .files
            .lock()
            .expect("MemBackend files mutex poisoned");
        Ok(files
            .get(&key)
            .filter(|file| file.commission_id == commission)
            .cloned())
    }

    /// Scans `commissions` for `owner`'s rows, drops archived ones (ZMVP-157 —
    /// the mem mirror of the pg `archived_at IS NULL` filter), and rebuilds
    /// each via [`StoredCommission::rebuild`] — the same reconstruction
    /// [`find`](Self::find) uses. Sorted by [`CommissionId`] afterward (UUIDv7
    /// sorts as creation order); the `HashMap` scan itself has no natural
    /// order, mirroring the pg `ORDER BY id`.
    async fn list_owned_by(&self, owner: UserId) -> anyhow::Result<Vec<Commission>> {
        let commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        let mut owned: Vec<Commission> = commissions
            .iter()
            .filter(|(_, stored)| stored.owner_id == owner && stored.archived_at.is_none())
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
    /// The commission's stream in ascending `seq` — the entries are pushed in
    /// seq order, so a filter preserves it (the mem mirror of `ORDER BY seq`).
    async fn entries(&self, commission: CommissionId) -> anyhow::Result<Vec<ChangelogEntry>> {
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

/// Commission seed/inspect helpers on the shared backend — they operate directly
/// on the shared state (reusing the read/write impls) so a test can arrange and
/// assert without the `begin()`/accessor/`commit()` ceremony.
impl MemBackend {
    /// Persist a commission directly onto the shared store (test seed of
    /// [`CommissionWrites::create`]) — e.g. one owned by a user who is *not* the
    /// app's signed-in identity, to exercise the closed door.
    pub async fn create_commission(&self, commission: &Commission) -> anyhow::Result<()> {
        MemCommissionWrites(self.clone()).create(commission).await
    }

    /// Resolve a commission by id (inspect helper; the read-port fake is
    /// [`MemCommissionStore`], reachable via [`MemBackend::commission_store`]).
    pub async fn find_commission(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        MemCommissionStore(self.clone()).find(id).await
    }

    /// Every stored commission, rebuilt from its parts, in unspecified order
    /// (inspect helper). Lets an api test that drives `POST /commissions` — which
    /// returns a bare `201` with no id — introspect what was persisted.
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

    /// A commission's changelog entries in stream order (inspect helper — the
    /// read-port fake reached without wiring a store).
    pub async fn changelog_entries(
        &self,
        commission: CommissionId,
    ) -> anyhow::Result<Vec<ChangelogEntry>> {
        MemChangelogStore(self.clone()).entries(commission).await
    }

    /// The declared Slot whose carrying element is `element`, or `None` (inspect
    /// helper — the satellite read; ZMVP-77 exposes no read port yet, the
    /// viewer-facing surface being ZMVP-170's projection).
    pub async fn find_slot(&self, element: ElementId) -> anyhow::Result<Option<Slot>> {
        let slots = self.slots.lock().expect("MemBackend slots mutex poisoned");
        Ok(slots.get(&element).map(|stored| stored.rebuild(element)))
    }

    /// Every Slot declared on `commission`, in declaration order (the carrying
    /// elements' ids are UUIDv7, so sorting by element id is creation order) —
    /// the "zero or more" count of ZMVP-77 AC2 (inspect helper).
    pub async fn slots_of(&self, commission: CommissionId) -> anyhow::Result<Vec<Slot>> {
        let slots = self.slots.lock().expect("MemBackend slots mutex poisoned");
        let mut found: Vec<Slot> = slots
            .iter()
            .filter(|(_, stored)| stored.commission_id == commission)
            .map(|(id, stored)| stored.rebuild(*id))
            .collect();
        found.sort_by_key(|slot| *slot.element_id);
        Ok(found)
    }

    /// The commission's tabs, in declared order (inspect helper). ZMVP-166
    /// exposes no *route* that hands a caller a tab id — reading the composition
    /// is ZMVP-163's `GET` — so this is how a test learns the id its element
    /// writes must address.
    pub async fn tabs_of(&self, commission: CommissionId) -> anyhow::Result<Vec<TabRow>> {
        let composition = MemCommissionStore(self.clone())
            .load_composition(commission)
            .await?;
        Ok(composition.map(|loaded| loaded.tabs).unwrap_or_default())
    }

    /// The commission's elements, in `(tab, surface, band, position)` order
    /// (inspect helper — the composition read reached without wiring a store).
    pub async fn elements_of(&self, commission: CommissionId) -> anyhow::Result<Vec<ElementRow>> {
        let composition = MemCommissionStore(self.clone())
            .load_composition(commission)
            .await?;
        Ok(composition
            .map(|loaded| loaded.elements)
            .unwrap_or_default())
    }

    /// Plant one extra tab row on the shared store under an arbitrary declared
    /// name, returning its [`TabId`] (test-only seeder; the mem mirror of a raw
    /// `INSERT INTO commission_tab`).
    ///
    /// Exists for **one** case: exercising an address whose tab is real and
    /// belongs to this commission, but whose `(tab, surface)` pair the skeleton
    /// does not declare. The placeholder skeleton has a single tab, so no
    /// ordinary path can produce that shape — and it is precisely the shape the
    /// pair check exists to refuse. ZMVP-171's real, multi-tab skeleton makes
    /// this reachable without a seeder; until then, this stands in.
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

    /// Widen (or narrow) a **tab's** mode directly on the shared store
    /// (test-only seeder). There is no widening port yet — ZMVP-74 owns that act
    /// — so this stands in for it, letting a test drive the three-term
    /// projection against a composition that isn't uniformly closed. Panics if
    /// `tab` is not a tab of this store (the test set it up wrong).
    pub fn set_tab_mode(&self, tab: TabId, mode: VisibilityMode) {
        self.tabs
            .lock()
            .expect("MemBackend tabs mutex poisoned")
            .get_mut(&tab)
            .expect("set_tab_mode: no such tab")
            .mode = mode;
    }

    /// Widen (or narrow) a **surface's** mode for one commission directly on the
    /// shared store (test-only seeder; ZMVP-74 owns the real act, as for
    /// [`set_tab_mode`](Self::set_tab_mode)). Writing the entry is what takes the
    /// surface off the "absent row = Total" default.
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

    /// Fill a declared Seat's occupant slot directly on the shared store
    /// (test-only seeder). There is no seat-fill port yet — accepting a seat
    /// invitation is ZMVP-79 — so this stands in for it, letting an api test
    /// exercise the "already occupied" refusal (ZMVP-78) against a truly filled
    /// seat. Panics if `seat` is not a declared seat (the test set it up wrong).
    pub fn occupy_seat(&self, seat: ElementId, occupant: UserId) {
        let mut seats = self.seats.lock().expect("MemBackend seats mutex poisoned");
        seats
            .get_mut(&seat)
            .expect("occupy_seat: no such declared seat")
            .occupant = Some(occupant);
    }

    /// Seed a (non-owner) participant membership row directly (test-only). There
    /// is no seat-accept path yet (ZMVP-79), so this stands in for a seated
    /// member — letting a test exercise the owner-vs-participant authority split
    /// (the `403` arm of `require_owner`: a participant who is not the owner).
    pub fn seed_participant(&self, commission: CommissionId, user: UserId) {
        // Mirrors add_participant.sql's ON CONFLICT DO NOTHING (ZMVP-140): a
        // re-seed of an already-seated pair is a no-op, preserving the
        // original created_at.
        self.participants
            .lock()
            .expect("MemBackend participants mutex poisoned")
            .entry((commission, user))
            .or_insert_with(chrono::Utc::now);
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use domain::elements::commission::{NewSlot, SKELETON, SeatInvitation, SeatKind, SlotTitle};
    use domain::ports::{ElementNotFound, UnknownSurface, UnknownTab};
    use serde_json::json;

    use super::*;

    fn user_id() -> UserId {
        UserId::new(uuid::Uuid::now_v7())
    }

    fn commission(title: &str, owner: UserId) -> Commission {
        Commission::create(
            title.parse::<CommissionTitle>().unwrap(),
            owner,
            Utc::now(),
            None,
        )
    }

    // ZMVP-65 AC1/AC2/AC3 (store layer) — a commission written through the
    // UnitOfWork's commission view (begin → commissions().create → commit) is read
    // back with its fixed metadata intact: the creating User is the owner and the
    // fresh commission is in `Draft`. The mem seam, end to end — proving the write
    // view and the shared store share state, mirroring the account seam test.
    #[tokio::test]
    async fn uow_create_commission_is_visible_after_commit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();

        let created = commission("A ref sheet", owner);
        let id = created.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        uow.commit().await.unwrap();

        let found = backend
            .find_commission(id)
            .await
            .unwrap()
            .expect("commission present");
        assert_eq!(found.id, id);
        assert_eq!(found.title.as_str(), "A ref sheet");
        assert_eq!(found.owner_id, owner, "the creating User owns it");
        assert!(
            matches!(found.lifecycle_step, LifecycleStep::Draft),
            "a fresh commission is in Draft"
        );
        assert!(
            matches!(found.visibility, Visibility::Private),
            "a fresh commission is Private (the closed-door default)"
        );
        assert!(
            found.linked_channel.is_none(),
            "a fresh commission declares no channel"
        );
    }

    // Dropping a unit of work before `commit()` discards the commission — the mem
    // mirror of pg's drop = rollback (DD 24150017), the commission analogue of
    // `a_dropped_unit_of_work_rolls_back_every_write`.
    #[tokio::test]
    async fn a_dropped_unit_of_work_rolls_back_the_commission() {
        let backend = MemBackend::new();
        let database = backend.database();

        let created = commission("Uncommitted", user_id());
        let id = created.id;

        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions().create(&created).await.unwrap();
            // `uow` drops here without `commit` → the staged write is discarded.
        }

        assert!(
            backend.find_commission(id).await.unwrap().is_none(),
            "a dropped unit of work persists no commission row"
        );
    }

    // An uncommitted unit's commission is invisible to a read off the shared store
    // *before* the unit commits — matching pg, where a pool read can't see another
    // connection's open transaction.
    #[tokio::test]
    async fn uncommitted_commission_is_invisible_until_commit() {
        let backend = MemBackend::new();
        let database = backend.database();

        let created = commission("Isolated", user_id());
        let id = created.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        assert!(
            backend.find_commission(id).await.unwrap().is_none(),
            "an open unit's staged commission is invisible to a shared read"
        );

        uow.commit().await.unwrap();
        assert!(
            backend.find_commission(id).await.unwrap().is_some(),
            "the commission becomes visible once the unit commits"
        );
    }

    // ZMVP-87 (store layer) — an appended entry commits with its unit and rolls
    // back with it (the mem mirror of the DD's atomic-with-domain-writes rule),
    // and the stream reads back in seq order, per commission.
    #[tokio::test]
    async fn changelog_appends_commit_and_roll_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Logged", owner);
        let id = created.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        uow.changelog()
            .append(&NewChangelogEntry::event(
                id,
                ChangelogEntryKind::Created,
                owner,
                json!({ "title": "Logged" }),
                Utc::now(),
            ))
            .await
            .unwrap();
        uow.commit().await.unwrap();

        // A rolled-back (dropped) unit's append is discarded.
        {
            let mut uow = database.begin().await.unwrap();
            uow.changelog()
                .append(&NewChangelogEntry::note(
                    id,
                    owner,
                    "never happened".to_string(),
                    Utc::now(),
                ))
                .await
                .unwrap();
        }

        let entries = backend.changelog_entries(id).await.unwrap();
        assert_eq!(entries.len(), 1, "only the committed entry survives");
        assert!(matches!(entries[0].kind, ChangelogEntryKind::Created));
        assert_eq!(entries[0].actor_id, Some(owner));

        // A second committed entry lands after the first, and other commissions'
        // streams stay separate.
        let mut uow = database.begin().await.unwrap();
        uow.changelog()
            .append(&NewChangelogEntry::note(
                id,
                owner,
                "traveling next week".to_string(),
                Utc::now(),
            ))
            .await
            .unwrap();
        uow.commit().await.unwrap();

        let entries = backend.changelog_entries(id).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].seq < entries[1].seq, "seq orders the stream");
        assert_eq!(entries[1].note.as_deref(), Some("traveling next week"));
        assert!(
            backend
                .changelog_entries(CommissionId::new(uuid::Uuid::now_v7()))
                .await
                .unwrap()
                .is_empty(),
            "an unknown commission has an empty stream"
        );
    }

    // ZMVP-66 AC1 (store layer) — `delete` removes the commission and its
    // changelog entries together (the mem mirror of the pg ON DELETE CASCADE),
    // leaving other commissions' streams untouched.
    #[tokio::test]
    async fn delete_removes_the_commission_and_cascades_its_changelog() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let doomed = commission("Doomed", owner);
        let doomed_id = doomed.id;
        let survivor = commission("Survivor", owner);
        let survivor_id = survivor.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&doomed).await.unwrap();
        uow.commissions().create(&survivor).await.unwrap();
        for (id, title) in [(doomed_id, "Doomed"), (survivor_id, "Survivor")] {
            uow.changelog()
                .append(&NewChangelogEntry::event(
                    id,
                    ChangelogEntryKind::Created,
                    owner,
                    json!({ "title": title }),
                    Utc::now(),
                ))
                .await
                .unwrap();
        }
        uow.commit().await.unwrap();

        let mut uow = database.begin().await.unwrap();
        uow.commissions().delete(doomed_id).await.unwrap();
        uow.commit().await.unwrap();

        assert!(
            backend.find_commission(doomed_id).await.unwrap().is_none(),
            "the deleted commission is gone"
        );
        assert!(
            backend
                .changelog_entries(doomed_id)
                .await
                .unwrap()
                .is_empty(),
            "its changelog cascaded away with it"
        );
        assert!(
            backend
                .find_commission(survivor_id)
                .await
                .unwrap()
                .is_some(),
            "other commissions survive"
        );
        assert_eq!(
            backend.changelog_entries(survivor_id).await.unwrap().len(),
            1,
            "other streams are untouched"
        );
    }

    // ZMVP-66 (store layer) — a delete staged in a dropped (uncommitted) unit of
    // work is discarded: the commission and its changelog survive. The gate that
    // precedes the delete runs in this same unit (ruling E17), so rollback must
    // undo the delete too.
    #[tokio::test]
    async fn a_dropped_unit_of_work_rolls_back_the_delete() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Kept", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions().delete(id).await.unwrap();
            // `uow` drops here without `commit` → the staged delete is discarded.
        }

        assert!(
            backend.find_commission(id).await.unwrap().is_some(),
            "a dropped unit of work deletes nothing"
        );
    }

    // ZMVP-66 (store layer) — deleting an absent commission is a no-op, not an
    // error (existence is the caller's separate check, per the port contract).
    #[tokio::test]
    async fn deleting_an_absent_commission_is_a_no_op() {
        let backend = MemBackend::new();
        let database = backend.database();

        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .delete(CommissionId::new(uuid::Uuid::now_v7()))
            .await
            .unwrap();
        uow.commit().await.unwrap();
    }

    fn account_id() -> AccountId {
        AccountId::new(uuid::Uuid::now_v7())
    }

    // ZMVP-70 (mem store layer) — placement appends to the log and repoints the
    // current pointer to the latest row; a view grant upserts and revoke
    // hard-deletes; ALL of it stages with the unit (drop = rollback) and confers
    // NO participant-hood (Ownership Separation DD Decision 8).
    #[tokio::test]
    async fn placement_and_grants_stage_lift_nothing_and_roll_back() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Positioned", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let store = backend.commission_store();
        let account = account_id();
        let member = user_id();

        // Place in `account` twice; the current pointer tracks the latest row.
        for _ in 0..2 {
            let mut uow = database.begin().await.unwrap();
            uow.commissions()
                .place(id, account, owner, Utc::now())
                .await
                .unwrap();
            uow.commit().await.unwrap();
        }
        let log = store.placement_log(id).await.unwrap();
        assert_eq!(
            log.len(),
            2,
            "each placement appends (the log is never rewritten)"
        );
        let current = store.current_placement(id).await.unwrap().expect("current");
        assert_eq!(
            (current.seq, current.account_id),
            (log.last().unwrap().seq, log.last().unwrap().account_id),
            "the cached current pointer equals the latest log row",
        );

        // Grant Total, then revoke — the key is gone immediately.
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .grant_view(id, account, GrantLevel::Total)
            .await
            .unwrap();
        uow.commit().await.unwrap();
        assert_eq!(
            store.view_grant(id, account).await.unwrap(),
            Some(GrantLevel::Total)
        );

        // A view grant / placement makes the account's members no Participant (D8).
        assert!(
            !store.is_participant(id, member).await.unwrap(),
            "positioning and keys confer no in-commission authority",
        );
        assert!(
            store.is_participant(id, owner).await.unwrap(),
            "the owner still is"
        );

        let mut uow = database.begin().await.unwrap();
        assert!(
            uow.commissions().revoke_view(id, account).await.unwrap(),
            "revoking an existing key reports a transition",
        );
        uow.commit().await.unwrap();
        assert!(
            store.view_grant(id, account).await.unwrap().is_none(),
            "a revoked key is gone immediately",
        );

        // A dropped unit rolls back a placement AND a grant.
        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions()
                .place(id, account_id(), owner, Utc::now())
                .await
                .unwrap();
            uow.commissions()
                .grant_view(id, account, GrantLevel::Description)
                .await
                .unwrap();
            // drop without commit
        }
        assert_eq!(
            store.placement_log(id).await.unwrap().len(),
            2,
            "the dropped placement left no row",
        );
        assert!(
            store.view_grant(id, account).await.unwrap().is_none(),
            "the dropped grant never landed",
        );
    }

    // ZMVP-57 AC1 (mem parity) — hard-deleting an account **severs** its positioning
    // rails (the placements it held and its view grants) while the placed commission
    // **survives untouched**. This mirrors pg's `ON DELETE CASCADE` on the positioning
    // FKs onto `accounts`: only the account-side positioning goes; the User-owned
    // commission stays (Ownership Separation DD 29130754).
    #[tokio::test]
    async fn hard_deleting_an_account_severs_its_positioning_but_keeps_the_commission() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Placed then orphaned", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let store = backend.commission_store();
        let account = account_id();

        // Place the commission in the account and grant it a view key.
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .place(id, account, owner, Utc::now())
            .await
            .unwrap();
        uow.commissions()
            .grant_view(id, account, GrantLevel::Total)
            .await
            .unwrap();
        uow.commit().await.unwrap();
        assert!(
            store.current_placement(id).await.unwrap().is_some(),
            "placed before the delete"
        );
        assert!(
            store.view_grant(id, account).await.unwrap().is_some(),
            "granted before the delete"
        );

        // Hard-delete the account.
        let mut uow = database.begin().await.unwrap();
        uow.accounts().hard_delete(account).await.unwrap();
        uow.commit().await.unwrap();

        // The positioning rails are severed...
        assert!(
            store.current_placement(id).await.unwrap().is_none(),
            "the current-placement pointer is severed with the account",
        );
        assert!(
            store.placement_log(id).await.unwrap().is_empty(),
            "the placement log is severed with the account",
        );
        assert!(
            store.view_grant(id, account).await.unwrap().is_none(),
            "the view grant is severed with the account",
        );
        // ...but the commission itself survives untouched.
        assert!(
            backend.find_commission(id).await.unwrap().is_some(),
            "the User-owned commission survives account deletion",
        );
    }

    // ZMVP-166 (store layer) — a commission is born with its SKELETON TABS in
    // the same unit of work: after create+commit the loaded composition holds
    // exactly the declared tabs, every one born Total (the closed door), and no
    // elements. Tab state exists explicitly from the first instant — the
    // withheld-at-birth discipline — so absence never has to mean anything.
    #[tokio::test]
    async fn creating_a_commission_mints_its_skeleton_tabs() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Composed", owner);
        let id = created.id;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        uow.commit().await.unwrap();

        let composition = backend
            .commission_store()
            .load_composition(id)
            .await
            .unwrap()
            .expect("a created commission always has its tabs");
        let names: Vec<&str> = composition
            .tabs
            .iter()
            .map(|tab| tab.tab.as_str())
            .collect();
        let declared: Vec<String> = declared_tabs()
            .iter()
            .map(|tab| tab.as_str().to_owned())
            .collect();
        assert_eq!(
            names, declared,
            "exactly the code-declared skeleton, nothing more"
        );
        assert!(
            composition
                .tabs
                .iter()
                .all(|tab| tab.mode == VisibilityMode::Total),
            "every tab is born Total — the commission's own visibility is NOT copied in"
        );
        assert!(
            composition.elements.is_empty(),
            "a fresh commission is composed of nothing"
        );
        assert!(
            composition.surface_modes.is_empty(),
            "no surface has been widened, so no override row exists"
        );
    }

    // ZMVP-166 — elements append within their (tab, surface, band) group: two
    // contributions keep append order, and every element is born Total.
    #[tokio::test]
    async fn add_element_appends_within_its_group() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Growing", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let address = only_address(&backend, id).await;

        let first = element_at(id, address.clone(), owner);
        let second = element_at(id, address.clone(), owner);
        let (first_id, second_id) = (first.id, second.id);
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_element(&first).await.unwrap();
        uow.commissions().add_element(&second).await.unwrap();
        uow.commit().await.unwrap();

        let elements = backend.elements_of(id).await.unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].id, first_id, "append order holds");
        assert_eq!(elements[0].position, 0);
        assert_eq!(elements[1].id, second_id);
        assert_eq!(elements[1].position, 1);
        assert!(
            elements
                .iter()
                .all(|element| element.mode == VisibilityMode::Total),
            "every element is born Total — over-claiming has to be an explicit act"
        );
        assert!(
            elements
                .iter()
                .all(|element| element.band == Band::default()),
            "everything lands in the placeholder band (ZMVP-171 owns the vocabulary)"
        );
    }

    // ZMVP-166 — the tab must exist in THIS commission: a fabricated tab id and
    // one belonging to another commission both fail with UnknownTab (one
    // indistinguishable answer — no probing other commissions), and neither
    // write lands. In pg the cross-commission case is additionally
    // unrepresentable (the composite foreign key); the fake has only this gate,
    // which is why the case is pinned here explicitly.
    #[tokio::test]
    async fn add_element_refuses_absent_and_foreign_tabs() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let mine = commission("Mine", owner);
        let theirs = commission("Theirs", user_id());
        let mine_id = mine.id;
        let theirs_id = theirs.id;
        backend.create_commission(&mine).await.unwrap();
        backend.create_commission(&theirs).await.unwrap();
        let their_address = only_address(&backend, theirs_id).await;

        // A tab id that exists nowhere.
        let fabricated_address = SurfaceAddress::new(TabId::mint(), only_surface());
        let fabricated = element_at(mine_id, fabricated_address, owner);
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .add_element(&fabricated)
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<UnknownTab>().is_some(),
            "absent tab surfaces as UnknownTab, got: {err:?}"
        );
        drop(uow);

        // A real tab — belonging to someone else's commission.
        let cross = element_at(mine_id, their_address, owner);
        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().add_element(&cross).await.unwrap_err();
        assert!(
            err.downcast_ref::<UnknownTab>().is_some(),
            "a foreign tab is indistinguishable from an absent one, got: {err:?}"
        );
        drop(uow);

        assert!(
            backend.elements_of(mine_id).await.unwrap().is_empty(),
            "no refused write landed"
        );
        assert!(
            backend.elements_of(theirs_id).await.unwrap().is_empty(),
            "and nothing leaked into the other commission either"
        );
    }

    // ZMVP-166 — the surface must be one the CODE SKELETON declares. Surfaces
    // have no rows, so the const is the only authority, and an unrecognized name
    // is refused rather than created (fail-closed) — the same answer the pg
    // adapter gives, from the same const.
    #[tokio::test]
    async fn add_element_refuses_an_undeclared_surface() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Fail-closed", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let tab = backend.tabs_of(id).await.unwrap()[0].id;

        let invented = SurfaceAddress::new(tab, "invented".parse::<SurfaceName>().unwrap());
        let element = element_at(id, invented, owner);
        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().add_element(&element).await.unwrap_err();
        assert!(
            err.downcast_ref::<UnknownSurface>().is_some(),
            "an undeclared surface surfaces as UnknownSurface, got: {err:?}"
        );
        drop(uow);

        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "nothing landed, and no surface was invented"
        );
    }

    // ZMVP-166 — the skeleton check is on the PAIR: a surface that is perfectly
    // real under its own tab, addressed under a DIFFERENT tab of the same
    // commission, is refused with UnknownSurface — the pair is not declared, and
    // the tab itself is real, so this is not an UnknownTab. Same answer the pg
    // adapter gives, from the same const.
    #[tokio::test]
    async fn add_element_refuses_a_real_surface_under_the_wrong_tab() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Wrongly addressed", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        // A real tab row of THIS commission whose name the skeleton does not
        // pair with the surface below. (The placeholder skeleton has one tab, so
        // the shape has to be seeded; ZMVP-171's real skeleton makes it
        // ordinary.)
        let other = backend.seed_tab(id, "other".parse::<TabName>().unwrap());
        let wrongly_addressed = SurfaceAddress::new(other, only_surface());
        let element = element_at(id, wrongly_addressed, owner);

        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().add_element(&element).await.unwrap_err();
        assert!(
            err.downcast_ref::<UnknownSurface>().is_some(),
            "the (tab, surface) pair is undeclared — UnknownSurface, not UnknownTab \
             (the tab is real), got: {err:?}"
        );
        drop(uow);

        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "nothing landed under a place the skeleton never described"
        );
    }

    // ZMVP-166 — the gate ORDER is part of the contract, because the pair check
    // needs the tab's declared name and so cannot run first. An address that is
    // wrong in BOTH ways answers UnknownTab; both adapters must agree, or one
    // request would get two different answers depending on the store.
    #[tokio::test]
    async fn an_address_wrong_in_both_ways_answers_unknown_tab() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Doubly wrong", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        let doubly_wrong =
            SurfaceAddress::new(TabId::mint(), "invented".parse::<SurfaceName>().unwrap());
        let element = element_at(id, doubly_wrong, owner);
        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().add_element(&element).await.unwrap_err();
        assert!(
            err.downcast_ref::<UnknownTab>().is_some(),
            "the tab is resolved FIRST, so a fabricated tab wins over an \
             undeclared surface, got: {err:?}"
        );
        drop(uow);
    }

    // ZMVP-166 — the opaque payload round-trips: whatever JSON went in reads back
    // as an equal value, and the core never interprets it.
    #[tokio::test]
    async fn an_elements_payload_round_trips_opaquely() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Opaque", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let address = only_address(&backend, id).await;

        let body = json!({
            "kind": "text",
            "body": "Reference: 三毛猫 🐾",
            "nested": { "list": [1, 2, 3], "flag": true, "nothing": null },
        });
        let payload = ElementPayload::from(body.clone());
        let element = NewElement::contributed(
            id,
            address,
            "note".parse::<ElementType>().unwrap(),
            payload,
            owner,
            Utc::now(),
        );
        let element_id = element.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_element(&element).await.unwrap();
        uow.commit().await.unwrap();

        let stored = backend.elements_of(id).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].id, element_id);
        assert_eq!(
            stored[0].payload.as_value(),
            &body,
            "the payload is carried opaque"
        );
        assert_eq!(stored[0].element_type.as_str(), "note");
    }

    // ZMVP-166 (transactionality) — a staged element is invisible until commit
    // and discarded on drop, exactly like every other unit-of-work write.
    #[tokio::test]
    async fn add_element_commits_and_rolls_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Tx", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let address = only_address(&backend, id).await;

        {
            let element = element_at(id, address, owner);
            let mut uow = database.begin().await.unwrap();
            uow.commissions().add_element(&element).await.unwrap();
            assert!(
                backend.elements_of(id).await.unwrap().is_empty(),
                "an open unit's staged element is invisible to a shared read"
            );
            // `uow` drops here without `commit` -> the staged element is discarded.
        }

        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "a dropped unit of work persists no element"
        );
    }

    // ZMVP-166 — the three-term projection, read end to end off a real
    // composition: an element that claims more than its surface allows is INERT.
    #[tokio::test]
    async fn effective_visibility_clamps_against_the_loaded_composition() {
        let backend = MemBackend::new();
        let owner = user_id();
        let created = commission("Clamped", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let address = only_address(&backend, id).await;

        let element = element_at(id, address.clone(), owner);
        let mut uow = backend.database().begin().await.unwrap();
        uow.commissions().add_element(&element).await.unwrap();
        uow.commit().await.unwrap();

        // Everything closed at birth: the element projects Total.
        let store = backend.commission_store();
        let composition = store.load_composition(id).await.unwrap().expect("composed");
        let only = &composition.elements[0];
        assert_eq!(
            composition.effective_visibility_of(only),
            VisibilityMode::Total,
            "a commission nobody widened shows nothing"
        );

        // Widen the tab and the surface wide open (ZMVP-74 owns the real act);
        // the element's OWN mode is still Total, so it stays closed.
        backend.set_tab_mode(address.tab, VisibilityMode::Description);
        backend.set_surface_mode(id, address.surface.clone(), VisibilityMode::Description);
        let composition = store.load_composition(id).await.unwrap().expect("composed");
        let only = &composition.elements[0];
        assert_eq!(
            composition.effective_visibility_of(only),
            VisibilityMode::Total,
            "the element is the narrowest term, and it can always close further"
        );

        // Narrow the surface back down and over-claim on the element: the
        // surface still wins, because the result is the MIN.
        backend.set_surface_mode(id, address.surface.clone(), VisibilityMode::Presentation);
        let mut composition = store.load_composition(id).await.unwrap().expect("composed");
        composition.elements[0].mode = VisibilityMode::Description;
        let only = &composition.elements[0];
        assert_eq!(
            composition.effective_visibility_of(only),
            VisibilityMode::Presentation,
            "an over-claiming element is inert, never a leak"
        );
    }

    // load_composition for a commission nobody created is None, mirroring `find` —
    // and distinct from a created-but-empty one, which is Some with tabs and no
    // elements.
    #[tokio::test]
    async fn load_composition_answers_none_for_an_unknown_commission() {
        let backend = MemBackend::new();
        assert!(
            backend
                .commission_store()
                .load_composition(CommissionId::new(uuid::Uuid::now_v7()))
                .await
                .unwrap()
                .is_none()
        );

        let created = commission("Empty", user_id());
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let composition = backend
            .commission_store()
            .load_composition(id)
            .await
            .unwrap()
            .expect("an existing commission composes to Some, however empty");
        assert!(composition.elements.is_empty(), "empty is not absent");
    }

    // ZMVP-85 (store layer) — the direction status sets, replaces, and clears
    // through the unit of work; a dropped unit discards the staged change (the
    // mem mirror of pg's drop = rollback).
    #[tokio::test]
    async fn direction_status_sets_replaces_and_clears_through_the_unit() {
        use domain::elements::commission::DirectionStatus;

        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Statused", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        let status_of = |backend: &MemBackend| {
            let backend = backend.clone();
            async move {
                backend
                    .find_commission(id)
                    .await
                    .unwrap()
                    .expect("exists")
                    .direction_status
            }
        };
        assert_eq!(status_of(&backend).await, None, "born clear");

        // Set, then replace — one nullable cell, so the second set wins whole.
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .set_direction_status(id, Some(DirectionStatus::WaitingForInput))
            .await
            .unwrap();
        uow.commit().await.unwrap();
        assert_eq!(
            status_of(&backend).await,
            Some(DirectionStatus::WaitingForInput)
        );

        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .set_direction_status(id, Some(DirectionStatus::ChangesRequested))
            .await
            .unwrap();
        uow.commit().await.unwrap();
        assert_eq!(
            status_of(&backend).await,
            Some(DirectionStatus::ChangesRequested),
            "a set replaces the current value"
        );

        // A dropped (uncommitted) unit discards its staged status write.
        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions()
                .set_direction_status(id, None)
                .await
                .unwrap();
        }
        assert_eq!(
            status_of(&backend).await,
            Some(DirectionStatus::ChangesRequested),
            "a dropped unit rolls the clear back"
        );

        // Clear commits to NULL; an absent commission is a no-op, not an error.
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .set_direction_status(id, None)
            .await
            .unwrap();
        uow.commissions()
            .set_direction_status(
                CommissionId::new(uuid::Uuid::now_v7()),
                Some(DirectionStatus::WaitingForApproval),
            )
            .await
            .unwrap();
        uow.commit().await.unwrap();
        assert_eq!(status_of(&backend).await, None, "cleared");
    }

    // ZMVP-86 (store layer) — the deadline and the MANUAL Delayed flag set and
    // clear through the unit of work; a dropped unit discards the staged change
    // (the mem mirror of pg's drop = rollback). Late is derived on lookup, never
    // persisted, so it is exercised separately below.
    #[tokio::test]
    async fn deadline_and_status_set_and_clear_through_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Deadlined", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        // A FUTURE deadline, so the derived Late never masks the manual flag.
        let deadline = Utc::now() + chrono::Duration::days(30);
        let mut uow = database.begin().await.unwrap();
        {
            let mut commissions = uow.commissions();
            commissions.set_deadline(id, Some(deadline)).await.unwrap();
            commissions
                .set_deadline_status(id, Some(DeadlineStatus::Delayed))
                .await
                .unwrap();
        }
        uow.commit().await.unwrap();
        let found = backend.find_commission(id).await.unwrap().expect("exists");
        assert_eq!(found.deadline, Some(deadline));
        assert_eq!(
            found.deadline_status,
            Some(DeadlineStatus::Delayed),
            "the manual flag persists; a future deadline is not Late"
        );

        // A dropped (uncommitted) unit discards its staged writes.
        {
            let mut uow = database.begin().await.unwrap();
            let mut commissions = uow.commissions();
            commissions.set_deadline(id, None).await.unwrap();
            commissions.set_deadline_status(id, None).await.unwrap();
        }
        let found = backend.find_commission(id).await.unwrap().expect("exists");
        assert_eq!(found.deadline, Some(deadline), "the clear rolled back");
        assert_eq!(found.deadline_status, Some(DeadlineStatus::Delayed));

        // Clear commits; an absent commission is a no-op, not an error.
        let mut uow = database.begin().await.unwrap();
        {
            let mut commissions = uow.commissions();
            commissions.set_deadline(id, None).await.unwrap();
            commissions.set_deadline_status(id, None).await.unwrap();
            commissions
                .set_deadline(CommissionId::new(uuid::Uuid::now_v7()), Some(deadline))
                .await
                .unwrap();
        }
        uow.commit().await.unwrap();
        let found = backend.find_commission(id).await.unwrap().expect("exists");
        assert_eq!(found.deadline, None);
        assert_eq!(
            found.deadline_status, None,
            "no deadline ⇒ no axis status (AC4)"
        );
    }

    // ZMVP-86 — Late is DERIVED on lookup from `deadline < now`, never persisted:
    // a past deadline reads Late, and it supersedes a standing manual Delayed
    // without overwriting it in storage.
    #[tokio::test]
    async fn late_is_derived_from_a_passed_deadline() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Slipping", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        let mut uow = database.begin().await.unwrap();
        {
            let mut commissions = uow.commissions();
            commissions
                .set_deadline(id, Some(Utc::now() - chrono::Duration::days(1)))
                .await
                .unwrap();
            commissions
                .set_deadline_status(id, Some(DeadlineStatus::Delayed))
                .await
                .unwrap();
        }
        uow.commit().await.unwrap();

        let found = backend.find_commission(id).await.unwrap().expect("exists");
        assert_eq!(
            found.deadline_status,
            Some(DeadlineStatus::Late),
            "a passed deadline derives Late, superseding the stored Delayed"
        );
    }

    // ZMVP-86 (store layer, ruling E12) — `lapsed_deadlines` returns exactly
    // the sweepable set: past-deadline commissions that are not already Late
    // and not in a terminal lifecycle, ordered by deadline; and it sees writes
    // staged on the same open unit (the no-TOCTOU posture).
    #[tokio::test]
    async fn lapsed_deadlines_scans_exactly_the_sweepable_set() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let now = Utc::now();
        let past = |days: i64| now - chrono::Duration::days(days);

        let seed = |title: &str, deadline, step: Option<LifecycleStep>| {
            let mut c = Commission::create(
                title.parse::<CommissionTitle>().unwrap(),
                owner,
                now,
                deadline,
            );
            if let Some(step) = step {
                c.lifecycle_step = step;
            }
            c
        };
        let missed = seed("Missed", Some(past(30)), None);
        let slipping = seed("Slipping", Some(past(20)), None);
        let already_late = seed("Late", Some(past(10)), None);
        let future = seed("Future", Some(now + chrono::Duration::days(30)), None);
        let no_deadline = seed("No deadline", None, None);
        let completed = seed("Done", Some(past(30)), Some(LifecycleStep::Completed));
        let cancelled = seed("Dropped", Some(past(30)), Some(LifecycleStep::Cancelled));
        let disputed = seed("Contested", Some(past(5)), Some(LifecycleStep::Disputed));
        for c in [
            &missed,
            &slipping,
            &already_late,
            &future,
            &no_deadline,
            &completed,
            &cancelled,
            &disputed,
        ] {
            backend.create_commission(c).await.unwrap();
        }

        let mut uow = database.begin().await.unwrap();
        {
            uow.commissions()
                .set_deadline_status(slipping.id, Some(DeadlineStatus::Delayed))
                .await
                .unwrap();
            // Late is deduped on the changelog (no persisted Late), staged on the
            // SAME unit: a commission already logged Late is skipped by the scan.
            uow.changelog()
                .append(&NewChangelogEntry::system(
                    already_late.id,
                    ChangelogEntryKind::Late,
                    serde_json::json!({}),
                    now,
                ))
                .await
                .unwrap();

            let lapsed = uow.commissions().lapsed_deadlines(now).await.unwrap();
            let ids: Vec<_> = lapsed.iter().map(|l| l.id).collect();
            assert_eq!(
                ids,
                vec![missed.id, slipping.id, disputed.id],
                "exactly the sweepable set, ordered by deadline"
            );
            assert_eq!(lapsed[0].status, None);
            assert_eq!(
                lapsed[1].status,
                Some(DeadlineStatus::Delayed),
                "the scan carries the standing flag"
            );
        }
    }

    // The owner-arm participant predicate and the linked-channel round-trip on
    // the mem read store (ZMVP-87).
    #[tokio::test]
    async fn commission_store_answers_participant_and_channel_reads() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Mine", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        let store = backend.commission_store();
        assert!(store.is_participant(id, owner).await.unwrap());
        assert!(!store.is_participant(id, user_id()).await.unwrap());
        assert!(
            !store
                .is_participant(CommissionId::new(uuid::Uuid::now_v7()), owner)
                .await
                .unwrap()
        );

        let pointer = "@artist on Telegram".parse::<ChannelPointer>().unwrap();
        let mut uow = database.begin().await.unwrap();
        assert!(
            uow.commissions()
                .set_linked_channel(id, Some(&pointer))
                .await
                .unwrap(),
            "the first link is a real change"
        );
        assert!(
            !uow.commissions()
                .set_linked_channel(id, Some(&pointer))
                .await
                .unwrap(),
            "re-linking the identical pointer answers false"
        );
        uow.commit().await.unwrap();
        assert_eq!(
            store
                .find(id)
                .await
                .unwrap()
                .expect("exists")
                .linked_channel
                .map(|c| c.as_str().to_owned()),
            Some("@artist on Telegram".to_owned()),
        );

        let mut uow = database.begin().await.unwrap();
        assert!(
            uow.commissions()
                .set_linked_channel(id, None)
                .await
                .unwrap(),
            "the clear is a real change"
        );
        assert!(
            !uow.commissions()
                .set_linked_channel(id, None)
                .await
                .unwrap(),
            "clearing an already-clear channel answers false"
        );
        uow.commit().await.unwrap();
        assert!(
            store
                .find(id)
                .await
                .unwrap()
                .expect("exists")
                .linked_channel
                .is_none(),
            "the pointer clears"
        );
    }

    // ZMVP-31 (store layer) — a fresh commission is unrated (the birth
    // invariant); set_maturity round-trips every axis/graphic pairing and a
    // later write REPLACES the posture (replace-only — no clear exists);
    // the write is unit-of-work-scoped (a dropped unit rates nothing); an
    // absent commission is a no-op, per the port contract.
    #[tokio::test]
    async fn set_maturity_round_trips_replaces_and_respects_the_unit() {
        use domain::elements::maturity::MaturityRating;

        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Rated", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        let unrated = backend.find_commission(id).await.unwrap().expect("exists");
        assert_eq!(unrated.maturity, None, "born unrated (the invariant)");

        for rating in MaturityRating::ALL {
            for graphic in [true, false] {
                let posture = Maturity {
                    rating: *rating,
                    graphic,
                };
                let mut uow = database.begin().await.unwrap();
                uow.commissions().set_maturity(id, posture).await.unwrap();
                uow.commit().await.unwrap();
                assert_eq!(
                    backend
                        .find_commission(id)
                        .await
                        .unwrap()
                        .expect("exists")
                        .maturity,
                    Some(posture),
                    "each write replaces the whole posture",
                );
            }
        }

        // A dropped unit's write is discarded — the last committed posture holds.
        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions()
                .set_maturity(
                    id,
                    Maturity {
                        rating: MaturityRating::Suggestive,
                        graphic: true,
                    },
                )
                .await
                .unwrap();
            // `uow` drops here without `commit` → the staged write is discarded.
        }
        assert_eq!(
            backend
                .find_commission(id)
                .await
                .unwrap()
                .expect("exists")
                .maturity,
            Some(Maturity {
                rating: MaturityRating::Adult,
                graphic: false,
            }),
            "a dropped unit of work changes nothing — the loop's last committed posture holds",
        );

        // An absent commission is a no-op, not an error (existence is the
        // caller's check).
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .set_maturity(
                CommissionId::new(uuid::Uuid::now_v7()),
                Maturity {
                    rating: MaturityRating::Adult,
                    graphic: false,
                },
            )
            .await
            .unwrap();
        uow.commit().await.unwrap();
    }

    /// Seeds a committed commission and returns `(its id, its only address)` —
    /// the placeholder skeleton declares exactly one tab holding exactly one
    /// surface, so "the address" is unambiguous.
    async fn composed_commission(
        backend: &MemBackend,
        owner: UserId,
    ) -> (CommissionId, SurfaceAddress) {
        let created = commission("Composed", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let address = only_address(backend, id).await;
        (id, address)
    }

    /// The commission's one skeleton address (see [`composed_commission`]).
    async fn only_address(backend: &MemBackend, commission: CommissionId) -> SurfaceAddress {
        let tab = backend.tabs_of(commission).await.unwrap()[0].id;
        SurfaceAddress::new(tab, only_surface())
    }

    /// The one surface the placeholder skeleton declares.
    fn only_surface() -> SurfaceName {
        SKELETON[0].surfaces[0]
            .parse::<SurfaceName>()
            .expect("the skeleton declares valid labels")
    }

    /// An untyped element at `address` — the shape most of these tests only need
    /// to exist, so its type tag and payload carry no meaning.
    fn element_at(commission: CommissionId, address: SurfaceAddress, owner: UserId) -> NewElement {
        NewElement::contributed(
            commission,
            address,
            "note".parse::<ElementType>().unwrap(),
            ElementPayload::default(),
            owner,
            Utc::now(),
        )
    }

    /// The `(id, position)` pairs of one ordering group, in position order —
    /// read straight off the shared element map, so a test can assert the
    /// renumbering invariant (positions contiguous from 0).
    fn group_positions(
        backend: &MemBackend,
        commission: CommissionId,
        address: &SurfaceAddress,
    ) -> Vec<(ElementId, i32)> {
        let elements = backend.elements.lock().expect("elements mutex");
        let mut pairs: Vec<(ElementId, i32)> = elements
            .iter()
            .filter(|(_, element)| {
                element.commission_id == commission && element.address == *address
            })
            .map(|(id, element)| (*id, element.position))
            .collect();
        pairs.sort_by_key(|(_, position)| *position);
        pairs
    }

    /// Runs `remove_element` in its own committed unit of work.
    async fn remove_element(
        database: &std::sync::Arc<dyn domain::ports::Database>,
        commission: CommissionId,
        element: ElementId,
    ) -> anyhow::Result<()> {
        let mut uow = database.begin().await?;
        uow.commissions()
            .remove_element(commission, element)
            .await?;
        uow.commit().await
    }

    // ZMVP-166 — removing an element takes exactly that element (there is no
    // subtree to take: elements are leaves, always) and renumbers the remaining
    // ordering group so positions stay contiguous from 0, in the same
    // transaction.
    #[tokio::test]
    async fn remove_element_takes_only_it_and_renumbers_the_group() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let first = element_at(id, address.clone(), owner);
        let doomed = element_at(id, address.clone(), owner);
        let last = element_at(id, address.clone(), owner);
        let (first_id, doomed_id, last_id) = (first.id, doomed.id, last.id);
        let mut uow = database.begin().await.unwrap();
        for element in [&first, &doomed, &last] {
            uow.commissions().add_element(element).await.unwrap();
        }
        uow.commit().await.unwrap();
        assert_eq!(
            group_positions(&backend, id, &address),
            vec![(first_id, 0), (doomed_id, 1), (last_id, 2)],
        );

        remove_element(&database, id, doomed_id).await.unwrap();

        assert_eq!(
            group_positions(&backend, id, &address),
            vec![(first_id, 0), (last_id, 1)],
            "the survivors renumber contiguously from 0, order preserved"
        );
    }

    // ZMVP-166 — the target must exist in THIS commission: a fabricated element
    // id and one belonging to another commission both fail with ElementNotFound
    // (one indistinguishable answer — removal probes reveal nothing about other
    // commissions), and neither removal lands.
    #[tokio::test]
    async fn remove_refuses_absent_and_foreign_elements() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (mine, my_address) = composed_commission(&backend, owner).await;
        let theirs = commission("Theirs", user_id());
        let theirs_id = theirs.id;
        backend.create_commission(&theirs).await.unwrap();
        let their_address = only_address(&backend, theirs_id).await;

        let ours = element_at(mine, my_address, owner);
        let theirs_element = element_at(theirs_id, their_address, user_id());
        let (our_id, their_id) = (ours.id, theirs_element.id);
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_element(&ours).await.unwrap();
        uow.commissions()
            .add_element(&theirs_element)
            .await
            .unwrap();
        uow.commit().await.unwrap();

        let err = remove_element(&database, mine, ElementId::new(uuid::Uuid::now_v7()))
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<ElementNotFound>().is_some(),
            "an absent element surfaces as ElementNotFound, got: {err:?}"
        );

        let err = remove_element(&database, mine, their_id).await.unwrap_err();
        assert!(
            err.downcast_ref::<ElementNotFound>().is_some(),
            "a foreign element is indistinguishable from an absent one, got: {err:?}"
        );

        assert_eq!(
            backend.elements_of(mine).await.unwrap()[0].id,
            our_id,
            "our element survives"
        );
        assert_eq!(
            backend.elements_of(theirs_id).await.unwrap()[0].id,
            their_id,
            "and so, untouched, does theirs"
        );
    }

    // ZMVP-166 / ruling E35 — deleting the commission sweeps its WHOLE
    // composition, so `load_composition` answers None afterwards exactly as pg
    // does. Without this the fake would answer Some for a commission that is
    // "gone entirely" — an adapter lie the api suites (which all run on the
    // fake) would inherit.
    #[tokio::test]
    async fn deleting_the_commission_sweeps_its_whole_composition() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let element = element_at(id, address.clone(), owner);
        let seat = NewSeat::contributed_at(
            id,
            address.clone(),
            "Creator".parse::<SeatKind>().unwrap(),
            None,
            None,
            owner,
            Utc::now(),
        );
        let seat_id = seat.id;
        let slot = NewSlot::contributed_at(
            id,
            address.clone(),
            "The knight".parse::<SlotTitle>().unwrap(),
            None,
            owner,
            Utc::now(),
        );
        let slot_id = slot.id;
        let invited = user_id();
        let invitation = SeatInvitation::issue(id, seat_id, invited, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_element(&element).await.unwrap();
        uow.commissions().declare_seat(&seat).await.unwrap();
        uow.commissions().declare_slots(&[slot]).await.unwrap();
        uow.commissions()
            .create_seat_invitation(&invitation)
            .await
            .unwrap();
        uow.commit().await.unwrap();
        backend.set_surface_mode(id, address.surface, VisibilityMode::Description);

        let mut uow = database.begin().await.unwrap();
        uow.commissions().delete(id).await.unwrap();
        uow.commit().await.unwrap();

        let store = backend.commission_store();
        assert!(
            store.load_composition(id).await.unwrap().is_none(),
            "a deleted commission composes to None, exactly as in pg"
        );
        assert!(store.seats(id).await.unwrap().is_empty());
        assert!(backend.slots_of(id).await.unwrap().is_empty());
        assert!(backend.find_slot(slot_id).await.unwrap().is_none());
        assert!(
            store
                .find_pending_seat_invitation(id, seat_id, invited)
                .await
                .unwrap()
                .is_none(),
            "a deleted commission's seat takes its pending offers with it"
        );
    }

    // ZMVP-166 (transactionality) — a staged removal is invisible until commit
    // and discarded on drop, like every other unit-of-work write.
    #[tokio::test]
    async fn remove_commits_and_rolls_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let element = element_at(id, address, owner);
        let element_id = element.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_element(&element).await.unwrap();
        uow.commit().await.unwrap();

        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions()
                .remove_element(id, element_id)
                .await
                .unwrap();
            assert_eq!(
                backend.elements_of(id).await.unwrap().len(),
                1,
                "an open unit's staged removal is invisible to a shared read"
            );
            // `uow` drops here without `commit` -> the removal is discarded.
        }

        assert_eq!(
            backend.elements_of(id).await.unwrap().len(),
            1,
            "a dropped unit of work removes nothing"
        );

        remove_element(&database, id, element_id).await.unwrap();
        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "a committed removal is visible"
        );
    }

    // ZMVP-76 (Engineer ruling B2, store layer) — creating a commission seats
    // its owner as a PERSISTED participant in the same unit of work: the
    // membership row exists (independent of the owner_id column), stamped with
    // the commission's own creation instant, and is_participant reads it.
    #[tokio::test]
    async fn creating_a_commission_persists_its_owners_participant_row() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Membered", owner);
        let id = created.id;
        let created_at = created.created_at;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        uow.commit().await.unwrap();

        let participants = backend
            .participants
            .lock()
            .expect("participants mutex poisoned")
            .clone();
        assert_eq!(
            participants.get(&(id, owner)),
            Some(&created_at),
            "the owner's membership row is born with the commission"
        );
        assert!(
            backend
                .commission_store()
                .is_participant(id, owner)
                .await
                .unwrap(),
            "the predicate reads the membership record"
        );
    }

    // ZMVP-76 — is_participant answers from the membership TABLE, not the
    // owner_id column: a directly seeded membership row for a non-owner (the
    // shape ZMVP-79's seated arm will write) already counts.
    #[tokio::test]
    async fn is_participant_reads_the_membership_record_not_the_owner_column() {
        let backend = MemBackend::new();
        let owner = user_id();
        let seated = user_id();
        let created = commission("Seated later", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();

        assert!(
            !backend
                .commission_store()
                .is_participant(id, seated)
                .await
                .unwrap(),
            "not a participant before any membership row exists"
        );
        backend
            .participants
            .lock()
            .expect("participants mutex poisoned")
            .insert((id, seated), Utc::now());
        assert!(
            backend
                .commission_store()
                .is_participant(id, seated)
                .await
                .unwrap(),
            "a membership row alone makes a participant (the ZMVP-79 seated arm's shape)"
        );
    }

    // ZMVP-76 AC1/AC2/AC3 (store layer) — declaring a seat contributes ONE
    // element AND its interpreted satellite sharing the id, atomically in the
    // unit: kind + requirements read back, the seat is born vacant, and kinds
    // repeat freely across a commission's seats.
    #[tokio::test]
    async fn declare_seat_lands_an_element_and_its_satellite_together() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let first = NewSeat::contributed_at(
            id,
            address.clone(),
            "Creator".parse::<SeatKind>().unwrap(),
            Some("Two refs, please.".parse::<SeatPrompt>().unwrap()),
            Some("https://forms.example/apply".parse::<SeatLink>().unwrap()),
            owner,
            Utc::now(),
        );
        // A second seat of the SAME kind — kinds repeat freely (AC1).
        let second = NewSeat::contributed_at(
            id,
            address.clone(),
            "Creator".parse::<SeatKind>().unwrap(),
            None,
            None,
            owner,
            Utc::now(),
        );
        let (first_id, second_id) = (first.id, second.id);

        let mut uow = database.begin().await.unwrap();
        uow.commissions().declare_seat(&first).await.unwrap();
        uow.commissions().declare_seat(&second).await.unwrap();
        uow.commit().await.unwrap();

        // The composition half: two elements at the address, in append order.
        let elements = backend.elements_of(id).await.unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].id, first_id, "append order");
        assert_eq!(elements[1].id, second_id);
        assert!(
            elements
                .iter()
                .all(|element| element.element_type == ElementType::seat()),
            "a seat's element carries the seat type tag"
        );
        assert!(
            elements
                .iter()
                .all(|element| element.mode == VisibilityMode::Total),
            "a seat's element is born Total like any other"
        );

        // The interpreted half: the satellite rows, keyed by the same ids.
        let seats = backend.commission_store().seats(id).await.unwrap();
        assert_eq!(seats.len(), 2);
        let first_seat = seats.iter().find(|s| s.id == first_id).expect("first");
        assert_eq!(first_seat.kind.as_str(), "Creator");
        assert_eq!(
            first_seat.prompt.as_ref().map(|p| p.as_str()),
            Some("Two refs, please.")
        );
        assert_eq!(
            first_seat.link.as_ref().map(|l| l.as_str()),
            Some("https://forms.example/apply")
        );
        assert!(first_seat.is_vacant(), "a seat is born vacant (AC3)");
        let second_seat = seats.iter().find(|s| s.id == second_id).expect("second");
        assert_eq!(
            second_seat.kind.as_str(),
            "Creator",
            "kinds repeat freely (AC1)"
        );
        assert!(second_seat.prompt.is_none());
        assert!(second_seat.link.is_none());
        assert!(second_seat.is_vacant());

        // An unknown commission simply has no seats.
        assert!(
            backend
                .commission_store()
                .seats(CommissionId::new(uuid::Uuid::now_v7()))
                .await
                .unwrap()
                .is_empty()
        );
    }

    // ZMVP-76 (transactionality) — a dropped unit discards BOTH halves of a
    // staged seat: neither the element nor the satellite survives, so a
    // half-declared seat is unrepresentable.
    #[tokio::test]
    async fn a_dropped_unit_discards_both_halves_of_a_seat() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        {
            let seat = NewSeat::contributed_at(
                id,
                address,
                "Client".parse::<SeatKind>().unwrap(),
                None,
                None,
                owner,
                Utc::now(),
            );
            let mut uow = database.begin().await.unwrap();
            uow.commissions().declare_seat(&seat).await.unwrap();
            // drops without commit -> rollback
        }

        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "no element landed"
        );
        assert!(
            backend
                .commission_store()
                .seats(id)
                .await
                .unwrap()
                .is_empty(),
            "no satellite landed"
        );
    }

    // ZMVP-76/166 — a seat walks the same address gate as every element write:
    // an absent/foreign tab refuses with UnknownTab, an undeclared surface with
    // UnknownSurface, and NEITHER half lands either time.
    #[tokio::test]
    async fn declare_seat_walks_the_shared_address_gate() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;
        let theirs = commission("Theirs", user_id());
        let theirs_id = theirs.id;
        backend.create_commission(&theirs).await.unwrap();
        let their_address = only_address(&backend, theirs_id).await;

        let seat_at = |address: SurfaceAddress| {
            NewSeat::contributed_at(
                id,
                address,
                "Creator".parse::<SeatKind>().unwrap(),
                None,
                None,
                owner,
                Utc::now(),
            )
        };

        // A tab id that exists nowhere.
        let fabricated = seat_at(SurfaceAddress::new(TabId::mint(), only_surface()));
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_seat(&fabricated)
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<UnknownTab>().is_some(),
            "absent tab surfaces as UnknownTab, got: {err:?}"
        );
        drop(uow);

        // A real tab — belonging to someone else's commission.
        let cross = seat_at(their_address);
        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().declare_seat(&cross).await.unwrap_err();
        assert!(
            err.downcast_ref::<UnknownTab>().is_some(),
            "a foreign tab is indistinguishable from an absent one, got: {err:?}"
        );
        drop(uow);

        // A surface the skeleton does not declare.
        let invented = seat_at(SurfaceAddress::new(
            address.tab,
            "invented".parse::<SurfaceName>().unwrap(),
        ));
        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().declare_seat(&invented).await.unwrap_err();
        assert!(
            err.downcast_ref::<UnknownSurface>().is_some(),
            "an undeclared surface surfaces as UnknownSurface, got: {err:?}"
        );
        drop(uow);

        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "no refused declaration left an element behind"
        );
        assert!(
            backend
                .commission_store()
                .seats(id)
                .await
                .unwrap()
                .is_empty(),
            "nor a satellite"
        );
    }

    // ZMVP-166 — removing a seat's element sweeps its satellite AND its pending
    // invitations, the mem mirror of the pg `ON DELETE CASCADE` chain off
    // `commission_element (id)`. This is what keeps "one identity, two rows"
    // honest through a removal.
    #[tokio::test]
    async fn removing_a_seats_element_sweeps_its_satellite_and_offers() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let seat = NewSeat::contributed_at(
            id,
            address,
            "Creator".parse::<SeatKind>().unwrap(),
            None,
            None,
            owner,
            Utc::now(),
        );
        let seat_id = seat.id;
        let invited = user_id();
        let invitation = SeatInvitation::issue(id, seat_id, invited, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        uow.commissions().declare_seat(&seat).await.unwrap();
        uow.commissions()
            .create_seat_invitation(&invitation)
            .await
            .unwrap();
        uow.commit().await.unwrap();
        assert!(
            backend
                .commission_store()
                .find_pending_seat_invitation(id, seat_id, invited)
                .await
                .unwrap()
                .is_some(),
            "the offer stands before the removal"
        );

        remove_element(&database, id, seat_id).await.unwrap();

        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "the element is gone"
        );
        assert!(
            backend
                .commission_store()
                .seats(id)
                .await
                .unwrap()
                .is_empty(),
            "the satellite cascaded away with it"
        );
        assert!(
            backend
                .commission_store()
                .find_pending_seat_invitation(id, seat_id, invited)
                .await
                .unwrap()
                .is_none(),
            "and so did the pending offer on that seat"
        );
    }

    // ZMVP-77 AC1 (store layer) — declaring Slots contributes one element per
    // Slot AND its title/notes satellite sharing the id, all in one unit: the
    // batch lands together, in request order, and nothing anywhere can name an
    // occupant.
    #[tokio::test]
    async fn declare_slot_creates_an_element_with_its_satellite() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let knight = NewSlot::contributed_at(
            id,
            address.clone(),
            "The knight".parse::<SlotTitle>().unwrap(),
            Some("plate, not chain".to_string()),
            owner,
            Utc::now(),
        );
        let mage = NewSlot::contributed_at(
            id,
            address.clone(),
            "The mage".parse::<SlotTitle>().unwrap(),
            None,
            owner,
            Utc::now(),
        );
        let (knight_id, mage_id) = (knight.id, mage.id);

        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .declare_slots(&[knight, mage])
            .await
            .unwrap();
        uow.commit().await.unwrap();

        let elements = backend.elements_of(id).await.unwrap();
        assert_eq!(elements.len(), 2, "one element per Slot");
        assert_eq!(elements[0].id, knight_id, "request order is append order");
        assert_eq!(elements[1].id, mage_id);
        assert!(
            elements
                .iter()
                .all(|element| element.element_type == ElementType::slot()),
            "a Slot's element carries the slot type tag"
        );
        assert!(
            elements
                .iter()
                .all(|element| *element.payload.as_value() == json!({})),
            "the carrying element's payload is empty — the substance is the satellite's"
        );

        let slots = backend.slots_of(id).await.unwrap();
        assert_eq!(slots.len(), 2, "zero or more Slots (AC2)");
        let knight = slots.iter().find(|s| s.element_id == knight_id).unwrap();
        assert_eq!(knight.title.as_str(), "The knight");
        assert_eq!(knight.notes.as_deref(), Some("plate, not chain"));
        let mage = slots.iter().find(|s| s.element_id == mage_id).unwrap();
        assert_eq!(mage.title.as_str(), "The mage");
        assert!(mage.notes.is_none(), "notes are optional");
    }

    // ZMVP-77/166 — Slots walk the same address gate, and the batch is
    // ALL-OR-NOTHING: a refusal partway through leaves nothing behind, because
    // the whole batch rides one unit of work.
    #[tokio::test]
    async fn declare_slot_refuses_bad_addresses_and_takes_the_batch_with_it() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let slot_at = |address: SurfaceAddress, title: &str| {
            NewSlot::contributed_at(
                id,
                address,
                title.parse::<SlotTitle>().unwrap(),
                None,
                owner,
                Utc::now(),
            )
        };

        // The first Slot is fine; the second names a tab that exists nowhere.
        let good = slot_at(address.clone(), "Fine");
        let bad = slot_at(SurfaceAddress::new(TabId::mint(), only_surface()), "Doomed");
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_slots(&[good, bad])
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<UnknownTab>().is_some(),
            "absent tab surfaces as UnknownTab, got: {err:?}"
        );
        drop(uow);
        assert!(
            backend.elements_of(id).await.unwrap().is_empty(),
            "the EARLIER Slot of the aborted batch left nothing behind (all-or-nothing)"
        );
        assert!(backend.slots_of(id).await.unwrap().is_empty());

        // An undeclared surface refuses the same way.
        let invented = slot_at(
            SurfaceAddress::new(address.tab, "invented".parse::<SurfaceName>().unwrap()),
            "Invented",
        );
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_slots(&[invented])
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<UnknownSurface>().is_some(),
            "an undeclared surface surfaces as UnknownSurface, got: {err:?}"
        );
        drop(uow);
        assert!(backend.slots_of(id).await.unwrap().is_empty());
    }

    // ZMVP-77 (transactionality) — a dropped unit discards both halves of a
    // staged Slot, and removing its element cascades the satellite away.
    #[tokio::test]
    async fn declare_slot_commits_and_rolls_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, address) = composed_commission(&backend, owner).await;

        let staged = NewSlot::contributed_at(
            id,
            address.clone(),
            "Uncommitted".parse::<SlotTitle>().unwrap(),
            None,
            owner,
            Utc::now(),
        );
        let staged_id = staged.id;
        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions().declare_slots(&[staged]).await.unwrap();
            assert!(
                backend.find_slot(staged_id).await.unwrap().is_none(),
                "an open unit's staged Slot is invisible to a shared read"
            );
            // drops without commit -> rollback
        }
        assert!(backend.find_slot(staged_id).await.unwrap().is_none());
        assert!(backend.elements_of(id).await.unwrap().is_empty());

        // Committed, then removed: the satellite leaves with its element.
        let kept = NewSlot::contributed_at(
            id,
            address,
            "Kept".parse::<SlotTitle>().unwrap(),
            None,
            owner,
            Utc::now(),
        );
        let kept_id = kept.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().declare_slots(&[kept]).await.unwrap();
        uow.commit().await.unwrap();
        assert!(backend.find_slot(kept_id).await.unwrap().is_some());

        remove_element(&database, id, kept_id).await.unwrap();
        assert!(
            backend.find_slot(kept_id).await.unwrap().is_none(),
            "the satellite cascaded away with its element"
        );
    }
}
