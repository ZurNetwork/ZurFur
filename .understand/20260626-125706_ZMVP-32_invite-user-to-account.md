# 🔎 Understanding ZMVP-32 — Owner invites a User to their Account

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-32 · **Generated:** 2026-06-26 12:57 · **Snapshot:** `.understand/20260626-125706_ZMVP-32_invite-user-to-account.md`

## 📊 Since last snapshot

Compared to `.understand/20260625-192829_ZMVP-32_invite-user-to-account.md` (2026-06-25 19:28).

- **Jira status:** In Progress → **In Progress** (unchanged; ticket last updated 2026-06-25, no new comments).
- **Done transitions — the whole work breakdown moved ⬜ → ✅:**
  - `Invitation` element + `InvitationState` + `issue`/`revoke`: ⬜ → ✅ (`domain/src/elements/invitation.rs`, wired at `elements.rs:32`)
  - Reuse `Role::can_grant`: ⬜ wired → ✅ wired (handler `api/src/lib.rs:691`, `:813`; pinned by test `invitation.rs:376`)
  - Port methods on `AccountRepo`: ⬜ → ✅ (`ports.rs:123-153`, extended `AccountRepo` as recommended)
  - Migration + partial unique index: ⬜ → ✅ (`…20260626065617_create_account_invitations.sql`, `WHERE state = 'pending'`)
  - adapter-pg impl: ⬜ → ✅ code-complete (`account.rs:216-319`); tests gated on `DATABASE_URL`
  - adapter-mem impl: ⬜ → ✅ (`lib.rs:227-455`, tests green)
  - API handlers + DTOs + routes: ⬜ → ✅ (`lib.rs:266-269`, `657-750`, `757-840`)
  - Tests: ⬜ → ✅ 14/19 green (5 domain + 4 mem + 5 e2e); 5 pg tests authored but un-run pending DB.
- **Decisions resolved** (all four §8 open questions closed):
  - Port home → **extended `AccountRepo`** ✅ (as recommended).
  - Authority → **reused `can_grant`** ✅ (as recommended).
  - Invitee identity → **DID + provision** ✅ (as recommended).
  - ⚠️ **Status-code decisions diverged from the prior lean:** existing-member refusal is **403** (snapshot leaned 409); revoking a non-pending/unknown invite is an **idempotent 200** (snapshot leaned 409). Both choices mirror the `grant_role`/`revoke_role` precedent rather than treating these as state conflicts — a defensible, consistent call, but a deviation from §8.
  - ⚠️ **Revoke shape diverged:** revoke is `DELETE /accounts/{id}/invitations` keyed by `{user}` in the body (`RevokeInvitationBody`), **not** the planned `DELETE …/invitations/{invite_id}` path. Worth a sanity check against REST conventions, but it keeps the issue/revoke pair symmetric on (account, user).
- **Net movement:** **All 8 pieces implemented in one pass; 14 of 19 tests green.** Remaining gap is mechanical, not design: run the 5 pg adapter tests against a live DB, then commit (the branch has **789 insertions of uncommitted work and zero ZMVP-32 commits**) and ship via `/prepare-pr`.

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

**This ticket is the *issuing seam* only**: create a pending invitation and revoke it. It does **not** create membership — acceptance and its onboarding effects are ZMVP-20. ZMVP-32 **blocks** ZMVP-20, so the data shape landed here is the contract ZMVP-20 consumes.

## 🗺️ 2. Domain

- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081)** — the aggregate root. Membership is the join `UserAccount(UserId, AccountId, Role)`.
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — `Owner < Admin < Manager < Member` (variant order = rank; lower position = higher authority). Two rules bear directly on this ticket:
  - **Rule 1 (strict-rank):** only Owner/Admin act, and the offered role must sit **strictly below the actor's own rank**. This is *exactly* `Role::can_grant` (`domain/src/elements/role.rs:90-101`): `matches!(self, Owner|Admin) && target > self`. The invite-authority check **reuses** it — confirmed wired at `api/src/lib.rs:691` and pinned by `invitation.rs:376` (`invite_authority_is_the_grant_rule`).
  - **Rule 4a (parent-by-invitation):** on acceptance the inviter becomes the invitee's Parent. That's *why* the invitation **records the inviter** — ZMVP-20 reads it back to set the parent edge.
- **Invitation** (new) — now a real domain element (`domain/src/elements/invitation.rs`), not yet a glossary entity. Value: `{ invited User, Account, offered Role, inviter, state ∈ {pending, accepted, revoked} }`. **No expiry** — valid until accepted or revoked.

## 🎯 3. Goal & scope

**Goal:** stand up a pending-invitation record plus its issue/revoke operations, authority-gated like a grant, idempotent at one-pending-per-(account,user), so ZMVP-20 has something to accept. **Status: achieved in code.**

**In scope** *(all landed)*
- An `Invitation` domain element + `InvitationState` enum (`pending`/`accepted`/`revoked`). ✅
- Persistence: a new `account_invitations` table (pg) + mem mirror, with a **partial unique index** enforcing ≤1 pending per (account, user). ✅
- Issue path: authority check (reuse `can_grant`), reject if already a member, idempotent re-issue (return existing pending, no second row). ✅
- Revoke path: issuing member transitions pending → revoked; a revoked invite can't later be accepted. ✅
- HTTP handlers + DTOs mirroring the grant/revoke handlers; tests at every layer. ✅

**Out of scope** *(unchanged)*
- **Acceptance & onboarding** → ZMVP-20 (the `accepted` state exists in the enum but is never *written* here).
- **The actual "ping"/notification** — no notification subsystem on main; "pings them rather than a second row" reduces to *idempotent re-issue returning the existing pending invitation*.
- Role-tree Parent wiring (ZMVP-20 territory).
- Expiry (explicitly none).

## 📦 4. Deliverables

- [x] `Invitation` struct + `InvitationId` (UUIDv7) + `InvitationState` enum — `domain/src/elements/invitation.rs:36-118`
- [x] An `Invitation::issue(...)` constructor enforcing invariants + a `revoke()` transition (pure domain) — `invitation.rs:217-234`, `265-272`
- [x] Port methods on `AccountRepo` — `create_invitation`, `find_invitation(id)`, `find_pending_invitation(account, user)`, `revoke_invitation(id)` — `domain/src/ports.rs:123-153`
- [x] Migration `…_create_account_invitations.sql` — table + partial unique index `WHERE state = 'pending'`
- [x] adapter-pg impl of the four methods — `adapter-pg/src/account.rs:216-319` *(tests un-run pending `DATABASE_URL`)*
- [x] adapter-mem impl (a `Mutex<HashMap<…>>` mirror) — `adapter-mem/src/lib.rs:227-455`
- [x] `POST /accounts/{id}/invitations` (issue) + `DELETE /accounts/{id}/invitations` (revoke, keyed by `{user}`) handlers + DTOs + route wiring — `api/src/lib.rs:266-269`, `657-750`, `757-840`
- [x] Tests: domain unit, mem + pg adapter round-trips, api e2e

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Invitation` element + `InvitationState` enum + `issue`/`revoke` | 2 | P0 | 🤖 Claude | ✅ `invitation.rs:36-272`; 5 unit tests green |
| Reuse `Role::can_grant` for invite authority | 1 | P0 | 🤖 Claude | ✅ wired `api/src/lib.rs:691`,`813`; pinned `invitation.rs:376` |
| Port methods on `AccountRepo` (4 fns) | 2 | P0 | 🤖 Claude | ✅ `ports.rs:123-153` (extended `AccountRepo`, not a new trait) |
| Migration: `account_invitations` + partial unique index | 3 | P1 | 🧑 Engineer | ✅ `…20260626065617_…sql`, `WHERE state = 'pending'` |
| adapter-pg impl | 3 | P1 | 🧑 Engineer | ✅ code `account.rs:216-319`; ⚠️ 5 tests un-run (needs `DATABASE_URL`) |
| adapter-mem impl | 2 | P1 | 🤖 Claude | ✅ `lib.rs:227-455`; 4 tests green |
| API issue/revoke handlers + DTO + routes | 3 | P0 | 🧑 Engineer | ✅ `lib.rs:651-840`; 5 e2e green |
| Tests (unit + adapter + e2e) | 3 | P0 | 👥 Group | ✅ 14/19 green; 5 pg tests authored, gated on DB |

*Everything Claude-owned is green. The Engineer-boundary pieces (migration, adapter-pg, handlers) are written; the only un-executed slice is the pg adapter test run against a live database.*

## ✅ 6. Test checklist (TDD)

All authored; ✅ = passing, 🟡 = written but un-run (pg, pending `DATABASE_URL`).

- ✅ **Unit** — `can_grant` permits Owner→{Admin,Manager,Member}, Admin→{Manager,Member}, rejects peer/above + Manager/Member-as-inviter → **AC2** (`invitation.rs:376`)
- ✅ **Unit** — a freshly issued `Invitation` is `pending` and carries its four facts → **AC3** (`invitation.rs:292`)
- ✅ **Unit** — `revoke()` moves `pending → revoked`; revoking a non-pending invite is rejected → **AC4** (`invitation.rs:310`, `:328`)
- ✅ **Unit** — state round-trips through its discriminant (`invitation.rs:355`)
- ✅/🟡 **Integration (mem ✅ / pg 🟡)** — `create_invitation` then `find_pending_invitation` round-trips → **AC3** (`lib.rs:614`, `account.rs:185`)
- ✅/🟡 **Integration** — a second `create_invitation` for the same (account, user) while pending does **not** create a second row → **AC5** (`lib.rs:630`, `account.rs:216`)
- ✅/🟡 **Integration** — `revoke_invitation` flips state; a subsequent read is no longer `pending` → **AC4** (`lib.rs:653`, `account.rs:253`)
- ✅ **E2E** — Owner `POST …/invitations` for a non-member returns **201** with the pending body → **AC1** (`invitations.rs:135`)
- ✅ **E2E** — Manager/Member inviter, or offered role ≥ inviter rank, returns **403** → **AC2** (`invitations.rs:187`)
- ✅ **E2E** — re-inviting an already-pending User returns the **same** invitation (200), not a second row → **AC5** (`invitations.rs:221`)
- ✅ **E2E** — issuer revokes the pending invite; follow-up shows it can't be accepted → **AC4** (`invitations.rs:277`)
- ✅ **E2E** — anonymous visitor cannot invite (401) (`invitations.rs:325`)
- ⚠️ **Gap vs. plan** — the planned "inviting an existing member → clear refusal" e2e landed as **403** (not 409); see §7.

## 🧠 7. Logic & shape

```
POST /accounts/{id}/invitations          DELETE /accounts/{id}/invitations  (body: {user})
  resolve session → User (401)             resolve session → User (401)
  find account (404)                       find account (404)
  role_of(actor) (403 if non-member)       resolve invitee by DID
  parse offered role (422)                 actor has can_grant authority (403)
  resolve invitee by DID (provision)       find_pending_invitation(acct,user)
  ┌─ already a member? → 403 ─┐             ├─ exists → revoke_invitation → 200
  └─ can_grant(offered)? (403)─┘            └─ none   → idempotent no-op  → 200
  find_pending_invitation(acct,user)
   ├─ exists → return it (idempotent, 200)
   └─ none   → Invitation::issue(...) → create_invitation → 201
```

State machine (this ticket writes only the **solid** edges):

```
        issue                 revoke
   ∅ ─────────▶ pending ──────────────▶ revoked   (terminal)
                   │
                   └····▶ accepted   (dotted = ZMVP-20, not here)
```

**Idempotency** is belt-and-suspenders: handler checks `find_pending_invitation` first; the DB backstops with the partial unique index `(account_id, invited_user) WHERE state = 'pending'`.

⚠️ **Two implemented choices diverge from the prior snapshot's §8 lean** — both follow the `grant_role`/`revoke_role` precedent (mirror, not conflict):
1. **Existing-member refusal is 403, not 409.** Treated as an authority/state seam consistent with grant, rather than an HTTP state conflict.
2. **Revoke is idempotent 200** (already-revoked / never-invited → 200 no-op), and is keyed by `{user}` in the body at `DELETE …/invitations` rather than by `{invite_id}` in the path. Symmetric with issue, but worth a deliberate confirm — a path-keyed `DELETE …/{invite_id}` is the more REST-conventional shape, and a 409 on existing-member is arguably clearer than 403.

## 🚀 8. Next steps

The build is feature-complete; what remains is verification and shipping.

1. **Run the pg adapter tests against a live DB** — `just up` (or a container runtime socket), then `just test` / `cargo test -p adapter-pg`. This closes the only 🟡 slice (5 tests) and validates the `sqlx::query!` macros against the migrated schema.
2. **Confirm the two diverged decisions** (existing-member 403 vs 409; idempotent-200 + body-keyed revoke vs path-keyed `{invite_id}`). If the 403/200/body shape stands, no code changes; otherwise adjust handlers + e2e.
3. **`/prepare-pr`** — the branch carries **789 insertions of uncommitted work and zero ZMVP-32 commits**. Organize into granular commits, rebase on main, run the full local CI suite (fmt/clippy/test), open the PR targeting main.

**Decisions — all four prior open questions are now resolved in code** (port home = extend `AccountRepo` ✅; authority = reuse `can_grant` ✅; invitee = DID + provision ✅; status codes = grant-mirrored 403/200, a deviation from the 409 lean — confirm and move on).
