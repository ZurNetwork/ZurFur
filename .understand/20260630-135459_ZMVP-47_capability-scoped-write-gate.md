# 🔎 Understanding ZMVP-47 — Capability-scoped write gating: account-scoped writes require an Account

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-47 · **Parent:** ZMVP-13 The Citizen (Accounts) · **Generated:** 2026-06-30 19:54 UTC · **Snapshot:** `.understand/20260630-135459_ZMVP-47_capability-scoped-write-gate.md`

## 🧭 1. Context (cold-start)

ZMVP-47 was **rescoped by DD [User as Actor & On-Demand Accounts](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/26247170)** (DECIDED 2026-06-30). The old premise — "a User must create an Account before they can write anything" — is **dead**. The replacement (DD decision 5): the write-gate is **capability-scoped**, not account-gated.

```
                 ┌───────────────────────────── can write ─────────────────────────────┐
 anonymous  ──▶  READ ONLY
 signed-in User ──▶  USER-SCOPED writes      (commission participation, Characters, reviews)   ← auth only
 signed-in User
   WITH a role on Account A ──▶  ACCOUNT-SCOPED writes on A   (Workflows, Portfolios,           ← auth + role-on-A
                                  plugin acquisition, set commission's managing account, brand)
```

The ticket's job: enforce that split — account-scoped writes demand "the actor owns / has the requisite role on the target Account" via **one shared check, not per-route checks that drift**; user-scoped writes need only auth; anonymous stays read-only. Rejections are RFC 9457 `application/problem+json`.

**The decisive cold-start fact (from code):** almost none of the routes this ticket is meant to gate exist yet. The named account-scoped *feature* surfaces (Workflows, Portfolios, plugin acquisition, brand) are domain stubs or absent; the user-scoped write surfaces (Characters, reviews, commission participation) are also unbuilt. The only write routes today are **account-management** (create account, grant/revoke role, leave, invitations) — and those are *already* gated by the exact seam this ticket describes building.

## 🗺️ 2. Domain

- **User vs Account** ([User 786439](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/786439), [Account 1966081](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081)): the User is the first-class actor (0..N Accounts); an Account is an on-demand `did:plc` brand/creator entity. Commission Participants and Characters are always User-anchored — that's *why* user-scoped writes need no Account.
- **Roles** ([2162692](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)): Owner > Admin > Manager > Member. "Has the requisite role on the Account" = the authorization predicate. Membership *is* the "has an Account" relation for account-scoped writes.
- **Capability scope (DD 26247170 decision 5)**: the settled taxonomy of which writes are user-scoped vs account-scoped. This is the design heart of the ticket and it is **already decided** — at the level of named surfaces.
- **Auth Surfaces** (DD [24543244](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/24543244)): cookie BFF (first-party) is the only write surface today; bearer `/plugin/v1` is exempt-by-construction and **not yet built** — so plugin-acquisition gating is forward-looking, not implementable now.
- **Error model** (DD [23592962](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/23592962)): bare success + RFC 9457 problem+json. Already adopted (ZMVP-35 merged).

## 🎯 3. Goal & scope

**Goal:** make the capability split *enforced and hard to forget* — so that every account-scoped write is gated by a single shared "actor has the requisite role on the target Account" check, user-scoped writes require only auth, and anonymous is read-only; rejections carry the correct RFC 9457 problem.

**In scope:**
- The enforcement *mechanism* — promote the existing ad-hoc per-handler gate (`require_user` + `actor_role` + `can_grant`) into **one reusable, drift-proof seam** (extractor / typed capability / guard) that a new account-scoped route cannot silently skip.
- A regression test layer asserting the three behaviors against whatever routes exist.
- Correct 401 (`not_authenticated`) / 403 (`forbidden`) wiring.

**Out of scope:**
- Building the account-scoped *feature* routes themselves (Workflows ZMVP-?, Portfolios, plugin acquisition, managing-account, brand) — they belong to their own tickets/epics.
- Building user-scoped write routes (Characters, reviews, commission participation).
- The bearer `/plugin/v1` surface and `app_key` handling (own track).
- Per-capability role thresholds for unbuilt surfaces (decided when each surface lands).

**⚠️ Scope tension:** with none of the target routes built, "gate the account-scoped writes" has almost nothing concrete to gate *today*. What ZMVP-47 actually ships now is a **scope/sequencing decision the Engineer owns** (see §8).

## 📦 4. Deliverables

- [ ] A **reusable account-capability seam** — one of: an axum extractor (`FromRequestParts`) that yields `(User, Role)` for a path-addressed account, or a typed `AccountCapability`/guard wrapping `require_user` + `load_account` + `actor_role`. Shape is an Engineer decision.
- [ ] (If the seam lands) the existing account-scoped handlers (`grant_role`, `revoke_role`, `leave_account`, invitation handlers) **refactored onto it** with no behavior change — proving the single path.
- [ ] Tests: anonymous → read-only / 401 on writes; signed-in User with no account → user-scoped write **succeeds** (needs ≥1 user-scoped write route, or a representative stand-in); signed-in non-member → account-scoped write **403**; signed-in member → succeeds.
- [ ] (Possibly) a dedicated problem variant if "requires an account" should read differently from the existing per-account `forbidden` (Engineer's call; today a non-member of account A simply gets 403).
- [ ] Doc comments on the new seam citing DD 26247170 decision 5.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Scope/sequencing decision** — given target routes don't exist, what does ZMVP-47 build *now* (mechanism+tests-on-existing vs defer/fold into first feature ticket)? | 4 — judgment, not effort | P1 | 🧑 Engineer | ⬜ — DD flags "ZMVP-47 rescope" as open follow-up; not yet disposed |
| **Enforcement-mechanism shape** — keep per-handler `actor_role()` calls vs extract a can't-forget extractor/typed capability | 4 — architecture choice × blast-radius (every future account route) | P1 | 🧑 Engineer | ⬜ — `actor_role`+`can_grant` exist (`api/src/lib.rs:667`, `domain/.../role.rs:97`) but as a *callable*, not an enforced single path |
| **Implement the seam** (extractor/guard) once shape is chosen | 3 — boilerplate over existing helpers | P2 | 🤖 Claude | ⬜ — nothing generic exists; gating is copy-pasted per handler |
| **Refactor existing account handlers onto the seam** | 2 | P2 | 🤖 Claude | 🟡 — the checks exist inline (lib.rs `grant_role`/`revoke_role`/`leave_account`/invitations); not yet unified |
| **Per-capability role thresholds** (which rank creates a Workflow/Portfolio, acquires a plugin, sets managing account) | 3 — domain | P3 | 🧑 Engineer | ⬜ — undefined; **moot until those surfaces exist** |
| **Test layer** (3 behaviors) | 2 | P1 | 🤖 Claude | 🟡 — account-scoped 401/403 already covered in `api/tests/accounts.rs`, `invitations.rs`, `leave.rs`; user-scoped "succeeds with no account" has **no route to test against** |
| **401/403 RFC 9457 wiring** | 1 | P2 | 🤖 Claude | ✅ — `Problem::not_authenticated`/`forbidden` exist (`api/src/problem.rs:70,82`) |

**Owner of the bulk:** 🧑 **Engineer / Split.** The load-bearing pieces are *decisions* (scope + mechanism shape), not mechanical code. The Claude-ownable code (extractor + refactor + tests) is small and **gated behind those decisions** and behind routes that don't exist.

## ✅ 6. Test checklist (TDD)

- **Integration** — _asserts that_ an anonymous (no-session) request to any write route gets `401 not_authenticated` problem+json → AC "anonymous read-only". (Covered today, e.g. `api/tests/accounts.rs`.)
- **Integration** — _asserts that_ a signed-in User who is **not a member** of account A is `403 forbidden` on an account-scoped write to A → AC "account-scoped writes reject actors lacking the role". (Covered today for membership/invitation routes.)
- **Integration** — _asserts that_ a signed-in member with the requisite role **succeeds** on the account-scoped write → AC. (Covered.)
- **Integration / ⚠️ blocked** — _asserts that_ a signed-in User **with no Account at all** succeeds on a **user-scoped** write → AC "User-scoped writes succeed for any signed-in User with no account". **No user-scoped write route exists yet** to assert against — this AC cannot be proven today without a stand-in.
- **Unit** — _asserts that_ the new shared seam yields `Role` for a member and `forbidden` for a non-member (mirrors `actor_role`) → AC "one shared check".
- **Regression** — _asserts that_ refactoring the existing handlers onto the seam changes no status code on any current path.

## 🧠 7. Logic & shape

What exists today (per-handler, hand-rolled — the "drift" the ticket warns about):

```
account-scoped write handler:
  actor   = require_user(state, session)?        # 401 if no session        (lib.rs:638)
  account = load_account(state, id)?             # 404 if unknown/deleted    (lib.rs:656)
  role    = actor_role(state, actor.id, id)?     # 403 if non-member  ◀── THE role-on-account check (lib.rs:667)
  if !role.can_grant(target) { 403 }             # per-action rank rule       (role.rs:97)
```

Used verbatim by `grant_role`, `revoke_role`, `invite_user_to_account`, `revoke_invitation_to_account`, `leave_account`. ZMVP-47's "one shared check, not per-route checks that drift" is essentially asking to **lift this block into a single seam** so a future Workflow/Portfolio route physically cannot forget it (cf. memory `feedback_make_unsoundness_unreachable`: make unsoundness unreachable, not caught). The check is *already correct*; the ticket is about **enforced singularity + a category taxonomy**, against routes that mostly don't exist yet.

```
WHAT THE TICKET TARGETS                WHAT EXISTS IN CODE
─────────────────────────────         ──────────────────────────────
account-scoped: Workflows             workflow.rs = empty stub
                Portfolios            (nonexistent)
                plugin acquisition    (nonexistent; /plugin/v1 unbuilt)
                managing-account      Commission.current_managing_account_id field only, no route
                brand                 (prose only)
                [account mgmt]        ✅ EXISTS + already gated
user-scoped:    commission particip.  (nonexistent)
                Characters            character.rs = stub
                reviews               (nonexistent)
```

## 🚀 8. Next steps

1. **⚠️ DECISION (Engineer) — disposition of ZMVP-47 right now.** Given the target routes don't exist, choose: **(a)** ship now as the reusable capability seam + refactor existing account handlers onto it + regression/behavior tests (delivers drift-proofing and a pattern the next account-scoped feature ticket inherits); or **(b)** defer / fold the gate into the first real account-scoped feature ticket (Workflows/Portfolios), keeping the inline `actor_role` pattern until then. The DD's own "Open/follow-up" table explicitly leaves "ZMVP-47 rescope" undisposed. *This is a real fork; pause here.*
2. **⚠️ DECISION (Engineer) — enforcement-mechanism shape.** If (a): extractor (`FromRequestParts` → `(User, Role)`) vs typed `AccountCapability` guard vs keep-and-document the helper trio. Interacts with whether a `requires_account`-style problem variant is wanted distinct from per-account `forbidden`.
3. **Then (Claude):** implement the chosen seam, refactor the 5 existing account handlers onto it (no behavior change), add the regression + behavior tests. Note the user-scoped "succeeds with no account" AC stays un-provable until a user-scoped write route exists — flag as handed-off rather than fake it.
4. **Defer (Engineer, moot now):** per-capability role thresholds — decided when each account-scoped surface is built.
5. Offer `/design-sync` on the Account / Roles glossary if the enforcement model gets a name worth recording; the DD itself is already authored.

**Open questions:** Does "the requisite role" mean *any membership* (floor, as `actor_role` does today) or a *minimum rank per capability*? (DD decision 5 says only "require an Account" = membership; rank-per-capability is per-surface, undefined.) — record as open.
