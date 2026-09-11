//! [`UserStore`] (reads) and [`UserWrites`] (recognition) over PostgreSQL:
//! Zurfur's record of recognized visitors in the `users` table, keyed by their
//! sovereign `did`. Reads are pool-backed; recognition (`provision`) is a write
//! and so is reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.users()`). See ZMVP-9, DESIGN/User, and DD `24150017`.
//!
//! Since the actor re-key (DD `57081857`) `users.id` **is** the visitor's DID:
//! there is no surrogate key and nothing to resolve, so the reads are plain
//! lookups. The table stays a projection of the actor super-table (DD
//! `34013187`) — `(id, kind='user')` is a composite FK into
//! `actor_identity (did, kind)` — so recognition remains a two-step write hidden
//! behind this one `provision` helper: `intern` the DID (the race-safe
//! one-DID-one-actor upsert), then land the `users` projection under it.
//!
//! The SQL lives in `queries/user/` and `queries/actor_identity/`; the typed
//! functions and row shapes are generated against the migrated schema (see
//! [`crate::queries`]).

use domain::ports::DidBelongsToAnotherActor;
use domain::{
    elements::{
        actor_identity::{ActorKind, ActorState},
        did::Did,
        user::{User, UserId},
    },
    ports::{UserStore, UserWrites},
};
use sqlx::{PgConnection, PgPool};

use crate::queries::actor_identity as actor_sql;
use crate::queries::user as sql;

/// PostgreSQL read store for recognized visitors (the [`UserStore`] read surface).
/// Resolves a User by id or DID off the pool; recognition (the write) lives on
/// [`PgUserWrites`]. See DESIGN/User.
pub struct PgUserStore {
    pool: PgPool,
}

impl PgUserStore {
    /// Wraps a [`PgPool`] as a [`UserStore`]. Clones the pool handle (an `Arc`),
    /// leaving the caller's intact.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// PostgreSQL write view for recognizing visitors (the [`UserWrites`] surface),
/// over an open transaction. Holds only a borrowed `&mut PgConnection` — the
/// transaction owned by the [`PgUnitOfWork`](crate::PgUnitOfWork) — so recognition
/// cannot skip a transaction. Built by `uow.users()`.
pub struct PgUserWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork);
    /// `provision` executes on `&mut *self.conn`. No pool here, by construction.
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl UserStore for PgUserStore {
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        Ok(sql::find(&self.pool, id.as_str()).await?.map(|row| User {
            id: UserId::new(Did::new(row.id)),
            created_at: row.created_at,
        }))
    }

    /// Read-only lookup by DID — no INSERT, so an unknown DID resolves to `None`
    /// rather than recognizing a new visitor (the no-mint counterpart to
    /// [`UserWrites::provision`]). Addresses the same column as
    /// [`find`](UserStore::find) now that the DID is the key; the two stay apart
    /// because they are two port methods with two argument types.
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        Ok(sql::find_by_did(&self.pool, did.as_str())
            .await?
            .map(|row| User {
                id: UserId::new(Did::new(row.id)),
                created_at: row.created_at,
            }))
    }
}

#[async_trait::async_trait]
impl UserWrites for PgUserWrites<'_> {
    /// Recognize a DID as a two-step write in one unit (ZMVP-123): `intern` the DID
    /// into the actor super-table, then land the `users` projection under that DID.
    /// Both steps ride the open transaction, so a half-recognized visitor can never
    /// be observed and the composite FK makes the reverse order unrepresentable.
    /// Idempotent and race-safe: the `intern` upsert is the arbiter of
    /// one-DID-one-actor, and the projection upsert on the DID key hands back the
    /// *existing* row's `created_at`.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User> {
        let now = chrono::Utc::now();

        // Step 1 — intern the DID (race-safe, idempotent one-DID-one-actor upsert, DD
        // 34013187 decision 6). A brand-new DID takes the candidate id; a repeat sign-in
        // collides on the unique `did` and RETURNING hands back the existing identity,
        // whose id is what the projection is keyed by.
        let candidate_id = uuid::Uuid::now_v7();
        let identity = actor_sql::intern(
            &mut *self.conn,
            candidate_id,
            ActorKind::User.as_str(),
            // The nullable `did` column widens the bind to Option; provision is a
            // DID-bearing path, so it is always present.
            Some(did.as_str()),
            ActorState::Active.as_str(),
            now,
        )
        .await?;
        // A DID already interned under another kind (e.g. an account's) would
        // otherwise surface as a bewildering composite-FK failure at step 2 —
        // fail here with the actual story instead (the account path's
        // `ensure!` twin; the FK remains the DB-level backstop).
        let identity_is_a_user = identity.kind == ActorKind::User.as_str();
        if !identity_is_a_user {
            let conflict = DidBelongsToAnotherActor {
                existing_kind: identity.kind,
            };
            return Err(anyhow::Error::new(conflict));
        }

        // Step 2 — the `users` projection row, keyed by the DID itself. Idempotent:
        // a repeat sign-in hits the row that already exists, so RETURNING yields its
        // ORIGINAL created_at.
        let row = sql::provision(&mut *self.conn, did.as_str(), now).await?;

        Ok(User {
            id: UserId::new(Did::new(row.id)),
            created_at: row.created_at,
        })
    }
}
