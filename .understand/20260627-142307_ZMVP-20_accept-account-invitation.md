# 🔎 Understanding ZMVP-20 — User accepts an invitation to join an Account

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-20 · **Generated:** 2026-06-27 14:23 · **Snapshot:** `.understand/20260627-142307_ZMVP-20_accept-account-invitation.md`

## 📊 Since last snapshot

Compared with `.understand/20260627-025310_ZMVP-20_accept-account-invitation.md` (2026-06-27 **02:53**, ~11½h earlier). Jira status **unchanged** (In Progress → In Progress). Work has started on the branch — **320 insertions across 5 files**, none committed yet (no new commit since `a243ad0` ZMVP-35).

**Done transitions** — the two 🤖 **Claude-owned (difficulty ≤ 2)** pieces landed; every 🧑 engineer-owned piece is still ⬜:

- `Invitation::accept()` transition + unit tests — **⬜ → ✅** (`invitation.rs:304-311`; tests at `:368`, `:386`)
- Decline endpoint + handler — **⬜ → ✅** (`api/src/lib.rs:273-276` route, `:823-851` handler; e2e at `invitations.rs:408`, `:451`)
- `no_pending_invitation()` problem constructor — **⬜ → ✅** (new, `problem.rs:117-125`)
- Accept e2e test — **⬜ → 🟡** scaffolded as a red anchor: `invitee_accepts_and_becomes_a_member` is `#[ignore]`d (`invitations.rs:493`)
- Parent-edge write · `listed_on_profile` migration · atomic `accept_invitation` port+adapters · `/accept` endpoint — **still ⬜** (port carries a new `TODO(ZMVP-20 — engineer-owned)` marker at `ports.rs:155-163`; no adapter-pg/adapter-mem/migration changes in the diff at all)

**Net movement:** the consent *shape* is in (pure transition, decline path, error type, ignored accept test) — but **0% of the persistence + accept-endpoint** exists. This is the classic /implement split: the 0–3 band is green, the difficulty-4 structural firsts (parent column + profile flag + composite txn) are untouched and are the whole remaining job. 3 of 7 breakdown rows done; the 4 hardest remain.

## 🧭 1. Context (cold-start)

Membership is consensual: a User joins someone else's Account only by saying **yes**. ZMVP-32 built the *issuing* seam (an Owner/Admin issues a pending `Invitation`, or revokes it). This ticket is the **accept** seam — the other half of invite-then-accept. When the invited User accepts, three things happen at once: the invitation flips `pending → accepted`, a **membership** is minted at the offered role, and — per [1DD decision 11](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/21594113) — a short **onboarding** step records this membership's *"list on profile?"* choice.

```
  ZMVP-32 (done, on main)                  ZMVP-20 (this)
  ┌─────────────────────┐  pending offer  ┌────────────────────────────────┐
  │ Owner/Admin ISSUES  │ ───────────────▶│ invited User ACCEPTS           │
  │ a pending invitation│                 │  → invitation pending→accepted │
  └─────────────────────┘                 │  → membership @ offered role   │
                                          │  → inviter becomes Parent (4a) │
                                          │  → record "list on profile?"   │
                                          └────────────────────────────────┘
```

Two things land here that ZMVP-32/ZMVP-14 **deliberately deferred**: the **role-tree parent edge** (the `parent` column has existed but was always `NULL`) and the **per-membership "list on profile?" flag** (doesn't exist yet). So this is more structural than "the accept half" suggests — and these two firsts are exactly what's still unbuilt.

## 🗺️ 2. Domain

- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081) / membership** — the join `UserAccount(UserId, AccountId, Role)` (`domain/src/elements/user_account.rs:20`), persisted in `account_members` (PK `(account_id, user_id)`, columns `role`, `parent`).
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — `Owner < Admin < Manager < Member`. Two rules bear on this ticket:
  - **Rule 4a (parent-by-invitation):** *"when a member invites a new member, the inviter becomes the invitee's Parent in the hierarchy tree at the invitee's initial role."* This is the edge ZMVP-20 writes — the invitation already records `inviter` precisely so acceptance can set it. **Still unwritten** — both adapters explicitly defer it (`adapter-pg/account.rs:133-139`, `adapter-mem/lib.rs:337`).
  - **Rule 5:** *"an Owner never has a parent."* Not triggered here (acceptance only mints Admin/Manager/Member), but it bounds the parent model.
  - The `Role` enum carries a parent slot today — `Role::Member(Option<String>)` (`role.rs:24-33`) — but it's **always `None` on the floor** ("deferred dressing"), and the slot is a `String`, not a `UserId`. ZMVP-20 is the first writer of that slot.
- **Invitation** (`domain/src/elements/invitation.rs`) — now has a working **`accept()`** (`:304-311`, mirrors `revoke()`): guards `Pending` (`InvitationError::NotPending`), flips to `Accepted`, stamps `updated_at`. `Accepted` is finally written by code. Carries `invited_user`, `account`, `role`, `inviter`.
- **"List on profile?"** — per decision 11, memberships list on the User-Profile **by default**; onboarding sets the per-membership **opt-out**. No such flag exists anywhere today (`profile.rs` is PDS-only; no `account_members.listed_on_profile` column). New surface — **not yet added**.

## 🎯 3. Goal & scope

**Goal:** give an invited User a way to accept their pending invitation, which atomically (a) marks the invitation accepted, (b) mints their membership at the offered role with the **inviter as Parent**, and (c) records the membership's **"list on profile?"** choice — and guarantees a revoked/absent invitation yields no membership.

**In scope**
- `Invitation::accept(now)` pure domain transition (`pending → accepted`, mirrors `revoke()`). ✅ **done** (`invitation.rs:304-311`).
- An atomic **`accept_invitation`** port op: flip invitation → accepted **and** create the membership, in one private-store transaction (never half-done). ⬜ TODO-marked only (`ports.rs:155-163`).
- **Writing the parent edge**: the membership write persists `parent = inviter` (first real use of the `account_members.parent` column / the `Role` parent slot). ⬜.
- **"list on profile?"** persisted per membership (new `account_members` column, default = listed) set at acceptance. ⬜. **[DECIDED: in scope]**
- `POST /accounts/{id}/invitations/accept` (⬜) + `POST /accounts/{id}/invitations/decline` (✅ `lib.rs:823-851`) handlers + DTO + routes; authority = *only the invited User acts on their own invite* (implicit via session-user lookup). **[DECIDED]**
- **Decline** = the invitee actively kills their own pending offer (reuses `Invitation::revoke()` pending→revoked and the existing `revoke_invitation` port — so just a new endpoint/handler, no new domain/state). ✅ **done**. **[DECIDED: in scope — "revoking is an active thing"]**
- adapter-pg + adapter-mem impls; tests at every layer. ⬜ adapter impls + round-trips outstanding.

**Out of scope**
- **Issuing/(issuer-)revoking** invitations → ZMVP-32 (done).
- **The full role-tree mechanics** — rule 2 (parent-may-demote), rule 3 (re-parent children on removal), promotion-sets-parent (rule 4b). ZMVP-20 writes *one* parent edge; the tree algebra is later tickets.
- **A distinct `Declined` state** — decline reuses the existing `Revoked` terminal state (identical effect: can't be accepted, can be re-invited). Splitting issuer-revoked vs invitee-declined for audit is a future refinement + DD update.
- **Backfilling ZMVP-14's founder onboarding** — account-creation also owes a "list on profile?" step per decision 11, but that's a separate gap; the founder row defaults to listed.
- **Notifications / "you've been invited"** — no notification subsystem exists; discovery is out-of-band.
- **Strongly typing the parent** as `UserId` instead of `String` — keep the existing slot type (resolved in §8: store the string form).

## 📦 4. Deliverables

- [x] `Invitation::accept(now)` → `Result<(), InvitationError>` (pending→accepted) — `domain/src/elements/invitation.rs:304-311`
- [ ] `account_members` migration: add `listed_on_profile BOOLEAN NOT NULL DEFAULT true` — `adapter-pg/migrations/`
- [ ] Port method `accept_invitation(invitation, membership, listed_on_profile)` (atomic) on `AccountRepo` — `domain/src/ports.rs` (TODO at `:155-163`)
- [ ] Membership write persists the **parent** edge (extend `grant_role`/the accept write so the `Role` parent slot reaches the `parent` column) — both adapters
- [ ] adapter-pg impl (one transaction: `UPDATE invitation SET state='accepted'` + `INSERT account_members` with role/parent/listed_on_profile) + `.sqlx` cache — `adapter-pg/src/account.rs`
- [ ] adapter-mem impl mirroring it — `adapter-mem/src/lib.rs`
- [ ] `POST /accounts/{id}/invitations/accept` handler + DTO (`{ list_on_profile?: bool }`) + route — `api/src/lib.rs`
- [x] `POST /accounts/{id}/invitations/decline` handler + route (invitee-keyed; reuses `revoke_invitation`) — `api/src/lib.rs:273-276`, `:823-851`
- [x] `no_pending_invitation()` 404 problem constructor — `api/src/problem.rs:117-125`
- [ ] Tests: domain unit (✅ `invitation.rs:368`, `:386`), mem + pg adapter round-trips (⬜), api e2e (decline ✅ `invitations.rs:408`,`:451`; accept 🟡 `#[ignore]`d `:493`)

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Invitation::accept()` transition + unit test | 2 — mirrors `revoke()` | P0 | 🤖 Claude | ✅ done — `invitation.rs:304-311`; tests `:368`, `:386` |
| Persist the **parent edge** (Role slot → `parent` column) | 4 — first writer; touches membership serialization in both adapters | P0 | 🧑 Engineer | ⬜ `parent` still NULL; both adapters defer (`adapter-pg/account.rs:133-139`, `adapter-mem/lib.rs:337`) |
| `account_members.listed_on_profile` migration + read/write | 3 — schema blast radius | P1 | 🧑 Engineer | ⬜ no such column/flag anywhere |
| Atomic `accept_invitation` port + both adapter impls | 4 — two writes in one txn (model on ZMVP-32 `create`) | P0 | 🧑 Engineer | ⬜ TODO only (`ports.rs:155-163`); impls absent (`adapter-pg/account.rs` ends `:298`, `adapter-mem` has only `revoke_invitation` `:427-439`) |
| Accept endpoint + handler + DTO + route + authority | 3 — "only the invitee accepts" + problem+json | P0 | 🧑 Engineer | ⬜ only `/decline` routed (`lib.rs:273`); no `/accept` route/handler/DTO |
| Decline endpoint + handler (invitee-keyed, reuses `revoke_invitation`) | 2 — small; no new domain/state | P1 | 🤖 Claude | ✅ done — `lib.rs:273-276`, `:823-851`; e2e `invitations.rs:408`, `:451` |
| Tests (unit + mem + pg + e2e, accept + decline) | 3 | P0 | 👥 Group | 🟡 domain ✅, decline e2e ✅, accept e2e `#[ignore]`d `:493`, adapter round-trips ⬜ |

*Net: the 🤖 Claude-owned pieces (accept transition, decline endpoint) + the shared error type are done; the two structural firsts — writing the parent edge and adding the profile-listing flag, both folded into the atomic `accept_invitation` txn + `/accept` endpoint — are the entire remaining (engineer-owned) job.*

## ✅ 6. Test checklist (TDD)

- **Unit** — _asserts that_ `accept()` moves `pending → accepted` and stamps `updated_at`; accepting a non-pending invite is rejected (`InvitationError::NotPending`) → **AC1/AC4** — ✅ `invitation.rs:368`, `:386`
- **Integration (mem + pg)** — _asserts that_ `accept_invitation` in one step: the invitation reads back `accepted` **and** a membership exists at the offered role → **AC1/AC2** — ⬜
- **Integration (mem + pg)** — _asserts that_ the new membership's **parent == inviter** (the `parent` column is populated, not NULL) → **AC2** — ⬜
- **Integration (mem + pg)** — _asserts that_ the membership's **`listed_on_profile`** reflects the accepted choice (default listed when omitted) → **AC3** — ⬜
- **Integration** — _asserts that_ accepting a **revoked** (or absent) invitation creates **no** membership and leaves the user a non-member → **AC4** — ⬜
- **E2E** — _asserts that_ the invited User `POST`s acceptance and gets `200` + becomes a member (`role_of` now returns the offered role) → **AC1/AC2** — 🟡 written but `#[ignore]`d (`invitations.rs:493`)
- **E2E** — _asserts that_ a User with **no pending invite** for the account gets `404`/problem+json and no membership (covers "only the invitee accepts" — we only ever look up the *session user's own* offer) → **AC1** — ✅ via decline path (`invitations.rs:451`); accept-path twin pending
- **E2E** — _asserts that_ after the issuer revokes, the invitee's acceptance is refused and mints nothing → **AC4** — ⬜
- **E2E** — _asserts that_ acceptance with `{ "list_on_profile": false }` records the opt-out on the membership → **AC3** — ⬜ (body shape referenced at `invitations.rs:506`)
- **E2E** — _asserts that_ the invitee `POST`s decline, their pending offer is gone (no membership), and a subsequent accept is refused → **AC1/AC4** — ✅ decline half (`invitations.rs:408`); "subsequent accept refused" lands with the accept endpoint
- **E2E** — _asserts that_ an anonymous caller gets `401` → **AC1** — ⬜

## 🧠 7. Logic & shape

```
POST /accounts/{id}/invitations/accept         (body: { "list_on_profile"?: bool, default true })
  require_user(session)                  → 401
  load_account(id)                       → 404
  find_pending_invitation(account, user) → 404 no_pending_invitation if None   ← authority is implicit:
        (we look up the SESSION USER's own pending offer; a revoked/accepted/absent
         offer simply isn't found, so no membership is minted — AC1/AC4)
  invitation.accept(now)                 → (guards pending; 409 if somehow not)   ✅ exists
  accept_invitation(                     ← ONE private-store transaction          ⬜ TODO
        invitation,                          UPDATE account_invitations SET state='accepted'
        UserAccount(user, account,           INSERT account_members(role, parent=inviter,
                    Role::X(Some(inviter))),                       listed_on_profile)
        list_on_profile)
  → 200 { "account", "user", "role" }
```

Role tree after acceptance (rule 4a — the edge ZMVP-20 first writes, **still unbuilt**):

```
        Owner (parent: none)            ← founder (ZMVP-14)
          │  issues invite, then invitee accepts
          ▼
        Member (parent: Owner)          ← parent = inviter, to be set on accept
```

`POST /accounts/{id}/invitations/decline` — ✅ **built** (`lib.rs:823-851`): the invitee kills their own offer: `require_user` → `find_pending_invitation(account, user)` → `invitation.revoke(now)` → `revoke_invitation(id)` → `200`. Same transition as an issuer revoke, just keyed by the session user (the invitee) instead of a DID in the body.

State machine — ZMVP-20 writes the **accept** edge (transition ✅, persistence ⬜); **decline** reuses the revoked edge (✅ wired end-to-end); ZMVP-32 wrote issue/issuer-revoke:

```
        issue                 accept
   ∅ ─────────▶ pending ──────────────▶ accepted   (terminal, mints membership)
                   │
                   └──────▶ revoked     (terminal; can't be accepted — find_pending returns None)
                      ▲   issuer revoke  (ZMVP-32)  /  invitee decline  (ZMVP-20 ✅)
```

**Atomicity:** the accept transaction mirrors ZMVP-32's `create` (account + owner membership in one tx). The invitation flip and the membership insert must commit together — a half-accept (membership without consuming the invite, or vice-versa) must be impossible. **This composite is the unbuilt core.**

## 🚀 8. Next steps

1. **Resume the engineer-owned core — it's the whole remaining job.** Order:
   a. Migration `account_members.listed_on_profile BOOLEAN NOT NULL DEFAULT true`.
   b. Atomic `accept_invitation(invitation, member, listed_on_profile)` port (turn the `ports.rs:155-163` TODO into a method) + adapter-mem impl + adapter-pg tx (`UPDATE … state='accepted'` + `INSERT account_members` with role/**parent=inviter**/listed_on_profile).
   c. Parent-edge persistence — sequence carefully; it's the highest-blast-radius part (touches existing membership serialization in both adapters).
   d. `POST /accounts/{id}/invitations/accept` handler + `AcceptInvitationBody { list_on_profile?: bool }` DTO + route, mirroring the finished `decline` handler.
2. **Un-`#[ignore]` the accept e2e** (`invitations.rs:493`) and fill in the still-missing assertions (parent==inviter, list_on_profile opt-out, revoked-can't-accept, anonymous 401) + the mem/pg adapter round-trips.
3. `.sqlx` cache will need regenerating (new accept/membership `query!`s) — same drill as ZMVP-32.

**Decisions — already resolved (2026-06-27, carried forward):**
- ✅ **"List on profile?" — in scope.** Add `account_members.listed_on_profile BOOLEAN NOT NULL DEFAULT true`, set from the accept body.
- ✅ **Decline — in scope** ("revoking is an active thing"). Invitee-initiated; reuses `Revoked` + `Invitation::revoke()` + `revoke_invitation`. **Now built.**
- ✅ **Endpoints** — `POST .../accept` and `POST .../decline` (both session-user-keyed; accept body carries `list_on_profile`).
- ✅ **Atomic op** — one composite `accept_invitation` port method (single private-store transaction).
- ✅ **Parent representation** — store the inviter's `UserId` as its string form in the existing `Option<String>` parent slot / `parent` column. Strong typing deferred.
- ✅ **Error model** — reuse the ZMVP-35 `Problem`: `not_authenticated` (401), `account_not_found` (404), plus `no_pending_invitation` (404). **`no_pending_invitation()` now exists** (`problem.rs:117-125`).

**Remaining notes (non-blocking):**
- The `parent` column write is the one piece touching existing membership serialization in both adapters — sequence it carefully (highest-blast-radius part).
- ZMVP-14's founder onboarding still owes a "list on profile?" step per decision 11 — out of scope here; founder row defaults to listed.
