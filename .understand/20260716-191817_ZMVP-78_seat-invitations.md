# ZMVP-78 — Owner invites a User to a Seat

## Context

Seats exist (ZMVP-76: `commission_seat` with nullable `occupant`, born vacant) but **no fill path exists** — `occupant` is永NULL today. ZMVP-78 builds the owner-side entry into the DD 28311564 handshake: issue an invitation to a vacant Seat, revoke a pending one. Accept/decline (the ACK that actually seats) is ZMVP-79; applications are ZMVP-80/81/82 — all out of scope here. The account-invitation stack (ZMVP-32/20/40) is the settled template to mirror.

**Engineer rulings (2026-07-16, this session):** AC4 (reject Golem invitees) is **vacuous-by-construction** — Golems are unrepresentable in the current model (closed `ActorKind`, DD 34013187); doc comment at the invite site + ticket note, no dead code. **No changelog entries** in this ticket. **Already-participants (incl. the owner) may be invited** to a vacant seat — no extra check.

Stale remote branch `feature/zmvp-78-seat-invitations` (at `04522229`, zero 78-work) gets deleted and re-cut at main's tip, per the born-at-tip convention.

## Deliverables

- [ ] Migration (`just migrate-add create_commission_invitation`): table **`commission_invitation`** (singular per convention; the ticket text's plural mirrors legacy `account_invitations`): `id uuid PK, commission_id FK→commission ON DELETE CASCADE, seat_id FK→commission_seat, invited_user FK→users, inviter FK→users, state text, created_at, updated_at` + partial unique `ON (seat_id, invited_user) WHERE state='pending'` (multiple users may hold pending invites to one vacant seat — ZMVP-79's race ruling implies this; duplicates per pair may not).
- [ ] Domain: `SeatInvitation` element in `domain/src/elements/commission/` — `issue(...)` (pure builder → Pending) + `revoke(&mut, now)`; **reuse `InvitationState` + `InvitationError`** from `elements/invitation.rs` (same pending|accepted|revoked machine, no expiry).
- [ ] Ports: `CommissionStore::{find_seat_invitation, find_pending_seat_invitation(seat, user)}`; `CommissionWrites::{create_seat_invitation, revoke_seat_invitation}` (idempotent, mirroring account semantics).
- [ ] adapter-pg: `queries/commission/{create_seat_invitation,find_seat_invitation,find_pending_seat_invitation,revoke_seat_invitation}.sql` (v2 zero-annotation grammar — prose headers only) + regenerated `src/queries.rs` + store impl in `src/commission.rs` (reads on pool, writes on the UoW conn).
- [ ] adapter-mem: `StoredSeatInvitation` + port impls mirroring pg (`adapter-mem/src/commission.rs`).
- [ ] api (`routes/commissions/invitations.rs`, wired in `commissions_router`):
  - `POST /commissions/{id}/invitations` body `{seat, user(did)}` — `require_owner` gate; seat must exist on this commission (404 `node_not_found`) and be vacant (**new `Problem::seat_filled()` 409**); invitee provisioned idempotently by DID; idempotent re-invite → 200, fresh → 201. Doc comment: Golem rejection is vacuous until the Golem epic (AC4).
  - `DELETE /commissions/{id}/invitations` body `{seat, user}` — `require_owner`; idempotent 200 (unknown DID / nothing pending = no-op), mirrors account revoke.
- [ ] Tests (three layers, sentence-style names):
  - domain unit: issue builds pending recording its facts; revoke pending→revoked; revoking non-pending rejected.
  - adapter-pg (`tests/it/` — needs a `mod` line, guard enforces): create-then-find-pending; second pending per (seat,user) not a second row; two users pending on one seat coexist; revoke flips and clears pending.
  - api (`tests/commission_seat_invitations.rs`): owner invites → 201 + pending recorded; filled seat → 409 seat_filled (seed occupant directly); unknown seat → 404; cross-commission seat id → 404; re-invite idempotent → 200; revoke → 200; revoke-nothing idempotent → 200; participant-non-owner → 403; anonymous → 401.

## Work breakdown (owners/models per policy)

| Piece | Diff | Owner | Model |
|---|---|---|---|
| Migration + SQL + pg store impl | 2 | 🤖 | Opus (migration is on the security-trigger list) |
| SeatInvitation element + ports | 2 | 🤖 | Sonnet |
| adapter-mem fake | 1 | 🤖 | Sonnet |
| api handlers + Problem::seat_filled + router wiring | 2 (boilerplate mirror of accounts.rs) | 🤖 | Sonnet |
| Tests (all layers) | 2 | 🤖 | Sonnet |

Ticket build model = Opus (max over pieces). All pieces Claude-band; Engineer holds review + merge. If you want a lane for yourself, the api handlers are the most domain-visible slice — say so at approval.

## Verification

- Full gate: `cargo fmt --all --check` · `cargo clippy --workspace --all-targets --locked -- -D warnings` · `cargo test --workspace --locked` (template-DB harness; adapter-pg suite is the single `--test it` binary).
- End-to-end: the new api tests drive the real HTTP surface against `MemBackend`; pg round-trips run in `tests/it/`.
- ⚠️ Security-review disposition: authz-only change on existing guards (no auth/session/DID/boundary surface) — recommend a cheap Opus `/security-review` pass anyway before the PR; Engineer calls it.
- Ship: slice PR(s) into fresh `feature/zmvp-78-seat-invitations` (re-cut at main tip; delete the stale remote head), Copilot review, `/address-comments`, Engineer merges. Jira note recording the three rulings + AC4 vacuousness.
