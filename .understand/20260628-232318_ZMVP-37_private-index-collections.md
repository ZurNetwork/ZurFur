# 🔎 Understanding ZMVP-37 — Private (Index-resident) Collections

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-37 · **Generated:** 2026-06-28 23:23 · **Snapshot:** `.understand/20260628-232318_ZMVP-37_private-index-collections.md`

## 🧭 1. Context (cold-start)

A **Collection** is Zurfur's generic curation primitive — "a bag of `Referenceable`" (a typed, homogeneous set of references to entities), in **Static** (hand-curated) or **Dynamic** (`Lens`-query) form. The DD *"Collection as a Generic Referenceable Membership Primitive"* (DESIGN 24182787, DECIDED 2026-06-28) generalized it from a Post-only noun to a parameterized member type, and ruled that **at v1 every Collection is a public atproto record**. That decision deliberately left **one thing unbuilt**: a user has no way to keep a *private* list — a will-not-commission or wishlist they don't want broadcast on the public boundary.

ZMVP-37 is that follow-up. It adds a **private Collection variant resident in The Index** (the PostgreSQL private-data boundary), alongside the public atproto-record variant. The membership model is unchanged — only **residency** (Index vs PDS) and **visibility** (owner-only vs public) differ.

```
        Collection (one membership model: bag of Referenceable, Static/Dynamic, homogeneous, typed ref)
                              │
            ┌─────────────────┴──────────────────┐
   PUBLIC variant (DD v1)              PRIVATE variant  ◀── ZMVP-37
   atproto record on the PDS           resident in The Index (PostgreSQL)
   Decentralized boundary              Private boundary
   needs a Lexicon (ZMVP-38)           owner-only visibility, no lexicon
```

**Critical gate: this ticket is Post-v1 and blocked.** Its own description and the DD both say it is "not actionable until the generic Collection primitive and its public record exist." That primitive **does not exist in code yet** — there is no `Collection`, no `Referenceable`, no public variant. ZMVP-37 cannot start until that foundation lands (and that foundational ticket does not appear to be filed yet — see §8).

## 🗺️ 2. Domain

- **[Collection](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/8912899)** — a structural grouping that is a **reference, not a label**; holds **live references** re-evaluated at view time (a privatized/deleted member silently drops; per-viewer visibility evaluation). **Static** = explicit hand-curated bag; **Dynamic** = a `Lens` query producing the bag.
- **Referenceable** (DD decision 1) — a *new domain trait* in the capability family (`Taggable`, `Historical`, `Commentable`, `Rateable`) that any collectable entity implements (`Post`, `Account`, `User`, `Character`, …). **Does not exist in code yet.** The DD's open table flags that **`Account` + `User` are the v1 floor**; whether `Character`/`Post` are declared from the start is *open* (Engineer's call).
- **Typed reference** (DD decision 5) — a **DID** for DID-anchored members (an artist may be a person's DID or an Account's DID; Characters carry `did:plc`), an **internal id** for Index-local members (e.g. a Post).
- **Homogeneous** (DD decision 4) — one Referenceable kind per collection (a bag of Accounts, *or* a bag of Characters — not mixed).
- **Display-only** (DD decision 6) — favorite / wishlist / will-not-commission enforce *no* behavior; they are visible facts, not blocklists. ZMVP-37's Notes reaffirm: "Behavioral enforcement on these lists remains out of scope."
- **[The Index](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/10125333)** — the private Data Layer (PostgreSQL); home for "the few things that genuinely cannot be public yet," access **authorization-gated, not openly fetchable**.
- **[Data Boundaries](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/10354698)** — *residency* (where bytes live) is a **separate axis** from *sensitivity*. ZMVP-37 sits a Collection on the `Private` boundary: centralized in The Index, auth-gated. This is the boundary's purpose; no cross-boundary connector (`Connected`) is needed for a purely-private list.

The DD itself is the governing design; the **Collection glossary page (8912899) still describes a Post-only collection** and has not been updated to the generalized model — a `/design-sync` debt that the foundational primitive ticket (not ZMVP-37) should clear.

## 🎯 3. Goal & scope

**Goal:** give a Collection a **private residency option** — created in The Index, visible only to its owner — so a user can keep a will-not-commission or wishlist list **without** a public atproto record, reusing the exact same membership model as the public variant.

**In scope**
- A way to create/mark a Collection as **private**, resident in The Index (PostgreSQL private store).
- The **same `Referenceable` membership model** as the public variant (Static/Dynamic, homogeneous, typed reference) — only residency/visibility differ.
- **Owner-only visibility** enforcement (auth-gated reads; The Index rule).
- A private will-not-commission / wishlist kept with **no** atproto record.

**Out of scope**
- The **generic Collection primitive + public atproto variant** — a *prerequisite*, not this ticket (and not yet built).
- The **Lexicon** for the public Collection record — that is **ZMVP-38**, and a private Index-resident collection needs *no* lexicon (only Decentralized entities get one).
- **Behavioral enforcement** on the lists (hiding openings, invite-blocking) — display-only by DD decision 6.
- The **Lens engine** for Dynamic collections (shared construct, owned elsewhere).
- Any **public↔private migration** of an existing collection (not asked for; would be a `Connected`-boundary dual-write — flag if it ever surfaces).

## 📦 4. Deliverables

> ⚠️ All deliverables are **provisional** — their concrete shape is determined by the not-yet-existing generic Collection primitive. This is a structural/foundational-dependent ticket; the list is a projection of the ACs onto the current architecture, not a settled build plan.

- [ ] A domain representation of a Collection's **residency/visibility** (e.g. a `Visibility`/`Residency` discriminant on the Collection type) — **Engineer-owned domain shape**.
- [ ] A **`Referenceable`** domain trait + the v1 member-kind coverage (`Account`/`User` floor) — *likely lands with the foundational primitive, not here; this ticket consumes it*.
- [ ] A **private-store schema** for Index-resident collections: migration(s) in `backend/crates/adapter-pg/migrations/` (collection header + members, UUIDv7 keys, owner FK, member-kind + typed-ref columns).
- [ ] A **port (role-named trait)** for collection persistence in `domain/src/ports.rs` (e.g. `CollectionRepo`) — joining `UserRepo`/`AccountRepo`/`DidMinter`.
- [ ] An **adapter-pg** implementation (private-store writes/reads, owner-scoped).
- [ ] An **adapter-mem** parallel implementation (parity, for core dev + tests).
- [ ] **api** wiring (composition root) + HTTP surface to create/read a private collection (owner-gated).
- [ ] Tests: domain unit (membership invariants), adapter integration (testcontainers, owner-only visibility), e2e (create private list, not publicly fetchable).

## 🧩 5. Work breakdown

> **Current state is greenfield-with-a-missing-foundation.** Confirmed by grep: **no `Collection` and no `Referenceable` exist anywhere** in `backend/crates/` (the only "collection" hit is `std::collections::HashSet` in `commission.rs:14`). Domain elements present: `account, character (21-line stub), commission, golem, invitation, profile, role, user, user_account` (`domain/src/elements/`). Ports are role-named traits — `UserRepo, Authenticator, ProfileSource, ProfileCache, AccountRepo, DidMinter` (`domain/src/ports.rs`). **No `UnitOfWork` exists** anywhere (ZMVP-36 unbuilt); private writes today are bare-pool — `.execute(&self.pool)` and `self.pool.begin()` (`adapter-pg/src/account.rs:106,215,228,268`).

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Domain model: residency/visibility on Collection** (private vs public discriminant; how it composes with Static/Dynamic) | 6 — *uncertainty/blast-radius*: shapes the Collection entity & a DESIGN glossary page | P1 | 👥 Group | ⬜ — no `Collection` type exists (grep: none in `domain/`) |
| **`Referenceable` trait + v1 member coverage** | 6 — domain-shaping; DD open table (`Account`/`User` floor vs `Character`/`Post`) is an unresolved fork | P1 | 👥 Group | ⬜ — no trait exists (`grep Referenceable` → ∅); likely lands with the foundational primitive |
| **Private-store schema + migration** | 3 — follows the established UUIDv7 + FK migration pattern | P2 | 🧑 Engineer | ⬜ — no collection table (8 migrations, newest `20260627235014_parent_uuid_fk.sql`); shape gated on domain model |
| **`CollectionRepo` port** | 2 — mirror existing role-named trait pattern | P2 | 🤖 Claude | ⬜ — not in `ports.rs` |
| **adapter-pg impl (owner-scoped private writes/reads)** | 4 — *new private-store write sites*; collides with ZMVP-36 (see §8) | P2 | 🧑 Engineer | ⬜ |
| **adapter-mem parity impl** | 2 — boilerplate mirror | P2 | 🤖 Claude | ⬜ |
| **api wiring + owner-gated HTTP surface** | 3 — follows existing handler/route pattern + session-owner auth gate | P2 | 🤖 Claude | ⬜ |
| **Owner-only visibility enforcement** | 4 — *private↔public boundary*; security-review territory | P1 | 🧑 Engineer | ⬜ |
| **Tests (unit/integration/e2e)** | 3 — testcontainers pattern exists | P2 | 🤖 Claude | ⬜ |

**Owner split summary:** the two **domain-shaping** rows (Collection residency model, `Referenceable` trait) are **👥 Group** (6) — they shape glossary entities and carry the DD's open fork. The schema, adapter-pg impl, and visibility-enforcement rows are **🧑 Engineer** (3–4, private-store + the private/public boundary). The port, mem parity, api wiring, and tests are **🤖 Claude** (2–3, mechanical mirrors of established patterns) — *but every Claude row is downstream of the Group rows and cannot start until they're decided and the foundational primitive exists.*

## ✅ 6. Test checklist (TDD)

- **Unit** — *asserts that* a Collection can be constructed with **private** residency and rejects an invalid (mixed-kind) member set → AC2
- **Unit** — *asserts that* the private variant carries the **identical** Static/Dynamic + homogeneous + typed-reference membership invariants as the public one (one model, two residencies) → AC2
- **Integration (adapter-pg, testcontainers)** — *asserts that* creating a private Collection persists it in The Index with **no** atproto record emitted → AC1, AC3
- **Integration** — *asserts that* a private Collection read is **owner-gated**: the owner can fetch it, a non-owner cannot → AC1
- **Integration (parity)** — *asserts that* adapter-mem and adapter-pg agree on create/read/owner-visibility → AC1, AC2
- **E2E** — *asserts that* a user creates a private will-not-commission list and it is retrievable by them but **not** publicly fetchable (no PDS record) → AC1, AC3

## 🧠 7. Logic & shape

The one real structural question is **where "private" lives on the model** and how residency composes with the existing Static/Dynamic axis. Two candidate shapes (this is a **domain fork — Engineer decides**, do not pick):

```
  Option A — residency as a field on one Collection type
  ┌─────────────────────────────┐
  │ Collection<M: Referenceable> │   residency: Public(atproto) | Private(Index)
  │   kind: Static | Dynamic     │   visibility derives from residency
  │   members / lens             │
  └─────────────────────────────┘
     one type, residency is data → adapter routes to PDS vs Index

  Option B — residency as the adapter/port boundary
   public  → PublicRecords (adapter-atproto)   ┐ same domain Collection,
   private → CollectionRepo (adapter-pg/Index)  ┘ two stores, chosen at the port
```

`A` keeps one noun and pushes routing to composition; `B` leans on the ports-and-adapters split (public = `adapter-atproto`, private = `adapter-pg`) the architecture already draws. **No cross-store transaction** is involved (a private list is wholly Index-resident), so neither option trips the dual-write rule — *unless* a future "make this public/private" toggle is added, which **would** be a `Connected`-boundary dual-write (outbox-style, out of scope here).

## 🚀 8. Next steps

1. **⚠️ BLOCKED — verify the prerequisite exists/ is ticketed.** ZMVP-37 is Post-v1 and explicitly gated on "the generic Collection primitive and its public record." That primitive is **not in code** (greenfield: no `Collection`/`Referenceable`) and I did **not** find a filed ticket for *building* it (ZMVP-38 authors the *lexicon*, not the Rust primitive; the DD lists it only as "Ticketed" follow-ups). **Decision for the Engineer:** is the foundational build ticket filed? If not, ZMVP-37 cannot be scheduled until it is. *Do not start ZMVP-37 first.*
2. **⚠️ Resolve the `Referenceable` v1-coverage fork** (DD open table): `Account` + `User` floor only, or declare `Character` + `Post` from the start? This shapes both the trait and the schema's member-kind column. **Engineer's call.**
3. **⚠️ Domain-shape decision:** residency-as-field (Option A) vs residency-as-port (Option B) — §7. **Engineer's call**; offer a DD if it proves substantial.
4. **`/design-sync` debt:** the **Collection glossary (8912899) still says "bag of Posts"** and predates the generalization DD. It needs updating to the generalized model — owned by the foundational-primitive ticket, but flag it so ZMVP-37 doesn't inherit stale design.
5. Once unblocked: scaffold `CollectionRepo` port + migration + mem/pg impls + owner-gated api, TDD per §6. The Claude-ownable rows (port, mem parity, api wiring, tests) are ready mirrors of established patterns *after* the Group rows land.

### ⚠️ ZMVP-36 (Unit of Work) collision — flagged per request

ZMVP-37's adapter-pg impl introduces **new private-store write sites**. Today those would be written bare-pool (`.execute(&self.pool)` / `self.pool.begin()`, as in `account.rs`). **ZMVP-36** mandates that *all* private writes route through a compile-enforced `UnitOfWork` and adds a **CI guard banning `.execute(&self.pool)`**. So:
- **Ordering hazard:** if ZMVP-37 lands **before** ZMVP-36, it adds fresh bare-pool write sites that ZMVP-36 must then sweep up (more surface for its CI-guard migration). If ZMVP-37 lands **after**, it must be written on the UoW from the start.
- **Not a hard blocker** (ZMVP-37 is itself blocked Post-v1 and far downstream), but the two **must not be in flight in the same parallel set** without sequencing — ZMVP-36 is already recorded as deferred precisely because it "collides with new write sites." Whichever lands first dictates the other's write style. **Low practical collision risk given both are deferred/blocked**, but real if scheduled together.

### Collision with ZMVP-38 (Lexicon) & account.rs tickets

- **ZMVP-38 (lexicon):** **no code collision** — ZMVP-38 authors the *public* atproto lexicon; ZMVP-37 is *private*/Index-resident and needs **no** lexicon. They share the **`Referenceable` member-encoding** design input (both gated on the DD's open coverage table), so they share a *decision* dependency, not a file. ZMVP-37 does **not depend on** ZMVP-38; both depend on the foundational primitive.
- **account.rs / Account feature tickets:** **no collision** — ZMVP-37 lives in a new `collection.rs`/`CollectionRepo` surface, not `account.rs`. (`Account` merely becomes one `Referenceable` member kind — a trait impl, not an edit to membership logic.)
