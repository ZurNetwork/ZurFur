# adapter-atproto — the public data boundary

AT Protocol adapter: **user-owned** records on the user's **PDS**, addressed by **AT-URI via DID**. This is the *public* side of the data boundary (the private side is `adapter-pg`).

## Settled invariants (fetch the DD before changing any of these)
- **`did:plc` everywhere** (not `did:web`); a Character carries its own DID from day one. → DD *DID:PLC vs DID:Web* (`4358151`).
- **No cross-store transactions.** Publishing an atproto record alongside private writes is a dual write — a separate, retryable step (outbox-style), never one unit of work.
- What is user-owned (public PDS) vs app-owned (private pg) → DD *Data Boundaries* (`10354698`); who operates DIDs / platform authority → *Platform Authority* (`9207856`); blob/PDS split → *Blobs, PDS & Private Storage* (`10125341`).

## Memory check
When a question arises about the public store, PDS records, DIDs/handles, or the data boundary, **check the relevant memories + the DDs above (fetch via `docs/confluence-design-index.md`) before asserting.** Confluence DESIGN is the source of truth; this file only points.
