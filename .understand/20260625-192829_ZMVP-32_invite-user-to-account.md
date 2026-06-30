# 🔎 Understanding ZMVP-32 — Owner invites a User to their Account

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-32 · **Generated:** 2026-06-25 19:28 · **Snapshot:** `.understand/20260625-192829_ZMVP-32_invite-user-to-account.md`

## 🧭 1. Context (cold-start)

Today, seating a member is **synchronous**: an Owner/Admin names a DID and `grant_role` upserts the membership immediately (ZMVP-15). That's fine for someone already in your orbit, but per [1DD decision 11](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/21594113), joining someone else's Account is *consequential* (shared brand, wallet, plugin entitlements, authority) and must be **consensual**. So membership splits into a two-step **invite-then-accept** handshake:

```
  ZMVP-32 (this)                          ZMVP-20 (blocked by this)
  ┌─────────────────────┐                 ┌──────────────────────────┐
  │ Owner/Admin ISSUES  │  pending invite │ invited User ACCEPTS     │
  │ a pending invitation│ ──────────────▶ │ → membership + onboarding│
  │ …or REVOKES it      │                 │ (inviter becomes Parent) │
  └─────────────────────┘                 └──────────────────────────┘
```

**This ticket is the *issuing seam* only**: create a pending invitation and revoke it. It does **not** create membership — acceptance and its onboarding effects are ZMVP-20. ZMVP-32 **blocks** ZMVP-20, so the data shape we land here is the contract ZMVP-20 consumes.

## 🗺️ 2. Domain

- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081)** — the aggregate root. Membership is the join `UserAccount(UserId, AccountId, Role)` (`domain/src/elements/user_account.rs:20`).
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — `Owner < Admin < Manager < Member` (variant order = rank; lower position = higher authority). Two rules bear directly on this ticket:
  - **Rule 1 (strict-rank):** only Owner/Admin act, and the offered role must sit **strictly below the actor's own rank**. This is *exactly* `Role::can_grant` (`domain/src/elements/role.rs:90`): `matches!(self, Owner|Admin) && target > self`. The invite-authority check **reuses** it — no new rule.
  - **Rule 4a (parent-by-invitation):** on acceptance the inviter becomes the invitee's Parent. That's *why* the invitation must **record the inviter** now — ZMVP-20 reads it back to set the parent edge.
- **Invitation** (new) — not yet a glossary entity. Value: `{ invited User, Account, offered Role, inviter, state ∈ {pending, accepted, revoked} }`. **No expiry** — valid until accepted or revoked.

## 🎯 3. Goal & scope

**Goal:** stand up a pending-invitation record plus its issue/revoke operations, authority-gated like a grant, idempotent at one-pending-per-(account,user), so ZMVP-20 has something to accept.

**In scope**
- An `Invitation` domain element + `InvitationState` enum (`pending`/`accepted`/`revoked`).
- Persistence: a new `account_invitations` table (pg) + mem mirror, with a **partial unique index** enforcing ≤1 pending per (account, user).
- Issue path: authority check (reuse `can_grant`), reject if already a member, idempotent re-issue (return existing pending, no second row).
- Revoke path: issuing member transitions pending → revoked; a revoked invite can't later be accepted.
- HTTP handlers + DTOs mirroring the grant/revoke handlers; tests at every layer.

**Out of scope**
- **Acceptance & onboarding** → ZMVP-20 (we add the `accepted` state to the enum but never *write* it here).
- **The actual "ping"/notification** — there is no notification subsystem on main. "Pings them rather than a second row" reduces, for us, to *idempotent re-issue returning the existing pending invitation*; the user-facing ping is a no-op stub / future ticket.
- Role-tree Parent wiring (the `parent` column is still `NULL` on main; ZMVP-20 territory).
- Expiry (explicitly none).

## 📦 4. Deliverables

- [ ] `Invitation` struct + `InvitationId` (UUIDv7) + `InvitationState` enum — `domain/src/elements/invitation.rs`
- [ ] An `Invitation::issue(...)` constructor that enforces invariants, and a `revoke()` state transition (pure domain)
- [ ] Port methods (on `AccountRepo`) — `create_invitation`, `find_invitation(id)`, `find_pending_invitation(account, user)`, `revoke_invitation(id)` — `domain/src/ports.rs`
- [ ] Migration `…_create_account_invitations.sql` — table + partial unique index `WHERE state = 'pending'`
- [ ] adapter-pg impl of the four methods — `adapter-pg/src/account.rs`
- [ ] adapter-mem impl (a `Mutex<HashMap<…>>` mirror) — `adapter-mem/src/lib.rs`
- [ ] `POST /accounts/{id}/invitations` (issue) + `DELETE /accounts/{id}/invitations/{invite_id}` (revoke) handlers + `InviteBody` DTO + route wiring — `api/src/lib.rs`
- [ ] Tests: domain unit, mem + pg adapter round-trips, api e2e

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Invitation` element + `InvitationState` enum + `issue`/`revoke` | 2 | P0 | 🤖 Claude | ⬜ — no `invitation` on main (only `commission.rs` `invited_by`, unrelated) |
| Reuse `Role::can_grant` for invite authority | 1 — rule is identical | P0 | 🤖 Claude | ✅ rule exists (`role.rs:90`); ⬜ wired for invite |
| Port methods on `AccountRepo` (4 fns) | 2 | P0 | 🤖 Claude | ⬜ `ports.rs:86` has only create/find/role_of/grant/revoke |
| Migration: `account_invitations` + partial unique index | 3 — schema choice has blast radius | P1 | 🧑 Engineer | ⬜ migrations dir has no invitations table |
| adapter-pg impl | 3 | P1 | 🧑 Engineer | ⬜ mirrors `account.rs:134-172` upsert/delete |
| adapter-mem impl | 2 | P1 | 🤖 Claude | ⬜ mirrors `lib.rs:306-329` |
| API issue/revoke handlers + DTO + routes | 3 — auth + idempotency + status mapping | P0 | 🧑 Engineer | ⬜ route table `lib.rs:252`; mirror grant `662-744` |
| Tests (unit + adapter + e2e) | 3 | P0 | 👥 Group | ⬜ mirror `role.rs:117`, `tests/account.rs`, `tests/accounts.rs` |

*Difficulty bands put the migration + adapter-pg + handlers at the Engineer boundary (real schema/HTTP blast radius); the pure-domain + mem pieces are Claude-owned; the test matrix is shared.*

## ✅ 6. Test checklist (TDD)

- **Unit** — _asserts that_ `inviter_role.can_grant(&offered)` permits Owner→{Admin,Manager,Member} and Admin→{Manager,Member}, and rejects peer/above and Manager/Member-as-inviter → **AC2** *(largely covered by the existing `can_grant` matrix `role.rs:117`; add an invite-framed case)*
- **Unit** — _asserts that_ a freshly issued `Invitation` is `pending` and carries invited-User, Account, offered-role, inviter → **AC3**
- **Unit** — _asserts that_ `revoke()` moves `pending → revoked` and that revoking a non-pending invite is rejected → **AC4**
- **Integration (adapter, pg+mem)** — _asserts that_ `create_invitation` then `find_pending_invitation(account, user)` round-trips the row → **AC3**
- **Integration (adapter, pg)** — _asserts that_ a second `create_invitation` for the same (account, user) while one is pending does **not** create a second row (partial unique index / upsert) → **AC5**
- **Integration (adapter)** — _asserts that_ `revoke_invitation` flips state and a subsequently-read invite is no longer `pending` → **AC4**
- **E2E** — _asserts that_ Owner `POST /accounts/{id}/invitations` for a non-member returns 201 with the pending invite body → **AC1**
- **E2E** — _asserts that_ a Manager/Member inviter, or an offered role ≥ inviter rank, returns 403 → **AC2**
- **E2E** — _asserts that_ inviting an existing member returns a clear refusal (409/422), minting nothing → **AC1**
- **E2E** — _asserts that_ re-inviting an already-pending User returns the **same** invitation, not a second row → **AC5**
- **E2E** — _asserts that_ the issuer `DELETE`s the pending invite (→ revoked), and a follow-up read shows it can't be accepted → **AC4**

## 🧠 7. Logic & shape

```
POST /accounts/{id}/invitations          DELETE /accounts/{id}/invitations/{invite_id}
  resolve session → User (401)             resolve session → User (401)
  find account (404)                       find account (404)
  role_of(actor) (403 if non-member)       find_invitation (404)
  parse offered role (422)                 actor is the inviter / has authority (403)
  resolve invitee by DID (provision)       state must be pending (409 if not)
  ┌─ already a member? → 409 ─┐            revoke_invitation(id)  // pending → revoked
  └─ can_grant(offered)? (403)─┘           200
  find_pending_invitation(acct,user)
   ├─ exists → return it (idempotent ping, 200)
   └─ none   → Invitation::issue(...) → create_invitation → 201
```

State machine (this ticket writes only the **solid** edges):

```
        issue                 revoke
   ∅ ─────────▶ pending ──────────────▶ revoked   (terminal)
                   │
                   └····▶ accepted   (dotted = ZMVP-20, not here)
```

**Idempotency** is belt-and-suspenders: handler checks `find_pending_invitation` first, and the DB backstops with a partial unique index `(account_id, invited_user_id) WHERE state = 'pending'`.

## 🚀 8. Next steps

1. **Start with the pure domain** (`Invitation`, `InvitationState`, `issue`/`revoke`) and its unit tests — red first. Cheapest, zero-I/O, unblocks the port signatures.
2. Add the four `AccountRepo` methods + mem impl, then the pg migration + impl with the partial unique index.
3. Wire the two handlers + DTO + routes, then the e2e matrix.

**Decisions needed (non-blocking — recommendations noted, will confirm at `/implement`):**
- ⚠️ **Port home:** extend `AccountRepo` with the four invitation methods vs. a new `InvitationRepo` trait. **Recommend extending `AccountRepo`** — invitations are account-scoped, there's one real consumer, and per the repo's "traits only when polymorphism is consumed" norm a separate trait isn't earned yet.
- ⚠️ **Authority check:** add a `can_invite` vs. reuse `can_grant`. **Recommend reusing `can_grant`** — AC2's rule is byte-for-byte the grant rule; a second method would be a lie-by-duplication.
- **Invitee identity at issue time:** invited User may never have visited. **Recommend mirroring grant** — accept a DID and `provision()` idempotently so the invite references a real `UserId`.
- **"Existing member" refusal status:** 409 Conflict vs 422. Lean **409** (state conflict, not malformed input); confirm against the repo's existing status conventions.
