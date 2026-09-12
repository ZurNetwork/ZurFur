# Fork 001 — the did:plc tombstone seam on account hard-delete (SCRUTINIZE)

Date 2026-09-10 · tree c6c7514a + working copy · judge: Claude (odd fork) · seats: GPT (gpt-5.6-sol, Responses API + web_search), Gemini (gemini-3.1-pro-preview + google_search) · Grok/Sonar absent (no keys)

---

# Brief (seat-facing, sealed content stripped)

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

---

# S0 — Verification

## GPT

1. **CONFIRMED** — `delete` begins the UoW, calls and awaits `did_minter.tombstone` before `uow.commit()`, and converts failure into only a `warn!`.
2. **CONFIRMED** — DD 23003138 D3 explicitly requires tombstoning after the private transaction as a separate retryable step.
3. **UNVERIFIABLE** — The supplied context contains no repository-wide grep output or complete `backend/crates/` tree proving absence of an outbox, queue, or retry table.
4. **CONFIRMED** — `require_live_account` precedes deletion; after current `hard_delete` removes the account row, a repeated delete cannot reach tombstoning.
5. **CONFIRMED** — The specification gives a 72-hour recovery window in which a higher-authority rotation key may rewrite history, and specifically says deactivation may be nullified during that window. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md)) URL: `https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md`
6. **CONFIRMED** — The `AccountWrites::hard_delete` contract explicitly leaves `account_keys` intact for PLC recovery.
7. **CONFIRMED** — DD 57081857 D6 decides that the DID-keyed actor row remains with `tombstoned_at`; it also identifies the re-key as still open, while the current port contract still deletes `accounts`.
8. **CONFIRMED** — The context identifies `transaction` as used once by `sweep_deadlines`; that use case injects `now`, while its timer and advisory-lock leadership live in `api`.
9. **CONFIRMED** — The `DidOperations` documentation says no adapter implements it yet; no caller is shown, while the supplied `RealDidMinter` excerpt implements the existing tombstone operation.
10. **UNVERIFIABLE** — The code confirms that retrying after a fully logged success reads the tombstone CID and constructs a tombstone chained onto that tombstone. The specification requires operations to reference prior state and tombstones contain no rotation-key set, supporting expected rejection; however, neither the supplied context nor fetched specification establishes how the directory responds to resubmission of the *identical already-accepted CID*. Therefore the assertion that the submit-succeeded/log-append-failed case is “safe” is not verified. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md)) URL: `https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md`
11. **UNVERIFIABLE** — The context shows the `facts::exist` query and a FIXME, but not the claimed implementation returning `has_facts: false`.
12. **CONFIRMED** — PLC operations are submitted by HTTP POST to a central external directory, and the specification expressly recognizes directory rejection/refusal; ordinary request/response failure permits an accepted write whose response is not received. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md)) URL: `https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md`
13. **CONFIRMED** — `AtprotoAuthStore` persists sealed OAuth state in PostgreSQL, while its module documentation explicitly assigns the schema migrations to `adapter-pg`.
14. **CONFIRMED** — `NoopPlcDirectory` accepts and does nothing; the `DidMinter` contract says v1 signs and logs locally without registration, and DD 23003138’s follow-up names retry/outbox, submission status, and eventual custody-key purge.
## Gemini

1. CONFIRMED. The code block for `application/src/account/delete.rs` shows the tombstone call occurring before `uow.commit().await?`, and errors are logged via `tracing::warn!` without being returned or retried.
2. CONFIRMED. DD 23003138 D3 explicitly states tombstoning is "a separate retryable step, never inside the private transaction".
3. UNVERIFIABLE. The context block does not contain the results of a workspace-wide `grep` for "outbox" or a full listing of `backend/crates/`.
4. CONFIRMED. `require_live_account` will fail if the row has been hard-deleted (and the current implementation still deletes rows), preventing the caller from re-running the tombstone step.
5. CONFIRMED. The did:plc specification explicitly defines a 72-hour recovery window using higher-priority rotation keys.
6. CONFIRMED. The `AccountWrites::hard_delete` documentation explicitly states that `account_keys` are left in place to preserve the 72-hour recovery window.
7. CONFIRMED. DD 57081857 D6 explicitly dictates this change, and the framing notes acknowledge it is decided but not yet built.
8. CONFIRMED. The `sweep_deadlines` code block calls `application::transaction`, and its doc comment confirms the wall-clock and advisory-lock elements live in the `api` driver.
9. CONFIRMED. The `DidOperations` trait doc explicitly notes "no adapter implements it yet", and `RealDidMinter::tombstone` is the current implementation in `adapter-atproto`.
10. CONFIRMED. The PLC directory rejects subsequent operations on a tombstoned DID as invalid. Because `RealDidMinter::tombstone` advances the local operation log on success, a replay would attempt to chain a new tombstone onto the previous tombstone, which is an invalid operation.
11. UNVERIFIABLE. The code block for `application/src/account/delete.rs` shows the usage of `facts::exist`, but the implementation of this function is not provided in the context block to verify it is stubbed to `false`.
12. CONFIRMED. The PLC directory is a central HTTP server where operations are submitted; as an external HTTP API, calls are inherently subject to network failures, timeouts, and replays.
13. CONFIRMED. The `adapter-atproto/src/auth_store.rs` doc string confirms the adapter writes to Postgres for OAuth state and that the DDL lives in `adapter-pg`.
14. CONFIRMED. The `NoopPlcDirectory` code confirms it is a gated no-op in v1, and DD 23003138's open items specifically track adding an outbox/retry mechanism prior to real directory launch.
## S0 — Judge consolidation (Claude, fork 001 is odd → Claude judges; Claude did not debate)

Seat verdicts agree on 1, 2, 4–9, 12–14 (CONFIRMED by both). Three claims needed resolution:

- **Claim 3 (no outbox anywhere in `crates/`)** — both seats UNVERIFIABLE (repo-wide fact not in the context block). Resolved by the orchestrator's repo access: `grep -rni outbox crates/` at c6c7514a + working copy returns only the two doc comments this fork itself introduced. **CONFIRMED.**
- **Claim 11 (`facts::exist` stubbed `has_facts: false`)** — both seats UNVERIFIABLE. Resolved by reading `application/src/account/facts/exist.rs:13-15`: `Ok(Output { has_facts: false })`. **CONFIRMED.**
- **Claim 10 (adapter not idempotent after success; identical-CID replay "safe")** — Gemini CONFIRMED whole; GPT CONFIRMED the tombstone-of-a-tombstone half and marked the "identical-CID replay is safe" half UNVERIFIABLE. Resolved by fetching the PLC server's validation source (did-method-plc `packages/lib/src/data.ts`, `assureValidNextOp`): `if (check.is(lastOp.operation, t.def.tombstone)) throw new MisorderedOperationError()` — **any** operation submitted after an accepted tombstone is rejected, and no CID-dedupe exists before that check. So the orchestrator's correction was itself half wrong: **a byte-identical replay after the directory accepted the tombstone is also rejected.** Corrected claim 10 for all later phases: *`RealDidMinter::tombstone` is never safely re-callable once the directory has accepted a tombstone — neither a re-signed op (chains onto the tombstone) nor the identical op (Misordered) is accepted. Idempotency must be established locally: check the own op log for an existing `plc_tombstone`, and/or read the directory's log (`GET /{did}/log/last`) and treat "already tombstoned" as success.* GPT's caution stands; Gemini's S2 concession ("the PLC directory will reject the exact duplicate") is consistent with the corrected form.

Struck / may not be argued from in original form: claim 10's "safe replay" sentence.

Runner notes: the skill's Gemini id `gemini-3.1-pro` does not exist on the API; `gemini-3.1-pro-preview` was used. GPT ran on the Responses API with `web_search` (reasoning tokens exhausted the first 2,000-token cap; the API budget was raised, the 2,000-token *output* instruction kept). Grok and Sonar seats absent (no keys).

---

# S1 — Attack (isolated)

## GPT

## A. Reorder in place

**Against:** This satisfies ordering but not reliability. “Log and swallow” converts account deletion into an untracked security failure. There is no durable evidence that tombstoning remains pending, and `require_live_account` prevents the caller from replaying deletion once the row is gone. A process crash creates the same loss without even a warning.

**For:** It is the smallest correction to the current violation: commit the private delete before touching PLC. While `NoopPlcDirectory` is the live implementation, it also avoids building machinery that currently cannot affect the real directory.

**Failure scenario:** Private hard-delete commits and frees the handle → process dies before `tombstone()` → DID remains active indefinitely, custody keys remain, and no retryable record exists.

**Assessment:** Disqualified for the real directory. It knowingly fails the stated “must be tombstoned” requirement.

---

## B. Transactional outbox

**Against:** A generic “DID to tombstone” row is insufficient because the current adapter operation is not generally idempotent. After a complete success, replay reads the logged tombstone as `latest_cid`, constructs a tombstone-of-a-tombstone, and gets rejected. Therefore the outbox must coordinate three states—directory accepted, operation log appended, outbox completed—or carry/recover a stable expected `prev`/CID. A naïve at-least-once worker is broken.

**For:** This is the only listed option that atomically couples private deletion with durable intent using the existing transaction, while keeping PLC execution after commit. It puts workflow ownership in `application`, where cross-boundary orchestration belongs, and matches the existing actor-less, driver-triggered sweep pattern. Attempts, next-attempt time, terminal/manual-review state and metrics have an obvious home.

**Failure scenario:** Worker submits successfully, appends the operation log, then crashes before marking the outbox row done → retry calls `RealDidMinter::tombstone` again → it chains from the logged tombstone → PLC rejects it → a genuinely completed deletion is reported perpetually failed.

**Assessment:** Best base, but only after fixing the operation contract/reconciliation semantics.

---

## C. Marker on the account row

**Against:** It depends on an unbuilt re-key and immortal-row design. Shipping it before D6 lands is impossible or lossy. It also overloads the account aggregate with queue mechanics: attempts, leases, backoff, directory acknowledgement and operator disposition are not account facts. This becomes especially ugly when key purge and recovery are added.

**For:** Once account rows are genuinely immortal, the DID lifecycle state belongs naturally beside `tombstoned_at`. Marking deletion and pending submission in one transaction is atomic, and account support tooling can expose whether public deactivation completed without joining a separate workflow table.

**Failure scenario:** A recovery operation restores the DID within the roughly 72-hour higher-priority-key window, but recovery clears `tombstoned_at` without clearing a stale pending marker → the drain later submits another tombstone → the recovered identity is deactivated again. The PLC specification confirms that tombstones use the normal recovery window and operations chain through `prev`. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md?utm_source=openai))

**Assessment:** Viable only after B5/D6 and only with an explicit lifecycle state machine—not a Boolean column.

---

## D. Compile-enforced ordering

**Against:** It proves at most “not before this receipt”; it does not prove “eventually called.” Rust permits dropping a receipt. It also supplies no persistence, retry, observability or crash recovery. More dangerously, a receipt may imply stronger correctness than it provides: database commit acknowledgement is not durable workflow completion.

**For:** As a supplementary guard, it makes the current category error harder to reintroduce. A privately constructed receipt tied to a committed deletion can prevent accidental in-transaction PLC calls at relevant application call sites.

**Failure scenario:** `commit()` succeeds and returns the receipt → task cancellation occurs before tombstone invocation → receipt is dropped → DID remains active with no pending state.

**Assessment:** Useful under B or C; meaningless as the seam itself.

---

## E. Adapter-owned durability

**Against:** It cannot close the fatal gap between private commit and invoking the adapter. If the process dies there, the adapter never persists intent. It also hides business-visible workflow state behind a driven adapter and makes application observability dependent on adapter-specific APIs. Claim 13’s correction matters: `adapter-atproto` already uses durable Postgres state, but `adapter-pg` owns its DDL. Thus “adapter-owned” durability already requires cross-adapter composition and migration coordination; it is not clean encapsulation.

**For:** PLC-specific exact-once/reconciliation knowledge does belong near the PLC implementation. The adapter is best positioned to preserve the signed operation, expected `prev`, deterministic CID and submission response, rather than forcing application code to understand PLC chain semantics. Existing durable OAuth storage shows persistence there is architecturally possible, though not self-contained.

**Failure scenario:** Private deletion commits → host is killed before `adapter.tombstone()` begins → no adapter intent exists → restart has nothing to retry.

**Assessment:** Good location for **submission mechanics**, bad location for the sole durable deletion intent. Combine with B; do not replace B with it.

---

## Missing framing

**F. Transactional application intent plus idempotent PLC command** — enqueue, in the deletion transaction, an immutable command keyed by DID and expected operation identity (`prev` and/or signed-operation CID); the PLC adapter reconciles local log and directory state before declaring success.

The brief also omits the **recovery/cancellation race**: tombstone retry, user-authorized recovery during the 72-hour window, and eventual custody-key purge require one coherent state machine. Without generation numbers or command cancellation, a delayed worker can tombstone an identity after it has been intentionally recovered.

**Recommendation:** F: effectively hardened B, with D as a guard and adapter-owned PLC reconciliation—not adapter-owned workflow durability.
## Gemini

**PHASE S1 — ATTACK**

### **Option A: Reorder in place**
*   **Strongest case AGAINST:** It violates the Doctrine floor (anti-domination/sanity). If the local `uow.commit()` succeeds but the synchronous `did_minter.tombstone` fails (e.g., network partition to `plc.directory`), the handle is freed and the local account is dead, but the DID remains active on the public network indefinitely. The user is locked out, the intent is swallowed into a `warn!` log, and there is no UI or automated way to retry. It creates an orphaned identity.
*   **Strongest case FOR:** Zero infrastructural bloat. It requires no new tables, no background polling, and fits the exact operational profile of the existing `application/src/account/delete.rs` code, shifting only the line order. 
*   **Concrete failure scenario:** The private hard-delete commits. The process receives a `SIGTERM` or OOM-kills exactly before `did_minter.tombstone` is invoked. The platform drops the public step entirely. The user's DID remains publicly queryable on the AT Protocol forever, despite them invoking account deletion.

### **Option B: Transactional outbox**
*   **Strongest case AGAINST:** Schizophrenic durability. `adapter-atproto` already handles its own durable state (`op_log`), meaning this approach splits the lifecycle of a single public intent across two storage concepts: an application-owned outbox row and an adapter-owned `op_log` row. This introduces complex, distributed two-phase state synchronization for a single operation.
*   **Strongest case FOR:** Textbook prevention of cross-store dual-write failures. Writing the outbox row in the same Postgres transaction guarantees that the intent to tombstone survives any crash post-commit, satisfying DD 23003138 D3’s mandate for a "separate retryable step" without blocking the user request.
*   **Concrete failure scenario:** The background sweep job picks up an outbox row and calls `tombstone`. The directory submission succeeds, and crucially, `adapter-atproto` successfully appends to `op_log`. The application then crashes *before* the outbox row is marked "done". On retry, the adapter reads the *new* `latest_cid(did)` (which is the tombstone itself) and blindly signs a tombstone-of-a-tombstone. The PLC directory rejects this invalid chain, trapping the outbox row in an infinite failing loop.

### **Option C: Marker on the account row**
*   **Strongest case AGAINST:** It halts addressing the immediate defect (Claim 11). By making infrastructure queues dependent on domain lifecycle states (`tombstoned_at`), this option cannot be implemented until DD 57081857 D6 (re-keying the system on DID instead of Account ID) is merged and stable. It inappropriately entangles external I/O tracking with the core entity schema.
*   **Strongest case FOR:** Eradicates the need for a separate outbox table entirely. By piggybacking on the imminent fact that account rows will become immortal (DD 57081857 D6), the `pending_tombstone` state becomes a highly observable, inherent property of the entity. 
*   **Concrete failure scenario:** A user deletes their account, setting the marker. A sweep job begins processing the tombstone. Simultaneously, an admin (or user via an out-of-band support flow) reverses the local deletion using the retained custody keys (Claim 6). The sweep job finishes and successfully submits the tombstone to the PLC directory, rendering the local restoration void and severing the recovered account from the network. 

### **Option D: Compile-enforced ordering**
*   **Strongest case AGAINST:** It is a category error. Type-state guards enforce code topology, not distributed durability. This option merely guarantees the developer places the tombstone call *after* the commit in the source text; it provides exactly zero guarantees that the tombstone actually executes or retries if the network drops.
*   **Strongest case FOR:** Satisfies the doctrine mandate to "make unsoundness unreachable." By forcing `tombstone` to consume a receipt from `commit()`, it guarantees no developer can accidentally place the network call inside the database unit-of-work, preventing long-running transactions and rollback anomalies.
*   **Concrete failure scenario:** The code is perfectly structured and compiles. At runtime, the database commit succeeds and returns the receipt. The host kernel panics one millisecond later. The receipt vanishes from memory, the process dies, and the tombstone intent is permanently lost without a retry mechanism.

### **Option E: Adapter-owned durability**
*   **Strongest case AGAINST:** Violates DD 55836674 D6 and D7. The adapter layer is meant to be driven. Forcing the adapter to persist intent and run its own internal retry loops leaks orchestration and transaction management out of the `application` layer into a bounded context that should only map I/O. 
*   **Strongest case FOR:** Maximum encapsulation of AT-Protocol specifics. The application layer shouldn't care about `prev` CID tracking or PLC directory transient downtime. Pushing durability into the adapter aligns with the fact that `adapter-atproto` already owns PostgreSQL persistence (Claim 13) for OAuth.
*   **Concrete failure scenario:** The application commits the private database transaction and calls the durable adapter port to register the intent. The adapter attempts to save this intent to its Postgres table, but the connection pool is exhausted or the disk is full, returning `Err`. The application cannot roll back the already-committed private transaction, resulting in a dropped tombstone intent and a permanently orphaned public identity.

### **MISSING OPTION**

**Option F: Public-First Ordering (The Reversible Saga)**
*   **Definition:** The use case executes the network write (`did_minter.tombstone`) *before* opening the private-store transaction (mirroring the exact ordering already used in `change_handle.rs`), utilizing the native 72-hour `did:plc` recovery window to roll back the PLC directory state if the subsequent database commit fails. 
*   **Why it exposes the framing:** The brief arbitrarily presumes the reliability seam *must* live after the private commit, completely ignoring that a `did:plc` tombstone is inherently reversible by design (Claim 5). Executing the public step first eliminates the need for complex outboxes entirely: if the database commit fails, the system safely reverses the public tombstone using the custody keys it deliberately retains for this exact reason (Claim 6).
## Board-added options (relabelled by the judge for S3)

- **F. Transactional intent + idempotent PLC command (proposed by the GPT seat)** — in the deletion
  transaction, enqueue an immutable command keyed by DID and the expected operation identity (`prev`
  and/or the signed-operation CID); the PLC adapter reconciles the local operation log and directory
  state before declaring success, so replay after a partial success is safe. Effectively a hardened B,
  with D as a guard and adapter-owned PLC *reconciliation* (not adapter-owned workflow durability).
  Also names a missing consideration: the recovery/cancellation race — tombstone retry, user-authorized
  recovery inside the 72h window, and eventual custody-key purge need one coherent state machine
  (generation numbers or command cancellation), or a delayed worker can tombstone a deliberately
  recovered identity.
- **G. Public-first ordering / reversible saga (proposed by the Gemini seat)** — the use case runs the
  tombstone on the directory BEFORE opening the private-store transaction (mirroring
  `change_handle.rs`), relying on the native 72h did:plc recovery window and the retained custody keys
  to reverse the public tombstone if the private commit then fails. No outbox at all. Attacks the
  framing's assumption that the seam must sit after the private commit.

---

# S2 — Rebuttal (one round; run because S1 attacks were not orthogonal — both seats rest on claim 10 and on each other's crash-gap / recovery-race claims)

## GPT rebuts Gemini

## Rebuttal

### 1. **A: “The DID remains publicly queryable forever” — wrong symptom, overstated conclusion**

The actual defect is that the DID remains **active**, not merely queryable. PLC operation history remains permanently public even after a valid tombstone; successful deletion does not make the DID or its history disappear. The PLC spec explicitly says tombstones deactivate the DID while the complete operation history remains auditable. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md?utm_source=openai))

“Indefinitely” is fair only under the stated absence of automated retry. “Forever” assumes no operator repair or later implementation and is therefore overstated. More importantly, at current v1 configuration `NoopPlcDirectory` means even a successful call registers nothing publicly. Reordering A fixes transaction placement but does **not** presently achieve public deactivation.

**CONCEDE:** A has an unrecoverable crash gap and is plainly insufficient as the final pre-launch design.

---

### 2. **B: “Complex, distributed two-phase state synchronization” — incorrect characterization**

There is no two-phase commit here. The outbox and private deletion are one PostgreSQL transaction; PLC submission and `op_log.append` happen later. That is specifically an alternative to distributed atomic synchronization.

Nor are the two records necessarily duplicative:

- outbox: application obligation—“this deletion still requires public completion”;
- `op_log`: protocol evidence/chain material—“this PLC operation was submitted.”

The concrete replay failure is valid **for the current `RealDidMinter`**, but “trapping … in an infinite failing loop” assumes a brain-dead worker. Attempt limits, terminal failure state, reconciliation against the directory, or making tombstone recognize an already-logged tombstone all stop infinity. Claim 10 establishes a serious idempotency defect, not that B is structurally impossible.

**CONCEDE:** B cannot safely ship while `RealDidMinter::tombstone` blindly chains from an already-recorded tombstone. The outbox worker needs an idempotency/reconciliation contract first.

---

### 3. **C: “It halts addressing Claim 11” — non sequitur**

Claim 11 is the stubbed `facts::exist`, causing every deletion to choose hard-delete. C’s dependency on D6/B5 does not prevent fixing that defect independently or concurrently. “Halts” invents a scheduling dependency absent from the brief.

The recovery race scenario is also **UNVERIFIABLE**. The context establishes retained custody keys and PLC’s recovery window; it does not establish an admin restoration command, user support flow, or local untombstoning semantics. PLC recovery requires a higher-authority rotation key and a deliberate fork operation; retained keys do not themselves constitute a local account-restore feature. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md?utm_source=openai))

A real race still exists if restoration is later introduced, but then the marker needs claiming/versioning or cancellation semantics. The S1 scenario cannot be presented as a current capability.

**CONCEDE:** C is blocked on the immortal-row re-key and couples delivery state to the account schema.

---

### 4. **D: strongest criticism is right, strongest defense is too absolute**

A commit receipt guarantees ordering only if it is unforgeable, transaction-specific, and constructible solely by successful commit. None of that receipt design exists in the brief. Saying it “guarantees” ordering outruns the proposed sketch.

**CONCEDE:** D alone provides no durability whatsoever; it is only a supplementary topology guard.

---

### 5. **E: “Violates DD 55836674 D6/D7” — not established**

D6 says the application use case owns **its private-store transaction** and drivers/background jobs do not open it. It does not categorically prohibit an adapter from transactionally persisting state internal to fulfilling a port contract. D7 allocates orchestration to application, but accepting a durable command behind a port can still be adapter behavior if retry semantics are part of that contract.

The stronger objection is the one S1 itself identifies: calling E only **after** private commit leaves a crash/failure gap before adapter acceptance. Thus E does not solve the original seam unless the intent is first recorded atomically with deletion—which effectively moves back toward B/C—or the application can reconstruct missing intents.

Also, OAuth persistence is weak precedent. The brief says `adapter-atproto` implements the store, while `adapter-pg` owns all DDL. It does not establish adapter-owned workers or lifecycle orchestration.

**CONCEDE:** E encapsulates PLC-specific replay logic better than B alone, but cannot durably bridge the initial private-commit→acceptance gap.

---

### 6. **F: “Public-first eliminates outboxes” — the weakest and most dangerous claim**

F directly contradicts decided DD 23003138 D3 and the port documentation: tombstone runs **after** private commit. `change_handle` is not authority to reverse that deletion-specific ruling.

More fundamentally, PLC recovery is not an automatic rollback. It requires:

1. a higher-authority rotation key;
2. a new operation pointing to the prior valid CID;
3. submission within approximately 72 hours. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md?utm_source=openai))

If PLC tombstone succeeds, the database commit fails, and the process dies before compensation, the user’s still-live local account has a deactivated public DID. F therefore creates the mirror-image orphan and still needs a **durable compensation intent**. It has not eliminated the reliability seam; it has made restoration time-bounded and more dangerous.

Whether Zurfur’s retained custody material definitely contains a suitably higher-priority key for every DID is **UNVERIFIABLE** from the context. Permanent public operation history also remains even after recovery. ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md?utm_source=openai))

**CONCEDE:** F correctly notices that PLC supplies a recovery mechanism. It incorrectly promotes that emergency mechanism into a transaction rollback protocol.
## Gemini rebuts GPT

## ATTACKS ON WEAKEST LOAD-BEARING CLAIMS

### 1. Option C "Failure Scenario": Invented Database Mechanics
**The Claim:** "A recovery operation restores the DID within the roughly 72-hour higher-priority-key window, but recovery clears `tombstoned_at` without clearing a stale pending marker → the drain later submits another tombstone..."
**The Rebuttal:** UNVERIFIABLE and demonstrably invented. You fabricated an internal application-level un-delete feature to manufacture a race condition. The brief dictates that `hard_delete` leaves custody keys in place strictly so the "native ~72h did:plc tombstone recovery window can still reverse the deletion" (DD 23003138). This recovery happens *externally* on the PLC directory. There is absolutely zero text in the context block indicating Zurfur possesses a local "recovery operation" that mutates the database to clear `tombstoned_at`. The immortal row design (DD 57081857 D6) sets `tombstoned_at` and detaches facts, but provides no mechanic to reverse it locally. Your failure scenario is fiction.

### 2. "Missing Framing" (Option F): Fundamental Misunderstanding of `did:plc`
**The Claim:** "Without generation numbers or command cancellation, a delayed worker can tombstone an identity after it has been intentionally recovered."
**The Rebuttal:** OVERSTATED and technically false. This ignores how the AT Protocol's cryptographic operation log functions. The PLC directory is a strongly-consistent, self-authenticating ledger. If a user recovers their DID during the 72-hour window using a higher-priority rotation key, they fork the operation chain on the directory. If Zurfur’s delayed worker subsequently wakes up and attempts to submit its pending tombstone, it will build the operation using its local, now-stale `prev` CID. The PLC directory will reject the tombstone outright because its `prev` no longer matches the directory's head. Zurfur does not need to build internal "command cancellation" or "generation numbers"; the protocol’s hash-chaining natively guarantees the stale tombstone will bounce. Demanding redundant local state to solve a problem the decentralized protocol already solves by design is unsound.

### 3. "Missing Framing" (Option F): Architectural Overstep
**The Claim:** "Recommendation: F: effectively hardened B... with adapter-owned PLC reconciliation—not adapter-owned workflow durability."
**The Rebuttal:** WRONG per the application layer's strict boundaries. The brief outlines: "D1 one plain `pub async fn` per use case... D7... orchestration = application" (DD 55836674). You are recommending pushing complex orchestration—evaluating the local log, fetching remote directory state, and reconciling them to decide if a workflow step is complete—down into the `adapter-atproto` layer. The driven adapter’s job is merely to fulfill the `DidOperations::tombstone` port contract. It is strictly the *application* layer's job to own the pending state, orchestrate the retry, and interpret the adapter's success or failure. Option F is nothing more than Option B paired with a bloated adapter that violates Zurfur's explicit ports-and-adapters strictures.

---

## CONCESSIONS

1. **Option B's Failure Scenario:** I CONCEDE that a crash between `directory.submit()` and `op_log.append()` leaves the worker perpetually failing on retry. As you correctly noted, `RealDidMinter::tombstone` will deterministically rebuild the same operation using the un-advanced local `prev`, and the PLC directory will reject the exact duplicate (or reject a tombstone-of-a-tombstone if it had fully succeeded). Your assessment that Option B requires fixing the reconciliation semantics to interpret a directory rejection as "already done" is accurate.
2. **Option E's Fatal Gap:** I CONCEDE your core argument against Option E. Placing durability strictly inside the adapter fails to close the gap between `uow.commit()` and `adapter.tombstone()`. A process crash in that window permanently loses the intent to tombstone, violating the Doctrine Floor's requirement that the non-toxic path outranks optimization.
3. **Claim 13 Implication for Option E:** I CONCEDE that because `adapter-pg` owns all DDL, "adapter-owned durability" inside `adapter-atproto` is a leaky abstraction. Attempting to isolate intent in the adapter would mandate cross-adapter migration coordination, destroying the supposed encapsulation benefits.
---

# S3 — Independent ranking

## GPT

1. **F — Transactional intent + idempotent PLC command:** Best: atomically preserves the deletion obligation, keeps orchestration in application, assigns PLC reconciliation to the adapter, and explicitly handles replay and recovery/cancellation races.

2. **B + D — Transactional outbox with compile-enforced ordering:** Correct ownership and crash durability, but unsafe until the current tombstone contract is made idempotent or reconciling; attempt counters cannot repair tombstone-of-tombstone replay.

3. **C + D — Versioned lifecycle state on the immortal account row:** Potentially cleaner than a separate outbox, but blocked on unbuilt D6/B5 and must be a generation-based state machine—not a `pending_tombstone` Boolean—to prevent stale workers.

4. **E — Adapter-owned durability:** Useful for preserving signed operations and reconciling ambiguous PLC outcomes, but unacceptable as the sole seam because a crash between private commit and adapter acceptance loses the intent completely.

5. **D — Compile-enforced ordering alone:** A worthwhile guard against repeating the current transaction-boundary violation, but dropped receipts, crashes and network failures remain invisible and unrecoverable.

6. **A — Reorder in place:** Meets the after-commit rule but knowingly discards the required operation on failure; `require_live_account` then prevents caller-driven replay, making this unsuitable once real PLC submission is enabled.

7. **G — Public-first reversible saga:** Worst: contradicts decided D3, mistakes a time-bounded emergency recovery mechanism for rollback, and merely replaces the tombstone gap with a more dangerous durable-compensation gap affecting a still-live local account.

**What would flip #1:** A merged, reviewed D6 implementation showing that the immortal account row already provides an atomic, generation-numbered DID lifecycle state machine covering deletion, PLC acknowledgement, authorized recovery/cancellation, worker leases and custody-key purge would flip me from F to C+D; that would prove the separate application outbox is redundant rather than merely unfashionable.
## Gemini

### PHASE S3 — INDEPENDENT RANKING

**1. B + D (Transactional Outbox + Compile-Enforced Ordering)** 
Atomically guarantees the tombstone intent survives any crash post-commit (satisfying DD 23003138 D3), while D’s receipt strictly isolates the network call from the database transaction; it requires the application layer to handle the current `adapter-atproto` infinite-retry flaw by gracefully swallowing "already tombstoned" directory rejections.

**2. F (Transactional intent + idempotent PLC command)** 
Effectively B + D but explicitly bundles the necessary idempotency fix; it is downranked only because delegating full directory state reconciliation to the adapter violates the application layer’s strict orchestration mandate (DD 55836674 D7).

**3. B (Transactional outbox alone)** 
Satisfies the fundamental cross-store durability mandate, but without D it relies purely on developer discipline to prevent transaction-blocking, and natively traps retries in infinite PLC-rejection loops without external idempotency handling.

**4. C (Marker on the account row)** 
Architecturally superior by eliminating the outbox and unifying lifecycle state with the domain entity, but strictly disqualified from immediate use because its hard dependency (DD 57081857 D6: immortal rows) is explicitly "not yet built" at HEAD.

**5. G (Public-first ordering / reversible saga)** 
Dangerously unsound; a private database crash after a successful public tombstone leaves the user alive locally but permanently dead publicly, and natively reversing this requires a *second* fallible network call, re-introducing the exact same reliability seam.

**6. E (Adapter-owned durability)** 
Fatally flawed by a physical execution gap; if the host is OOM-killed in the microsecond between `uow.commit()` and invoking the adapter port, the intent is never persisted in the adapter and the deletion is permanently dropped.

**7. D (Compile-enforced ordering alone)** 
A category error conflating source-code topology with distributed systems durability; guarantees the developer typed the code in the right order but provides absolutely zero runtime retry, crash recovery, or execution guarantees.

**8. A (Reorder in place)** 
Violates the doctrine floor's anti-domination mandate; an arbitrary network timeout permanently swallows the user's explicit intent to deactivate their public identity, abandoning the DID on the network while destroying the local means to retry it.

***

### EVIDENCE TO FLIP #1

My #1 ranking would flip to **C + D** if a codebase inspection revealed that DD 57081857 D6 ("Actor Addressing — DID as the Only Identifier") was actually fully merged at HEAD, overriding Claim 7's assertion that it is "not yet built". Piggybacking the pending network intent on an immortal account row structurally prevents the outbox-divergence anomalies inherent in Option B and averts the need for a separate polling table. Alternatively, if Zurfur's bounded context rules strictly prohibit the `application` layer from introspecting the external PLC directory state to resolve B's infinite-retry flaw, I would be forced to flip to **Option F**, accepting the D7 orchestration violation (leaking reconciliation into the adapter) as a strict operational necessity to satisfy the AT Protocol's cryptographic hash-chaining rules.
---

<sealed>
Engineer's ranking (given 2026-09-10, before any seat ran):
1. E — Adapter-owned durability
2. D — Compile-enforced ordering
3. B — Transactional outbox
4–5. A, C — unranked tail
No per-rank reasons given.
</sealed>

# S4 — Judge reveal (Claude)

## Sealed ranking (unsealed here, first time any phase sees it)

Engineer, 2026-09-10, before any seat ran: **E > D > B > {A, C}** (no per-rank reasons).

## Board rankings (S3, independent)

| Rank | GPT | Gemini |
|---|---|---|
| 1 | F (intent-in-uow + idempotent PLC command) | B + D |
| 2 | B + D | F |
| 3 | C + D (generation-based, post-D6) | B |
| 4 | E | C |
| 5 | D alone | G |
| 6 | A | E |
| 7 | G | D alone |
| 8 | — | A |

## 1. Delta — the headline

**The board inverts the Engineer's top and third.** The Engineer's #1 (E) sits 4th and 6th; the Engineer's #3 (B, with D as a guard) is the board's consensus top — Gemini #1 outright, GPT #2 behind its own hardened-B variant F. Both seats' #1 and #2 are the same two entries in swapped order, and both are B-family.

**Why E fell (unanimous, held through S2, Gemini conceded its own S1 pro-E case):**
- *The crash gap.* Under E the use case commits the private delete, *then* calls the adapter. Between those two instructions nothing durable records the obligation; a SIGTERM/OOM/pool-exhaustion there loses the tombstone with no trace (GPT S1 §E, Gemini S1 §E scenario, Gemini S2 concession 2). Only a row written **inside** the delete's own unit of work closes that gap — which is what B is.
- *Claim 13 corrected.* `adapter-atproto` does own Postgres tables, but `adapter-pg` owns all DDL, so "adapter-owned durability" already needs cross-crate migration coordination; the encapsulation E buys is partial (Gemini S2 concession 3).
- *Ownership.* Gemini reads DD 55836674 D7 as forbidding orchestration/retry loops in a driven adapter; GPT does **not** accept that reading ("D6 does not categorically prohibit an adapter from transactionally persisting state internal to fulfilling a port contract") — see divergence map. The seats reject E for the gap, not for the layering.
- **Judge red-team on the rejection:** the seats attacked E *as defined* (adapter called after commit). A variant the brief did not name — call it **E′: the adapter registers its intent row through the caller's `UnitOfWork`, then drains its own table** — has no crash gap. E′ is B with the table and drain owned by `adapter-atproto` instead of `application`. If the Engineer's E meant E′, the board's objection reduces to the D7 ownership split, which is unresolved (fork 1). If it meant E as written, the objection stands and is fatal.

**Why D fell (unanimous):** a receipt proves "not before commit", never "eventually called"; the receipt can be dropped, the task cancelled, the host killed. Both seats rank *+D combinations* at #1/#2 and *D alone* near the bottom: right as a guard, wrong as the seam. GPT adds that the sketch's guarantee only holds if the receipt is unforgeable and transaction-specific — no such design exists yet.

**Why B rose:** it is the only listed option that records the obligation atomically with the delete (DD 23003138 D3's "separate retryable step" needs a durable step to retry), matches the `sweep_deadlines` job template, and puts attempt/terminal state in an obvious home. Its one real defect is claim 10 (corrected in S0): `RealDidMinter::tombstone` is never safely re-callable after the directory accepts a tombstone — re-signed **or** byte-identical, `assureValidNextOp` throws `MisorderedOperationError`. A naive at-least-once worker therefore loops forever on a *completed* deletion. Both seats concede this; both say it is a contract fix, not a structural defeat.

**Board-added options the Engineer never considered:**
- **F** (GPT): B + the outbox row carries the expected operation identity (`prev`/CID) + the adapter reconciles own-log and directory state before declaring success + one lifecycle state machine covering retry / recovery / key purge. Ranked 1 (GPT) and 2 (Gemini). This is where the board's centre of mass is.
- **G** (Gemini): public-first "reversible saga" — tombstone before `begin()`, mirroring `change_handle`, using the 72 h window as rollback. GPT's S2 dismantled it (contradicts decided D3; PLC recovery is a higher-priority-key fork within 72 h, not a rollback; a still-live local account with a dead public DID is the mirror-image orphan and needs its own durable compensation intent). Gemini then ranked its own proposal 5th. **Dead.**

**On A and C (the Engineer's unranked tail):** the board agrees A is disqualified once real submission is on (it discards the obligation on failure and `require_live_account` blocks caller replay) — but GPT notes A "fixes placement but registers nothing" under v1's `NoopPlcDirectory`, i.e. A is an acceptable *interim* only while the directory is a no-op. C is mid-table for both and is **both seats' stated flip condition**: once DD 57081857 D6 lands (immortal row), a generation-numbered lifecycle state on the account row could make a separate outbox redundant. Neither seat wants C as a boolean column.

## 2. Divergence map (seat vs seat)

| Split | GPT | Gemini | Evidence | After S2 |
|---|---|---|---|---|
| Where "already tombstoned?" is answered | In the adapter: PLC chain semantics (`prev`, deterministic CID, directory read-back) belong beside the PLC impl; application owns the *obligation* only | In the application job: D7 says orchestration = application; adapter fulfils the port, application interprets rejection as done | Claim 10 (corrected), DD 55836674 D7 | **Held.** This is the F-vs-B+D ordering and the only live seat split. |
| Recovery/cancellation race | Needs generation numbers or command cancellation, else a delayed worker tombstones a recovered identity | Protocol bounces it: a stale tombstone's `prev` ≠ head → nullification path → operational key is lower-priority than the recovery key → rejected | `assureValidNextOp` (judge-fetched) confirms Gemini for a **PLC-side** recovery fork; both seats agree no **local** un-delete feature exists today (GPT: UNVERIFIABLE; Gemini: "fiction") | **Narrowed.** Only bites if Zurfur adds a local un-delete; then B/F need cancellation. |
| Does E violate D6/D7? | Not established — adapter-internal durable state can be part of a port contract | Yes — retry loops in a driven adapter leak orchestration | DD 55836674 D6/D7 text | **Held**, but moot for E-as-written (both reject it on the gap). Live again for E′. |
| Severity of E | 4th — "good location for submission mechanics, bad for the sole intent" | 6th — "fatally flawed" | same | Held; degree only. |
| G | Worst | Proposed it, demoted to 5th | GPT S2 §6 | **Moved** — proposer abandoned it. |

## 3. Consensus check + red team

Unanimous: A disqualified at launch · D guard-only · E-as-written has an unrecoverable gap · B is the right base but unsafe until the tombstone contract is idempotent · C blocked on B5 and must be a state machine, not a flag · G unsound · claims 1–14 as consolidated in S0.

Red team on the unanimity:
- *"E has a crash gap"* — true for E as briefed; **false for E′** (intent enqueued via the uow). The board never saw E′ because the brief didn't name it. The unanimity is on a narrower claim than it looks.
- *"B needs directory reconciliation"* — under v1's `NoopPlcDirectory` there is no directory to reconcile against; a local op-log check ("does my log already hold a `plc_tombstone` for this DID?") is sufficient until real submission is enabled, and directory read-back is a launch-gate item (DD 23003138's Open row already lists the outbox + status column + key purge as pre-launch). The consensus is right but front-loads a launch requirement.
- *"A is disqualified"* — only once submission is real. As a same-day stop-gap for defect A6 (tombstone inside the transaction) while the directory is a no-op, A + the A5 fix removes the *transactional* unsoundness now and loses nothing that v1 could have registered anyway. The board did not weigh sequencing; the Engineer should.

## 4. Forks for ruling (nothing resolved here)

1. **Seam owner.** B (application-owned table + drain job, adapter stays thin), F (B + adapter-owned reconciliation of own-log vs directory), or E′ (adapter-owned table enqueued via the caller's `UnitOfWork`, adapter-owned drain). The only live seat split; hinges on how D7 is read.
2. **The `DidOperations::tombstone` contract.** Return a tri-state (`Submitted` / `AlreadyTombstoned` / error)? Check own op log before signing? Read `GET /{did}/log/last` at launch? Who maps a `MisorderedOperationError` on a tombstoned DID to "done"?
3. **D as guard.** Adopt a commit receipt (must be unforgeable and tx-specific), or rely on the `application::transaction` closure (report C2) — noting the closure alone does *not* stop a port call inside it.
4. **C after D6.** When the immortal-row re-key lands, does the outbox fold into a generation-numbered lifecycle state on the account row (both seats' flip condition), or stay a separate job table?
5. **Local un-delete.** Will Zurfur ever reverse a hard-delete locally inside the 72 h window? If never, no cancellation/generation machinery is needed (the protocol bounces stale tombstones after a PLC-side recovery fork). If ever, B/F need it from day one.
6. **Custody-key purge.** DD 23003138's Open row wants the keys purged once the window closes — same drain job, a second job, or deferred?
7. **`DidMinter` vs `DidOperations`.** Migrate `tombstone`/`update_handle` off `DidMinter` (the other session's open follow-up) and drop the `Did::tombstone` forwarding methods, or keep both ports?
8. **Sequencing.** A5 (`has_facts` stub) must land before or with any of this. Ship A now as the v1-no-op stop-gap and B/F/E′ as the launch gate, or go straight to the durable seam?

## Cost

Tokens (input+output, provider-reported): GPT ≈ 51k this run (+≈49k on the aborted first S0, truncated) · Gemini ≈ 44k. Judge (Claude) not metered separately. Runner: `run.py`; seat outputs `S{0..3}-{gpt,gemini}.md`; this file is S4.
