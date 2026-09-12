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