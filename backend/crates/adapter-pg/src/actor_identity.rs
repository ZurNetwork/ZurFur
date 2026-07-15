//! PostgreSQL adapter for the actor super-table (ZMVP-122, DD `34013187`).
//!
//! Slices 1–4: existence, kind, the optional DID, and liveness state —
//! `create` for DID-less actors (Characters), the race-safe `intern` upsert
//! for DID-bearing ones, reads by id and by DID. The module deliberately
//! exposes **no delete**: identity rows are immortal — liveness is a state
//! (whose transitions are ZMVP-125), never a removal.

use anyhow::Context;
use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind, ActorState};
use domain::elements::did::Did;
use domain::ports::{ActorIdentityStore, ActorIdentityWrites};
use sqlx::{PgConnection, PgPool};

use crate::queries::actor_identity as sql;
use crate::queries::actor_identity::ActorIdentityRow;

/// Rebuild the domain row from its stored columns. The schema's CHECK admits
/// only the known kind spellings, so a parse failure here is a corrupted row —
/// surfaced loudly, never a guess.
fn rebuild(row: ActorIdentityRow) -> anyhow::Result<ActorIdentity> {
    let kind = ActorKind::try_from(row.kind.as_str())
        .with_context(|| format!("actor_identity {}: corrupted kind", row.id))?;
    let state = ActorState::try_from(row.state.as_str())
        .with_context(|| format!("actor_identity {}: corrupted state", row.id))?;
    Ok(ActorIdentity {
        id: ActorIdentityId::new(row.id),
        kind,
        did: row.did.map(Did::new),
        state,
        handle: row.handle,
        first_seen: row.first_seen,
    })
}

/// The actor-super-table write view over one open transaction — vended only by
/// [`PgUnitOfWork::actor_identities`](crate::PgUnitOfWork), so a write cannot
/// skip the transaction (DD `24150017`).
pub struct PgActorIdentityWrites<'a> {
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait]
impl ActorIdentityWrites for PgActorIdentityWrites<'_> {
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()> {
        // The DID-less path by contract: intern owns DID-bearing rows.
        anyhow::ensure!(
            identity.did.is_none(),
            "create is the DID-less path; intern DID-bearing actors instead"
        );
        // Born active by invariant (DD 34013187 decisions 3/5): transitions
        // are ZMVP-125's machinery and never pass through creation.
        anyhow::ensure!(
            identity.state == ActorState::Active,
            "create only persists born-active identities"
        );
        // Born uncached by invariant: the handle is a display cache filled
        // via cache_handle, never supplied at creation.
        anyhow::ensure!(
            identity.handle.is_none(),
            "create only persists born-uncached identities; fill via cache_handle"
        );
        sql::create(
            &mut *self.conn,
            *identity.id,
            identity.kind.as_str(),
            identity.state.as_str(),
            identity.first_seen,
        )
        .await?;
        Ok(())
    }

    async fn intern(
        &mut self,
        did: &Did,
        kind: ActorKind,
        now: DateTimeUtc,
    ) -> anyhow::Result<ActorIdentity> {
        // A freshly minted candidate loses to an existing DID at the unique
        // index; RETURNING yields whichever row survived (DD decision 6) —
        // including its ORIGINAL first_seen (the upsert never restamps).
        let candidate = uuid::Uuid::now_v7();
        let row = sql::intern(
            &mut *self.conn,
            candidate,
            kind.as_str(),
            did.as_str(),
            ActorState::Active.as_str(),
            now,
        )
        .await?;
        rebuild(row)
    }

    async fn cache_handle(
        &mut self,
        id: ActorIdentityId,
        handle: Option<&str>,
    ) -> anyhow::Result<()> {
        let affected = sql::cache_handle(&mut *self.conn, *id, handle).await?;
        anyhow::ensure!(affected == 1, "actor identity not found: {}", *id);
        Ok(())
    }
}

/// The pool-backed, non-transactional read store for the actor super-table.
pub struct PgActorIdentityStore {
    pool: PgPool,
}

impl PgActorIdentityStore {
    /// Wraps a [`PgPool`] (clones the `Arc` handle, so the caller keeps its own).
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ActorIdentityStore for PgActorIdentityStore {
    async fn find(&self, id: ActorIdentityId) -> anyhow::Result<Option<ActorIdentity>> {
        let row = sql::find(&self.pool, *id).await?;
        row.map(rebuild).transpose()
    }

    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<ActorIdentity>> {
        let row = sql::find_by_did(&self.pool, did.as_str()).await?;
        row.map(rebuild).transpose()
    }
}
