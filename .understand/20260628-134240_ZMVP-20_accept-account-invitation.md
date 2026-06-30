# 🔎 Understanding ZMVP-20 — User accepts an invitation to join an Account

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-20 · **Generated:** 2026-06-28 13:42 · **Snapshot:** `.understand/20260628-134240_ZMVP-20_accept-account-invitation.md`

## 📊 Since last snapshot

Compared with `.understand/20260627-233245_ZMVP-20_accept-account-invitation.md` (2026-06-27 **23:32**, ~14h earlier). Jira **effectively unchanged** (In Progress, Medium, assigned to you, ACs identical — `updated` field touched 2026-06-28 **01:27**, but the description/ACs read the same). All real movement is in code: the uncommitted diff grew **350 ins / 36 del / 6 files → 456 ins / 82 del / 10 files**, plus a **second new migration**. Still uncommitted on the branch (no ZMVP-20 commits yet).

**The hard engineer-owned core that was unbuilt last snapshot has largely LANDED — but the work is now build-broken on the one piece that didn't.**

- **Both ⚠️ port-signature flags from last snapshot are RESOLVED** (`ports.rs:167-171`):
  1. `self`-by-value **→ `&self`** ✅ — callable behind `Arc<dyn AccountRepo>`.
  2. `member: &Account` **→ dropped entirely.** Signature is now `accept_invitation(&self, invitation: Invitation, listed_on_profile: bool) -> Result<UserAccount>` — the membership is *derived from the invitation* (which already carries `invited_user`/`account`/`role`/`inviter`) and the minted `UserAccount` is **returned**. Clean resolution of "the impl had nothing to INSERT from."
- adapter-pg `accept_invitation` — **🟡 stub (flip-only) → ✅ FULLY built** (`account.rs:309-364`): one `sqlx` transaction — guarded `UPDATE … state='accepted'` (0-rows ⇒ rollback on lost race) **+ `INSERT account_members(account_id,user_id,parent,role,listed_on_profile)`** + `commit()`, RETURNING → `UserAccount`. The atomic two-write — the actual point — is there.
- **Parent edge — ⬜ → ✅ (in pg)**: `parent = *invitation.inviter` (`account.rs:350`), the first real write of `account_members.parent` (rule 4a). Plus a **NEW migration** `20260627235014_parent_uuid_fk.sql` promotes `parent` **text → uuid** with `FK → users.id`. Structural firsts, done.
- **`listed_on_profile` read/write — ⬜ → ✅ (in pg)**: written from the param at `account.rs:352`.
- **`UserAccount` refactored** tuple `(user, account_id, role)` **→ named-field struct `{ user_id, account_id, role }`** (`user_account.rs`), rippling into adapter-mem (+32, call-site adaptation only), `account.rs`, `role.rs`.

**⛔ New regression — the workspace does NOT compile:**
- adapter-mem still has **no `accept_invitation`** (`lib.rs`), and the trait method has **no default body** — so `cargo check` fails hard: `adapter-mem … error[E0046]: not all trait items implemented, missing: accept_invitation`. Last snapshot the mem impl was merely *absent*; now that absence is a **build break** for the whole workspace. (The adapter-pg `Connection refused` / `type annotations needed` errors are **not** code defects — they're sqlx's offline query check needing a live DB or a regenerated `.sqlx` cache; see §8.4.)

**Net movement:** of the engineer-owned remaining job, the **two biggest, highest-blast-radius pieces landed** — the atomic accept transaction **and** the parent edge (with a typed-FK migration) — and **both port-signature blockers resolved**. What remains is the **two smallest** pieces: the **adapter-mem mirror** (now a compile error, so it's the #1 blocker, not optional) and the **`POST /accept` endpoint** (+ un-ignoring the e2e). 5 of 8 breakdown rows are now ✅/near; the structural core is built; the build is red purely on the missing mem mirror.

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

Two things land here that ZMVP-32/ZMVP-14 **deliberately deferred**: the **role-tree parent edge** (the `parent` column existed but was always `NULL`) and the **per-membership "list on profile?" flag**. As of this snapshot **both are now written in adapter-pg** (parent = inviter, promoted to a `uuid` FK; `listed_on_profile` set from the accept choice) — so the structural novelty of this ticket is no longer hypothetical, it's in the pg adapter. What's missing is parity (mem) and the HTTP surface.

## 🗺️ 2. Domain

- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081) / membership** — the join `UserAccount { user_id, account_id, role }` (`domain/src/elements/user_account.rs`, **refactored from a tuple struct this snapshot**), persisted in `account_members` (PK `(account_id, user_id)`, columns `role`, `parent` — **now `uuid` FK → `users.id`** — and `listed_on_profile`).
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — `Owner < Admin < Manager < Member`. Two rules bear on this ticket:
  - **Rule 4a (parent-by-invitation):** *"when a member invites a new member, the inviter becomes the invitee's Parent in the hierarchy tree at the invitee's initial role."* This is the edge ZMVP-20 writes — **now written in pg** (`account.rs:350`, `parent = *invitation.inviter`); mem still owes the mirror.
  - **Rule 5:** *"an Owner never has a parent."* Not triggered here (acceptance only mints Admin/Manager/Member), but it bounds the parent model.
  - The `Role` enum carries a parent slot — `Role::Member(RoleTitle = Option<String>)` (`role.rs:26-35`) — `None` on the floor; ZMVP-20 is the first writer of that slot (via the inviter on the persisted membership; the pg row stores it as the `parent` uuid column).
- **Invitation** (`domain/src/elements/invitation.rs`) — has a working **`accept()`** (`:304-311`, mirrors `revoke()`): guards `Pending` (`InvitationError::NotPending`), flips to `Accepted`, stamps `updated_at`. Carries `invited_user`, `account`, `role`, `inviter` — enough for the adapter to mint the whole membership from the invitation alone (which is why the port no longer needs a separate membership param).
- **"List on profile?"** — per decision 11, memberships list on the User-Profile **by default**; onboarding sets the per-membership **opt-out**. The **column exists** (migration ✅, default `true`) and is now **written by the pg accept** (`account.rs:352`).

## 🎯 3. Goal & scope

**Goal:** give an invited User a way to accept their pending invitation, which atomically (a) marks the invitation accepted, (b) mints their membership at the offered role with the **inviter as Parent**, and (c) records the membership's **"list on profile?"** choice — and guarantees a revoked/absent invitation yields no membership.

**In scope**
- `Invitation::accept(now)` pure domain transition (`pending → accepted`, mirrors `revoke()`). ✅ **done** (`invitation.rs:304-311`).
- An atomic **`accept_invitation`** port op: flip invitation → accepted **and** create the membership, in one private-store transaction (never half-done). ✅ **port settled** (`ports.rs:167-171`, both flags resolved) and ✅ **pg impl complete** (`account.rs:309-364`); ❌ **mem impl missing → build break**.
- **Writing the parent edge** (`parent = inviter`, first real use of `account_members.parent`). ✅ **in pg** (`account.rs:350`) + ✅ migration to a typed `uuid` FK (`20260627235014_parent_uuid_fk.sql`); ⬜ mem mirror.
- **"list on profile?"** persisted per membership. **column ✅**, **pg read/write ✅** (`account.rs:352`); ⬜ mem mirror. Default = listed. **[DECIDED: in scope]**
- `POST /accounts/{id}/invitations/accept` (⬜) + `POST /accounts/{id}/invitations/decline` (✅ `lib.rs:823-851`) handlers + DTO + routes; authority = *only the invited User acts on their own invite* (implicit via session-user lookup). **[DECIDED]**
- **Decline** = the invitee actively kills their own pending offer (reuses `Invitation::revoke()` and `revoke_invitation`). ✅ **done**. **[DECIDED: in scope]**
- adapter-pg + adapter-mem impls; tests at every layer. ✅ pg complete; ❌ mem absent (build-breaking); round-trips can't run until the build is green.

**Out of scope**
- **Issuing/(issuer-)revoking** invitations → ZMVP-32 (done).
- **The full role-tree mechanics** — rule 2 (parent-may-demote), rule 3 (re-parent children on removal), promotion-sets-parent (rule 4b). ZMVP-20 writes *one* parent edge; the tree algebra is later tickets.
- **A distinct `Declined` state** — decline reuses the existing `Revoked` terminal state. Splitting issuer-revoked vs invitee-declined is a future refinement + DD update.
- **Backfilling ZMVP-14's founder onboarding** — account-creation also owes a "list on profile?" step per decision 11; the founder row defaults to listed. Separate gap.
- **Strongly typing the in-memory parent** as `UserId` — note the *pg column* did get promoted to `uuid` this snapshot; the domain `Role` slot stays `Option<String>`.

## 📦 4. Deliverables

- [x] `Invitation::accept(now)` → `Result<(), InvitationError>` (pending→accepted) — `domain/src/elements/invitation.rs:304-311`
- [x] `account_members` migration: `listed_on_profile BOOLEAN NOT NULL DEFAULT true` — `adapter-pg/migrations/20260627224853_add_listed_on_profile.sql`
- [x] `account_members` migration: promote `parent` **text → uuid** + `FK → users.id` — `adapter-pg/migrations/20260627235014_parent_uuid_fk.sql` **(new this snapshot)**
- [x] Port method `accept_invitation(&self, invitation, listed_on_profile) -> Result<UserAccount>` (atomic) on `AccountRepo` — `domain/src/ports.rs:167-171` **(both prior ⚠️ flags resolved)**
- [x] Membership write persists the **parent** edge (inviter → `parent` uuid column) — **adapter-pg** `account.rs:350`; [ ] **adapter-mem** mirror
- [x] adapter-pg impl (ONE transaction: guarded `UPDATE … 'accepted'` + `INSERT account_members` with role/parent/listed_on_profile, lost-race rollback) — `account.rs:309-364` · [ ] `.sqlx` cache regenerated (see §8.4)
- [ ] **adapter-mem impl mirroring it — `adapter-mem/src/lib.rs` (ABSENT → `error[E0046]`, breaks the build)**
- [ ] `POST /accounts/{id}/invitations/accept` handler + DTO (`{ list_on_profile?: bool }`) + route — `api/src/lib.rs` (absent)
- [x] `POST /accounts/{id}/invitations/decline` handler + route (invitee-keyed; reuses `revoke_invitation`) — `api/src/lib.rs:273-276`, `:823-851`
- [x] `no_pending_invitation()` 404 problem constructor — `api/src/problem.rs:117-125`
- [ ] Tests: domain unit (✅ `invitation.rs:368`, `:385`), mem + pg adapter round-trips (⬜, blocked on build), api e2e (decline ✅ `invitations.rs:408`; accept 🟡 `#[ignore]`d `:491-522`)

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Invitation::accept()` transition + unit test | 2 — mirrors `revoke()` | P0 | 🤖 Claude | ✅ done — `invitation.rs:304-311`; tests `:368`, `:385` |
| `account_members` migrations (`listed_on_profile`; `parent` → uuid FK) | 3 — schema blast radius | P1 | 🧑 Engineer | ✅ both present — `20260627224853_*`, `20260627235014_*` |
| Atomic `accept_invitation` port + **pg** impl | 4 — two writes in one txn | P0 | 🧑 Engineer | ✅ port settled (`ports.rs:167-171`, flags resolved) **+ pg complete** (`account.rs:309-364`) |
| Atomic `accept_invitation` **mem** impl | 2 — mirror the pg semantics in the map | P0 | 🧑 Engineer | ❌ **absent → `error[E0046]`, workspace won't compile** |
| Persist the **parent edge** (inviter → `parent` column) | 4 — first writer; touches membership serialization | P0 | 🧑 Engineer | ✅ in pg (`account.rs:350`) + uuid-FK migration; ⬜ mem mirror |
| `listed_on_profile` read/write on the membership | 2 — once column + txn exist | P1 | 🧑 Engineer | ✅ in pg (`account.rs:352`); ⬜ mem mirror |
| Accept endpoint + handler + DTO + route + authority | 3 — "only the invitee accepts" + problem+json | P0 | 🧑 Engineer | ⬜ only `/decline` routed (`lib.rs:273`); no `/accept` route/handler/DTO |
| Decline endpoint + handler (invitee-keyed, reuses `revoke_invitation`) | 2 — small; no new domain/state | P1 | 🤖 Claude | ✅ done — `lib.rs:273-276`, `:823-851`; e2e `invitations.rs:408` |
| Tests (unit + mem + pg + e2e, accept + decline) | 3 | P0 | 👥 Group | 🟡 domain ✅, decline e2e ✅, accept e2e `#[ignore]`d `:491-522`, adapter round-trips ⬜ (blocked on build) |

*Net: the atomic accept transaction, the parent edge (+ typed-FK migration), and the profile-listing write all landed in **pg**, and both port-signature blockers are resolved. The remaining engineer-owned job is small but blocking: the **adapter-mem mirror** (currently a compile error — the #1 thing) and the **`POST /accept` endpoint**, then un-ignore the e2e and regenerate the `.sqlx` cache.*

## ✅ 6. Test checklist (TDD)

- **Unit** — _asserts that_ `accept()` moves `pending → accepted` and stamps `updated_at`; accepting a non-pending invite is rejected (`InvitationError::NotPending`) → **AC1/AC4** — ✅ `invitation.rs:368`, `:385`
- **Integration (mem + pg)** — _asserts that_ `accept_invitation` in one step: the invitation reads back `accepted` **and** a membership exists at the offered role → **AC1/AC2** — 🟡 pg impl ready but unrun (build red on mem); mem impl absent
- **Integration (mem + pg)** — _asserts that_ the new membership's **parent == inviter** (`parent` populated, not NULL) → **AC2** — 🟡 pg writes it (`account.rs:350`); test unrun; mem ⬜
- **Integration (mem + pg)** — _asserts that_ the membership's **`listed_on_profile`** reflects the accepted choice (default listed when omitted) → **AC3** — 🟡 pg writes it (`account.rs:352`); test unrun; mem ⬜
- **Integration** — _asserts that_ accepting a **revoked** (or absent) invitation creates **no** membership → **AC4** — 🟡 pg guards via 0-rows rollback (`account.rs:335-340`); test unrun
- **E2E** — _asserts that_ the invited User `POST`s acceptance, gets `200`, and becomes a member (`role_of` returns the offered role) → **AC1/AC2** — 🟡 written but `#[ignore]`d (`invitations.rs:491-522`)
- **E2E** — _asserts that_ a User with **no pending invite** gets `404`/problem+json and no membership → **AC1** — ✅ via decline path; accept-path twin pending
- **E2E** — _asserts that_ after the issuer revokes, the invitee's acceptance is refused and mints nothing → **AC4** — ⬜
- **E2E** — _asserts that_ acceptance with `{ "list_on_profile": false }` records the opt-out → **AC3** — ⬜
- **E2E** — _asserts that_ the invitee `POST`s decline, their pending offer is gone, and a subsequent accept is refused → **AC1/AC4** — ✅ decline half (`invitations.rs:408`); "subsequent accept refused" lands with the accept endpoint
- **E2E** — _asserts that_ an anonymous caller gets `401` → **AC1** — ⬜

## 🧠 7. Logic & shape

```
POST /accounts/{id}/invitations/accept         (body: { "list_on_profile"?: bool, default true })   ⬜ endpoint absent
  require_user(session)                  → 401
  load_account(id)                       → 404
  find_pending_invitation(account, user) → 404 no_pending_invitation if None   ← authority is implicit:
        (we look up the SESSION USER's own pending offer; a revoked/accepted/absent
         offer simply isn't found, so no membership is minted — AC1/AC4)
  invitation.accept(now)                 → (guards pending; 409 if somehow not)   ✅ exists
  accept_invitation(invitation,          ← ONE private-store transaction          ✅ pg complete / ❌ mem missing
                    list_on_profile)         UPDATE account_invitations SET state='accepted'  (0-rows ⇒ rollback)
        → UserAccount                        INSERT account_members(role, parent=inviter, listed_on_profile)
  → 200 { "account", "user", "role" }
```

Role tree after acceptance (rule 4a — the edge ZMVP-20 first writes; **now written in pg**):

```
        Owner (parent: none)            ← founder (ZMVP-14)
          │  issues invite, then invitee accepts
          ▼
        Member (parent: Owner)          ← parent = inviter, set on accept (account.rs:350)
```

`POST /accounts/{id}/invitations/decline` — ✅ **built** (`lib.rs:823-851`): the invitee kills their own offer: `require_user` → `find_pending_invitation(account, user)` → `invitation.revoke(now)` → `revoke_invitation(id)` → `200`.

State machine — ZMVP-20 writes the **accept** edge (transition ✅, pg persistence ✅, mem ⬜); **decline** reuses the revoked edge (✅ end-to-end); ZMVP-32 wrote issue/issuer-revoke:

```
        issue                 accept
   ∅ ─────────▶ pending ──────────────▶ accepted   (terminal — ALSO mints membership; ✅ in pg)
                   │
                   └──────▶ revoked     (terminal; can't be accepted — find_pending returns None)
                      ▲   issuer revoke  (ZMVP-32)  /  invitee decline  (ZMVP-20 ✅)
```

**Atomicity:** the accept transaction mirrors ZMVP-32's `create` (account + owner membership in one tx). The invitation flip and the membership insert now commit together in the pg adapter, with a 0-rows-affected rollback closing the lost-race window — a half-accept is impossible there. The remaining risk to the invariant is **parity**: the mem adapter must mirror the same all-or-nothing semantics once it exists.

## 🚀 8. Next steps

1. **Unbreak the build — adapter-mem `accept_invitation` (Engineer-owned, #1).** Mirror the pg semantics against the in-memory maps: only seat the member if the offer is still `Pending` (else mint nothing), set `parent = inviter`, store `listed_on_profile`, return the `UserAccount`. This is the literal compile blocker (`error[E0046]`) — nothing else can run until it's in.
2. **`POST /accounts/{id}/invitations/accept`** handler + `AcceptInvitationBody { list_on_profile?: bool }` (default `true`) + route, mirroring the finished `decline` handler: `require_user` → `find_pending_invitation` → `invitation.accept(now)` → `accept_invitation(invitation, list_on_profile)` → `200 { account, user, role }`.
3. **Un-`#[ignore]` the accept e2e** (`invitations.rs:491-522`) and add the missing assertions (parent==inviter, `list_on_profile:false` opt-out, revoked-can't-accept, anonymous 401) + the mem/pg adapter round-trips.
4. **Regenerate the `.sqlx` offline cache** for the new accept/membership `query!`s — the `cargo check` "Connection refused / type annotations needed" errors in adapter-pg are *only* sqlx's offline verification with no DB and a stale cache, not code defects. Run against a live DB (`just up`) or `cargo sqlx prepare`; commit the `.sqlx/` changes (same drill as ZMVP-32).
5. Then the full gate: `just test` green, `/document` the changed signatures, `/close-gaps --post`, and — since this touches the private↔public membership boundary only on the private side — confirm whether `/security-review` applies before the PR.

**Decisions — already resolved (carried forward):**
- ✅ **port signature** — `accept_invitation(&self, invitation: Invitation, listed_on_profile: bool) -> Result<UserAccount>`; membership derived from the invitation, `UserAccount` returned (both prior ⚠️ flags closed).
- ✅ **"List on profile?" — in scope.** Column added; set from the accept body (default listed); pg writes it.
- ✅ **Decline — in scope** ("revoking is an active thing"). Invitee-initiated; reuses `Revoked` + `Invitation::revoke()`. **Built.**
- ✅ **Endpoints** — `POST .../accept` and `POST .../decline` (both session-user-keyed; accept body carries `list_on_profile`).
- ✅ **Atomic op** — one composite `accept_invitation` port method (single private-store transaction). pg done.
- ✅ **Parent representation** — pg `account_members.parent` promoted to `uuid` FK → `users.id` (new migration); domain `Role` slot stays `Option<String>`.
- ✅ **Error model** — reuse ZMVP-35 `Problem`: `not_authenticated` (401), `account_not_found` (404), `no_pending_invitation` (404, `problem.rs:117-125`).

**Open / ⚠️ flags (need you):**
- ⛔ **The workspace does not compile** until adapter-mem implements `accept_invitation`. This is the gating item — it was merely "absent" last snapshot; the added trait method (no default) turned it into a hard build break.
- ⚠️ **Mem/pg parity of the all-or-nothing semantics** — the pg adapter guards the lost race via 0-rows rollback; the mem mirror must reproduce "pending-or-nothing" or the invariant holds only in prod.
- The `parent` text→uuid migration rewrites an existing column with a new FK — confirm no existing rows violate the FK before it ships (fine on a fresh dev DB; worth a note for any seeded environment).
- ZMVP-14's founder onboarding still owes a "list on profile?" step per decision 11 — out of scope here; founder row defaults to listed.
