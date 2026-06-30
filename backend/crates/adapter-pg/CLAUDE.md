# adapter-pg — the private data boundary

PostgreSQL adapter: **app-owned** rows, **UUIDv7** keys, transactions. This is the *private* side of the data boundary (the public side is `adapter-atproto`). Migrations live in `migrations/` (embedded via `sqlx::migrate!`, run on boot).

## Settled invariants (fetch the DD before changing any of these)
- **No cross-store transactions.** Anything touching both boundaries (lock private facts here **and** publish an atproto record) is a dual write — a separate, retryable step (outbox-style), never one unit of work.
- **Transactions are a compile-enforced capability** — the Unit of Work is enforced by the type system, not by convention. → DD *Transactions as a capability — a compile-enforced Unit of Work in the private store* (`24150017`).
- Private vs public ownership of every fact → DD *Data Boundaries* (`10354698`); blob/PDS split → *Blobs, PDS & Private Storage* (`10125341`).

## Memory check
When a question arises about the private store, the data boundary, or transaction scope, **check the relevant memories + the DDs above (fetch via `docs/confluence-design-index.md`) before asserting.** Confluence DESIGN is the source of truth; this file only points.
