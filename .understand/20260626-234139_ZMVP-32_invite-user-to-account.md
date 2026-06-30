# 🔎 Understanding ZMVP-32 — Owner invites a User to their Account

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-32 · **Generated:** 2026-06-26 23:41 · **Snapshot:** `.understand/20260626-234139_ZMVP-32_invite-user-to-account.md`

## 📊 Since last snapshot

Compared to `.understand/20260626-125706_ZMVP-32_invite-user-to-account.md` (2026-06-26 12:57).

- **Jira status:** In Progress → **In Progress** (unchanged). New comment added linking the design decision [API Response Shape & Error Model (RFC 9457)](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/23592962) and noting the error-shape migration is tracked in **ZMVP-35**, keeping ZMVP-32 scoped.
- **Both prior §7 divergences are now CLOSED:**
  - ✅ **Existing-member refusal 403 → 409.** Added a `conflict()` helper (`api/src/lib.rs:643`) and an explicit "already a member" check (`:712`), with a new e2e test `inviting_an_existing_member_is_a_conflict` (`invitations.rs:326`). This was the divergence the prior snapshot flagged; the user asked for it fixed, and it now matches the original §8 lean.
  - ✅ **Revoke response realigned for consistency.** Was `{ id, account, state }` on success / `{}` on the idempotent no-ops; now returns `{ account, user }` on **every** path via a shared `revoked()` closure (`:815`), mirroring `revoke_role` (always-available request inputs). The idempotent-200 + body-keyed-`{user}` shape was **confirmed and kept** by the user (no longer an open question).
- **Tests:** e2e 5 → **6** (added the conflict case); green count 14 → **15 of 20** (5 unit + 4 mem + 6 e2e green; 5 pg still un-run pending `DATABASE_URL`).
- **Uncommitted work:** 789 → **818 insertions** (lib.rs +29 for the conflict helper/check + revoke realignment + new test). Still **zero ZMVP-32 commits on the branch.**
- **New scope context:** the error body shape (`{ "error": "<string>" }`, including the `conflict()` helper just added) is now **known-stale** — superseded by the RFC 9457 problem+json decision and migrated wholesale under **ZMVP-35**, explicitly **out of scope** for ZMVP-32.
- **Net movement:** **All design questions are now settled** — the two lingering §7 divergences are resolved (409 fixed; revoke shape confirmed + cleaned). The remaining gap is unchanged and purely mechanical: run the 5 pg tests on a live DB, commit, and ship.

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
  - **Rule 1 (strict-rank):** only Owner/Admin act, and the offered role must sit **strictly below the actor's own rank**. This is *exactly* `Role::can_grant` (`domain/src/elements/role.rs:90-101`): `matches!(self, Owner|Admin) && target > self`. The invite-authority check **reuses** it — wired at `api/src/lib.rs:691`, `:847` and pinned by `invitation.rs:376` (`invite_authority_is_the_grant_rule`).
  - **Rule 4a (parent-by-invitation):** on acceptance the inviter becomes the invitee's Parent. That's *why* the invitation **records the inviter** — ZMVP-20 reads it back to set the parent edge.
- **Invitation** (new) — now a real domain element (`domain/src/elements/invitation.rs`), not yet a glossary entity. Value: `{ invited User, Account, offered Role, inviter, state ∈ {pending, accepted, revoked} }`. **No expiry** — valid until accepted or revoked.

## 🎯 3. Goal & scope

**Goal:** stand up a pending-invitation record plus its issue/revoke operations, authority-gated like a grant, idempotent at one-pending-per-(account,user), so ZMVP-20 has something to accept. **Status: achieved in code; all design questions settled.**

**In scope** *(all landed)*
- An `Invitation` domain element + `InvitationState` enum (`pending`/`accepted`/`revoked`). ✅
- Persistence: a new `account_invitations` table (pg) + mem mirror, with a **partial unique index** enforcing ≤1 pending per (account, user). ✅
- Issue path: authority check (reuse `can_grant`), reject if already a member (**409**), idempotent re-issue (return existing pending, no second row). ✅
- Revoke path: issuing member transitions pending → revoked; a revoked invite can't later be accepted. ✅
- HTTP handlers + DTOs mirroring the grant/revoke handlers; tests at every layer. ✅

**Out of scope** *(unchanged, plus one addition)*
- **Acceptance & onboarding** → ZMVP-20 (the `accepted` state exists in the enum but is never *written* here).
- **The actual "ping"/notification** — no notification subsystem on main; "pings them rather than a second row" reduces to *idempotent re-issue returning the existing pending invitation*.
- Role-tree Parent wiring (ZMVP-20 territory).
- Expiry (explicitly none).
- **NEW: the RFC 9457 error-body migration** → ZMVP-35. The endpoints keep the current `{ "error": "<string>" }` shape on this branch; problem+json is a separate sweep.

## 📦 4. Deliverables

- [x] `Invitation` struct + `InvitationId` (UUIDv7) + `InvitationState` enum — `domain/src/elements/invitation.rs:36-118`
- [x] An `Invitation::issue(...)` constructor enforcing invariants + a `revoke()` transition (pure domain) — `invitation.rs:217-234`, `265-272`
- [x] Port methods on `AccountRepo` — `create_invitation`, `find_invitation(id)`, `find_pending_invitation(account, user)`, `revoke_invitation(id)` — `domain/src/ports.rs:123-153`
- [x] Migration `…_create_account_invitations.sql` — table + partial unique index `WHERE state = 'pending'`
- [x] adapter-pg impl of the four methods — `adapter-pg/src/account.rs:216-319` *(tests un-run pending `DATABASE_URL`)*
- [x] adapter-mem impl (a `Mutex<HashMap<…>>` mirror) — `adapter-mem/src/lib.rs:227-455`
- [x] `POST /accounts/{id}/invitations` (issue) + `DELETE /accounts/{id}/invitations` (revoke, keyed by `{user}`) handlers + DTOs + route wiring — `api/src/lib.rs:266-269`, `665-772`, `774-869`; `conflict()` helper `:643`
- [x] Tests: domain unit, mem + pg adapter round-trips, api e2e (6 cases incl. existing-member 409)

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Invitation` element + `InvitationState` enum + `issue`/`revoke` | 2 | P0 | 🤖 Claude | ✅ `invitation.rs:36-272`; 5 unit tests green |
| Reuse `Role::can_grant` for invite authority | 1 | P0 | 🤖 Claude | ✅ wired `api/src/lib.rs:691`,`847`; pinned `invitation.rs:376` |
| Port methods on `AccountRepo` (4 fns) | 2 | P0 | 🤖 Claude | ✅ `ports.rs:123-153` (extended `AccountRepo`, not a new trait) |
| Migration: `account_invitations` + partial unique index | 3 | P1 | 🧑 Engineer | ✅ `…20260626065617_…sql`, `WHERE state = 'pending'` |
| adapter-pg impl | 3 | P1 | 🧑 Engineer | ✅ code `account.rs:216-319`; ⚠️ 5 tests un-run (needs `DATABASE_URL`) |
| adapter-mem impl | 2 | P1 | 🤖 Claude | ✅ `lib.rs:227-455`; 4 tests green |
| API issue/revoke handlers + DTO + routes | 3 | P0 | 🧑 Engineer | ✅ `lib.rs:643-869`; existing-member **409**; 6 e2e green |
| Tests (unit + adapter + e2e) | 3 | P0 | 👥 Group | ✅ 15/20 green; 5 pg tests authored, gated on DB |

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
- ✅ **E2E** — offering Owner-rank (≥ inviter rank) returns **403** → **AC2** (`invitations.rs:187`)
- ✅ **E2E** — re-inviting an already-pending User returns the **same** invitation (2xx), not a second row → **AC5** (`invitations.rs:221`)
- ✅ **E2E** — issuer revokes the pending invite; follow-up shows it can't be accepted → **AC4** (`invitations.rs:277`)
- ✅ **E2E** — inviting an **existing member** returns **409**, minting nothing → **AC1** (`invitations.rs:326`) *(new this snapshot; closes the prior §7 gap)*
- ✅ **E2E** — anonymous visitor cannot invite (401) (`invitations.rs:376`)

## 🧠 7. Logic & shape

```
POST /accounts/{id}/invitations          DELETE /accounts/{id}/invitations  (body: {user})
  resolve session → User (401)             resolve session → User (401)
  find account (404)                       find account (404)
  role_of(actor) (403 if non-member)       resolve invitee by DID (no mint)
  parse offered role (422)                 role_of(actor) (403 if non-member)
  resolve invitee by DID (provision)       find_pending_invitation(acct,user)
  ┌─ already a member? → 409 ─┐             ├─ exists → can_grant? (403) → revoke → 200
  └─ can_grant(offered)? (403)─┘            └─ none / unknown DID → idempotent 200 no-op
  find_pending_invitation(acct,user)        (every path returns { account, user })
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

**Settled shape decisions** (both prior §7 divergences resolved):
1. ✅ **Existing-member refusal is 409** (`conflict()` helper, `lib.rs:643`/`:712`) — a state conflict, not an authority 403. Matches the original lean.
2. ✅ **Revoke is idempotent 200, keyed by `{user}` in the body**, returning `{ account, user }` on every path (`revoked()` closure `:815`). Confirmed and kept by the user; the body now echoes only always-available request inputs, so the no-op paths are consistent with the success path.

⚠️ **Known-stale, deferred:** the error body is `{ "error": "<string>" }` (the `conflict()`/`forbidden()`/etc. helpers). Per the new DD it migrates to RFC 9457 `application/problem+json` under **ZMVP-35** — not on this branch.

## 🚀 8. Next steps

The build is feature-complete and all design questions are settled; what remains is verification and shipping.

1. **Run the pg adapter tests against a live DB** — `just up` (or a container runtime socket), then `just test` / `cargo test -p adapter-pg`. This closes the only 🟡 slice (5 tests) and validates the `sqlx::query!` macros against the migrated schema.
2. **`/prepare-pr`** — the branch carries **818 insertions of uncommitted work and zero ZMVP-32 commits**. Organize into granular commits, rebase on main, run the full local CI suite (fmt/clippy/test), open the PR targeting main.

**Decisions — all resolved.** Port home = extend `AccountRepo` ✅; authority = reuse `can_grant` ✅; invitee = DID + provision ✅; existing-member = **409** ✅; revoke = idempotent **200**, body-keyed, echoes `{ account, user }` ✅. Error-body standardization is **out of scope** (ZMVP-35).
