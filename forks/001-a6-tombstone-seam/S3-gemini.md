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