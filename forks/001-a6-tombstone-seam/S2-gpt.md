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