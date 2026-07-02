# 🔎 Understanding ZMVP-47 — Capability-scoped write gating: account-scoped writes require an Account

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-47 · **Generated:** 2026-07-01 15:56 MDT · **Snapshot:** `.understand/20260701-155620_ZMVP-47_capability-scoped-write-gating.md`

## 🧭 1. Context (cold-start)

Zurfur just inverted its onboarding model. The **old** rule — "a signed-in user can't write anything until they create an Account" — is **dead** (DD [User as Actor & On-Demand Accounts](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/26247170), decided 2026-06-30). A signed-in **User** is now a first-class actor: they can browse, participate in commissions (Client and/or Creator), keep Characters, and leave reviews **with zero Accounts**. An **Account** is an opt-in `did:plc` brand/creator entity created on demand via `POST /accounts`.

That splits every write into three tiers of authority:

```
  anonymous (no session)  ──►  READ-ONLY. every write = 401.
  authenticated User      ──►  USER-scoped writes OK (commissions, Characters, reviews)
                               ACCOUNT-scoped writes still need a role on the target Account
  User w/ role on Acct A  ──►  ACCOUNT-scoped writes on A OK (Workflows, Portfolios,
                               plugin acquisition, managing-account, brand surface)
```

The ticket's job: make **account-scoped** writes reject an actor who lacks the requisite role **on the target Account**, via **one shared check** rather than per-route checks that drift (memory `feedback_make_unsoundness_unreachable` — prefer one shared enforced path over per-site code). User-scoped writes need only auth; anonymous stays read-only.

**Reality check from the code** (evidence in §5): the shared floor *already essentially exists*. Auth is procedural (no `FromRequestParts` extractor) — every account write handler already threads `require_user` (401 floor) → `load_account` (404) → `actor_role` (403 for non-members) at the top, and `actor_role` (`accounts.rs:99`) over the `AccountStore::role_of` port already returns `Problem::forbidden()` for an actor with no role. **No user-scoped routes exist yet**, and the named account-scoped *capabilities* (Workflows, Portfolios, plugins, managing-account, brand) also have **no routes yet**. So this ticket is less "add enforcement everywhere" and more "**lift the ad-hoc floor into one named, hard-to-bypass shared gate** that the existing account routes retrofit onto and every future route inherits."

## 🗺️ 2. Domain

- **[User](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/786439)** — the first-class actor; member of **0..N** Accounts. In code: `domain::elements::user::User { id: UserId, did: Did }` (`crates/domain/src/elements/user.rs:47`). The actor's `UserId` feeds every role lookup.
- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081)** — opt-in `did:plc` creator/brand entity; owns the account-scoped capability surface. `domain::elements::account::Account` (`account.rs:122`).
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — each Account has a 4-role hierarchy `Owner < Admin < Manager < Member`. **Crucially, different account-scoped capabilities demand different ranks**: adding plugins = **Admin+**, changing account info = **Owner**, creating/handling commissions = **Manager+**, and there are Owner-only actions (delete, transfer). So "the requisite role on the target Account" is **not** a flat "is a member" test for every capability — the minimum role is **per-capability**. The membership row is `UserAccount { user_id, account_id, role }` (`user_account.rs:18`); the rank rule is `Role::can_grant` (`role.rs:97`); the lookup port is `AccountStore::role_of` (`ports.rs:207`).
- **DD 26247170 decision 5** — the governing rule: "The write-gate is capability-scoped, not account-gated." Names the account-scoped set (Workflows, Portfolios, plugin acquisition, being a commission's managing account, the brand surface) and the user-scoped set (commission participation, Characters, reviews).
- **[API Response Shape & Error Model](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/23592962)** (RFC 9457) — rejections are `Problem` (`problem+json`); `Problem::forbidden()` → `urn:zurfur:error:forbidden` (403) already exists and is the exact variant this gate returns; `Problem::not_authenticated()` → 401 is the anonymous floor.
- **[Transactions as a capability](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/24150017)** — writes go through `Database::begin()` → `UnitOfWork`; the write ports (`AccountWrites`, …) document "authorization is the caller's concern, settled before this is reached" — so the gate belongs in the **handler/shared helper**, never the store.
- **[Auth Surfaces, Plugin Trust Boundary & CSRF](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/24543244)** — first-party writes ride the cookie BFF (`zurfur.sid`, `SameSite=Lax` + Origin allowlist); the future `/plugin/v1` bearer surface is a distinct namespace (not yet implemented). This gate lives on the cookie surface for now.

## 🎯 3. Goal & scope

**Goal:** establish a **single shared authorization seam** for account-scoped writes so that any write targeting a specific Account is rejected (403 `forbidden`) unless the actor holds the requisite role on that Account — enforced in **one place** every such route flows through, not re-checked per handler. Keep user-scoped writes at the auth-only floor and anonymous read-only. Prove all three tiers with tests.

**In scope**
- A named, reusable **account-scope check** (generalizing the existing `actor_role` floor) that maps "no role on target Account" → `Problem::forbidden()` and "no session" → 401, and yields the actor's `Role` for capability-specific rank checks.
- **Retrofit** the existing account-management handlers in `accounts.rs` onto that one seam (dedupe the `require_user → load_account → actor_role` chain) so the shared path is the *only* path — establishing the pattern future routes inherit.
- Wiring the RFC 9457 error shape (reuse `Problem::forbidden()` / `not_authenticated()`; add a constructor only if the Engineer wants a capability-specific variant).
- The three-tier test matrix (anonymous 401, authed-User-no-role 403 on an account-scoped write, authed User success user-scoped + role-holder success account-scoped).

**Out of scope**
- Building the account-scoped **feature routes** themselves (Workflows, Portfolios, plugin acquisition, managing-account, brand) — they don't exist yet; this ticket makes their gate ready.
- Building the user-scoped routes (commissions, Characters, reviews) — also not yet present; the ticket only asserts they'd sit at the auth-only floor.
- The `/plugin/v1` bearer surface, CSRF (owned by ZMVP-23), session establishment/OAuth (ZMVP-23).
- Any change to role **semantics** or the role hierarchy itself.

## 📦 4. Deliverables

- [ ] A shared account-scope authorization primitive in `crates/api/src/routes/` (helper or `FromRequestParts` extractor — **shape is a fork, §8**) that: resolves the actor (401 floor), loads the target Account (404), looks up `role_of` (403 `forbidden` when absent), and returns the `Role` for optional rank refinement. Generalizes today's `actor_role` (`accounts.rs:99`).
- [ ] Existing account-management handlers in `accounts.rs` retrofitted to route through that one seam (no bespoke `require_user`/`actor_role` chains left that could drift).
- [ ] Reuse of `Problem::forbidden()` (403) and `Problem::not_authenticated()` (401); a new capability-scoped `Problem` constructor only if the Engineer wants distinct messaging.
- [ ] A documented **classification** of which route families are account-scoped vs user-scoped (owned by Engineer, §5) — recorded where routes are composed and/or reflected back to DESIGN via `/design-sync`.
- [ ] Integration tests covering all three tiers (see §6).
- [ ] Updated doc comments on the changed signatures (`/document`).

## 🧩 5. Work breakdown

> **Ownership boundary (explicit):** 🧑 **Engineer owns the *classification & role semantics*** — which route families are account-scoped vs user-scoped, and *what minimum role* each account-scoped capability requires (flat membership floor vs per-capability rank). 🤖 **Claude owns the *plumbing*** — the one shared check, retrofitting existing handlers onto it, the RFC 9457 error wiring, and the three-tier tests. The seam sits exactly at the check's **signature**: the Engineer decides *what the check must express* (its parameters — does it take a required `Role`? is it flat membership?); Claude *builds and wires* whatever shape that yields. Claude must **not** decide the classification or the min-role policy.

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Classify account-scoped vs user-scoped route families** (incl. forward-looking: Workflows/Portfolios/plugins/managing-account/brand = account; commissions/Characters/reviews = user) | 3 — judgment, not effort; per DD-5 but must be pinned as policy | P0 | 🧑 Engineer | ⬜ DD 26247170 §5 names the sets; not yet encoded in code |
| **Role semantics: flat membership floor vs per-capability minimum rank** (Roles page implies plugins=Admin+, account-info=Owner, commissions=Manager+) — sets the shared check's *signature* | 4 — domain invariant; shapes the API contract | P0 | 🧑 Engineer | ⬜ Roles page 2162692 defines ranks; the check's parameterization is undecided |
| **Shared account-scope check** — generalize `actor_role` into one named seam (helper/extractor) returning `Role` or `forbidden()`/`401` | 3 — mechanical once signature is fixed; small blast radius | P0 | 🤖 Claude | 🟡 `actor_role` (`accounts.rs:99`) + `role_of` port (`ports.rs:207`) already do the membership floor; needs lifting into one reusable, hard-to-bypass gate |
| **Retrofit existing `accounts.rs` handlers onto the shared seam** (grant/revoke/leave/invite/transfer/delete dedupe onto one call) | 3 — boilerplate, but touches 8 handlers | P1 | 🤖 Claude | 🟡 handlers already call the chain individually (`grant_role` 439, `delete_account` 267, `transfer` 858, invite 613, …) — consolidate |
| **RFC 9457 error wiring** — reuse `Problem::forbidden()`/`not_authenticated()`; add a capability-scoped constructor only if Engineer wants distinct messaging | 1 — constructors exist | P1 | 🤖 Claude | ✅ `Problem::forbidden()` (`problem.rs:82`), `not_authenticated()` (`problem.rs:70`) already present |
| **Three-tier test matrix** (anonymous 401 · authed-no-role 403 · role-holder success · user-scoped auth-only success) | 3 — mem-adapter integration harness | P0 | 🤖 Claude | ⬜ existing account tests assert role rules per handler; the three-tier capability matrix as a unit is new |
| **Docs + design-sync** — doc comments on changed signatures; reflect the classification into DESIGN if it hardens policy | 2 | P2 | 🤖 Claude (docs) / 🧑 Engineer (design call) | ⬜ |

**Difficulty note:** nothing here is individually hard — the value is *soundness*, not effort. The two Engineer rows are difficulty-3/4 not because they're laborious but because they're **domain judgment** (classification + min-role policy) that must not be Claude's call.

## ✅ 6. Test checklist (TDD)

Three tiers × the two scope classes. Written against the mem adapter (each test seeds a User, optionally an Account + membership row).

- **Integration** — _asserts that_ an **anonymous** request (no session cookie) to any write route is rejected **401 `urn:zurfur:error:not-authenticated`** and mutates nothing → **AC3** (anonymous read-only).
- **Integration** — _asserts that_ an **authenticated User with no role on target Account A** calling an **account-scoped** write on A is rejected **403 `urn:zurfur:error:forbidden`** (`application/problem+json`) → **AC1** (account-scoped writes reject actors lacking the Account/role with the correct RFC 9457 error).
- **Integration** — _asserts that_ an **authenticated User holding the requisite role** on Account A succeeds on the account-scoped write on A → **AC1** (positive path; proves the gate isn't a blanket deny).
- **Integration** — _asserts that_ an **authenticated User with zero Accounts** succeeds on a **user-scoped** write (representative: an account-scope-*exempt* route; if no user-scoped route exists yet, assert via a stub/representative handler that the shared gate is **not** applied) → **AC2** (User-scoped writes succeed for any signed-in User with no account).
- **Integration** — _asserts that_ a User with an *insufficient* rank (e.g. `Member` attempting an `Admin+`/`Owner`-only capability) is **403**, distinct from the non-member case → **AC1** refinement (only if the Engineer chooses per-capability rank, not flat membership).
- **Unit** — _asserts that_ the shared check maps `role_of == None` → `forbidden()` and missing session → `not_authenticated()`, and returns the `Role` on success → **AC1/AC3** (the one enforced path behaves identically regardless of caller).
- **Unit (regression)** — _asserts that_ the retrofitted `accounts.rs` handlers preserve their existing rank rules (`can_grant`, Owner-only delete/transfer) after consolidation onto the shared seam → guards against behavior drift during retrofit.

> Note: because there are no user-scoped or new account-scoped feature routes yet, the AC2 test and some AC1 tests may need a **representative/stub route** or must ride the existing `accounts.rs` surface. Whether to introduce a test-only route or wait for a real one is a small fork (§8).

## 🧠 7. Logic & shape

Current per-handler shape (each account write repeats this — the thing to consolidate):

```
require_user(state, session)?            // 401 floor  → Problem::not_authenticated()
  → load_account(state, path.id)?        // 404        → Problem::account_not_found()
    → actor_role(state, actor.id, id)?   // 403 if no role → Problem::forbidden()
      → [capability-specific rank check]  // e.g. role.can_grant(&target) → forbidden()
        → transaction(db, |uow| … )       // the write (UnitOfWork, DD 24150017)
```

Target shape — **one** shared seam every account-scoped route flows through:

```
        account-scoped write route
                  │
                  ▼
   ┌───────────────────────────────────────┐
   │  require_account_role(state, session,  │   ← THE shared gate (generalizes actor_role)
   │        account_id, [min_role?])        │
   │  · no session      → 401 not_auth      │
   │  · account missing → 404 not_found     │
   │  · role_of == None → 403 forbidden     │
   │  · role < min_role → 403 forbidden     │   ← only if Engineer picks per-capability rank
   │  · else            → Ok(Role)          │
   └───────────────────────────────────────┘
                  │ Ok(role)
                  ▼
            transaction(db, write)

   user-scoped write route → require_user only (auth floor); gate NOT applied
   anonymous              → require_user fails → 401 everywhere
```

The open design question is whether that seam is a **plain helper** (called first line, like today), a **`FromRequestParts` extractor** keyed on the `{id}` path param (harder to forget — closest to "make unsoundness unreachable"), or a **typestate token** the write port demands. That's §8.

## 🚀 8. Next steps

1. **⚠️ Engineer decision — classification (blocking domain fork):** confirm the account-scoped vs user-scoped route families. DD 26247170 §5 names them (account: Workflows, Portfolios, plugin acquisition, managing-account, brand; user: commission participation, Characters, reviews), but they must be pinned as policy since most routes don't exist yet. Claude must not decide this.
2. **⚠️ Engineer decision — role semantics (blocking domain fork):** does "the requisite role" mean a **flat membership floor** (any role passes; rank enforced separately per handler as today) or does the shared check take a **per-capability minimum `Role`** (plugins=Admin+, account-info=Owner, commissions=Manager+ per the Roles page)? This sets the shared check's **signature** and is the Claude/Engineer boundary. Recommendation to weigh: keep the shared check as the **membership floor** (returns `Role`), and let each handler apply its own `can_grant`-style rank check on the returned `Role` — mirrors today's split, keeps the one gate simple, avoids a capability→role table the Engineer would have to maintain. Engineer disposes.
3. **⚠️ Fork — scope reality:** since neither the new account-scoped feature routes nor the user-scoped routes exist yet, decide the deliverable's vehicle: **(a)** build the shared gate now and **retrofit the existing `accounts.rs` handlers** onto it (proves the one-path pattern; recommended — it's the make-unsoundness-unreachable move and gives the future routes a ready seam), or **(b)** defer until a first account-scoped feature route lands. Recommend (a).
4. **Fork — seam shape:** helper vs `FromRequestParts` extractor vs typestate token. Recommend a **`FromRequestParts` extractor** (e.g. `AccountRole`) keyed on the `{id}` path param so an account-scoped handler *cannot compile without* declaring it — the structural "unreachable" version — over today's easy-to-forget first-line helper. Confirm with Engineer (touches how routes declare scope). This is mechanical to build once chosen.
5. Once decisions 1–4 land: 🤖 Claude writes the §6 tests red, builds the shared check + error wiring, retrofits `accounts.rs`, greens the suite, `/document`, then `/close-gaps --post`.
6. **Offer:** if the classification/semantics decisions harden policy beyond DD 26247170, offer to reflect them into DESIGN (Roles / Account pages) via `/design-sync`, and note ZMVP-30's disposition (creator onboarding) is a related but separate ticket.
7. **Security note:** this change touches **authentication and the authorization boundary** — it must pass `/security-review` before the PR opens (Definition of Done).

**Dangling / open questions**
- No formal ACs on the ticket — the three "Done when" bullets are treated as AC1–AC3 above.
- Is there any account-scoped capability that a **non-member** should reach (e.g. a public read masquerading as a write)? Assumed no.
- Cap on Accounts per User is explicitly out (DD open item), no bearing here.
