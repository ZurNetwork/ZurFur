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
