//! [`CommissionStore`] (reads) and [`CommissionWrites`] (writes) over PostgreSQL:
//! commissions in the `commission` table (ZMVP-65/87). Reads are pool-backed;
//! writes are reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.commissions()`), so no commission write can skip a transaction. See
//! DESIGN/Commission and DD `24150017` (compile-enforced Unit of Work).
//!
//! The SQL lives in `queries/commission/` (one statement per file, embedded via
//! `include_str!`) and is verified against the migrated schema by the
//! `query_files_prepare` test.

use crate::queries::CommissionQuery;
use chrono::{DateTime, Utc};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        commission::{
            ChannelPointer, Commission, CommissionFile, CommissionId, CommissionTitle,
            CommissionTree, DeadlineStatus, DirectionStatus, FileKey, GrantLevel, LapsedDeadline,
            LifecycleStep, NewComponent, NewSurface, NodeId, NodeKind, NodeRow, Placement,
            RootSurface, SurfaceMode, Visibility, derive_deadline_status,
        },
        maturity::{Maturity, MaturityRating},
        user::UserId,
    },
    ports::{
        CannotRemoveRoot, CommissionStore, CommissionWrites, NodeNotFound, ParentNodeNotFound,
        ParentNotASurface,
    },
};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

/// THE FACT REGISTRY (ZMVP-67; Deletion DD `3014657`): the tables whose rows are
/// commission [`Fact`](domain::elements::commission::Fact)s — evidence that blocks
/// hard deletion. [`commission_has_facts`](CommissionWrites::commission_has_facts)
/// must query **every** table listed here; the DD's canonical trigger list names
/// the kinds to expect (Products, ratings, EXP, achievements, payments), none of
/// which exist yet.
///
/// Registering a table here is a **deliberate act with teeth**: the schema
/// tripwire test (`adapter-pg/tests/commission.rs`) fails the moment a migration
/// adds a commission-referencing table that is classified in neither this list nor
/// [`COMMISSION_NON_FACT_TABLES`], and the compile-time guard below refuses to
/// build while this list is non-empty but the predicate is still constant `false`.
/// A fact-minter therefore wires its storage into the predicate in the same change
/// that creates it — it cannot merge past either trip by accident.
pub const COMMISSION_FACT_TABLES: &[&str] = &[];

/// Tables that hold a foreign key onto `commission(id)` but whose rows are
/// **deliberately not facts** — commission-owned bookkeeping that cascades away
/// with the commission instead of blocking its deletion. Every
/// commission-referencing table must appear in exactly one of this list or
/// [`COMMISSION_FACT_TABLES`]; the schema tripwire test enforces the
/// classification.
///
/// - `commission_changelog` (ZMVP-87): the commission's own memory. The Changelog
///   DD's retention rule — entries hard-delete **only** with the commission itself
///   (or legal duty) — is exactly `ON DELETE CASCADE`, not a deletion block.
/// - `commission_placement` / `commission_current_placement` / `commission_view_grant`
///   (ZMVP-70): account positioning — the append-only placement log, its cached
///   current pointer, and the view-grant keys. Commission-owned bookkeeping that
///   cascades with the commission (Ownership Separation DD `29130754`), never a
///   fact that blocks its deletion.
/// - `commission_node` (ZMVP-71): the content tree. Nodes are the commission's
///   own composition, not evidence that work happened — the Tree Storage DD
///   (`28409880`) has the whole tree cascade with its commission, which is what
///   ZMVP-66's "gone entirely" relies on.
/// - `commission_file` (ZMVP-88): the Index-canonical link for a file entry (an
///   uploaded work-in-progress). A file entry is **not** a Product — no fact-lock —
///   so it cascades away with the commission, keeping a commission with only file
///   entries hard-deletable (AC2). Its bytes live in `file_blob`, which holds no
///   commission foreign key (blobs know nothing of commissions) and so is not
///   commission-referencing — the hard-delete cascade (ZMVP-66) severs them through
///   [`FileStore::delete`](domain::ports::FileStore::delete), not the row cascade.
pub const COMMISSION_NON_FACT_TABLES: &[&str] = &[
    "commission_changelog",
    "commission_file",
    "commission_node",
    "commission_placement",
    "commission_current_placement",
    "commission_view_grant",
];

// Tripwire (conductor ruling E18): the constant-`false` body of
// `commission_has_facts` below is sound ONLY while the fact registry is empty.
// Registering the first fact table makes this fail to compile, forcing whoever
// wires a fact-minter to replace the constant with a real EXISTS query over every
// registered table — and to delete this guard in the same, deliberate edit.
const _: () = assert!(
    COMMISSION_FACT_TABLES.is_empty(),
    "COMMISSION_FACT_TABLES gained an entry: replace the constant-`false` body of \
     PgCommissionWrites::commission_has_facts with a real query over every \
     registered fact table (and mirror it in adapter-mem), then remove this guard"
);

/// PostgreSQL write view over an open transaction (the [`CommissionWrites`] surface).
/// Holds **only** a borrowed `&mut PgConnection` — the transaction owned by the
/// [`PgUnitOfWork`](crate::PgUnitOfWork) — so no pool is in scope here and a
/// bare-pool write is unrepresentable. Built by `uow.commissions()`; its borrow ties
/// it to the shared transaction, so its write commits (or rolls back) with the rest
/// of the unit. See DD `24150017`.
pub struct PgCommissionWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    /// The write executes on `&mut *self.conn`; there is deliberately no pool here.
    pub(crate) conn: &'a mut PgConnection,
}

impl PgCommissionWrites<'_> {
    /// The shared **parent gate** of every tree-growing write (ZMVP-71/72), on
    /// the open transaction: the named parent must exist in `commission`'s own
    /// tree — an absent id and a node from another commission both refuse with
    /// [`ParentNodeNotFound`], indistinguishably, *before* anything about the
    /// node is revealed — and must be a surface, else [`ParentNotASurface`]
    /// (components are leaves; nothing grows under one). Locks the parent row
    /// (`FOR UPDATE`), so concurrent appends under one parent serialize instead
    /// of racing to the same `position` slot and aborting on the deferred
    /// UNIQUE at commit (PR #103 review; Engineer-ruled fix) — one path,
    /// shared by both add ops, so neither can drift out of the lock's
    /// protection. Returns the parent's [`SurfaceMode`] on success — the mode
    /// `add_surface` inherits (Engineer ruling 2026-07-07, PR #103);
    /// `add_component` has no use for it.
    async fn require_surface_parent(
        &mut self,
        parent: NodeId,
        commission: CommissionId,
    ) -> anyhow::Result<SurfaceMode> {
        let row: Option<(String, Option<String>)> =
            sqlx::query_as(CommissionQuery::RequireSurfaceParent.sql())
                .bind(*parent)
                .bind(*commission)
                .fetch_optional(&mut *self.conn)
                .await?;
        let Some((type_tag, mode)) = row else {
            return Err(ParentNodeNotFound.into());
        };
        match NodeKind::from_columns(&type_tag, mode.as_deref()) {
            Some(NodeKind::Surface { mode }) => Ok(mode),
            Some(NodeKind::Component) => Err(ParentNotASurface.into()),
            None => Err(anyhow::anyhow!(
                "unknown node envelope ({:?}, {:?})",
                type_tag,
                mode
            )),
        }
    }
}

#[async_trait::async_trait]
impl CommissionWrites for PgCommissionWrites<'_> {
    /// Insert a freshly created commission as one row (`INSERT INTO commission`)
    /// **plus its root surface** as one `commission_node` row ([`RootSurface::of`]),
    /// on this same open transaction (ZMVP-71 AC1) — a commission can never land
    /// without its tree. The [`LifecycleStep`](domain::elements::commission::LifecycleStep)
    /// and [`Visibility`](domain::elements::commission::Visibility) are each stored as
    /// their stable `as_str()` token in the `lifecycle` / `visibility` text columns,
    /// the root's `mode` token is the visibility's alias mapping
    /// ([`Visibility::as_root_mode`](domain::elements::commission::Visibility::as_root_mode)),
    /// and the nullable deadline maps to a nullable `timestamptz`. The ids are
    /// caller-/adapter-minted UUIDv7, so no conflict handling is needed; any store
    /// failure surfaces as an opaque error.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::CreateCommission.sql())
            .bind(*commission.id)
            .bind(commission.title.as_str())
            .bind(*commission.owner_id)
            .bind(commission.lifecycle_step.as_str())
            .bind(commission.visibility.as_str())
            .bind(commission.deadline)
            .bind(commission.maturity.map(|m| m.rating.as_str()))
            .bind(commission.maturity.map(|m| m.graphic))
            .bind(commission.created_at)
            .execute(&mut *self.conn)
            .await?;

        let root = RootSurface::of(commission);
        sqlx::query(CommissionQuery::CreateRootSurface.sql())
            .bind(*root.id)
            .bind(*commission.id)
            .bind(root.mode.as_str())
            .bind(*root.created_by)
            .bind(root.created_at)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Grow the tree under an existing parent surface (ZMVP-71 AC2), on the
    /// open transaction, behind the shared parent gate
    /// ([`require_surface_parent`](Self::require_surface_parent) —
    /// [`ParentNodeNotFound`] for absent/foreign, [`ParentNotASurface`] for a
    /// component parent), which also locks the row and hands back its mode —
    /// **inherited** by the new surface (Engineer ruling 2026-07-07, PR #103;
    /// inheritance never widens — see [`NewSurface::under`]). `position` is
    /// assigned as `max(sibling position) + 1` in a subquery on this same
    /// transaction, so append order can't race.
    async fn add_surface(&mut self, surface: &NewSurface) -> anyhow::Result<()> {
        let mode = self
            .require_surface_parent(surface.parent, surface.commission_id)
            .await?;

        sqlx::query(CommissionQuery::AddSurface.sql())
            .bind(*surface.id)
            .bind(*surface.commission_id)
            .bind(*surface.parent)
            .bind(mode.as_str())
            .bind(*surface.created_by)
            .bind(surface.created_at)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Grow a leaf under an existing parent surface (ZMVP-72 AC1), on the open
    /// transaction — the component mirror of [`add_surface`](Self::add_surface):
    /// the same shared parent gate, the same racing-proof append `position`
    /// subquery. The row stores `type = 'component'` with a **NULL `mode`**
    /// (the surface-XOR-mode CHECK's other arm — a component projects with its
    /// parent, AC2) and the opaque payload as jsonb, semantically unmodified —
    /// round-trips as an equal JSON value (jsonb is not byte-preserving)
    /// (AC3; a top-level JSON `null` lands as jsonb `'null'`, never SQL `NULL`).
    async fn add_component(&mut self, component: &NewComponent) -> anyhow::Result<()> {
        self.require_surface_parent(component.parent, component.commission_id)
            .await?;

        sqlx::query(CommissionQuery::AddComponent.sql())
            .bind(*component.id)
            .bind(*component.commission_id)
            .bind(*component.parent)
            .bind(*component.created_by)
            .bind(component.created_at)
            .bind(&component.payload)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Prune the tree (ZMVP-73), on the open transaction — three statements
    /// sharing it. First the target gate: one `SELECT` scoped to
    /// `commission_id`, so an absent node id and a node in another
    /// commission's tree refuse as one indistinguishable [`NodeNotFound`]
    /// **before** anything about the node is revealed (a foreign *root* is
    /// therefore never [`CannotRemoveRoot`]); a root here — `parent IS NULL` —
    /// refuses with [`CannotRemoveRoot`] (AC3). Then one `DELETE` of the node
    /// row, scoped by `(id, commission_id)` and asserted to affect exactly one
    /// row (a node that vanished between the gate and here re-refuses as
    /// [`NodeNotFound`] rather than silently proceeding); the self-referential
    /// `ON DELETE CASCADE` takes the entire subtree with it (Tree Storage DD
    /// `28409880` Decision 5). Finally the remaining sibling group — matched by
    /// `(parent, commission_id)` — renumbers to contiguous positions
    /// (`ROW_NUMBER` over the surviving order); the `UNIQUE (parent, position)`
    /// constraint is deferred, so intermediate states inside the transaction
    /// can't trip it. Every write is scoped by `commission_id`, not just the
    /// unique `id`/`parent`, so each statement is self-contained (PR #109 review).
    async fn remove_node(&mut self, commission: CommissionId, node: NodeId) -> anyhow::Result<()> {
        let row: Option<Option<Uuid>> = sqlx::query_scalar(CommissionQuery::RemoveNodeGate.sql())
            .bind(*node)
            .bind(*commission)
            .fetch_optional(&mut *self.conn)
            .await?;
        let Some(parent) = row else {
            return Err(NodeNotFound.into());
        };
        let Some(parent) = parent else {
            return Err(CannotRemoveRoot.into());
        };

        // Scope the delete by `commission_id` too — not just the unique `id` — so the
        // statement is self-contained rather than leaning on `id` uniqueness, and
        // assert it removed exactly the target row: a node that vanished between the
        // SELECT above and here (a concurrent removal) affects zero rows and surfaces
        // as `NodeNotFound` instead of silently proceeding to renumber (PR #109
        // review). The subtree still leaves via `ON DELETE CASCADE`, whose rows the
        // command count does not include.
        let deleted = sqlx::query(CommissionQuery::RemoveNodeDelete.sql())
            .bind(*node)
            .bind(*commission)
            .execute(&mut *self.conn)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(NodeNotFound.into());
        }

        // Renumber the vacated sibling group, scoped by `commission_id` as well so
        // the subquery matches the gate above and never leans on `parent` UUID
        // uniqueness as the sole scoping mechanism (PR #109 review).
        sqlx::query(CommissionQuery::RemoveNodeRenumber.sql())
            .bind(parent)
            .bind(*commission)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Insert a file entry's link row (`INSERT INTO commission_file`) on the open
    /// transaction, so it lands atomically with the caller's `file_added` changelog
    /// entry (ZMVP-88; Changelog DD D4). The bytes were already stored through
    /// [`FileStore`](domain::ports::FileStore) before this unit — never here. The id
    /// is a caller-minted UUIDv7, so no conflict handling is needed.
    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::AddFile.sql())
            .bind(*file.id)
            .bind(*file.commission_id)
            .bind(*file.uploaded_by)
            .bind(file.created_at)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Whether the commission bears any fact — answered **on the open transaction**,
    /// so a delete gate's check-then-delete has no TOCTOU window (ZMVP-67, ruling E17).
    ///
    /// Constant `false` today, and sound only by construction: the fact registry
    /// ([`COMMISSION_FACT_TABLES`]) is empty because no fact-minter exists — no
    /// table anywhere holds commission-anchored facts, so no query could find one.
    /// This is **not** a stub to fill casually: the compile-time guard on the
    /// registry refuses to build the moment a table is registered, and the schema
    /// tripwire test refuses any commission-referencing table that skips
    /// classification — so this body becomes a real `EXISTS` over every registered
    /// table in the same change that mints the first fact (Deletion DD `3014657`).
    async fn commission_has_facts(&mut self, _id: CommissionId) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Remove the commission row — one `DELETE FROM commission` on the open
    /// transaction, so the caller's fact gate
    /// ([`commission_has_facts`](CommissionWrites::commission_has_facts)) and the
    /// delete commit or roll back together (ZMVP-66, ruling E17). Child rows reap
    /// via each commission-referencing table's `ON DELETE CASCADE` (ruling E35;
    /// today `commission_changelog` — see [`COMMISSION_NON_FACT_TABLES`], whose
    /// tripwire keeps every future child classified). An absent commission
    /// matches no row: a no-op, per the port contract.
    async fn delete(&mut self, id: CommissionId) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::Delete.sql())
            .bind(*id)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Flip the `commission.archived_at` column (ZMVP-68) — one **conditional**
    /// `UPDATE` on the open transaction: the row matches only when the write is
    /// a real transition (`archived_at IS NULL` differs between the row and the
    /// requested state), so the returned rows-affected IS the transition answer
    /// and a repeat in the same direction touches nothing (keeping the original
    /// stamp). The caller keys its changelog append on the bool in this same
    /// unit of work, so a duplicate `archived`/`unarchived` entry is
    /// unrepresentable. An absent commission matches no row and answers `false`.
    async fn set_archived(
        &mut self,
        id: CommissionId,
        archived_at: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(CommissionQuery::SetArchived.sql())
            .bind(*id)
            .bind(archived_at)
            .execute(&mut *self.conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Write the posture's two column halves (`maturity`, `graphic`) — one
    /// `UPDATE` on the open transaction (ZMVP-31). Always both together: the
    /// signature has no clear arm and the schema's both-or-neither CHECK
    /// refuses a half-set pair, so an unrated-with-graphic (or rated-without)
    /// row is unrepresentable from any direction. An absent commission
    /// matches no row: a no-op here, per the port contract.
    async fn set_maturity(&mut self, id: CommissionId, maturity: Maturity) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::SetMaturity.sql())
            .bind(*id)
            .bind(maturity.rating.as_str())
            .bind(maturity.graphic)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Repoint (or clear) the `commission.linked_channel` column — one
    /// **conditional** `UPDATE` on the open transaction: the row matches only
    /// when the stored value differs from the requested one
    /// (`IS DISTINCT FROM`, so NULLs compare honestly), making rows-affected THE
    /// changed answer. The caller keys its changelog append on the bool in this
    /// same unit of work (ZMVP-87 AC3; Changelog DD D4), so a duplicate
    /// `channel_linked`/`channel_unlinked` entry is unrepresentable even under
    /// concurrent writers. An absent commission matches no row and answers
    /// `false`, per the port contract (existence is the caller's check).
    async fn set_linked_channel(
        &mut self,
        id: CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(CommissionQuery::SetLinkedChannel.sql())
            .bind(*id)
            .bind(channel.map(ChannelPointer::as_str))
            .execute(&mut *self.conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Append one placement-log row and repoint the current-placement cache to it,
    /// both on the open transaction (ZMVP-70) — so the cache equals the latest log
    /// row atomically, never via a second transaction. `RETURNING seq` carries the
    /// freshly-assigned ordering key straight into the cache upsert, so the two
    /// always agree on which row is current. Re-placement always appends (the log
    /// is never rewritten); the cache upsert overwrites on the `commission_id`
    /// primary key. A bad `commission`/`account` (no such row) fails the FK — the
    /// store-level backstop for a check the caller settles first.
    async fn place(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        placed_by: UserId,
        at: DateTimeUtc,
    ) -> anyhow::Result<()> {
        let seq: i64 = sqlx::query_scalar(CommissionQuery::PlaceAppend.sql())
            .bind(*commission)
            .bind(*account)
            .bind(*placed_by)
            .bind(at)
            .fetch_one(&mut *self.conn)
            .await?;

        sqlx::query(CommissionQuery::PlaceRepointCurrent.sql())
            .bind(*commission)
            .bind(*account)
            .bind(seq)
            .bind(*placed_by)
            .bind(at)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Upsert the account's key on the open transaction (ZMVP-70): one row per
    /// (commission, account), so re-granting replaces the level ("issuing anew").
    /// The level persists as its stable [`GrantLevel::as_str`] token. A bad
    /// `commission`/`account` fails the FK (the caller settled existence first).
    async fn grant_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::GrantView.sql())
            .bind(*commission)
            .bind(*account)
            .bind(level.as_str())
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Hard-delete the account's key on the open transaction (ZMVP-70; DD D5) —
    /// one `DELETE`, whose rows-affected IS the transition answer: `true` when a
    /// key existed and is now gone, `false` when the account held none (an
    /// idempotent no-op). The caller keys its `view_grant_revoked` changelog append
    /// on this bool in the same unit, so a duplicate entry is unrepresentable.
    async fn revoke_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(CommissionQuery::RevokeView.sql())
            .bind(*commission)
            .bind(*account)
            .execute(&mut *self.conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Repoint (or clear) the `commission.direction_status` column — one
    /// `UPDATE` on the open transaction, so the caller's matching
    /// `status_changed` changelog entry lands atomically with it (ZMVP-85;
    /// Changelog DD D4). The value is stored as its stable `as_str()` token; an
    /// absent commission matches no row: a no-op here, per the port contract
    /// (existence is the caller's check).
    async fn set_direction_status(
        &mut self,
        id: CommissionId,
        status: Option<DirectionStatus>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(CommissionQuery::SetDirectionStatus.sql())
            .bind(*id)
            .bind(status.map(|s| s.as_str()))
            .execute(&mut *self.conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Repoint (or clear) the `commission.deadline` column — one `UPDATE` on
    /// the open transaction, so the caller's matching
    /// `deadline_set`/`deadline_extended` changelog entry lands atomically
    /// with it (ZMVP-86; Changelog DD D4). An absent commission matches no
    /// row: a no-op here, per the port contract (existence is the caller's
    /// check).
    async fn set_deadline(
        &mut self,
        id: CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::SetDeadline.sql())
            .bind(*id)
            .bind(deadline)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Repoint (or clear) the `commission.deadline_status` column — one
    /// `UPDATE` on the open transaction, so the caller's matching entry (the
    /// manual `delayed` flag or the system `late` mark) lands atomically with
    /// it (ZMVP-86; Changelog DD D4). The value is stored as its stable
    /// `as_str()` token; an absent commission matches no row: a no-op here,
    /// per the port contract.
    async fn set_deadline_status(
        &mut self,
        id: CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<()> {
        sqlx::query(CommissionQuery::SetDeadlineStatus.sql())
            .bind(*id)
            .bind(status.map(|s| s.as_str()))
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// The sweeper's candidate scan (ZMVP-86, ruling E12), **on the open
    /// transaction** so the scan and the marks it feeds land in one unit (the
    /// [`commission_has_facts`](CommissionWrites::commission_has_facts)
    /// posture — no TOCTOU window). One `SELECT` filtered to: a deadline
    /// strictly before `now`, not already `late`, and a non-terminal
    /// lifecycle — the terminal tokens are derived from
    /// [`LifecycleStep::ALL`]/[`is_terminal`](LifecycleStep::is_terminal), so
    /// the enum (not this query) owns that vocabulary. A stored
    /// `deadline_status` outside the vocabulary means row tampering and
    /// surfaces as an `Err`, matching [`PgCommissionStore::find`].
    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>> {
        let terminal: Vec<String> = LifecycleStep::ALL
            .iter()
            .filter(|step| step.is_terminal())
            .map(|step| step.as_str().to_owned())
            .collect();
        // Late is never persisted, so dedup the log on the changelog itself
        // (Engineer ruling 2026-07-08). A commission is skipped only if it has a
        // `late` entry *since its latest deadline change* — a `deadline_set` /
        // `deadline_extended` re-arms the log, so each fresh miss is its own
        // event. Its Late *state* is derived on lookup ([`derive_deadline_status`]);
        // this pass only appends the entry.
        let rows: Vec<LapsedRow> = sqlx::query_as(CommissionQuery::LapsedDeadlines.sql())
            .bind(now)
            .bind(&terminal)
            .fetch_all(&mut *self.conn)
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok(LapsedDeadline {
                    id: CommissionId::new(row.id),
                    deadline: row.deadline,
                    status: row
                        .deadline_status
                        .as_deref()
                        .map(|token| {
                            DeadlineStatus::try_from(token).map_err(|_| {
                                anyhow::anyhow!("unknown deadline_status token {token:?}")
                            })
                        })
                        .transpose()?,
                })
            })
            .collect()
    }
}

/// The sweeper scan's row shape (`lapsed_deadlines.sql`).
#[derive(sqlx::FromRow)]
struct LapsedRow {
    id: Uuid,
    deadline: DateTime<Utc>,
    deadline_status: Option<String>,
}

/// The commission envelope row (`find.sql`); [`Commission`] is rebuilt from it
/// with every stored token re-validated through its domain gate.
#[derive(sqlx::FromRow)]
struct CommissionRow {
    title: String,
    owner_id: Uuid,
    lifecycle: String,
    visibility: String,
    deadline: Option<DateTime<Utc>>,
    maturity: Option<String>,
    graphic: Option<bool>,
    direction_status: Option<String>,
    deadline_status: Option<String>,
    linked_channel: Option<String>,
    archived_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

/// A placement row as both placement reads select it.
#[derive(sqlx::FromRow)]
struct PlacementRow {
    seq: i64,
    account_id: Uuid,
    placed_by: Uuid,
    placed_at: DateTime<Utc>,
}

impl PlacementRow {
    /// Attach the commission id the row was queried by, yielding the domain
    /// [`Placement`].
    fn into_placement(self, commission_id: CommissionId) -> Placement {
        Placement {
            seq: self.seq,
            commission_id,
            account_id: AccountId::new(self.account_id),
            placed_by: UserId::new(self.placed_by),
            placed_at: self.placed_at,
        }
    }
}

/// A `commission_node` row (`load_tree.sql`); the domain [`NodeRow`] is rebuilt
/// from it with the envelope re-validated through [`NodeKind::from_columns`].
#[derive(sqlx::FromRow)]
struct TreeNodeRow {
    id: Uuid,
    parent: Option<Uuid>,
    type_tag: String,
    mode: Option<String>,
    position: i32,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    payload: serde_json::Value,
}

/// PostgreSQL read store for commissions (the [`CommissionStore`] surface) —
/// the one canonical commission read port, born with the changelog (ZMVP-87).
/// Holds the pool directly — reads pay no transaction tax; the writes live on
/// [`PgCommissionWrites`], reached through the [`UnitOfWork`](domain::ports::UnitOfWork).
pub struct PgCommissionStore {
    pool: PgPool,
}

impl PgCommissionStore {
    /// Wraps a [`PgPool`] as a [`CommissionStore`]. Clones the pool handle (cheap —
    /// it's an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl CommissionStore for PgCommissionStore {
    /// Rebuild the [`Commission`] from its row. The stored `lifecycle`,
    /// `visibility`, `maturity`, `direction_status`, `deadline_status`, and
    /// `linked_channel` values are re-validated through their domain gates
    /// (`TryFrom<&str>` on [`LifecycleStep`] / [`Visibility`] / [`MaturityRating`]
    /// / [`DirectionStatus`] / [`DeadlineStatus`], with [`ChannelPointer::try_new`]
    /// and [`CommissionTitle::try_new`] for the
    /// title); a value outside its vocabulary means row tampering and surfaces
    /// as an `Err`, never a panic or a silent default — as does a half-set
    /// maturity posture, which the migration's CHECK already makes
    /// unrepresentable at the database.
    async fn find(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        let Some(row) = sqlx::query_as::<_, CommissionRow>(CommissionQuery::Find.sql())
            .bind(*id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };

        let maturity = match (row.maturity, row.graphic) {
            (None, None) => None,
            (Some(token), Some(graphic)) => Some(Maturity {
                rating: MaturityRating::try_from(token.as_str())
                    .map_err(|_| anyhow::anyhow!("unknown maturity token {token:?}"))?,
                graphic,
            }),
            (token, graphic) => {
                anyhow::bail!("half-set maturity posture (maturity {token:?}, graphic {graphic:?})")
            }
        };
        let lifecycle_step = LifecycleStep::try_from(row.lifecycle.as_str())
            .map_err(|_| anyhow::anyhow!("unknown lifecycle token {:?}", row.lifecycle))?;
        // The stored `deadline_status` is the manual `Delayed` flag only — `Late`
        // is never persisted (Engineer ruling 2026-07-08). Derive the effective
        // status fresh at lookup from the deadline, the same math the sweep's log
        // pass uses.
        let stored_deadline_status = row
            .deadline_status
            .as_deref()
            .map(|token| {
                DeadlineStatus::try_from(token)
                    .map_err(|_| anyhow::anyhow!("unknown deadline_status token {token:?}"))
            })
            .transpose()?;
        let deadline_status = derive_deadline_status(
            row.deadline,
            &lifecycle_step,
            stored_deadline_status,
            chrono::Utc::now(),
        );
        Ok(Some(Commission {
            id,
            title: CommissionTitle::try_new(row.title)?,
            owner_id: UserId::new(row.owner_id),
            lifecycle_step,
            visibility: Visibility::try_from(row.visibility.as_str())
                .map_err(|_| anyhow::anyhow!("unknown visibility token {:?}", row.visibility))?,
            deadline: row.deadline,
            maturity,
            direction_status: row
                .direction_status
                .as_deref()
                .map(|token| {
                    DirectionStatus::try_from(token)
                        .map_err(|_| anyhow::anyhow!("unknown direction_status token {token:?}"))
                })
                .transpose()?,
            deadline_status,
            linked_channel: row
                .linked_channel
                .map(ChannelPointer::try_new)
                .transpose()?,
            archived_at: row.archived_at,
            created_at: row.created_at,
        }))
    }

    /// The current-placement pointer row (ZMVP-70), or `None` if the commission
    /// was never placed. Read straight from the denormalized
    /// `commission_current_placement` cache — kept equal to the latest log row by
    /// [`place`](CommissionWrites::place).
    async fn current_placement(
        &self,
        commission: CommissionId,
    ) -> anyhow::Result<Option<Placement>> {
        let row: Option<PlacementRow> = sqlx::query_as(CommissionQuery::CurrentPlacement.sql())
            .bind(*commission)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| row.into_placement(commission)))
    }

    /// The whole placement log in append order (ascending `seq`) — the current
    /// placement is the last row, the origin the first (ZMVP-70). An unplaced
    /// commission has an empty log.
    async fn placement_log(&self, commission: CommissionId) -> anyhow::Result<Vec<Placement>> {
        let rows: Vec<PlacementRow> = sqlx::query_as(CommissionQuery::PlacementLog.sql())
            .bind(*commission)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_placement(commission))
            .collect())
    }

    /// The [`GrantLevel`] `account` holds on `commission`, or `None` (ZMVP-70).
    /// The stored token is re-validated through [`GrantLevel::parse`]; a value
    /// outside the vocabulary means row tampering and surfaces as an `Err`, never
    /// a silent default.
    async fn view_grant(
        &self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<Option<GrantLevel>> {
        let Some(level) = sqlx::query_scalar::<_, String>(CommissionQuery::ViewGrant.sql())
            .bind(*commission)
            .bind(*account)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        GrantLevel::parse(&level)
            .ok_or_else(|| anyhow::anyhow!("unknown grant level token {:?}", level))
            .map(Some)
    }

    /// Load and assemble the commission's whole tree: **one** indexed query
    /// (`WHERE commission_id = $1`, the Tree Storage DD's read model), rows
    /// re-validated through the domain gates ([`NodeKind::from_columns`] — an
    /// unknown type tag, a modeless surface, or a mode token outside the
    /// vocabulary means row tampering and surfaces as an `Err`), then nested by
    /// [`CommissionTree::assemble`] in Rust. `None` when no rows exist — no
    /// commission (a created one always has its root).
    async fn load_tree(&self, id: CommissionId) -> anyhow::Result<Option<CommissionTree>> {
        let rows: Vec<TreeNodeRow> = sqlx::query_as(CommissionQuery::LoadTree.sql())
            .bind(*id)
            .fetch_all(&self.pool)
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let rows =
            rows.into_iter()
                .map(|row| {
                    let kind = NodeKind::from_columns(&row.type_tag, row.mode.as_deref())
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "unknown node envelope ({:?}, {:?})",
                                row.type_tag,
                                row.mode
                            )
                        })?;
                    Ok(NodeRow {
                        id: NodeId::new(row.id),
                        parent: row.parent.map(NodeId::new),
                        kind,
                        position: row.position,
                        created_by: UserId::new(row.created_by),
                        created_at: row.created_at,
                        payload: row.payload,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Some(CommissionTree::assemble(rows)?))
    }

    /// The **owner arm** of participant-hood (ZMVP-87): one `EXISTS` over the
    /// owner column — the owner IS a Participant without holding a Seat
    /// (DESIGN/Commission). ZMVP-79 extends this query with the seated arm; an
    /// unknown commission matches nothing and answers `false`. **Unaffected by
    /// placement or view grants** (Ownership Separation DD Decision 8): a key is
    /// only a view, and positioning is environmental — neither makes an account's
    /// members Participants.
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool> {
        let is_participant: bool = sqlx::query_scalar(CommissionQuery::IsParticipant.sql())
            .bind(*commission)
            .bind(*user)
            .fetch_one(&self.pool)
            .await?;
        Ok(is_participant)
    }

    /// The file-entry link `key` names **within `commission`** (ZMVP-88) — one
    /// `SELECT` filtered by **both** id and commission_id, so a key belonging to a
    /// different commission matches no row and answers `None` (never a
    /// cross-commission existence oracle). The bytes live in `file_blob` behind the
    /// [`FileStore`](domain::ports::FileStore); this settles only the link the
    /// retrieval gate authorizes against.
    async fn find_file(
        &self,
        commission: CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>> {
        let row: Option<(Uuid, Uuid, Uuid, DateTime<Utc>)> =
            sqlx::query_as(CommissionQuery::FindFile.sql())
                .bind(*key)
                .bind(*commission)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(
            |(id, commission_id, uploaded_by, created_at)| CommissionFile {
                id: FileKey::new(id),
                commission_id: CommissionId::new(commission_id),
                uploaded_by: UserId::new(uploaded_by),
                created_at,
            },
        ))
    }
}
