//! [`CommissionStore`] (reads) and [`CommissionWrites`] (writes) over PostgreSQL:
//! commissions in the `commission` table (ZMVP-65/87). Reads are pool-backed;
//! writes are reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.commissions()`), so no commission write can skip a transaction. See
//! DESIGN/Commission and DD `24150017` (compile-enforced Unit of Work).

use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        commission::{
            ChannelPointer, Commission, CommissionId, CommissionTitle, GrantLevel, LifecycleStep,
            Placement, Visibility,
        },
        user::UserId,
    },
    ports::{CommissionStore, CommissionWrites},
};
use sqlx::{PgConnection, PgPool, query};

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
pub const COMMISSION_NON_FACT_TABLES: &[&str] = &[
    "commission_changelog",
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

#[async_trait::async_trait]
impl CommissionWrites for PgCommissionWrites<'_> {
    /// Insert a freshly created commission as one row (`INSERT INTO commission`).
    /// The [`LifecycleStep`](domain::elements::commission::LifecycleStep) and
    /// [`Visibility`](domain::elements::commission::Visibility) are each stored as their
    /// stable `as_str()` token in the `lifecycle` / `visibility` text columns, and the
    /// nullable deadline maps to a nullable `timestamptz`. The id is a caller-minted
    /// UUIDv7, so no conflict handling is needed; any store failure surfaces as an
    /// opaque error.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO
            commission (
                id,
                title,
                owner_id,
                lifecycle,
                visibility,
                deadline,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
            *commission.id,
            commission.title.as_str(),
            *commission.owner_id,
            commission.lifecycle_step.as_str(),
            commission.visibility.as_str(),
            commission.deadline,
            commission.created_at
        )
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
        query!(r#"DELETE FROM commission WHERE id = $1"#, *id)
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
        let result = query!(
            r#"
            UPDATE commission
            SET archived_at = $2
            WHERE id = $1 AND (archived_at IS NULL) <> ($2::timestamptz IS NULL)
            "#,
            *id,
            archived_at,
        )
        .execute(&mut *self.conn)
        .await?;
        Ok(result.rows_affected() > 0)
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
        let result = query!(
            r#"
            UPDATE commission
            SET linked_channel = $2
            WHERE id = $1 AND linked_channel IS DISTINCT FROM $2
            "#,
            *id,
            channel.map(ChannelPointer::as_str),
        )
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
        let seq = query!(
            r#"
            INSERT INTO commission_placement (commission_id, account_id, placed_by, placed_at)
            VALUES ($1, $2, $3, $4)
            RETURNING seq
            "#,
            *commission,
            *account,
            *placed_by,
            at,
        )
        .fetch_one(&mut *self.conn)
        .await?
        .seq;

        query!(
            r#"
            INSERT INTO commission_current_placement (commission_id, account_id, seq, placed_by, placed_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (commission_id)
            DO UPDATE SET account_id = EXCLUDED.account_id,
                          seq = EXCLUDED.seq,
                          placed_by = EXCLUDED.placed_by,
                          placed_at = EXCLUDED.placed_at
            "#,
            *commission,
            *account,
            seq,
            *placed_by,
            at,
        )
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
        query!(
            r#"
            INSERT INTO commission_view_grant (commission_id, account_id, level)
            VALUES ($1, $2, $3)
            ON CONFLICT (commission_id, account_id)
            DO UPDATE SET level = EXCLUDED.level
            "#,
            *commission,
            *account,
            level.as_str(),
        )
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
        let result = query!(
            r#"
            DELETE FROM commission_view_grant
            WHERE commission_id = $1 AND account_id = $2
            "#,
            *commission,
            *account,
        )
        .execute(&mut *self.conn)
        .await?;
        Ok(result.rows_affected() > 0)
    }
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
    /// `visibility`, and `linked_channel` values are re-validated through their
    /// domain gates ([`LifecycleStep::parse`] / [`Visibility::parse`] /
    /// [`ChannelPointer::try_new`], with [`CommissionTitle::try_new`] for the
    /// title); a value outside its vocabulary means row tampering and surfaces
    /// as an `Err`, never a panic or a silent default.
    async fn find(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        let Some(row) = query!(
            r#"
            SELECT title, owner_id, lifecycle, visibility, deadline, linked_channel,
                   archived_at, created_at
            FROM commission
            WHERE id = $1
            "#,
            *id,
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(Commission {
            id,
            title: CommissionTitle::try_new(row.title)?,
            owner_id: UserId::new(row.owner_id),
            lifecycle_step: LifecycleStep::parse(&row.lifecycle)
                .ok_or_else(|| anyhow::anyhow!("unknown lifecycle token {:?}", row.lifecycle))?,
            visibility: Visibility::parse(&row.visibility)
                .ok_or_else(|| anyhow::anyhow!("unknown visibility token {:?}", row.visibility))?,
            deadline: row.deadline,
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
        let Some(row) = query!(
            r#"
            SELECT seq, account_id, placed_by, placed_at
            FROM commission_current_placement
            WHERE commission_id = $1
            "#,
            *commission,
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        Ok(Some(Placement {
            seq: row.seq,
            commission_id: commission,
            account_id: AccountId::new(row.account_id),
            placed_by: UserId::new(row.placed_by),
            placed_at: row.placed_at,
        }))
    }

    /// The whole placement log in append order (ascending `seq`) — the current
    /// placement is the last row, the origin the first (ZMVP-70). An unplaced
    /// commission has an empty log.
    async fn placement_log(&self, commission: CommissionId) -> anyhow::Result<Vec<Placement>> {
        let rows = query!(
            r#"
            SELECT seq, account_id, placed_by, placed_at
            FROM commission_placement
            WHERE commission_id = $1
            ORDER BY seq
            "#,
            *commission,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| Placement {
                seq: row.seq,
                commission_id: commission,
                account_id: AccountId::new(row.account_id),
                placed_by: UserId::new(row.placed_by),
                placed_at: row.placed_at,
            })
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
        let Some(row) = query!(
            r#"
            SELECT level
            FROM commission_view_grant
            WHERE commission_id = $1 AND account_id = $2
            "#,
            *commission,
            *account,
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        GrantLevel::parse(&row.level)
            .ok_or_else(|| anyhow::anyhow!("unknown grant level token {:?}", row.level))
            .map(Some)
    }

    /// The **owner arm** of participant-hood (ZMVP-87): one `EXISTS` over the
    /// owner column — the owner IS a Participant without holding a Seat
    /// (DESIGN/Commission). ZMVP-79 extends this query with the seated arm; an
    /// unknown commission matches nothing and answers `false`. **Unaffected by
    /// placement or view grants** (Ownership Separation DD Decision 8): a key is
    /// only a view, and positioning is environmental — neither makes an account's
    /// members Participants.
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool> {
        let row = query!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM commission WHERE id = $1 AND owner_id = $2
            ) AS "is_participant!"
            "#,
            *commission,
            *user,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.is_participant)
    }
}
