1. **F — Transactional intent + idempotent PLC command:** Best: atomically preserves the deletion obligation, keeps orchestration in application, assigns PLC reconciliation to the adapter, and explicitly handles replay and recovery/cancellation races.

2. **B + D — Transactional outbox with compile-enforced ordering:** Correct ownership and crash durability, but unsafe until the current tombstone contract is made idempotent or reconciling; attempt counters cannot repair tombstone-of-tombstone replay.

3. **C + D — Versioned lifecycle state on the immortal account row:** Potentially cleaner than a separate outbox, but blocked on unbuilt D6/B5 and must be a generation-based state machine—not a `pending_tombstone` Boolean—to prevent stale workers.

4. **E — Adapter-owned durability:** Useful for preserving signed operations and reconciling ambiguous PLC outcomes, but unacceptable as the sole seam because a crash between private commit and adapter acceptance loses the intent completely.

5. **D — Compile-enforced ordering alone:** A worthwhile guard against repeating the current transaction-boundary violation, but dropped receipts, crashes and network failures remain invisible and unrecoverable.

6. **A — Reorder in place:** Meets the after-commit rule but knowingly discards the required operation on failure; `require_live_account` then prevents caller-driven replay, making this unsuitable once real PLC submission is enabled.

7. **G — Public-first reversible saga:** Worst: contradicts decided D3, mistakes a time-bounded emergency recovery mechanism for rollback, and merely replaces the tombstone gap with a more dangerous durable-compensation gap affecting a still-live local account.

**What would flip #1:** A merged, reviewed D6 implementation showing that the immortal account row already provides an atomic, generation-numbered DID lifecycle state machine covering deletion, PLC acknowledgement, authorized recovery/cancellation, worker leases and custody-key purge would flip me from F to C+D; that would prove the separate application outbox is redundant rather than merely unfashionable.