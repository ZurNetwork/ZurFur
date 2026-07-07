//! In-process fakes of the commission seam (ZMVP-65/87): the stored shapes, the
//! [`CommissionWrites`]/[`ChangelogWrites`] write views (staged by the
//! [`MemUnitOfWork`](crate::MemUnitOfWork), so they commit-or-discard with the
//! unit), the pool-shaped [`MemCommissionStore`]/[`MemChangelogStore`] read
//! stores, and the commission seed/inspect helpers on [`MemBackend`]. Split out
//! of the backend file along the domain seam (the `public_records` precedent) so
//! later commission tickets extend this module instead of one shared hotspot.

use async_trait::async_trait;
use domain::elements::{
    commission::{
        ChangelogEntry, ChangelogEntryKind, ChannelPointer, Commission, CommissionId,
        CommissionTitle, LifecycleStep, NewChangelogEntry, Visibility,
    },
    user::UserId,
};
use domain::ports::{ChangelogStore, ChangelogWrites, CommissionStore, CommissionWrites};
use serde_json::Value;

use crate::MemBackend;

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
    /// The external linked-channel pointer, or `None` while none is declared
    /// (ZMVP-87 AC3) — the mem mirror of the pg `linked_channel` column.
    pub(crate) linked_channel: Option<ChannelPointer>,
    /// When the commission was created.
    pub(crate) created_at: domain::datetime::DateTimeUtc,
}

impl StoredCommission {
    /// Rebuild the aggregate from its stored parts (the commission analogue of
    /// how `find` rebuilds an `Account`).
    fn rebuild(&self, id: CommissionId) -> Commission {
        Commission {
            id,
            title: self.title.clone(),
            owner_id: self.owner_id,
            lifecycle_step: self.lifecycle_step.clone(),
            visibility: self.visibility.clone(),
            deadline: self.deadline,
            linked_channel: self.linked_channel.clone(),
            created_at: self.created_at,
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

/// In-memory [`CommissionWrites`] view: commission writes land on the shared
/// state. Vended by [`MemUnitOfWork::commissions`](crate::MemUnitOfWork), where
/// the [`MemBackend`] it wraps is the unit's *staging* snapshot — so a write
/// reaches the shared store only on commit (drop = rollback), exactly like
/// `MemAccountWrites`.
pub struct MemCommissionWrites(pub(crate) MemBackend);

#[async_trait]
impl CommissionWrites for MemCommissionWrites {
    /// Insert the freshly created commission, keyed by its id — the in-memory
    /// mirror of the pg adapter's single `INSERT INTO commission`. The pg `id` is a
    /// PRIMARY KEY, so a duplicate would raise a violation there; the fake does not
    /// model that (a plain `insert`, the same as `MemAccountWrites::create` does
    /// for its own account id), because commission ids are freshly-minted UUIDv7 —
    /// a collision is unreachable by construction, never a case a test can reach.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
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
                linked_channel: commission.linked_channel.clone(),
                created_at: commission.created_at,
            },
        );
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

    /// The **owner arm** of participant-hood — the owner IS a Participant
    /// without holding a Seat (DESIGN/Commission); ZMVP-79 adds the seated arm.
    /// An unknown commission has no participants, so it answers `false`.
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool> {
        let commissions = self
            .0
            .commissions
            .lock()
            .expect("MemBackend commissions mutex poisoned");
        Ok(commissions
            .get(&commission)
            .is_some_and(|stored| stored.owner_id == user))
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
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::*;

    fn user_id() -> UserId {
        UserId::new(uuid::Uuid::now_v7())
    }

    fn commission(title: &str, owner: UserId) -> Commission {
        Commission::create(
            CommissionTitle::try_new(title).unwrap(),
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

        let pointer = ChannelPointer::try_new("@artist on Telegram").unwrap();
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
}
