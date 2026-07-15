//! PostgreSQL adapter for the actor super-table (ZMVP-122, DD `34013187`).
//!
//! Slices 1–2: existence + kind — create and find by the app-private id. The
//! module deliberately exposes **no delete**: identity rows are immortal, so an
//! FK into `actor_identity` can never break.

use anyhow::Context;
use async_trait::async_trait;
use domain::elements::actor_identity::{ActorIdentity, ActorIdentityId, ActorKind};
use domain::ports::{ActorIdentityStore, ActorIdentityWrites};
use sqlx::{PgConnection, PgPool};

use crate::queries::actor_identity as sql;

/// The actor-super-table write view over one open transaction — vended only by
/// [`PgUnitOfWork::actor_identities`](crate::PgUnitOfWork), so a write cannot
/// skip the transaction (DD `24150017`).
pub struct PgActorIdentityWrites<'a> {
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait]
impl ActorIdentityWrites for PgActorIdentityWrites<'_> {
    async fn create(&mut self, identity: &ActorIdentity) -> anyhow::Result<()> {
        sql::create(&mut *self.conn, *identity.id, identity.kind.as_str()).await?;
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
        row.map(|row| {
            // The schema's CHECK admits only the known spellings, so a parse
            // failure here is a corrupted row — surfaced loudly, never a guess.
            let kind = ActorKind::try_from(row.kind.as_str())
                .with_context(|| format!("actor_identity {}: corrupted kind", row.id))?;
            Ok(ActorIdentity {
                id: ActorIdentityId::new(row.id),
                kind,
            })
        })
        .transpose()
    }
}
