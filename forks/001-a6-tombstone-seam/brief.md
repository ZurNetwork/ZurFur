# Proposal brief — fork 001: the did:plc tombstone seam on account hard-delete

Zurfur is an AT Protocol-native art-commission platform in Rust, built ports-and-adapters:
`domain` (entities + ports named by role) → `application` (use cases, one `pub async fn` each, owning
the private-store transaction) → adapters (`adapter-pg` = private store, PostgreSQL; `adapter-atproto` =
public boundary: PLC directory, PDS, OAuth) → `composition` (wires the live adapters). Rule of the house:
**no cross-store transaction** — anything touching both boundaries is a dual write, run as separate
retryable steps, never one unit of work.

## The fork (one question)

When an Account is hard-deleted, its `did:plc` must be tombstoned on the PLC directory (a public,
external service) after the private-store delete commits. **Where does the reliability seam between the
private store and the PLC port live, and what shape does it take?**

## Framing

The did:plc is not a local fact — it lives on plc.directory, an external service with its own state,
availability and recovery window. In ports-and-adapters terms it is a **driven port on the public
boundary** (`DidMinter` today; a new, still-unimplemented `DidOperations` trait was just added),
`adapter-atproto` its adapter — on par with `PublicRecords` for the PDS. The fork is therefore *where
the reliability seam lives* between the private store and that port: in the use case, in a dedicated
application job, in the port's contract, or inside the adapter. Any option is a contract between two
services (private store ↔ PLC) and inherits the usual concerns: idempotency, retry, partial failure,
observability of the pending state.

## Options (unranked, neutrally worded)

- **A. Reorder in place** — the use case commits the private unit of work, then calls the tombstone
  port; a failure is logged, not retried.
- **B. Transactional outbox** — the use case enqueues a tombstone row (own table) in the same unit of
  work as the hard delete; a separate sweep-style application job performs it after commit, recording
  done / failed-with-attempt-count per attempt.
- **C. Marker on the account row** — a `pending_tombstone`-style column (or submission-status column)
  on the account row, which per DD 57081857 D6 becomes immortal (`tombstoned_at`, never deleted); a
  drain job scans for it. Depends on that re-key (B5) landing.
- **D. Compile-enforced ordering** — `commit()` returns a receipt value that the tombstone call requires
  as a parameter, so an in-transaction tombstone cannot compile; no queue, no retry.
- **E. Adapter-owned durability** — the port call itself is durable: `adapter-atproto` persists the
  intent (in Postgres, as it already does for OAuth state) and retries internally; the use case calls it
  after commit and treats `Ok` as "accepted", not "done".

Options combine: D is a guard that can sit under A, B, C or E. B, C and E differ in *where* the retry
state lives (application-owned table / account row / adapter-owned table).

## Enumerated factual claims

Verify each against the context block below and your own search where the claim is external.

1. `application/src/account/delete.rs:51-66` awaits `did_minter.tombstone(&account_id)` **between**
   `database.begin()` and `uow.commit()`; a tombstone failure is caught and logged at `warn!`, never
   surfaced or retried.
2. DD 23003138 ("Account Deletion, Tombstoning & Handle Reuse") D3 rules the `plc_tombstone` a
   **separate retryable step, never inside the private transaction**.
3. No outbox, job queue, or retry table exists anywhere under `backend/crates/` (grep for "outbox" finds
   only nothing outside this fork's own doc comments).
4. `require_live_account` at the top of `delete` makes a repeated delete call return not-found once the
   row is gone, so the *caller* cannot retry the tombstone by calling delete again.
5. A `did:plc` tombstone is reversible for roughly 72 hours by a higher-priority rotation key, after
   which it is permanent (did:plc spec).
6. `hard_delete` (the private-store port) deliberately leaves the custody keys (`account_keys`) in
   place so the recovery window can still be used.
7. DD 57081857 D6 makes the account row immortal: hard-delete sets `tombstoned_at` and detaches facts,
   never deletes the row. This is decided but **not yet built** (the current `hard_delete` still deletes
   rows).
8. `sweep_deadlines` (`application/src/commission.rs`) is the only caller of `application::transaction`
   and the existing template for an actor-less, `now`-injected application job; its wall-clock timer and
   advisory-lock leader election live in the `api` driver.
9. The new `DidOperations` trait has no adapter implementation and no caller; `DidMinter::tombstone`
   is implemented by `RealDidMinter` in `adapter-atproto`.
10. `RealDidMinter::tombstone` is **not** idempotent after a full success: it chains onto
    `op_log.latest_cid(did)`, so a second call after a logged tombstone signs a tombstone-of-a-tombstone,
    which the PLC directory rejects. The only safe replay is the submit-succeeded-but-log-append-failed
    case (same `prev`, deterministic signature, same CID). *(Corrected by the orchestrator's pre-check
    from "safe to re-submit"; verify the correction.)*
11. `facts::exist` is stubbed to `has_facts: false`, so at HEAD **every** account delete takes the hard
    branch (a separate defect, A5, that must land before or with this fix).
12. The PLC directory is an external service; a tombstone submission is a remote call that can fail,
    succeed without acknowledgement, or be replayed.
13. `adapter-atproto` already owns durable Postgres state (`AtprotoAuthStore`: OAuth sessions and
    auth requests, sealed at rest), but the DDL for those tables lives in `adapter-pg`'s migrations —
    adapter-pg owns all DDL. *(Corrected by the orchestrator's pre-check from "holds no persistent
    state"; verify the correction.)*
14. In v1 the directory submission is a **gated no-op** (`NoopPlcDirectory`): the tombstone signs and
    logs locally but registers nowhere until real submission is switched on by config. DD 23003138's
    Open table already names, as a pre-launch follow-up: "an outbox/retry for a failed tombstone (the
    handler currently logs & swallows), a submission-status column, and a purge of the custody keys once
    the recovery window closes."

## Doctrine floor (always applies)

Anti-domination (exit and voice preserved by construction); the non-toxic path outranks optimization —
any mechanic failing "does this hurt anyone's sanity?" is disqualified, not discounted; Zurfur never
holds funds absent an explicit ruling; consensus is not evidence. Security and viability over speed:
soundness is the goal, never velocity. Make unsoundness unreachable, not caught.

## Context block (verbatim excerpts at tree c6c7514a + uncommitted working copy)

### `application/src/account/delete.rs` (whole file)

```rust
use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::account::{AccountError, AccountResult, Accounts, facts, require_live_account};

pub struct Command {
    pub actor_id: UserId,
    pub account_id: AccountId,
}
pub enum DeleteOutcome {
    Soft,
    Hard,
}
// ... Display impl elided ...
pub struct Output {
    pub outcome: DeleteOutcome,
}

impl<'a> Accounts<'a> {
    pub async fn delete(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command { account_id, actor_id } = cmd;

        // Existence before standing, as `change_handle` already does: without
        // the first check a deletion aimed at an account that does not exist
        // came back `403 forbidden` — a refusal implying there is something
        // there to be refused.
        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner))
            .ok_or(AccountError::IncorrectRole)?;
        // FIXME: Add facts in the correct way here
        let existing_facts = facts::exist::Query { account_id: account_id.clone() };

        let mut uow = ports.database.begin().await?;
        let outcome = if self.facts().exist(existing_facts).await?.has_facts {
            uow.accounts().soft_delete(&account_id).await?;
            DeleteOutcome::Soft
        } else {
            uow.accounts().hard_delete(&account_id).await?;
            if let Err(err) = ports.did_minter.tombstone(&account_id).await {
                tracing::warn!(
                    error = ?err,
                    did = %account_id.as_str(),
                    "did:plc tombstone failed after hard delete; the PLC recovery window still applies"
                )
            };
            DeleteOutcome::Hard
        };
        uow.commit().await?;
        Ok(Output { outcome })
    }
}
```

### `application/src/account/change_handle.rs` (the ordering the sibling uses)

```rust
        // ... existence, role, rate-limit, handle-taken, quarantine checks ...
        ports.did_minter.update_handle(&account.id, &handle).await?;   // public step FIRST, outside any uow

        let mut uow = ports.database.begin().await?;
        uow.accounts()
            .change_handle(&account_id, &account.handle, &handle, now)
            .await?;
        uow.commit().await?;
```

### `application/src/transaction.rs` — the one orchestrator (used once, by `sweep_deadlines`)

```rust
/// ... opens a UnitOfWork via Database::begin, hands it to `f`, then
/// **commits on `Ok`, rolls back on `Err`**: the closure body *is* the
/// transaction boundary, so a commit can never be forgotten. Strictly
/// intra-Postgres; never a cross-store dual write.
pub async fn transaction<T, F>(db: &dyn Database, f: F) -> anyhow::Result<T>
where F: for<'a> UnitOfWorkFn<'a, T> + Send, T: Send,
{
    let mut uow = db.begin().await?;
    match f(&mut *uow).await {
        Ok(value) => { uow.commit().await?; Ok(value) }
        Err(err) => { let _ = uow.rollback().await; Err(err) }
    }
}
```

### `application/src/commission.rs` — the actor-less job template

```rust
/// Run **one** deadline sweep as of `now`.
/// A use case with no actor: `now` is injected (never read from a wall clock
/// here) and the whole pass is **one unit of work** ... The wall-clock timer and
/// the advisory-lock leader election that drive it live in `api` — a driver concern.
pub async fn sweep_deadlines(database: &dyn Database, now: DateTimeUtc)
    -> Result<SweepResult, CommissionError>
{
    let marked_late = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let lapsed = uow.commissions().lapsed_deadlines(now).await?;
        for lapse in &lapsed { /* append a system changelog entry */ }
        Ok(lapsed.len())
    }).await?;
    Ok(SweepResult { marked_late })
}
```

### `domain/src/ports/mod.rs` — `AccountWrites::hard_delete` doc (excerpt)

```text
Hard-delete an empty account: remove its account_invitations, account_members, and accounts rows in
one unit of work (ZMVP-34, DD 23003138). Removing the accounts row frees the handle for reuse ...
The custody keys (account_keys) are left in place so the native ~72h did:plc tombstone recovery window
can still reverse the deletion. **Tombstoning the DID is a separate retryable atproto step**, never
part of this private transaction (no cross-store dual write — the mint path's mirror). Idempotent:
hard-deleting an absent account is a no-op.
```

### `domain/src/ports/mod.rs` — `DidMinter::tombstone` doc (excerpt) and the new `DidOperations`

```text
DidMinter::tombstone — Signs a plc_tombstone with the account's custodied operational rotation key,
chaining onto the DID's most recent operation (its prev, read from the PlcOperationLog), records it in
the log, and submits it to the directory ... A public-boundary step, run **after** the private
hard-delete has committed — never inside that transaction. Fallible and retryable: a directory failure
leaves the freed handle and removed rows as they are; the tombstone is re-submittable. In v1 the
directory is a gated no-op, so this signs and logs but registers nowhere. The stub/mem minters are no-ops.
```

```rust
/// The public-boundary lifecycle operations an already-minted DID runs for
/// itself. Owned by the `Did`: call sites go through `Did::tombstone` /
/// `Did::update_handle`, never this port directly. Every call is its own
/// retryable step, reached only **after** the owning private-store transaction
/// has committed — never inside one (DD 23003138 D3; no cross-store dual write).
///
/// Interface only for now (Engineer ruling 2026-09-10): no adapter implements
/// it yet, and whether tombstone/update_handle migrate here off DidMinter is a
/// follow-up ruling.
#[async_trait]
pub trait DidOperations: Send + Sync {
    async fn tombstone(&self, did: &Did) -> anyhow::Result<()>;
    async fn update_handle(&self, did: &Did, handle: &Handle) -> anyhow::Result<()>;
}

// domain/src/elements/did.rs (uncommitted): forwarding methods on the value type
impl Did {
    pub async fn tombstone(&self, operations: &dyn DidOperations) -> anyhow::Result<()> {
        operations.tombstone(self).await
    }
    pub async fn update_handle(&self, handle: &Handle, operations: &dyn DidOperations) -> anyhow::Result<()> {
        operations.update_handle(self, handle).await
    }
}
```

### `adapter-atproto/src/did_minter.rs` — `RealDidMinter::tombstone` (excerpt)

```rust
async fn tombstone(&self, did: &Did) -> anyhow::Result<()> {
    let keys = self.key_store.get(did).await?
        .ok_or_else(|| anyhow!("no custody keys to tombstone {}", did.as_str()))?;
    let prev = self.op_log.latest_cid(did).await?
        .ok_or_else(|| anyhow!("no prior PLC operation to chain a tombstone onto for {}", did.as_str()))?;

    let operational = Secp256k1Keypair::import(keys.operational.expose())?;
    let op = TombstoneOperation::new(prev.clone());
    let sig_bytes = operational.sign(&op.signing_bytes()?)?;
    let signed = op.into_signed(URL_SAFE_NO_PAD.encode(&sig_bytes));
    let cid = signed.cid()?;
    let op_json = signed.to_json()?;

    // (4) Public submission FIRST — so a failed submit never advances our local
    // chain. A retry then re-reads the correct `prev` and re-signs the same
    // tombstone (deterministic) rather than chaining onto an unsubmitted op.
    self.directory.submit(did.as_str(), &op_json).await?;
    // (5) Private write — record the now-submitted tombstone (chains onto `prev`).
    self.op_log.append(&PlcOperationRecord {
        did: did.clone(), cid, op_type: "plc_tombstone".to_string(),
        prev: Some(prev), operation_json: op_json.to_string(),
    }).await?;
    Ok(())
}
```

### `adapter-atproto/src/plc_directory.rs` (excerpt)

```rust
pub trait PlcDirectory: Send + Sync {
    /// Submit `operation` registering/updating `did`. Fallible: the HTTP impl
    /// performs a network write; the no-op never fails.
    async fn submit(&self, did: &str, operation: &serde_json::Value) -> anyhow::Result<()>;
}
/// Local/dev directory: accepts the operation and does nothing. (v1 default)
pub struct NoopPlcDirectory;
/// Real submitter: POST {base_url}/{did}. Kept off the live path by config until launch.
pub struct HttpPlcDirectory { base_url: String, client: reqwest::Client }
// HttpPlcDirectory::submit: non-2xx → bail!("PLC directory rejected {did}: {status} {text}")
```

### `adapter-atproto/src/auth_store.rs` (module doc, excerpt)

```text
Postgres persistence for jacquard's OAuth state ... implements ClientAuthStore against the
`atproto_oauth` Postgres schema so the OAuth grant for a visitor outlives a single process ... every
`data` blob is sealed at rest before it is written ... The schema lives in adapter-pg's migrations (it
owns all DDL); the pool and vault are injected by `api`.
```

### DD 23003138 "Account Deletion, Tombstoning & Handle Reuse" (DECIDED 2026-06-25, v3 2026-08-30) — excerpts

> **D3. Hard-delete = did:plc tombstone.** Empty Accounts only. The DID is tombstoned (clears the DID
> document, permanently deactivates the identity) on the native ~72h PLC recovery window, during which
> it can be reversed (Zurfur custodies the keys); after it, the DID is gone. No custom Zurfur retention.
> **v1:** hard-delete marks the actor row `tombstoned_at` and frees the handle, then signs and submits a
> `plc_tombstone` (operational rotation key, chained onto the DID's latest op) as a separate retryable
> step, never inside the private transaction (ZMVP-34). *(2026-08-30 → 57081857 D6: the actor row is
> never deleted — `did` is its NOT NULL primary key; `tombstoned_at` is set and the facts detached.)*

> **D4.** Accounts carry a handle; freed immediately on hard-delete. **D7.** Owner-only.

> **Open / follow-up — Tombstone durability (before the real PLC directory is enabled):** v1's directory
> submission is a gated no-op, so the tombstone signs and logs but registers nowhere. Before real
> submission is switched on: an outbox/retry for a failed tombstone (the handler currently logs &
> swallows), a submission-status column, and a purge of the custody keys once the recovery window closes.
> Tracked as a ZMVP-34 security follow-up.

> Grounding: a did:plc tombstone clears all DID-document fields and permanently deactivates the DID,
> subject to the ~72h recovery window; permanent after it. The full PLC operation log is permanently
> public and cannot be purged, even after tombstone. (did:plc spec, https://web.plc.directory/spec/v0.1/did-plc)

### DD 57081857 "Actor Addressing — DID as the Only Identifier, Everywhere" (DECIDED 2026-08-30) — excerpt

> **D6. Tombstoning is Bluesky-style.** A deleted actor's DID row is never removed and its `did` never
> nulled (impossible on a NOT NULL PK): `tombstoned_at` is set and the facts are detached. Rescopes
> ZMVP-125. *Open: "Re-key actor tables on DID — new ticket; lands before ZMVP-124/125; /security-review
> mandatory."*

### DD 55836674 "The Application Layer" (DECIDED 2026-08-27) — the relevant rulings, paraphrased

D1 one plain `pub async fn` per use case, no mediator. D6 the use case owns the transaction
(`application::transaction`); no driver or background job opens one. D7 authn = driver / authz +
orchestration = application / invariants = domain. `now` and runtime config are parameters.
