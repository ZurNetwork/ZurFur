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
        ChangelogEntry, ChangelogEntryKind, ChannelPointer, Commission, CommissionFile,
        CommissionId, CommissionTitle, CommissionTree, DeadlineStatus, DirectionStatus, FileKey,
        GrantLevel, LapsedDeadline, LifecycleStep, NewChangelogEntry, NewComponent, NewSeat,
        NewSlot, NewSurface, NodeId, NodeKind, NodeRow, Placement, RootSurface, Seat,
        SeatInvitation, SeatInvitationId, SeatKind, SeatLink, SeatPrompt, Slot, SlotTitle,
        SurfaceMode, Visibility, derive_deadline_status,
    },
    invitation::InvitationState,
    maturity::Maturity,
    user::UserId,
};
use domain::ports::{
    CannotRemoveRoot, ChangelogStore, ChangelogWrites, CommissionStore, CommissionWrites,
    NodeNotFound, ParentNodeNotFound, ParentNotASurface,
};
use serde_json::Value;

use crate::MemBackend;

/// The shared **parent gate** of every tree-growing write — the mem mirror of
/// `PgCommissionWrites::require_surface_parent` (ZMVP-71/72): the named parent
/// must exist in `commission`'s own tree (an absent id and a node from another
/// commission both refuse with [`ParentNodeNotFound`], indistinguishably,
/// *before* anything about the node is revealed) and must be a surface, else
/// [`ParentNotASurface`] (components are leaves; nothing grows under one). One
/// path, so the two add ops can't drift apart on either rule. Returns the
/// parent's [`SurfaceMode`] on success — the mode `add_surface` inherits;
/// `add_component` has no use for it.
fn require_surface_parent(
    nodes: &HashMap<NodeId, StoredNode>,
    parent: NodeId,
    commission: CommissionId,
) -> anyhow::Result<SurfaceMode> {
    match nodes.get(&parent) {
        Some(node) if node.commission_id == commission => match node.kind {
            NodeKind::Surface { mode } => Ok(mode),
            NodeKind::Component => Err(ParentNotASurface.into()),
        },
        _ => Err(ParentNodeNotFound.into()),
    }
}

/// The next append `position` within `parent`'s sibling group — the mem mirror
/// of the pg `COALESCE(MAX(position) + 1, 0)` subquery.
fn next_position(nodes: &HashMap<NodeId, StoredNode>, parent: NodeId) -> i32 {
    nodes
        .values()
        .filter(|node| node.parent == Some(parent))
        .map(|node| node.position + 1)
        .max()
        .unwrap_or(0)
}

/// The fields of a [`Commission`] we keep behind the lock. Like `Account`,
/// `Commission` isn't `Clone` (an aggregate root, not a value), so we store its
/// parts and rebuild a fresh `Commission` on read. `Clone` so a unit of work can
/// deep-copy the commissions map into its staging snapshot (see
/// [`MemBackend::stage`]).
#[derive(Clone)]
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

/// One commission tree node as the mem backend keeps it — the in-memory mirror
/// of a pg `commission_node` row (ZMVP-71). Keyed by [`NodeId`] in the backend
/// map, so the row's own id lives in the key. `Clone` so a unit of work can
/// deep-copy the node map into its staging snapshot.
#[derive(Clone)]
pub(crate) struct StoredNode {
    /// The tree (commission) this node belongs to.
    pub(crate) commission_id: CommissionId,
    /// The parent node, or `None` for the root surface.
    pub(crate) parent: Option<NodeId>,
    /// The typed envelope half (kind + mode on surfaces).
    pub(crate) kind: NodeKind,
    /// Sibling order within the parent (append = max + 1).
    pub(crate) position: i32,
    /// Who created the node.
    pub(crate) created_by: UserId,
    /// When the node was created.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
    /// The type-owned payload, opaque here exactly as in pg.
    pub(crate) payload: Value,
}

/// One declared Slot's **satellite** as the mem backend keeps it — the
/// in-memory mirror of a pg `commission_slot` row (ZMVP-77). Keyed in the
/// backend map by the [`NodeId`] of the component that carries the Slot (the
/// satellite's own key), exactly like the pg table. Deliberately occupant-less: fill is unrepresentable until the
/// Character epic adds it. `Clone` so a unit of work can deep-copy the map into
/// its staging snapshot.
#[derive(Clone)]
pub(crate) struct StoredSlot {
    /// The commission the Slot belongs to (the pg row's own commission FK).
    pub(crate) commission_id: CommissionId,
    /// The Slot's required title, validated at the boundary.
    pub(crate) title: SlotTitle,
    /// The optional freeform notes, exactly as declared.
    pub(crate) notes: Option<String>,
}

impl StoredSlot {
    /// Rebuild the read shape for the component node `id` that keys this
    /// satellite.
    fn rebuild(&self, id: NodeId) -> Slot {
        Slot {
            node_id: id,
            commission_id: self.commission_id,
            title: self.title.clone(),
            notes: self.notes.clone(),
        }
    }
}

/// One declared Seat's interpreted half as the mem backend keeps it — the
/// in-memory mirror of a pg `commission_seat` row (ZMVP-76), keyed by the
/// seat's [`NodeId`] in the backend map (one identity: the tree node in
/// [`StoredNode`], this satellite here). `Clone` so a unit of work can
/// deep-copy the seat map into its staging snapshot.
#[derive(Clone)]
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
/// deep-copy the map into its staging snapshot.
#[derive(Clone)]
pub(crate) struct StoredSeatInvitation {
    /// The commission whose Seat is offered.
    pub(crate) commission: CommissionId,
    /// The Seat being offered (its tree node id).
    pub(crate) seat: NodeId,
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
/// work can deep-copy the log into its staging snapshot. Append-only like the pg
/// table: nothing in this crate mutates or removes one once pushed.
#[derive(Clone)]
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
/// like the pg log: nothing here mutates a pushed row.
#[derive(Clone)]
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
    /// its root surface** ([`RootSurface::of`], ZMVP-71 AC1) **and its owner's
    /// participant row** (ZMVP-76: the owner is a permanent Participant from
    /// birth, stamped with the commission's creation instant), the mem mirror
    /// of the pg adapter's three inserts in one transaction: all three maps
    /// belong to this unit's staging snapshot, so commission, root, and
    /// membership commit or vanish together — a treeless or owner-less
    /// commission is unrepresentable. The pg `id` is a
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
        let root = RootSurface::of(commission);
        {
            let mut nodes = self
                .0
                .nodes
                .lock()
                .expect("MemBackend nodes mutex poisoned");
            nodes.insert(
                root.id,
                StoredNode {
                    commission_id: commission.id,
                    parent: None,
                    kind: NodeKind::Surface { mode: root.mode },
                    position: 0,
                    created_by: root.created_by,
                    created_at: root.created_at,
                    payload: Value::Object(Default::default()),
                },
            );
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

    /// Grow the tree under an existing parent surface — the mem mirror of the
    /// pg `INSERT … position = max(sibling) + 1` (ZMVP-71 AC2), behind the same
    /// shared parent gate ([`require_surface_parent`]): absent/foreign refuses
    /// with [`ParentNodeNotFound`], a component parent with
    /// [`ParentNotASurface`] (ZMVP-72 — components are leaves). The mode is
    /// **inherited from the parent** (Engineer ruling 2026-07-07, PR #103) —
    /// the gate hands it back on success, since a surface parent always
    /// carries one.
    async fn add_surface(&mut self, surface: &NewSurface) -> anyhow::Result<()> {
        let mut nodes = self
            .0
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        let mode = require_surface_parent(&nodes, surface.parent, surface.commission_id)?;
        let position = next_position(&nodes, surface.parent);
        nodes.insert(
            surface.id,
            StoredNode {
                commission_id: surface.commission_id,
                parent: Some(surface.parent),
                kind: NodeKind::Surface { mode },
                position,
                created_by: surface.created_by,
                created_at: surface.created_at,
                payload: Value::Object(Default::default()),
            },
        );
        Ok(())
    }

    /// Grow a leaf under an existing parent surface — the mem mirror of the pg
    /// component insert (ZMVP-72 AC1): the same shared parent gate, the same
    /// append order, kind [`NodeKind::Component`] (no mode exists to store —
    /// AC2), and the opaque payload held verbatim so it reads back exactly as
    /// written (AC3).
    async fn add_component(&mut self, component: &NewComponent) -> anyhow::Result<()> {
        let mut nodes = self
            .0
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        require_surface_parent(&nodes, component.parent, component.commission_id)?;
        let position = next_position(&nodes, component.parent);
        nodes.insert(
            component.id,
            StoredNode {
                commission_id: component.commission_id,
                parent: Some(component.parent),
                kind: NodeKind::Component,
                position,
                created_by: component.created_by,
                created_at: component.created_at,
                payload: component.payload.clone(),
            },
        );
        Ok(())
    }

    /// Prune the tree — the mem mirror of the pg gate + cascading `DELETE` +
    /// renumber (ZMVP-73): the target must exist in `commission`'s own tree
    /// (an absent id and a foreign node both refuse with [`NodeNotFound`],
    /// indistinguishably — a foreign *root* included, so removal probes reveal
    /// nothing) and must not be the root ([`CannotRemoveRoot`], AC3). The
    /// subtree the pg self-referential cascade takes is walked and dropped
    /// here explicitly, and the remaining sibling group renumbers to
    /// contiguous positions — all on the unit's staging snapshot, so prune and
    /// renumber commit or vanish together.
    async fn remove_node(&mut self, commission: CommissionId, node: NodeId) -> anyhow::Result<()> {
        let mut nodes = self
            .0
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        let parent = match nodes.get(&node) {
            Some(stored) if stored.commission_id == commission => match stored.parent {
                Some(parent) => parent,
                None => return Err(CannotRemoveRoot.into()),
            },
            _ => return Err(NodeNotFound.into()),
        };

        // The subtree, walked breadth-first from the target (the mem mirror of
        // the pg cascade).
        let mut doomed = vec![node];
        let mut next = 0;
        while next < doomed.len() {
            let current = doomed[next];
            doomed.extend(
                nodes
                    .iter()
                    .filter(|(_, stored)| stored.parent == Some(current))
                    .map(|(id, _)| *id),
            );
            next += 1;
        }
        for id in doomed {
            nodes.remove(&id);
        }

        // Renumber the surviving sibling group to contiguous positions.
        let mut siblings: Vec<(NodeId, i32)> = nodes
            .iter()
            .filter(|(_, stored)| stored.parent == Some(parent))
            .map(|(id, stored)| (*id, stored.position))
            .collect();
        siblings.sort_by_key(|(_, position)| *position);
        for (index, (id, _)) in siblings.into_iter().enumerate() {
            nodes
                .get_mut(&id)
                .expect("sibling was just enumerated")
                .position = index as i32;
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
    /// Slot, the same shared parent gate ([`require_surface_parent`]) and
    /// append order as [`add_component`](Self::add_component) plant an
    /// ordinary [`NodeKind::Component`] leaf with the empty payload, and the
    /// Slot itself lands as the [`StoredSlot`] satellite keyed by that
    /// component's node id. All maps belong to this unit's staging snapshot,
    /// so the whole batch commits or vanishes together — a refusal mid-batch
    /// errors the unit and nothing is applied. No changelog entry (the frozen
    /// taxonomy has no Slot variant), and no occupant exists to store.
    async fn declare_slots(&mut self, new_slots: &[NewSlot]) -> anyhow::Result<()> {
        for slot in new_slots {
            {
                let mut nodes = self
                    .0
                    .nodes
                    .lock()
                    .expect("MemBackend nodes mutex poisoned");
                require_surface_parent(&nodes, slot.parent, slot.commission_id)?;
                let position = next_position(&nodes, slot.parent);
                nodes.insert(
                    slot.id,
                    StoredNode {
                        commission_id: slot.commission_id,
                        parent: Some(slot.parent),
                        kind: NodeKind::Component,
                        position,
                        created_by: slot.created_by,
                        created_at: slot.created_at,
                        payload: Value::Object(Default::default()),
                    },
                );
            }
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

    /// Remove the commission and, with it, its changelog entries — the mem
    /// mirror of the pg `DELETE FROM commission` plus `commission_changelog`'s
    /// `ON DELETE CASCADE` (ZMVP-66; ruling E35). Lands on the unit's staged
    /// snapshot, so it commits or rolls back with the caller's fact gate
    /// (ruling E17), like every write here. An absent commission is a no-op,
    /// per the port contract. A future commission-child map added to
    /// [`MemBackend`] must cascade here too, mirroring its pg table's cascade.
    async fn delete(&mut self, id: CommissionId) -> anyhow::Result<()> {
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        commissions.remove(&id);
        let mut changelog = self
            .0
            .changelog
            .lock()
            .expect("MemBackend changelog mutex poisoned");
        changelog.retain(|entry| entry.commission_id != id);
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
    /// Declare a seat — the mem mirror of the pg adapter's node + satellite
    /// pair (ZMVP-76): behind the same shared parent gate
    /// ([`require_surface_parent`]), one [`StoredNode`] (an ordinary component
    /// — the untyped ZMVP-72 contract) and one [`StoredSeat`] land under the
    /// same [`NodeId`] in this unit's staging snapshot, so both halves commit
    /// or vanish together. The occupant is never written here: every seat is
    /// born vacant (AC3; ZMVP-79 fills it).
    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()> {
        {
            let mut nodes = self
                .0
                .nodes
                .lock()
                .expect("MemBackend nodes mutex poisoned");
            require_surface_parent(&nodes, seat.parent, seat.commission_id)?;
            let position = next_position(&nodes, seat.parent);
            nodes.insert(
                seat.id,
                StoredNode {
                    commission_id: seat.commission_id,
                    parent: Some(seat.parent),
                    kind: NodeKind::Component,
                    position,
                    created_by: seat.created_by,
                    created_at: seat.created_at,
                    payload: Value::Object(Default::default()),
                },
            );
        }
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
    ) -> anyhow::Result<()> {
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        if let Some(stored) = commissions.get_mut(&id) {
            stored.deadline = deadline;
        }
        Ok(())
    }

    /// Repoint (or clear) the stored deadline-axis Status — the mem mirror of
    /// the pg `UPDATE commission SET deadline_status` (ZMVP-86): one nullable
    /// slot, so a set replaces whole. An absent commission is a no-op, per the
    /// port contract.
    async fn set_deadline_status(
        &mut self,
        id: CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<()> {
        let mut commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        if let Some(stored) = commissions.get_mut(&id) {
            stored.deadline_status = status;
        }
        Ok(())
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

    /// Load and assemble the whole tree — the mem mirror of the pg one-query
    /// read (ZMVP-71): filter the node map by commission, then share the same
    /// [`CommissionTree::assemble`] the pg adapter uses. `None` for a
    /// commission nobody created (no rows = no root); assembly failures on a
    /// non-empty row set surface as errors (corruption, unreachable through
    /// the write ports).
    async fn load_tree(&self, id: CommissionId) -> anyhow::Result<Option<CommissionTree>> {
        let rows: Vec<NodeRow> = {
            let nodes = self
                .0
                .nodes
                .lock()
                .expect("MemBackend nodes mutex poisoned");
            nodes
                .iter()
                .filter(|(_, node)| node.commission_id == id)
                .map(|(node_id, node)| NodeRow {
                    id: *node_id,
                    parent: node.parent,
                    kind: node.kind,
                    position: node.position,
                    created_by: node.created_by,
                    created_at: node.created_at,
                    payload: node.payload.clone(),
                })
                .collect()
        };
        if rows.is_empty() {
            return Ok(None);
        }
        Ok(Some(CommissionTree::assemble(rows)?))
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
        seat: NodeId,
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

    /// The declared Slot whose component node is `node`, or `None` (inspect
    /// helper — the satellite read; ZMVP-77 exposes no read port yet, the
    /// viewer-facing surface being ZMVP-75's projection).
    pub async fn find_slot(&self, node: NodeId) -> anyhow::Result<Option<Slot>> {
        let slots = self.slots.lock().expect("MemBackend slots mutex poisoned");
        Ok(slots.get(&node).map(|stored| stored.rebuild(node)))
    }

    /// Every Slot declared on `commission`, in declaration order (the carrying
    /// components' node ids are UUIDv7, so sorting by node id is creation
    /// order) — the "zero or more" count of ZMVP-77 AC2 (inspect helper).
    pub async fn slots_of(&self, commission: CommissionId) -> anyhow::Result<Vec<Slot>> {
        let slots = self.slots.lock().expect("MemBackend slots mutex poisoned");
        let mut found: Vec<Slot> = slots
            .iter()
            .filter(|(_, stored)| stored.commission_id == commission)
            .map(|(id, stored)| stored.rebuild(*id))
            .collect();
        found.sort_by_key(|slot| *slot.node_id);
        Ok(found)
    }

    /// Fill a declared Seat's occupant slot directly on the shared store
    /// (test-only seeder). There is no seat-fill port yet — accepting a seat
    /// invitation is ZMVP-79 — so this stands in for it, letting an api test
    /// exercise the "already occupied" refusal (ZMVP-78) against a truly filled
    /// seat. Panics if `seat` is not a declared seat (the test set it up wrong).
    pub fn occupy_seat(&self, seat: NodeId, occupant: UserId) {
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
    use domain::elements::commission::{NewSlot, NodeId, NodeKind, SlotTitle, SurfaceMode};
    use domain::ports::{CannotRemoveRoot, NodeNotFound, ParentNodeNotFound, ParentNotASurface};
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

    // ZMVP-71 AC1 (store layer) — a commission is born with its root surface in
    // the same unit of work: after create+commit the loaded tree is exactly one
    // root — kind Surface, mode Total (a birth commission is Private), the
    // owner as creator, the commission's own creation instant — with no
    // children. There is no write that could remove it (removal is ZMVP-73's
    // guarded op), so "cannot be removed" holds by construction.
    #[tokio::test]
    async fn creating_a_commission_mints_its_root_surface() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Rooted", owner);
        let id = created.id;
        let created_at = created.created_at;

        let mut uow = database.begin().await.unwrap();
        uow.commissions().create(&created).await.unwrap();
        uow.commit().await.unwrap();

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("a created commission always has a tree");
        assert!(
            matches!(
                tree.root.kind,
                NodeKind::Surface {
                    mode: SurfaceMode::Total
                }
            ),
            "born Private = root Total (the closed-door default)"
        );
        assert_eq!(tree.root.created_by, owner, "the owner is the creator");
        assert_eq!(
            tree.root.created_at, created_at,
            "the root is born with the commission"
        );
        assert!(tree.root.children.is_empty(), "a fresh tree is just a root");
    }

    // ZMVP-71 AC2/AC3 (store layer) — surfaces grow under any existing surface:
    // two siblings under the root keep append order, a nested surface attaches
    // under its parent, and every new surface is born Total.
    #[tokio::test]
    async fn add_surface_grows_the_tree_in_append_order() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Growing", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let root = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists")
            .root
            .id;

        let first = NewSurface::under(id, root, owner, Utc::now());
        let second = NewSurface::under(id, root, owner, Utc::now());
        let (first_id, second_id) = (first.id, second.id);
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_surface(&first).await.unwrap();
        uow.commissions().add_surface(&second).await.unwrap();
        uow.commit().await.unwrap();

        // Nest one under the first child.
        let nested = NewSurface::under(id, first_id, owner, Utc::now());
        let nested_id = nested.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_surface(&nested).await.unwrap();
        uow.commit().await.unwrap();

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.children.len(), 2);
        assert_eq!(tree.root.children[0].id, first_id, "append order holds");
        assert_eq!(tree.root.children[1].id, second_id);
        assert!(
            tree.root.children.iter().all(|child| matches!(
                child.kind,
                NodeKind::Surface {
                    mode: SurfaceMode::Total
                }
            )),
            "every new surface is born Total (AC3)"
        );
        assert_eq!(
            tree.root.children[0].children[0].id, nested_id,
            "a surface grows under any existing surface, not just the root"
        );
    }

    // ZMVP-71 — the parent must exist in THIS commission's tree: a fabricated
    // parent id and a parent belonging to another commission both fail with
    // ParentNodeNotFound (one indistinguishable answer — no probing other
    // trees), and neither write lands.
    #[tokio::test]
    async fn add_surface_refuses_absent_and_foreign_parents() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let mine = commission("Mine", owner);
        let theirs = commission("Theirs", user_id());
        let mine_id = mine.id;
        let theirs_id = theirs.id;
        backend.create_commission(&mine).await.unwrap();
        backend.create_commission(&theirs).await.unwrap();
        let their_root = backend
            .commission_store()
            .load_tree(theirs_id)
            .await
            .unwrap()
            .expect("their tree exists")
            .root
            .id;

        // A parent id that exists nowhere.
        let fabricated = NewSurface::under(
            mine_id,
            NodeId::new(uuid::Uuid::now_v7()),
            owner,
            Utc::now(),
        );
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .add_surface(&fabricated)
            .await
            .unwrap_err();
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "absent parent surfaces as ParentNodeNotFound, got: {err:?}"
        );
        drop(uow);

        // A real node — in someone else's tree.
        let cross = NewSurface::under(mine_id, their_root, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow.commissions().add_surface(&cross).await.unwrap_err();
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
        );
        drop(uow);

        let tree = backend
            .commission_store()
            .load_tree(mine_id)
            .await
            .unwrap()
            .expect("tree exists");
        assert!(tree.root.children.is_empty(), "no refused write landed");
    }

    // ZMVP-71 (transactionality) — a staged surface is invisible until commit
    // and discarded on drop, exactly like every other unit-of-work write.
    #[tokio::test]
    async fn add_surface_commits_and_rolls_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let created = commission("Tx", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let root = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists")
            .root
            .id;

        {
            let surface = NewSurface::under(id, root, owner, Utc::now());
            let mut uow = database.begin().await.unwrap();
            uow.commissions().add_surface(&surface).await.unwrap();
            let shared = backend
                .commission_store()
                .load_tree(id)
                .await
                .unwrap()
                .expect("tree exists");
            assert!(
                shared.root.children.is_empty(),
                "an open unit's staged surface is invisible to a shared read"
            );
            // `uow` drops here without `commit` -> the staged surface is discarded.
        }

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert!(
            tree.root.children.is_empty(),
            "a dropped unit of work persists no surface"
        );
    }

    // load_tree for a commission nobody created is None, mirroring `find`.
    #[tokio::test]
    async fn load_tree_answers_none_for_an_unknown_commission() {
        let backend = MemBackend::new();
        assert!(
            backend
                .commission_store()
                .load_tree(CommissionId::new(uuid::Uuid::now_v7()))
                .await
                .unwrap()
                .is_none()
        );
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

    /// Seeds a committed commission and returns `(its id, its root node id)`.
    async fn rooted_commission(backend: &MemBackend, owner: UserId) -> (CommissionId, NodeId) {
        let created = commission("Componented", owner);
        let id = created.id;
        backend.create_commission(&created).await.unwrap();
        let root = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists")
            .root
            .id;
        (id, root)
    }

    // ZMVP-72 AC1/AC2/AC3 (store layer) — a component grows as a leaf under a
    // surface (root and non-root alike), carries kind Component (no mode is
    // even representable), and its opaque payload reads back exactly as
    // written — semantically byte-for-byte, nested structure, unicode, numbers,
    // booleans, in-payload nulls and all.
    #[tokio::test]
    async fn add_component_grows_a_leaf_whose_payload_round_trips() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        let surface = NewSurface::under(id, root, owner, Utc::now());
        let surface_id = surface.id;
        let payload = json!({
            "kind": "text",
            "body": "Reference: 三毛猫 🐾 — \"line\\break\"",
            "revision": 3,
            "ratio": 1.5,
            "flags": [true, false, null],
            "nested": { "empty": {}, "list": [] },
        });
        let on_root = NewComponent::under(id, root, payload.clone(), owner, Utc::now());
        let nested = NewComponent::under(id, surface_id, json!(null), owner, Utc::now());
        let (on_root_id, nested_id) = (on_root.id, nested.id);

        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_surface(&surface).await.unwrap();
        uow.commissions().add_component(&on_root).await.unwrap();
        uow.commissions().add_component(&nested).await.unwrap();
        uow.commit().await.unwrap();

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.children.len(), 2);
        assert_eq!(tree.root.children[0].id, surface_id, "append order");
        let component = &tree.root.children[1];
        assert_eq!(component.id, on_root_id);
        assert!(
            matches!(component.kind, NodeKind::Component),
            "a component carries no mode of its own"
        );
        assert_eq!(component.created_by, owner);
        assert_eq!(component.payload, payload, "the payload is opaque (AC3)");
        assert!(component.children.is_empty());

        let nested = &tree.root.children[0].children[0];
        assert_eq!(nested.id, nested_id, "grows under a non-root surface too");
        assert_eq!(
            nested.payload,
            json!(null),
            "even a top-level JSON null round-trips verbatim"
        );
    }

    // ZMVP-72 AC1/AC2 — components are leaves: growing ANYTHING under one —
    // another component or a surface — refuses with ParentNotASurface, and the
    // refused write leaves nothing behind.
    #[tokio::test]
    async fn nothing_grows_under_a_component() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        let component = NewComponent::under(id, root, json!({}), owner, Utc::now());
        let component_id = component.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_component(&component).await.unwrap();
        uow.commit().await.unwrap();

        let child_component = NewComponent::under(id, component_id, json!({}), owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .add_component(&child_component)
            .await
            .expect_err("a component under a component refuses");
        assert!(
            err.downcast_ref::<ParentNotASurface>().is_some(),
            "expected ParentNotASurface, got: {err:?}"
        );
        drop(uow);

        let child_surface = NewSurface::under(id, component_id, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .add_surface(&child_surface)
            .await
            .expect_err("a surface under a component refuses too");
        assert!(
            err.downcast_ref::<ParentNotASurface>().is_some(),
            "expected ParentNotASurface, got: {err:?}"
        );
        drop(uow);

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.children.len(), 1);
        assert!(
            tree.root.children[0].children.is_empty(),
            "a component never has children"
        );
    }

    /// The `(id, position)` pairs of `parent`'s current children, in position
    /// order — read straight off the shared node map, so a test can assert the
    /// renumbering invariant (positions contiguous from 0) that the assembled
    /// tree deliberately hides.
    fn sibling_positions(backend: &MemBackend, parent: NodeId) -> Vec<(NodeId, i32)> {
        let nodes = backend.nodes.lock().expect("nodes mutex");
        let mut pairs: Vec<(NodeId, i32)> = nodes
            .iter()
            .filter(|(_, node)| node.parent == Some(parent))
            .map(|(id, node)| (*id, node.position))
            .collect();
        pairs.sort_by_key(|(_, position)| *position);
        pairs
    }

    /// Runs `remove_node` in its own committed unit of work.
    async fn remove_node(
        database: &std::sync::Arc<dyn domain::ports::Database>,
        commission: CommissionId,
        node: NodeId,
    ) -> anyhow::Result<()> {
        let mut uow = database.begin().await?;
        uow.commissions().remove_node(commission, node).await?;
        uow.commit().await
    }

    // ZMVP-73 AC1 (store layer) — removing a mid-tree surface takes its ENTIRE
    // subtree (a component and a nested surface with its own component), leaves
    // the other siblings intact, and renumbers the remaining sibling group so
    // positions stay contiguous, in the same transaction.
    #[tokio::test]
    async fn remove_surface_takes_its_whole_subtree_and_renumbers() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        // root -> [first, doomed, last]; under doomed: a component and a
        // surface holding another component.
        let first = NewSurface::under(id, root, owner, Utc::now());
        let doomed = NewSurface::under(id, root, owner, Utc::now());
        let last = NewSurface::under(id, root, owner, Utc::now());
        let in_doomed =
            NewComponent::under(id, doomed.id, json!({"kind": "text"}), owner, Utc::now());
        let nested = NewSurface::under(id, doomed.id, owner, Utc::now());
        let in_nested = NewComponent::under(id, nested.id, json!({}), owner, Utc::now());
        let (first_id, doomed_id, last_id) = (first.id, doomed.id, last.id);

        let mut uow = database.begin().await.unwrap();
        {
            let mut commissions = uow.commissions();
            commissions.add_surface(&first).await.unwrap();
            commissions.add_surface(&doomed).await.unwrap();
            commissions.add_surface(&last).await.unwrap();
            commissions.add_component(&in_doomed).await.unwrap();
            commissions.add_surface(&nested).await.unwrap();
            commissions.add_component(&in_nested).await.unwrap();
        }
        uow.commit().await.unwrap();

        remove_node(&database, id, doomed_id).await.unwrap();

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(
            tree.root.children.len(),
            2,
            "the surface and its whole subtree went together"
        );
        assert_eq!(tree.root.children[0].id, first_id, "sibling order holds");
        assert_eq!(tree.root.children[1].id, last_id);
        assert!(
            tree.root.children.iter().all(|c| c.children.is_empty()),
            "nothing of the subtree survives"
        );
        assert_eq!(
            sibling_positions(&backend, root),
            vec![(first_id, 0), (last_id, 1)],
            "the remaining siblings renumber to contiguous positions"
        );
    }

    // ZMVP-73 AC2 (store layer) — removing a component removes just that leaf;
    // its siblings survive in order with contiguous positions.
    #[tokio::test]
    async fn remove_component_removes_only_the_leaf() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        let doomed = NewComponent::under(id, root, json!({"kind": "text"}), owner, Utc::now());
        let surface = NewSurface::under(id, root, owner, Utc::now());
        let kept = NewComponent::under(id, root, json!({}), owner, Utc::now());
        let (doomed_id, surface_id, kept_id) = (doomed.id, surface.id, kept.id);

        let mut uow = database.begin().await.unwrap();
        {
            let mut commissions = uow.commissions();
            commissions.add_component(&doomed).await.unwrap();
            commissions.add_surface(&surface).await.unwrap();
            commissions.add_component(&kept).await.unwrap();
        }
        uow.commit().await.unwrap();

        remove_node(&database, id, doomed_id).await.unwrap();

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.children.len(), 2, "only the one leaf went");
        assert_eq!(tree.root.children[0].id, surface_id, "order holds");
        assert_eq!(tree.root.children[1].id, kept_id);
        assert_eq!(
            sibling_positions(&backend, root),
            vec![(surface_id, 0), (kept_id, 1)],
            "positions renumber contiguously"
        );
    }

    // ZMVP-73 AC3 (store layer) — the root surface refuses removal with
    // CannotRemoveRoot, and the whole tree (root and children) is untouched.
    #[tokio::test]
    async fn removing_the_root_is_refused() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        let child = NewSurface::under(id, root, owner, Utc::now());
        let child_id = child.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_surface(&child).await.unwrap();
        uow.commit().await.unwrap();

        let err = remove_node(&database, id, root)
            .await
            .expect_err("the root refuses removal");
        assert!(
            err.downcast_ref::<CannotRemoveRoot>().is_some(),
            "expected CannotRemoveRoot, got: {err:?}"
        );

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.id, root, "the root survives");
        assert_eq!(tree.root.children[0].id, child_id, "so does its subtree");
    }

    // ZMVP-73 — the target must exist in THIS commission's tree: a fabricated
    // node id and a node belonging to another commission both refuse with
    // NodeNotFound (one indistinguishable answer). Someone else's ROOT through
    // my commission id is also NodeNotFound — never CannotRemoveRoot, which
    // would leak what a foreign node is — and nothing is removed anywhere.
    #[tokio::test]
    async fn remove_refuses_absent_and_foreign_nodes() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (mine, _) = rooted_commission(&backend, owner).await;
        let (theirs, their_root) = rooted_commission(&backend, user_id()).await;

        let err = remove_node(&database, mine, NodeId::new(uuid::Uuid::now_v7()))
            .await
            .expect_err("a fabricated node refuses");
        assert!(
            err.downcast_ref::<NodeNotFound>().is_some(),
            "expected NodeNotFound, got: {err:?}"
        );

        let err = remove_node(&database, mine, their_root)
            .await
            .expect_err("a foreign node refuses");
        assert!(
            err.downcast_ref::<NodeNotFound>().is_some(),
            "a foreign root is indistinguishable from an absent node, got: {err:?}"
        );

        assert!(
            backend
                .commission_store()
                .load_tree(theirs)
                .await
                .unwrap()
                .is_some(),
            "the foreign tree is untouched"
        );
    }

    // ZMVP-73 (transactionality) — a staged removal is invisible until commit
    // and discarded on drop, exactly like every other unit-of-work write.
    #[tokio::test]
    async fn remove_commits_and_rolls_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        let surface = NewSurface::under(id, root, owner, Utc::now());
        let surface_id = surface.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_surface(&surface).await.unwrap();
        uow.commit().await.unwrap();

        {
            let mut uow = database.begin().await.unwrap();
            uow.commissions().remove_node(id, surface_id).await.unwrap();
            let shared = backend
                .commission_store()
                .load_tree(id)
                .await
                .unwrap()
                .expect("tree exists");
            assert_eq!(
                shared.root.children.len(),
                1,
                "an open unit's staged removal is invisible to a shared read"
            );
            // `uow` drops here without `commit` -> the staged removal is discarded.
        }

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(
            tree.root.children.len(),
            1,
            "a dropped unit of work removes nothing"
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

    // ZMVP-76 AC1/AC2/AC3 (store layer) — declaring a seat grows ONE component
    // node in the tree AND its interpreted satellite sharing the id, atomically
    // in the unit: kind + requirements read back, the seat is born vacant, and
    // kinds repeat freely across a commission's seats.
    #[tokio::test]
    async fn declare_seat_lands_a_node_and_its_satellite_together() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        let first = NewSeat::under(
            id,
            root,
            "Creator".parse::<SeatKind>().unwrap(),
            Some("Two refs, please.".parse::<SeatPrompt>().unwrap()),
            Some("https://forms.example/apply".parse::<SeatLink>().unwrap()),
            owner,
            Utc::now(),
        );
        // A second seat of the SAME kind — kinds repeat freely (AC1).
        let second = NewSeat::under(
            id,
            root,
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

        // The tree half: two component nodes under the root, in append order.
        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.children.len(), 2);
        assert_eq!(tree.root.children[0].id, first_id, "append order");
        assert_eq!(tree.root.children[1].id, second_id);
        assert!(
            tree.root
                .children
                .iter()
                .all(|child| matches!(child.kind, NodeKind::Component)),
            "a seat's node is an ordinary component (the untyped v1 contract)"
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
    // staged seat: neither the node nor the satellite survives, so a
    // half-declared seat is unrepresentable.
    #[tokio::test]
    async fn a_dropped_unit_discards_both_halves_of_a_seat() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        {
            let seat = NewSeat::under(
                id,
                root,
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

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert!(tree.root.children.is_empty(), "no node landed");
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

    // ZMVP-76 — a seat walks the same parent gate as every tree-growing write:
    // absent/foreign parents refuse with ParentNodeNotFound, a component
    // parent with ParentNotASurface, and nothing lands either time.
    #[tokio::test]
    async fn declare_seat_walks_the_shared_parent_gate() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;
        let (_, their_root) = rooted_commission(&backend, user_id()).await;

        let kind = || "Creator".parse::<SeatKind>().unwrap();
        // Absent parent.
        let fabricated = NewSeat::under(
            id,
            NodeId::new(uuid::Uuid::now_v7()),
            kind(),
            None,
            None,
            owner,
            Utc::now(),
        );
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_seat(&fabricated)
            .await
            .expect_err("absent parent refuses");
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "expected ParentNodeNotFound, got: {err:?}"
        );
        drop(uow);

        // Foreign parent — indistinguishable from absent.
        let cross = NewSeat::under(id, their_root, kind(), None, None, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_seat(&cross)
            .await
            .expect_err("foreign parent refuses");
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
        );
        drop(uow);

        // A component parent — seats live under surfaces, like every leaf.
        let component = NewComponent::under(id, root, json!({}), owner, Utc::now());
        let component_id = component.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_component(&component).await.unwrap();
        uow.commit().await.unwrap();
        let under_component =
            NewSeat::under(id, component_id, kind(), None, None, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_seat(&under_component)
            .await
            .expect_err("a component parent refuses");
        assert!(
            err.downcast_ref::<ParentNotASurface>().is_some(),
            "expected ParentNotASurface, got: {err:?}"
        );
        drop(uow);

        assert!(
            backend
                .commission_store()
                .seats(id)
                .await
                .unwrap()
                .is_empty(),
            "no refused seat landed"
        );
    }

    // ZMVP-72 — the component's parent must exist in THIS commission's tree: a
    // fabricated parent id and a surface belonging to another commission both
    // fail with ParentNodeNotFound (one indistinguishable answer), never
    // ParentNotASurface (which would leak that a foreign node exists and is a
    // component or not).
    #[tokio::test]
    async fn add_component_refuses_absent_and_foreign_parents() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (mine, _) = rooted_commission(&backend, owner).await;
        let (_, their_root) = rooted_commission(&backend, user_id()).await;

        let fabricated = NewComponent::under(
            mine,
            NodeId::new(uuid::Uuid::now_v7()),
            json!({}),
            owner,
            Utc::now(),
        );
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .add_component(&fabricated)
            .await
            .expect_err("absent parent refuses");
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "expected ParentNodeNotFound, got: {err:?}"
        );
        drop(uow);

        let cross = NewComponent::under(mine, their_root, json!({}), owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .add_component(&cross)
            .await
            .expect_err("foreign parent refuses");
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
        );
        drop(uow);

        let tree = backend
            .commission_store()
            .load_tree(mine)
            .await
            .unwrap()
            .expect("tree exists");
        assert!(tree.root.children.is_empty(), "no refused write landed");
    }

    // ZMVP-77 AC1/AC2 (store layer) — declaring a Slot lands an ordinary
    // component leaf (empty payload; the substance is the satellite's) PLUS the
    // satellite carrying the title and optional notes, keyed by the node id. A
    // commission holds zero, then several, Slots; nothing about an occupant is
    // representable anywhere (fill is the Character epic's).
    #[tokio::test]
    async fn declare_slot_creates_a_leaf_with_its_satellite() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        assert!(
            backend.slots_of(id).await.unwrap().is_empty(),
            "a fresh commission holds zero Slots (a valid state)"
        );

        let noted = NewSlot::under(
            id,
            root,
            "The knight".parse::<SlotTitle>().unwrap(),
            Some("full plate, no cape".to_string()),
            owner,
            Utc::now(),
        );
        let bare = NewSlot::under(
            id,
            root,
            "The mage".parse::<SlotTitle>().unwrap(),
            None,
            owner,
            Utc::now(),
        );
        let (noted_id, bare_id) = (noted.id, bare.id);

        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .declare_slots(&[noted, bare])
            .await
            .unwrap();
        uow.commit().await.unwrap();

        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert_eq!(tree.root.children.len(), 2);
        assert_eq!(tree.root.children[0].id, noted_id, "append order holds");
        assert_eq!(tree.root.children[1].id, bare_id);
        for child in &tree.root.children {
            assert!(
                matches!(child.kind, NodeKind::Component),
                "each Slot's carrying node is an ordinary component leaf"
            );
            assert_eq!(child.payload, json!({}), "the substance is the satellite's");
            assert!(child.children.is_empty());
        }

        let noted_slot = backend
            .find_slot(noted_id)
            .await
            .unwrap()
            .expect("satellite exists");
        assert_eq!(noted_slot.title.as_str(), "The knight");
        assert_eq!(noted_slot.notes.as_deref(), Some("full plate, no cape"));
        assert_eq!(noted_slot.commission_id, id);
        let bare_slot = backend
            .find_slot(bare_id)
            .await
            .unwrap()
            .expect("satellite exists");
        assert!(bare_slot.notes.is_none(), "omitted notes stay absent");

        assert_eq!(
            backend.slots_of(id).await.unwrap().len(),
            2,
            "the commission holds two declared Slots (zero or more, AC2)"
        );
    }

    // ZMVP-77 — the parent gates match every other tree write: absent and
    // foreign parents are one indistinguishable ParentNodeNotFound, a component
    // parent is ParentNotASurface, and no refused declaration leaves a node or
    // a satellite behind.
    #[tokio::test]
    async fn declare_slot_refuses_bad_parents() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (mine, my_root) = rooted_commission(&backend, owner).await;
        let (_, their_root) = rooted_commission(&backend, user_id()).await;

        let component = NewComponent::under(mine, my_root, json!({}), owner, Utc::now());
        let component_id = component.id;
        let mut uow = database.begin().await.unwrap();
        uow.commissions().add_component(&component).await.unwrap();
        uow.commit().await.unwrap();

        let title = || "The knight".parse::<SlotTitle>().unwrap();

        let fabricated = NewSlot::under(
            mine,
            NodeId::new(uuid::Uuid::now_v7()),
            title(),
            None,
            owner,
            Utc::now(),
        );
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_slots(&[fabricated])
            .await
            .expect_err("absent parent refuses");
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "expected ParentNodeNotFound, got: {err:?}"
        );
        drop(uow);

        let cross = NewSlot::under(mine, their_root, title(), None, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_slots(&[cross])
            .await
            .expect_err("foreign parent refuses");
        assert!(
            err.downcast_ref::<ParentNodeNotFound>().is_some(),
            "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
        );
        drop(uow);

        let nested = NewSlot::under(mine, component_id, title(), None, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        let err = uow
            .commissions()
            .declare_slots(&[nested])
            .await
            .expect_err("component parent refuses");
        assert!(
            err.downcast_ref::<ParentNotASurface>().is_some(),
            "expected ParentNotASurface, got: {err:?}"
        );
        drop(uow);

        // The batch is all-or-nothing (PR #108 ruling): a refused Slot
        // mid-batch takes the valid ones down with it.
        let good = NewSlot::under(mine, my_root, title(), None, owner, Utc::now());
        let bad = NewSlot::under(mine, component_id, title(), None, owner, Utc::now());
        let mut uow = database.begin().await.unwrap();
        uow.commissions()
            .declare_slots(&[good, bad])
            .await
            .expect_err("a refused Slot fails the whole batch");
        drop(uow);

        assert!(
            backend.slots_of(mine).await.unwrap().is_empty(),
            "no refused declaration — including a batch's valid half — left a satellite behind"
        );
    }

    // ZMVP-77 (transactionality) — the node and its satellite land (or vanish)
    // together: staged writes are invisible until commit and a dropped unit
    // discards both halves.
    #[tokio::test]
    async fn declare_slot_commits_and_rolls_back_with_the_unit() {
        let backend = MemBackend::new();
        let database = backend.database();
        let owner = user_id();
        let (id, root) = rooted_commission(&backend, owner).await;

        {
            let slot = NewSlot::under(
                id,
                root,
                "Never lands".parse::<SlotTitle>().unwrap(),
                None,
                owner,
                Utc::now(),
            );
            let slot_id = slot.id;
            let mut uow = database.begin().await.unwrap();
            uow.commissions().declare_slots(&[slot]).await.unwrap();
            assert!(
                backend.find_slot(slot_id).await.unwrap().is_none(),
                "an open unit's staged Slot is invisible to a shared read"
            );
            // `uow` drops here without `commit` -> both halves are discarded.
        }

        assert!(backend.slots_of(id).await.unwrap().is_empty());
        let tree = backend
            .commission_store()
            .load_tree(id)
            .await
            .unwrap()
            .expect("tree exists");
        assert!(
            tree.root.children.is_empty(),
            "a dropped unit of work persists neither the node nor the satellite"
        );
    }
}
