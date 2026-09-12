//! [`CommissionStore`] (reads, pool-backed) and [`CommissionWrites`] (writes,
//! reachable only via an open [`UnitOfWork`](domain::ports::UnitOfWork)) over
//! the `commission` table.
//!
//! SQL lives in `queries/commission/`; row shapes are generated (see [`crate::queries`]).

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChannelPointer, Commission, CommissionComposition, CommissionFile, CommissionId,
            CommissionMarkup, CommissionTitle, DeadlineStatus, DirectionStatus, ElementId,
            ElementPayload, ElementRow, ElementType, FileKey, GrantLevel, LapsedDeadline,
            LifecycleStep, Markup, MarkupKey, NewElement, NewSeat, NewSlot, Seat, SeatInvitation,
            SeatInvitationId, SeatKind, SeatLink, SeatPrompt, SurfaceAddress, SurfaceName, TabId,
            TabName, TabRow, Visibility, VisibilityMode, declared_tabs, declares_surface,
            derive_deadline_status,
        },
        did::Did,
        invitation::InvitationState,
        maturity::{Maturity, MaturityRating},
        user::UserId,
        workflow::{Column, ColumnId, WorkflowId},
    },
    ports::{
        ColumnStore, CommissionReads, CommissionStore, CommissionWrites, ElementNotFound,
        UnknownSurface, UnknownTab,
    },
};
use sqlx::{PgConnection, PgPool};

use crate::{PgColumnStore, queries::commission as sql};

/// The commission [`Fact`](domain::elements::commission::Fact) tables —
/// [`commission_has_facts`](CommissionWrites::commission_has_facts)
/// must query every table listed here. Currently empty; see the compile-time guard below.
pub const COMMISSION_FACT_TABLES: &[&str] = &[];

/// FK-to-`commission` tables whose rows are commission bookkeeping, not facts —
/// they cascade away with the commission rather than blocking deletion. Every
/// commission-referencing table must appear in exactly one of this list or
/// [`COMMISSION_FACT_TABLES`]; a schema tripwire test enforces it.
pub const COMMISSION_NON_FACT_TABLES: &[&str] = &[
    "commission_changelog",
    "commission_element",
    "commission_file",
    "commission_markup",
    "commission_invitation",
    "commission_participant",
    "commission_seat",
    "commission_slot",
    "commission_surface_mode",
    "commission_tab",
    "commission_view_grant",
    "workflow_column_commission",
];

// Sound only while COMMISSION_FACT_TABLES is empty; fails to compile once a
// table is registered, forcing a real query to replace the constant `false`.
const _: () = assert!(
    COMMISSION_FACT_TABLES.is_empty(),
    "COMMISSION_FACT_TABLES gained an entry: replace the constant-`false` body of \
     PgCommissionWrites::commission_has_facts with a real query over every \
     registered fact table (and mirror it in adapter-mem), then remove this guard"
);

/// The [`CommissionWrites`] surface: a borrowed transaction connection, so a
/// bare-pool write is unrepresentable. Built by `uow.commissions()`.
pub struct PgCommissionWrites<'a> {
    /// The open transaction; there is deliberately no pool here.
    pub(crate) conn: &'a mut PgConnection,
}

impl PgCommissionWrites<'_> {
    /// Locks the tab row within `commission`, returning its declared name. Every
    /// composition write takes this lock before touching an element, serializing
    /// appends and removals aimed at the same tab. Refuses [`UnknownTab`] for an
    /// absent or foreign tab.
    async fn require_tab(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<TabName> {
        let located = self.locate_tab(commission, tab).await?;
        located.map(|row| row.tab).ok_or_else(|| UnknownTab.into())
    }

    /// [`require_tab`](Self::require_tab)'s whole-row form: answers `None` instead
    /// of [`UnknownTab`] and returns the [`TabRow`]. Also backs
    /// [`tab_for_update`](CommissionReads::tab_for_update).
    async fn locate_tab(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<Option<TabRow>> {
        let Some(row) = sql::require_tab(&mut *self.conn, **tab, **commission).await? else {
            return Ok(None);
        };
        let located = TabRow {
            id: TabId::new(row.id),
            tab: TabName::try_from(row.tab)?,
            mode: to_mode(&row.mode)?,
        };
        Ok(Some(located))
    }

    /// The shared address gate for every element write: the tab must exist
    /// ([`require_tab`](Self::require_tab)), then the skeleton must declare this
    /// surface inside that tab, else [`UnknownSurface`]. Order matters — an address
    /// wrong both ways refuses as [`UnknownTab`].
    async fn require_address(
        &mut self,
        commission: &CommissionId,
        address: &SurfaceAddress,
    ) -> anyhow::Result<()> {
        let tab = self.require_tab(commission, &address.tab).await?;
        if !declares_surface(&tab, &address.surface) {
            return Err(UnknownSurface.into());
        }
        Ok(())
    }

    /// Inserts one element behind [`require_address`](Self::require_address); the
    /// shared write path for a generic element, a Slot carrier, and a Seat carrier.
    /// `position` is assigned as max + 1 within `(tab, surface, band)`.
    async fn insert_element(&mut self, element: &NewElement) -> anyhow::Result<()> {
        self.require_address(&element.commission_id, &element.address)
            .await?;

        sql::add_element(
            &mut *self.conn,
            *element.id,
            *element.commission_id,
            *element.address.tab,
            element.address.surface.as_str(),
            element.element_type.as_str(),
            VisibilityMode::default().as_str(),
            element.band.as_str(),
            element.created_by.as_str(),
            element.created_at,
            // The only place an element's payload is unwrapped for the jsonb bind.
            element.payload.as_value(),
        )
        .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl CommissionWrites for PgCommissionWrites<'_> {
    /// Inserts the commission row, one `commission_tab` row per
    /// [`declared_tabs`], and the owner's `commission_participant` row, all in one
    /// transaction. Every tab is minted [`VisibilityMode::Total`].
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        sql::create_commission(
            &mut *self.conn,
            *commission.id,
            commission.title.as_str(),
            commission.owner_id.as_str(),
            commission.lifecycle_step.as_str(),
            commission.visibility.as_str(),
            commission.deadline,
            commission.maturity.map(|m| m.rating.as_str()),
            commission.maturity.map(|m| m.graphic),
            commission.created_at,
        )
        .await?;

        for tab in declared_tabs() {
            sql::create_tab(
                &mut *self.conn,
                *TabId::mint(),
                *commission.id,
                tab.as_str(),
                VisibilityMode::default().as_str(),
            )
            .await?;
        }

        sql::add_participant(
            &mut *self.conn,
            *commission.id,
            commission.owner_id.as_str(),
            commission.created_at,
        )
        .await?;
        Ok(())
    }

    /// Contributes one element into a declared surface behind
    /// `require_address`; position assigned by
    /// `max(position) + 1` within `(tab, surface, band)`.
    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()> {
        self.insert_element(element).await
    }

    /// Removes one element scoped to `commission_id` ([`ElementNotFound`] if
    /// absent/foreign), taking the same tab lock `require_tab`
    /// uses on add before deleting and renumbering the vacated group.
    async fn remove_element(
        &mut self,
        commission: &CommissionId,
        element: &ElementId,
    ) -> anyhow::Result<()> {
        let Some(group) =
            sql::remove_element_gate(&mut *self.conn, **element, **commission).await?
        else {
            return Err(ElementNotFound.into());
        };
        // Locks the same tab row the add path locks, serializing the two orderings.
        self.require_tab(commission, &TabId::new(group.tab_id))
            .await?;

        let deleted = sql::remove_element_delete(&mut *self.conn, **element, **commission).await?;
        if deleted != 1 {
            return Err(ElementNotFound.into());
        }

        sql::remove_element_renumber(
            &mut *self.conn,
            **commission,
            group.tab_id,
            &group.surface,
            &group.band,
        )
        .await?;
        Ok(())
    }

    /// Inserts a file entry's link row so it lands atomically with the caller's
    /// `file_added` changelog entry. Bytes already live in [`FileStore`](domain::ports::FileStore).
    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()> {
        sql::add_file(
            &mut *self.conn,
            *file.id,
            *file.commission_id,
            file.uploaded_by.as_str(),
            file.created_at,
        )
        .await?;
        Ok(())
    }

    /// Inserts one annotation row so it commits with the `markup_added` changelog
    /// entry. Re-serializes the already-validated
    /// [`MarkupShape`](domain::elements::commission::MarkupShape), never raw text.
    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {
        let shape = serde_json::to_value(&markup.markup.shape)?;
        sql::add_markup(
            &mut *self.conn,
            *markup.id,
            *markup.commission_id,
            *markup.file_id,
            markup.added_by.as_str(),
            &shape,
            markup.markup.text.as_deref(),
            markup.created_at,
        )
        .await?;
        Ok(())
    }

    /// Declares a batch of Slots: per Slot, an ordinary element (typed
    /// [`ElementType::slot`]) via `insert_element` plus its
    /// `commission_slot` satellite, all in one transaction.
    async fn declare_slots(&mut self, slots: &[NewSlot]) -> anyhow::Result<()> {
        for slot in slots {
            let carrier = NewElement::carrying(
                slot.id,
                slot.commission_id,
                slot.address.clone(),
                ElementType::slot(),
                slot.created_by.clone(),
                slot.created_at,
            );
            self.insert_element(&carrier).await?;

            sql::declare_slot_satellite(
                &mut *self.conn,
                *slot.id,
                *slot.commission_id,
                slot.title.as_str(),
                slot.notes.as_deref(),
            )
            .await?;
        }
        Ok(())
    }

    /// Inserts a pending seat invitation; a duplicate pending offer is silently
    /// dropped by the partial unique index. Returns the offer that now stands —
    /// freshly inserted, or the pre-existing pending one.
    async fn create_seat_invitation(
        &mut self,
        invitation: &SeatInvitation,
    ) -> anyhow::Result<SeatInvitation> {
        sql::create_seat_invitation(
            &mut *self.conn,
            *invitation.id,
            *invitation.commission,
            *invitation.seat,
            invitation.invited_user.as_str(),
            invitation.inviter.as_str(),
            invitation.state.as_str(),
            invitation.created_at,
            invitation.updated_at,
        )
        .await?;

        let standing = sql::find_pending_seat_invitation(
            &mut *self.conn,
            *invitation.commission,
            *invitation.seat,
            invitation.invited_user.as_str(),
            InvitationState::Pending.as_str(),
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "seat invitation for seat {} user {} is not pending immediately after issuing it",
                *invitation.seat,
                invitation.invited_user.as_str()
            )
        })?;
        to_seat_invitation(standing)
    }

    /// Flips a pending offer to revoked; matching no row (absent or already
    /// terminal) is a harmless no-op.
    async fn revoke_seat_invitation(&mut self, id: &SeatInvitationId) -> anyhow::Result<()> {
        sql::revoke_seat_invitation(
            &mut *self.conn,
            InvitationState::Revoked.as_str(),
            chrono::Utc::now(),
            **id,
            InvitationState::Pending.as_str(),
        )
        .await?;
        Ok(())
    }

    /// Whether the commission bears any fact, checked on the open transaction (no
    /// TOCTOU window). Constant `false` while [`COMMISSION_FACT_TABLES`] is empty.
    async fn commission_has_facts(&mut self, _id: &CommissionId) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Deletes the commission row on the open transaction. Child rows cascade via
    /// `ON DELETE CASCADE`; an absent commission is a no-op.
    async fn delete(&mut self, id: &CommissionId) -> anyhow::Result<()> {
        sql::delete(&mut *self.conn, **id).await?;
        Ok(())
    }

    /// Flips `commission.archived_at` via a conditional `UPDATE`; matches a row
    /// only on a real transition, so the return value IS the transition answer.
    async fn set_archived(
        &mut self,
        id: &CommissionId,
        archived_at: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        let affected = sql::set_archived(&mut *self.conn, **id, archived_at).await?;
        Ok(affected > 0)
    }

    /// Writes `maturity` and `graphic` together; the schema's both-or-neither
    /// CHECK makes a half-set pair unrepresentable.
    async fn set_maturity(&mut self, id: &CommissionId, maturity: Maturity) -> anyhow::Result<()> {
        sql::set_maturity(
            &mut *self.conn,
            **id,
            Some(maturity.rating.as_str()),
            Some(maturity.graphic),
        )
        .await?;
        Ok(())
    }

    /// Declares a seat: an ordinary element (typed [`ElementType::seat`]) via the
    /// shared address gate plus its `commission_seat` satellite. Every seat is
    /// born vacant.
    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()> {
        let carrier = NewElement::carrying(
            seat.id,
            seat.commission_id,
            seat.address.clone(),
            ElementType::seat(),
            seat.created_by.clone(),
            seat.created_at,
        );
        self.insert_element(&carrier).await?;

        sql::declare_seat_satellite(
            &mut *self.conn,
            *seat.id,
            *seat.commission_id,
            seat.kind.as_str(),
            seat.prompt.as_ref().map(|p| p.as_str()),
            seat.link.as_ref().map(|l| l.as_str()),
        )
        .await?;
        Ok(())
    }

    /// Repoints or clears `commission.linked_channel` via a conditional `UPDATE`
    /// (`IS DISTINCT FROM`), so the return value IS the changed answer.
    async fn set_linked_channel(
        &mut self,
        id: &CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        let affected =
            sql::set_linked_channel(&mut *self.conn, **id, channel.map(ChannelPointer::as_str))
                .await?;
        Ok(affected > 0)
    }

    /// Upserts the grantee's key; re-granting replaces the level. The `commission`
    /// FK assumes existence was already checked by the caller.
    async fn grant_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        sql::grant_view(
            &mut *self.conn,
            **commission,
            to_user.as_str(),
            &level.to_string(),
        )
        .await?;
        Ok(())
    }

    /// Hard-deletes the grantee's key; the return value is `true` only when a key
    /// existed (an idempotent no-op otherwise).
    async fn revoke_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
    ) -> anyhow::Result<bool> {
        let affected = sql::revoke_view(&mut *self.conn, **commission, to_user.as_str()).await?;
        Ok(affected > 0)
    }

    /// Repoints or clears `commission.direction_status`; an absent commission is a
    /// no-op.
    async fn set_direction_status(
        &mut self,
        id: &CommissionId,
        status: Option<DirectionStatus>,
    ) -> anyhow::Result<bool> {
        let affected =
            sql::set_direction_status(&mut *self.conn, **id, status.map(|s| s.as_str())).await?;
        Ok(affected > 0)
    }

    /// Repoints or clears `commission.deadline`; an absent commission is a no-op.
    async fn set_deadline(
        &mut self,
        id: &CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        let affected = sql::set_deadline(&mut *self.conn, **id, deadline).await?;
        Ok(affected > 0)
    }

    /// Repoints or clears `commission.deadline_status` (the manual `Delayed` flag
    /// only — `Late` is derived fresh at read, never persisted).
    async fn set_deadline_status(
        &mut self,
        id: &CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<bool> {
        let affected =
            sql::set_deadline_status(&mut *self.conn, **id, status.map(|s| s.as_str())).await?;
        Ok(affected > 0)
    }

    /// The sweeper's candidate scan, on the open transaction: deadline strictly
    /// before `now`, not already `late`, non-terminal lifecycle
    /// ([`LifecycleStep::is_terminal`]).
    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>> {
        let terminal: Vec<String> = LifecycleStep::ALL
            .iter()
            .filter(|step| step.is_terminal())
            .map(|step| step.as_str().to_owned())
            .collect();
        // Late is never persisted; dedup against the changelog's own last entry
        // since the latest deadline change, so each fresh miss is its own event.
        let rows = sql::lapsed_deadlines(&mut *self.conn, now, &terminal).await?;

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

/// Re-validates a stored `mode` token into [`VisibilityMode`]; the one gate
/// every composition read passes through.
fn to_mode(token: &str) -> anyhow::Result<VisibilityMode> {
    VisibilityMode::parse(token)
        .ok_or_else(|| anyhow::anyhow!("unknown visibility mode token {token:?}"))
}

/// Rebuilds a [`SeatInvitation`] from its row, re-validating the stored
/// `state` discriminant.
fn to_seat_invitation(row: sql::CommissionInvitationRow) -> anyhow::Result<SeatInvitation> {
    Ok(SeatInvitation {
        id: SeatInvitationId::new(row.id),
        commission: CommissionId::new(row.commission_id),
        seat: ElementId::new(row.seat_id),
        invited_user: UserId::new(Did::new(row.invited_user)),
        inviter: UserId::new(Did::new(row.inviter)),
        state: InvitationState::try_from(row.state)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// The [`CommissionStore`] read surface: pool-backed, no transaction tax.
/// Writes live on [`PgCommissionWrites`] via [`UnitOfWork`](domain::ports::UnitOfWork).
pub struct PgCommissionStore {
    pool: PgPool,
}

impl PgCommissionStore {
    /// Wraps a [`PgPool`] as a [`CommissionStore`].
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Fields shared by the `find`, `find_for_update`, and `list_owned_by` rows, so
/// [`to_commission`] re-validates them through one body. Every field is a raw
/// column value — nothing here is trusted.
struct CommissionFields {
    /// Re-validated into [`CommissionTitle`].
    title: String,
    /// The owning User's DID.
    owner_id: String,
    /// The stored [`LifecycleStep`] token.
    lifecycle: String,
    /// The stored [`Visibility`] token.
    visibility: String,
    /// The nullable deadline.
    deadline: Option<DateTimeUtc>,
    /// The stored [`MaturityRating`] token, or `None` while unrated (both-or-neither with `graphic`).
    maturity: Option<String>,
    /// The maturity posture's graphic flag; see `maturity`.
    graphic: Option<bool>,
    /// The stored [`DirectionStatus`] token, or `None`.
    direction_status: Option<String>,
    /// The stored [`DeadlineStatus`] token — the manual `Delayed` flag only.
    deadline_status: Option<String>,
    /// The stored [`ChannelPointer`], or `None`.
    linked_channel: Option<String>,
    /// When archived, or `None` while active.
    archived_at: Option<DateTimeUtc>,
    /// When created.
    created_at: DateTimeUtc,
}

impl From<sql::FindRow> for CommissionFields {
    /// From `queries/commission/find.sql`'s row (no `id` — the caller supplies it).
    fn from(row: sql::FindRow) -> Self {
        Self {
            title: row.title,
            owner_id: row.owner_id,
            lifecycle: row.lifecycle,
            visibility: row.visibility,
            deadline: row.deadline,
            maturity: row.maturity,
            graphic: row.graphic,
            direction_status: row.direction_status,
            deadline_status: row.deadline_status,
            linked_channel: row.linked_channel,
            archived_at: row.archived_at,
            created_at: row.created_at,
        }
    }
}

impl From<sql::FindForUpdateRow> for CommissionFields {
    /// From `queries/commission/find_for_update.sql`'s row (same columns as
    /// [`FindRow`](sql::FindRow) under a locking read).
    fn from(row: sql::FindForUpdateRow) -> Self {
        Self {
            title: row.title,
            owner_id: row.owner_id,
            lifecycle: row.lifecycle,
            visibility: row.visibility,
            deadline: row.deadline,
            maturity: row.maturity,
            graphic: row.graphic,
            direction_status: row.direction_status,
            deadline_status: row.deadline_status,
            linked_channel: row.linked_channel,
            archived_at: row.archived_at,
            created_at: row.created_at,
        }
    }
}

impl From<sql::CommissionRow> for CommissionFields {
    /// From `queries/commission/list_owned_by.sql`'s row; the `id` column is
    /// dropped here and lifted separately by the caller.
    fn from(row: sql::CommissionRow) -> Self {
        Self {
            title: row.title,
            owner_id: row.owner_id,
            lifecycle: row.lifecycle,
            visibility: row.visibility,
            deadline: row.deadline,
            maturity: row.maturity,
            graphic: row.graphic,
            direction_status: row.direction_status,
            deadline_status: row.deadline_status,
            linked_channel: row.linked_channel,
            archived_at: row.archived_at,
            created_at: row.created_at,
        }
    }
}

/// Rebuilds the [`Commission`] from its persisted fields, re-validating each
/// stored token through its domain gate; a value outside its vocabulary
/// surfaces as an `Err`, never a panic or silent default.
fn to_commission(id: CommissionId, fields: CommissionFields) -> anyhow::Result<Commission> {
    let maturity = match (fields.maturity, fields.graphic) {
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
    let lifecycle_step = LifecycleStep::try_from(fields.lifecycle.as_str())
        .map_err(|_| anyhow::anyhow!("unknown lifecycle token {:?}", fields.lifecycle))?;
    // The stored value is the manual `Delayed` flag only; derive the effective
    // status fresh here from the deadline, matching the sweep's math.
    let stored_deadline_status = fields
        .deadline_status
        .as_deref()
        .map(|token| {
            DeadlineStatus::try_from(token)
                .map_err(|_| anyhow::anyhow!("unknown deadline_status token {token:?}"))
        })
        .transpose()?;
    let deadline_status = derive_deadline_status(
        fields.deadline,
        &lifecycle_step,
        stored_deadline_status,
        chrono::Utc::now(),
    );
    let commission = Commission {
        id,
        title: CommissionTitle::try_from(fields.title)?,
        owner_id: UserId::new(Did::new(fields.owner_id)),
        lifecycle_step,
        visibility: Visibility::try_from(fields.visibility.as_str())
            .map_err(|_| anyhow::anyhow!("unknown visibility token {:?}", fields.visibility))?,
        deadline: fields.deadline,
        maturity,
        direction_status: fields
            .direction_status
            .as_deref()
            .map(|token| {
                DirectionStatus::try_from(token)
                    .map_err(|_| anyhow::anyhow!("unknown direction_status token {token:?}"))
            })
            .transpose()?,
        deadline_status,
        linked_channel: fields
            .linked_channel
            .map(ChannelPointer::try_from)
            .transpose()?,
        archived_at: fields.archived_at,
        created_at: fields.created_at,
    };
    Ok(commission)
}

/// The read half of a commission unit of work, executed on the unit's own
/// connection so reads see its uncommitted writes. Vended by `uow.commissions()`.
#[async_trait::async_trait]
impl CommissionReads for PgCommissionWrites<'_> {
    /// [`CommissionStore::find`] on the unit's connection.
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        let Some(row) = sql::find(&mut *self.conn, **id).await? else {
            return Ok(None);
        };
        to_commission(*id, row.into()).map(Some)
    }

    /// [`find`](Self::find) with `FOR NO KEY UPDATE`; concurrent writers of this
    /// commission wait, but inserts of child rows stay free.
    async fn find_for_update(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        let Some(row) = sql::find_for_update(&mut *self.conn, **id).await? else {
            return Ok(None);
        };
        to_commission(*id, row.into()).map(Some)
    }

    /// [`CommissionStore::is_participant`] on the unit's connection.
    async fn is_participant(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        Ok(sql::is_participant(&mut *self.conn, **commission, user.as_str()).await?)
    }

    /// The tab's row, locked for the rest of the unit — the same statement
    /// `require_tab` uses.
    async fn tab_for_update(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<Option<TabRow>> {
        self.locate_tab(commission, tab).await
    }
}

#[async_trait::async_trait]
impl CommissionStore for PgCommissionStore {
    /// Rebuilds the [`Commission`] via `to_commission`; `None` if absent.
    async fn find(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        let Some(row) = sql::find(&self.pool, **id).await? else {
            return Ok(None);
        };
        to_commission(*id, row.into()).map(Some)
    }

    /// Which column of `workflow_id` currently holds this commission, if any.
    /// Scoped by board — a commission sits in at most one column per board but
    /// on many boards. Delegates to [`PgColumnStore::find_column`].
    async fn current_column_of_workflow(
        &self,
        commission: &CommissionId,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<Column>> {
        PgColumnStore::new(self.pool.clone())
            .find_column(workflow_id, commission)
            .await
    }

    /// This commission's index within `column_id`, or `None` if absent. A stored
    /// value that can't be a `u8` surfaces as an `Err`.
    async fn current_position_in_column(
        &self,
        commission: &CommissionId,
        column_id: &ColumnId,
    ) -> anyhow::Result<Option<u8>> {
        let Some(position) =
            crate::queries::column::position_in_column(&self.pool, **column_id, **commission)
                .await?
        else {
            return Ok(None);
        };

        u8::try_from(position)
            .map_err(|_| anyhow::anyhow!("stored card position {position} is out of range"))
            .map(Some)
    }

    /// The [`GrantLevel`] `user` holds on `commission`, or `None`.
    /// The stored token is re-validated through [`GrantLevel`]'s `FromStr`.
    async fn view_grant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<GrantLevel>> {
        let Some(level) = sql::view_grant(&self.pool, **commission, user.as_str()).await? else {
            return Ok(None);
        };
        level
            .parse::<GrantLevel>()
            .map_err(|_| anyhow::anyhow!("unknown grant level token {level:?}"))
            .map(Some)
    }

    /// Loads the commission's whole composition: three indexed queries (tabs,
    /// widened surface modes, elements), each re-validated through its domain
    /// gate. `None` only when the commission has no tabs at all; an empty
    /// `elements` list is the ordinary state of a fresh one.
    async fn load_composition(
        &self,
        id: &CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>> {
        let tab_rows = sql::load_tabs(&self.pool, **id).await?;
        if tab_rows.is_empty() {
            return Ok(None);
        }
        let tabs = tab_rows
            .into_iter()
            .map(|row| {
                let mode = to_mode(&row.mode)?;
                Ok(TabRow {
                    id: TabId::new(row.id),
                    tab: row.tab.try_into()?,
                    mode,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let surface_modes = sql::load_surface_modes(&self.pool, **id)
            .await?
            .into_iter()
            .map(|row| Ok((SurfaceName::try_from(row.surface)?, to_mode(&row.mode)?)))
            .collect::<anyhow::Result<_>>()?;

        let elements = sql::load_elements(&self.pool, **id)
            .await?
            .into_iter()
            .map(|row| {
                let mode = to_mode(&row.mode)?;
                let address = SurfaceAddress::new(
                    TabId::new(row.tab_id),
                    SurfaceName::try_from(row.surface)?,
                );
                Ok(ElementRow {
                    id: ElementId::new(row.id),
                    address,
                    element_type: row.element_type.try_into()?,
                    mode,
                    band: row.band.try_into()?,
                    position: row.position,
                    created_by: UserId::new(Did::new(row.created_by)),
                    created_at: row.created_at,
                    // Wrapped on the way out of the row, so nothing downstream
                    // ever holds the raw, serializable value.
                    payload: ElementPayload::from(row.payload),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let composition = CommissionComposition {
            tabs,
            surface_modes,
            elements,
        };
        Ok(Some(composition))
    }

    /// One `EXISTS` over the persisted `commission_participant` membership record
    /// — never a computed owner-∪-seated union. Unaffected by placement or view
    /// grants.
    async fn is_participant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        Ok(sql::is_participant(&self.pool, **commission, user.as_str()).await?)
    }

    /// The file-entry link `key` names within `commission` — scoped by both id and
    /// commission_id, so a foreign key answers `None`. Bytes live behind
    /// [`FileStore`](domain::ports::FileStore).
    async fn find_file(
        &self,
        commission: &CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>> {
        let row = sql::find_file(&self.pool, *key, **commission).await?;

        Ok(row.map(|row| CommissionFile {
            id: FileKey::new(row.id),
            commission_id: CommissionId::new(row.commission_id),
            uploaded_by: UserId::new(Did::new(row.uploaded_by)),
            created_at: row.created_at,
        }))
    }

    /// One file entry's annotations in draw order, scoped by `commission`. Each
    /// row's `shape` is re-validated through
    /// [`MarkupShape`](domain::elements::commission::MarkupShape)'s deserializer.
    async fn markups_for_file(
        &self,
        commission: &CommissionId,
        file: FileKey,
    ) -> anyhow::Result<Vec<CommissionMarkup>> {
        let rows = sql::markups_for_file(&self.pool, **commission, *file).await?;

        rows.into_iter()
            .map(|row| {
                let markup = Markup {
                    shape: serde_json::from_value(row.shape)?,
                    text: row.text,
                };

                Ok(CommissionMarkup {
                    id: MarkupKey::new(row.id),
                    commission_id: CommissionId::new(row.commission_id),
                    file_id: FileKey::new(row.file_id),
                    added_by: UserId::new(Did::new(row.added_by)),
                    markup,
                    created_at: row.created_at,
                })
            })
            .collect()
    }

    /// The lone `state = 'pending'` offer for `(seat, invited_user)`, or `None`.
    /// Accepted/revoked invitations never match.
    async fn find_pending_seat_invitation(
        &self,
        commission: &CommissionId,
        seat: &ElementId,
        user: &UserId,
    ) -> anyhow::Result<Option<SeatInvitation>> {
        sql::find_pending_seat_invitation(
            &self.pool,
            **commission,
            **seat,
            user.as_str(),
            InvitationState::Pending.as_str(),
        )
        .await?
        .map(to_seat_invitation)
        .transpose()
    }

    /// The commission's seat satellites in declaration order (element ids are
    /// UUIDv7). Each row re-validated through its domain gate.
    async fn seats(&self, commission: &CommissionId) -> anyhow::Result<Vec<Seat>> {
        let rows = sql::seats(&self.pool, **commission).await?;
        rows.into_iter()
            .map(|row| {
                Ok(Seat {
                    id: ElementId::new(row.id),
                    kind: SeatKind::try_from(row.kind)?,
                    prompt: row.prompt.map(SeatPrompt::try_from).transpose()?,
                    link: row.link.map(SeatLink::try_from).transpose()?,
                    occupant: row.occupant.map(|did| UserId::new(Did::new(did))),
                })
            })
            .collect()
    }

    /// Active commissions (`archived_at IS NULL`) owned by `owner`, ordered by
    /// id, each re-validated through `to_commission`.
    async fn list_owned_by(&self, owner: &UserId) -> anyhow::Result<Vec<Commission>> {
        sql::list_owned_by(&self.pool, owner.as_str())
            .await?
            .into_iter()
            .map(|row| {
                let id = CommissionId::new(row.id);
                to_commission(id, row.into())
            })
            .collect()
    }
}
