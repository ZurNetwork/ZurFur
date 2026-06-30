# 🔎 Understanding ZMVP-30 — First sign-in provisions a default Account

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-30 · **Generated:** 2026-06-28 23:23 · **Snapshot:** `.understand/20260628-232316_ZMVP-30_default-account-provisioning.md`

## 🧭 1. Context (cold-start)

Zurfur recognizes identity rather than registering it. Two prior tickets built the halves this one joins:

- **ZMVP-9 (Done)** — first successful sign-in mints a `User` keyed by the visitor's DID (`UserRepo::provision`, idempotent). The human fills out nothing.
- **ZMVP-14 (Done)** — `User` founds an `Account`: mints the Account's own `did:plc`, writes the account row + the founder's `Owner` membership atomically (`Account::open` → `AccountRepo::create`). Today this is a manual `POST /accounts { name }` call.

ZMVP-30 **wires the trigger**: a User's *first* sign-in should automatically leave them owning exactly one default Account — invisible to the human, same "recognise-don't-ask" spirit as User provisioning. It does not invent new account machinery; it fires the existing creation path (ZMVP-14) once, at first login.

The provisioning point is the OAuth callback `signin_callback` in `backend/crates/api/src/lib.rs` (~lines 447–520), immediately after `user_repo.provision(&did)` (line 488) and before the redirect to `/me`.

## 🗺️ 2. Domain

- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081)** — a collection of users + a sovereign decentralized identity (`UUIDv7`/`ULID` internal key **and** `did:plc`). Glossary rule already states: *"A user gets their first account as part of onboarding."* Account type is **emergent, not a flag** — a one-member account is "personal," a multi-member one is a "studio." So the default Account is just a normal Account with one Owner member; nothing special to model.
- **[User-Profiles, the Handle Swap & Content Maturity 1DD (DECIDED 2026-06-22)](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/21594113)** — decision **7**: *"The first account is minted on first login."* This is the settled trigger ZMVP-30 implements. Decision **6**: personal account = one member (emergent). The old privacy objection (a public PLC directory correlating a sign-in-adjacent Account DID to the User) is **moot** — the new model exposes the User↔Account link by design; unlinkability lives between a person's separate *handles* (separate Users), not between a User and its Account.
- **Membership / Role** — `UserAccount { user_id, account_id, role }`, `Role::Owner(None)` for the founder. `Account::open(owner, did, name, now) -> (Account, UserAccount)` produces the pair.
- **Maturity** — explicitly **not** in play: Accounts carry no maturity (decision 8); it lives on commissions/products.

## 🎯 3. Goal & scope

**Goal:** at first sign-in, automatically run the existing account-creation path once so every User ends sign-in owning exactly one Owner-held Account with its own minted DID — idempotent across repeat sign-ins.

**In scope:**
- A trigger in the sign-in callback that provisions a default Account for a User who has none.
- Idempotency: minted once, never again (one default per User); repeat sign-ins create nothing.
- The default Account is minted with its own DID via the ZMVP-14 creation path.

**Out of scope:**
- Account *type* flag (it's emergent — do not add one).
- Maturity (Accounts carry none).
- The per-membership "list on profile?" onboarding choice (decision 11) — a separate concern.
- Invitations / multi-member studios / transfer / deletion.
- Any new public-profile surface.

## 📦 4. Deliverables

- [ ] A provisioning step invoked from `signin_callback` (api) that ensures the just-provisioned User owns a default Account.
- [ ] An **idempotent** "provision default account" operation — domain + repo support to detect "this User already owns ≥1 account" and skip (one default, minted once). Likely a new `AccountRepo` query (e.g. `owns_any(user)` / `list_for_user`) + a provisioning routine.
- [ ] DID minted for the default Account via the existing `DidMinter::mint()` path (verify it is the real ZMVP-14 path, not a stub).
- [ ] A **default Account name** value (the auto-mint needs one — `AccountName::try_new` requires a name). ⚠️ domain decision (see §8).
- [ ] E2E test: first sign-in → `GET /me`/account-listing shows exactly one Owner Account; second sign-in adds none.
- [ ] mem-adapter coverage for the idempotency query.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| Default-Account **name** decision (what does an auto-minted account get called?) | 2 — *decision, not effort* | P1 | 🧑 Engineer | ⬜ `AccountName::try_new` requires a non-empty name (`domain/src/elements/account.rs`); manual path takes it from the request body — auto-mint has no human input |
| Idempotent "user owns no account?" check + `AccountRepo` method | 4 — invariant + new port method | P1 | 🧑 Engineer | ⬜ No `owns_any`/`list_for_user` on `AccountRepo` today (`domain/src/ports.rs`); only `find`, `role_of`, `create`, `grant_role` |
| Wire the trigger into `signin_callback` (provision account after user) | 3 — mechanical but in the hot path | P1 | 🤖 Claude | ⬜ Callback provisions User only (`api/src/lib.rs:488`); no account step |
| Verify DID mint is the real ZMVP-14 path (not stub) | 2 — verification | P2 | 🤖 Claude | 🟡 `DidMinter::mint()` exists & used by `create_account` (`api/src/lib.rs:702`); agent flagged it as possibly a stub — confirm |
| Tests: e2e first-vs-repeat + mem-adapter idempotency | 3 — patterned after existing e2e | P1 | 🤖 Claude | ⬜ `e2e.rs` covers user provisioning; `accounts.rs` covers manual founding — neither covers auto-default |

## ✅ 6. Test checklist (TDD)

- **Integration/E2E** — *asserts that* after a first `POST /signin` → `GET /signin-callback?code=…`, the User owns exactly one Account with role `Owner(None)` → AC1.
- **Integration/E2E** — *asserts that* a second sign-in by the same DID provisions no additional Account (still exactly one) → AC2.
- **Unit/Integration** — *asserts that* the provisioned default Account carries a minted `did:plc` distinct from the User's DID → AC3.
- **Unit (mem adapter)** — *asserts that* the "owns any account?" query returns false pre-provision, true after → AC2 (idempotency mechanism).
- **Unit** — *asserts that* the default account name is well-formed via `AccountName::try_new` (no panic / no empty) → AC1 (depends on §8 decision).

## 🧠 7. Logic & shape

```
GET /signin-callback?code=…
        │
        ▼
auth.complete(code,state,iss) ──► did
        │
        ▼
user = user_repo.provision(did)        ← ZMVP-9 (idempotent: first contact)
        │
        ▼  ★ NEW (ZMVP-30) — idempotent guard
if !account_repo.owns_any(user.id):
        did_acct = did_minter.mint()                 ← ZMVP-14 path
        (acct, owner) = Account::open(user.id, did_acct, DEFAULT_NAME, now)
        account_repo.create(&acct, &owner)           ← one tx: account + Owner membership
        │
        ▼
session.cycle_id(); session.insert(user_id)          ← ZMVP-24 region (in flight)
        │
        ▼
redirect /me
```

Idempotency must key on **User ownership**, not "is this a new User" — a User minted in a prior sign-in that failed before account creation (partial state, dual-write spirit) must still get its default on the next login. "Owns no account → mint one" is the safe predicate.

## 🚀 8. Next steps

1. ⚠️ **DECISION (Engineer):** what name does the auto-minted default Account carry? The manual path takes it from the user; the auto path has no input. Options: derive from the User's handle/profile (needs a profile read at callback time), a fixed placeholder (e.g. "Personal"), or leave it editable later. This shapes a glossary detail and may warrant a one-line Account-page note. **Recommendation:** placeholder/handle-derived, renameable later — but the Engineer decides.
2. ⚠️ **DECISION (Engineer):** is the idempotency predicate "owns no account" (re-home partial state) vs "is a brand-new User"? Recommend the former for crash-safety; confirm.
3. Verify `DidMinter::mint()` is the real minting path ZMVP-14 shipped, not a stub — AC3 ("minted with its own DID") hinges on it.
4. **Sequencing:** this edits the exact `signin_callback` region that **ZMVP-24** (session rotation, *In Progress*) is actively rewriting and **ZMVP-23** (CSRF, *In Review*) just touched. Strongly prefer landing after both merge to avoid rebasing on a moving handler.
5. Add `AccountRepo::owns_any` (or `list_for_user`) to the port + pg + mem adapters; write the failing tests first.

**Open questions:** default account name (Q1); idempotency predicate (Q2); DID-mint stub status (Q3). All recorded; none resolved by Claude.
