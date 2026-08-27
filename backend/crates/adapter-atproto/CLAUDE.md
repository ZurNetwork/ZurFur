# adapter-atproto — the public data boundary

AT Protocol adapter: **user-owned** records on the user's **PDS**, addressed by **AT-URI via DID**. This is the *public* side of the data boundary (the private side is `adapter-pg`).

## Settled invariants (fetch the DD before changing any of these)
- **`did:plc` everywhere** DIDs are minted (not `did:web`). For **Characters, publicness — not kind — determines DID presence** (2026-08-23, DD `54427650` *Characters on ATProto*, superseding the 07-14 "no DID" absolute): a PUBLIC Character carries its own did:plc (+ post-alpha a full PDS repo); a PRIVATE Character has no ATProto footprint — `did` stays NULLABLE on `actor_identity` (`34013187`, schema unchanged). Keeper = ACP `owner`; source of record = vulpes `docs/characters-atproto.md`. → DD *DID:PLC vs DID:Web* (`4358151`); *Identities — the Actor Super-Table* (`34013187`).
- **No cross-store transactions.** Publishing an atproto record alongside private writes is a dual write — a separate, retryable step (outbox-style), never one unit of work.
- What is user-owned (public PDS) vs app-owned (private pg) → DD *Data Boundaries* (`10354698`); who operates DIDs / platform authority → *Platform Authority* (`9207856`); blob/PDS split → *Blobs, PDS & Private Storage* (`10125341`).

## Memory check
When a question arises about the public store, PDS records, DIDs/handles, or the data boundary, **check the relevant memories + the DDs above (fetch via `docs/confluence-design-index.md`) before asserting.** Confluence DESIGN is the source of truth; this file only points.
