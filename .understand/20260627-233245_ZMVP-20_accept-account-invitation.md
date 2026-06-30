# 🔎 Understanding ZMVP-20 — User accepts an invitation to join an Account

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-20 · **Generated:** 2026-06-27 23:32 · **Snapshot:** `.understand/20260627-233245_ZMVP-20_accept-account-invitation.md`

## 📊 Since last snapshot

Compared with `.understand/20260627-142307_ZMVP-20_accept-account-invitation.md` (2026-06-27 **14:23**, ~9h earlier). Jira **unchanged** (In Progress, Medium, assigned to you, ACs identical — last Jira edit 2026-06-25). All movement is in code: the diff grew **320 insertions / 5 files → 350 insertions, 36 deletions / 6 files**. Still uncommitted on the branch (the only new commits are the two tooling/conventions commits we just made, unrelated to ZMVP-20).

**The engineer-owned core has *started* — but the hard part is still unbuilt:**

- `account_members.listed_on_profile` migration — **⬜ → ✅** (`adapter-pg/migrations/20260627224853_add_listed_on_profile.sql`: `ALTER TABLE account_members ADD COLUMN listed_on_profile BOOLEAN NOT NULL DEFAULT true`).
- `accept_invitation` port method — **⬜ → 🟡** (signature now exists, `ports.rs:164-169`) — but ⚠️ see the two flags below; the `TODO(ZMVP-20 — engineer-owned)` note still sits above it at `:155-163`.
- adapter-pg `accept_invitation` — **⬜ → 🟡 partial** (`account.rs:299-321`): it runs the **invitation `UPDATE … state='accepted'` only**. It does **not** `INSERT` the membership, the parent edge, or `listed_on_profile` — `member` and `listed_on_profile` are accepted but unused. The atomic two-write transaction (the actual point) is not there yet.
- **Unchanged ⬜:** adapter-mem impl (no `accept_invitation` at all), the parent edge, the `POST …/accept` endpoint+DTO+route. Accept e2e still 🟡 `#[ignore]`d (`invitations.rs:492`, now with a fuller hand-off reason).

**Net movement:** ~one and a half of the four engineer-owned rows moved — the **schema** is in (✅) and the **port/adapter scaffolding** is stubbed (🟡), but the **composite accept transaction, the parent edge, the profile-listing write, the mem mirror, and the `/accept` endpoint remain the job.** 4 of 7 breakdown rows now show motion; the structural core is half-stubbed (invitation flip only).

**⚠️ Two correctness flags on the new port signature (engineer's call — flagging, not fixing):**
1. **`accept_invitation(self, …)` takes `self` by value.** `AccountRepo` is consumed as `Arc<dyn AccountRepo>` in `AppState`, and you can't move out of an `Arc` — a by-value `self` method isn't callable on the shared trait object. This very likely won't compile at the call site once `/accept` wires it. Expected shape: `&self`, like the sibling ops.
2. **`member: &Account` probably can't carry the membership.** Writing the membership needs the **user + role + parent (=inviter)** — i.e. the `UserAccount`, not the `Account`. As typed, the impl has nothing to `INSERT` the member row from (which may be *why* the adapter-pg stub skips the insert). Worth confirming the intended parameter before building the transaction on top of it.

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

Two things land here that ZMVP-32/ZMVP-14 **deliberately deferred**: the **role-tree parent edge** (the `parent` column has existed but was always `NULL`) and the **per-membership "list on profile?" flag** (now has a *column* via the new migration, but nothing writes it yet). So this is more structural than "the accept half" suggests — and these two firsts are exactly what's still unbuilt.

## 🗺️ 2. Domain

- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081) / membership** — the join `UserAccount(UserId, AccountId, Role)` (`domain/src/elements/user_account.rs:20`), persisted in `account_members` (PK `(account_id, user_id)`, columns `role`, `parent`, and now **`listed_on_profile`**).
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — `Owner < Admin < Manager < Member`. Two rules bear on this ticket:
  - **Rule 4a (parent-by-invitation):** *"when a member invites a new member, the inviter becomes the invitee's Parent in the hierarchy tree at the invitee's initial role."* This is the edge ZMVP-20 writes — the invitation already records `inviter` precisely so acceptance can set it. **Still unwritten** — adapter-pg's stub inserts no member at all; adapter-mem has no impl.
  - **Rule 5:** *"an Owner never has a parent."* Not triggered here (acceptance only mints Admin/Manager/Member), but it bounds the parent model.
  - The `Role` enum carries a parent slot today — `Role::Member(Option<String>)` (`role.rs:24-33`) — always `None` on the floor, and the slot is a `String`, not a `UserId`. ZMVP-20 is the first writer of that slot.
- **Invitation** (`domain/src/elements/invitation.rs`) — has a working **`accept()`** (`:304-311`, mirrors `revoke()`): guards `Pending` (`InvitationError::NotPending`), flips to `Accepted`, stamps `updated_at`. Carries `invited_user`, `account`, `role`, `inviter`.
- **"List on profile?"** — per decision 11, memberships list on the User-Profile **by default**; onboarding sets the per-membership **opt-out**. The **column now exists** (migration ✅, default `true`); **no code reads or writes it yet** (the adapter param is unused).

## 🎯 3. Goal & scope

**Goal:** give an invited User a way to accept their pending invitation, which atomically (a) marks the invitation accepted, (b) mints their membership at the offered role with the **inviter as Parent**, and (c) records the membership's **"list on profile?"** choice — and guarantees a revoked/absent invitation yields no membership.

**In scope**
- `Invitation::accept(now)` pure domain transition (`pending → accepted`, mirrors `revoke()`). ✅ **done** (`invitation.rs:304-311`).
- An atomic **`accept_invitation`** port op: flip invitation → accepted **and** create the membership, in one private-store transaction (never half-done). 🟡 **signature only** (`ports.rs:164-169`), with the two ⚠️ flags above; the composite body is unbuilt.
- **Writing the parent edge**: the membership write persists `parent = inviter` (first real use of `account_members.parent` / the `Role` parent slot). ⬜.
- **"list on profile?"** persisted per membership: **column ✅** (migration), **read/write ⬜** (param unused). Set at acceptance, default = listed. **[DECIDED: in scope]**
- `POST /accounts/{id}/invitations/accept` (⬜) + `POST /accounts/{id}/invitations/decline` (✅ `lib.rs:823-851`) handlers + DTO + routes; authority = *only the invited User acts on their own invite* (implicit via session-user lookup). **[DECIDED]**
- **Decline** = the invitee actively kills their own pending offer (reuses `Invitation::revoke()` and `revoke_invitation`). ✅ **done**. **[DECIDED: in scope — "revoking is an active thing"]**
- adapter-pg + adapter-mem impls; tests at every layer. 🟡 adapter-pg stubbed (invitation flip only), adapter-mem absent, round-trips outstanding.

**Out of scope**
- **Issuing/(issuer-)revoking** invitations → ZMVP-32 (done).
- **The full role-tree mechanics** — rule 2 (parent-may-demote), rule 3 (re-parent children on removal), promotion-sets-parent (rule 4b). ZMVP-20 writes *one* parent edge; the tree algebra is later tickets.
- **A distinct `Declined` state** — decline reuses the existing `Revoked` terminal state. Splitting issuer-revoked vs invitee-declined is a future refinement + DD update.
- **Backfilling ZMVP-14's founder onboarding** — account-creation also owes a "list on profile?" step per decision 11; the founder row defaults to listed. Separate gap.
- **Notifications / "you've been invited"** — no notification subsystem exists.
- **Strongly typing the parent** as `UserId` instead of `String` — keep the existing slot type (store the string form).

## 📦 4. Deliverables

- [x] `Invitation::accept(now)` → `Result<(), InvitationError>` (pending→accepted) — `domain/src/elements/invitation.rs:304-311`
- [x] `account_members` migration: add `listed_on_profile BOOLEAN NOT NULL DEFAULT true` — `adapter-pg/migrations/20260627224853_add_listed_on_profile.sql`
- [~] Port method `accept_invitation(invitation, member, listed_on_profile)` (atomic) on `AccountRepo` — `domain/src/ports.rs:164-169` ⚠️ signature exists but `self`-by-value + `&Account` param need a second look
- [ ] Membership write persists the **parent** edge (Role parent slot → `parent` column) — both adapters
- [~] adapter-pg impl (ONE transaction: `UPDATE invitation … 'accepted'` + `INSERT account_members` with role/parent/listed_on_profile) + `.sqlx` cache — `adapter-pg/src/account.rs:299-321` (only the UPDATE exists; no INSERT)
- [ ] adapter-mem impl mirroring it — `adapter-mem/src/lib.rs` (absent)
- [ ] `POST /accounts/{id}/invitations/accept` handler + DTO (`{ list_on_profile?: bool }`) + route — `api/src/lib.rs` (absent)
- [x] `POST /accounts/{id}/invitations/decline` handler + route (invitee-keyed; reuses `revoke_invitation`) — `api/src/lib.rs:273-276`, `:823-851`
- [x] `no_pending_invitation()` 404 problem constructor — `api/src/problem.rs:117-125`
- [ ] Tests: domain unit (✅ `invitation.rs:368`, `:386`), mem + pg adapter round-trips (⬜), api e2e (decline ✅ `invitations.rs:407`; accept 🟡 `#[ignore]`d `:492`)

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Invitation::accept()` transition + unit test | 2 — mirrors `revoke()` | P0 | 🤖 Claude | ✅ done — `invitation.rs:304-311`; tests `:368`, `:386` |
| `account_members.listed_on_profile` migration | 3 — schema blast radius | P1 | 🧑 Engineer | ✅ column added — `migrations/20260627224853_add_listed_on_profile.sql` (read/write still ⬜) |
| Atomic `accept_invitation` port + both adapter impls | 4 — two writes in one txn (model on ZMVP-32 `create`) | P0 | 🧑 Engineer | 🟡 sig `ports.rs:164-169` (⚠️ `self`/`&Account`); pg stub flips invite only `account.rs:299-321`; mem absent |
| Persist the **parent edge** (Role slot → `parent` column) | 4 — first writer; touches membership serialization in both adapters | P0 | 🧑 Engineer | ⬜ `parent` still NULL; no membership INSERT anywhere |
| `listed_on_profile` read/write on the membership | 2 — once the column + txn exist | P1 | 🧑 Engineer | ⬜ param threaded but unused (`account.rs:299-321`) |
| Accept endpoint + handler + DTO + route + authority | 3 — "only the invitee accepts" + problem+json | P0 | 🧑 Engineer | ⬜ only `/decline` routed (`lib.rs:273`); no `/accept` route/handler/DTO |
| Decline endpoint + handler (invitee-keyed, reuses `revoke_invitation`) | 2 — small; no new domain/state | P1 | 🤖 Claude | ✅ done — `lib.rs:273-276`, `:823-851`; e2e `invitations.rs:407` |
| Tests (unit + mem + pg + e2e, accept + decline) | 3 | P0 | 👥 Group | 🟡 domain ✅, decline e2e ✅, accept e2e `#[ignore]`d `:492`, adapter round-trips ⬜ |

*Net: the 🤖 Claude-owned pieces (accept transition, decline endpoint) + the shared error type are done; the engineer-owned core has begun (schema ✅, port/adapter scaffolding 🟡) but the atomic two-write transaction, the parent edge, and the profile-listing write — plus the mem mirror and `/accept` endpoint — are the remaining job.*

## ✅ 6. Test checklist (TDD)

- **Unit** — _asserts that_ `accept()` moves `pending → accepted` and stamps `updated_at`; accepting a non-pending invite is rejected (`InvitationError::NotPending`) → **AC1/AC4** — ✅ `invitation.rs:368`, `:386`
- **Integration (mem + pg)** — _asserts that_ `accept_invitation` in one step: the invitation reads back `accepted` **and** a membership exists at the offered role → **AC1/AC2** — ⬜ (pg stub flips invite but inserts no member; mem absent)
- **Integration (mem + pg)** — _asserts that_ the new membership's **parent == inviter** (`parent` populated, not NULL) → **AC2** — ⬜
- **Integration (mem + pg)** — _asserts that_ the membership's **`listed_on_profile`** reflects the accepted choice (default listed when omitted) → **AC3** — ⬜ (column exists; unwritten)
- **Integration** — _asserts that_ accepting a **revoked** (or absent) invitation creates **no** membership → **AC4** — ⬜
- **E2E** — _asserts that_ the invited User `POST`s acceptance, gets `200`, and becomes a member (`role_of` returns the offered role) → **AC1/AC2** — 🟡 written but `#[ignore]`d (`invitations.rs:492`)
- **E2E** — _asserts that_ a User with **no pending invite** gets `404`/problem+json and no membership → **AC1** — ✅ via decline path (`invitations.rs`); accept-path twin pending
- **E2E** — _asserts that_ after the issuer revokes, the invitee's acceptance is refused and mints nothing → **AC4** — ⬜
- **E2E** — _asserts that_ acceptance with `{ "list_on_profile": false }` records the opt-out → **AC3** — ⬜
- **E2E** — _asserts that_ the invitee `POST`s decline, their pending offer is gone, and a subsequent accept is refused → **AC1/AC4** — ✅ decline half (`invitations.rs:407`); "subsequent accept refused" lands with the accept endpoint
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
  accept_invitation(                     ← ONE private-store transaction          🟡 only the UPDATE so far
        invitation,                          UPDATE account_invitations SET state='accepted'   ✅ in stub
        <membership: user, account,          INSERT account_members(role, parent=inviter,      ⬜ MISSING
                     role, parent=inviter>,                          listed_on_profile)         ⬜ MISSING
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

`POST /accounts/{id}/invitations/decline` — ✅ **built** (`lib.rs:823-851`): the invitee kills their own offer: `require_user` → `find_pending_invitation(account, user)` → `invitation.revoke(now)` → `revoke_invitation(id)` → `200`.

State machine — ZMVP-20 writes the **accept** edge (transition ✅, persistence 🟡 invite-flip-only); **decline** reuses the revoked edge (✅ end-to-end); ZMVP-32 wrote issue/issuer-revoke:

```
        issue                 accept
   ∅ ─────────▶ pending ──────────────▶ accepted   (terminal — must ALSO mint membership; not yet)
                   │
                   └──────▶ revoked     (terminal; can't be accepted — find_pending returns None)
                      ▲   issuer revoke  (ZMVP-32)  /  invitee decline  (ZMVP-20 ✅)
```

**Atomicity:** the accept transaction mirrors ZMVP-32's `create` (account + owner membership in one tx). The invitation flip and the membership insert must commit together — a half-accept must be impossible. **Today the adapter does only the flip, so it is exactly the half-accept the design forbids — finishing the composite is the unbuilt core, and the ⚠️ port-signature questions block building it cleanly.**

## 🚀 8. Next steps

1. **Resolve the two port-signature questions first (Engineer's call) — they gate the rest:**
   - `accept_invitation` taking **`self` by value** vs `&self` (it's used behind `Arc<dyn AccountRepo>`; by-value won't be callable).
   - Whether the membership param should be the **`UserAccount` (user + role + parent)** rather than `&Account` — the impl currently has nothing to `INSERT` the member from.
2. **Finish the engineer-owned core** (order):
   a. With the signature settled, make the adapter-pg body **one transaction**: `UPDATE … 'accepted'` **+ `INSERT account_members`** with role / **parent=inviter** / `listed_on_profile` (a `sqlx::Transaction`, like ZMVP-32 `create`).
   b. **Parent-edge persistence** — highest blast-radius (touches existing membership serialization in both adapters); sequence carefully.
   c. **adapter-mem** mirror of `accept_invitation` (currently absent).
   d. `POST /accounts/{id}/invitations/accept` handler + `AcceptInvitationBody { list_on_profile?: bool }` DTO + route, mirroring the finished `decline` handler.
3. **Un-`#[ignore]` the accept e2e** (`invitations.rs:492`) and add the missing assertions (parent==inviter, list_on_profile opt-out, revoked-can't-accept, anonymous 401) + mem/pg adapter round-trips.
4. `.sqlx` cache regenerates with the new accept/membership `query!`s — same drill as ZMVP-32.

**Decisions — already resolved (carried forward):**
- ✅ **"List on profile?" — in scope.** Column added; set from the accept body (default listed).
- ✅ **Decline — in scope** ("revoking is an active thing"). Invitee-initiated; reuses `Revoked` + `Invitation::revoke()`. **Built.**
- ✅ **Endpoints** — `POST .../accept` and `POST .../decline` (both session-user-keyed; accept body carries `list_on_profile`).
- ✅ **Atomic op** — one composite `accept_invitation` port method (single private-store transaction).
- ✅ **Parent representation** — store the inviter's `UserId` as its string form in the existing `Option<String>` parent slot / `parent` column. Strong typing deferred.
- ✅ **Error model** — reuse the ZMVP-35 `Problem`: `not_authenticated` (401), `account_not_found` (404), `no_pending_invitation` (404, `problem.rs:117-125`).

**Open / ⚠️ flags (need you):**
- ⚠️ The `accept_invitation` signature (`self`-by-value, `member: &Account`) — confirm/adjust before the transaction is built on it. *Domain/architecture call — yours.*
- The `parent` column write is the one piece touching existing membership serialization in both adapters — highest blast-radius; sequence it carefully.
- ZMVP-14's founder onboarding still owes a "list on profile?" step per decision 11 — out of scope here; founder row defaults to listed.
