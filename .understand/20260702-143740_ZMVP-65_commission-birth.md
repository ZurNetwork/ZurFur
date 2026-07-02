# 🔎 Understanding ZMVP-65 — User creates a commission and owns it

> **Status:** To Do (branch active, Engineer building) · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-65 · **Generated:** 2026-07-02 14:37 · **Snapshot:** `.understand/20260702-143740_ZMVP-65_commission-birth.md`

## 🧭 1. Context (cold-start)
The **birth ticket** for the `Commission` aggregate — deliberately minimal: only the row and the *fixed metadata that always exists*. No tree, no seats, no participants beyond the creator-owner. Any authenticated **User** may create one — **no Account required** (user-scoped write, per ZMVP-47/DD 26247170). Everything else (content tree of Surfaces/Components, Seats/Slots, managing account, lifecycle transitions) arrives in later tickets.

The Engineer is mid-implementation on branch `feature/ZMVP-65-...`: the domain element, the pg adapter (migration + `PgCommissionWrites`), the `CommissionWrites` port + its `UnitOfWork` accessor, and the `POST /commissions` route already exist in the working tree. **The `adapter-mem` half is missing — which currently breaks the build** (`MemUnitOfWork` doesn't implement the new required `commissions()` method).

## 🗺️ 2. Domain
- **Commission** (DESIGN/Commission `3276807`, re-modelled 2026-07-01) — the platform's most basic unit of work; a first-class citizen isolated from Accounts (survives account deletion; participants are always Users). Fixed skeleton in v1 birth: **UUIDv7 id, Title (fixed, always present), owner (the creating User), Lifecycle, created_at**, nullable deadline.
- **Lifecycle** — a commission always holds exactly one state; birth state is `Draft` (the only hard-deletable, fact-free state).
- **Visibility** — the flat `Private`/`Listed`/`Public` field is **superseded** (Surfaces DD `28246028`). `Private` is now the **default closed-door** that falls out of the root surface's `Total` mode. Since the surface tree materializes in the *tree-birth* ticket (not here), **no visibility field belongs on the birth row** — AC "Private visibility" is satisfied by the default, not a stored column.
- **UnitOfWork / transactions-as-a-capability** (DD `24150017`) — the commission write lives only on `uow.commissions()`; mem must mirror pg's stage/commit-or-rollback fidelity.

## 🎯 3. Goal & scope
**Goal:** a signed-in User `POST`s a Title and gets a `Draft` commission they own, persisted through the Unit of Work — with the `adapter-mem` fake at parity so the in-process/e2e path compiles and runs.

**In scope (this session — the missing piece the user asked for):**
- `adapter-mem`: a `CommissionWrites` fake + `MemUnitOfWork::commissions()`, staged/rolled-back exactly like accounts, with tests.

**Out of scope (Engineer's lane / already theirs):** the domain element, pg adapter, port, route (all already in the working tree, authored by the Engineer). Tree/surfaces, seats, participants, managing-account, lifecycle transitions — later tickets.

## 📦 4. Deliverables
- [x] `domain::elements::commission::{Commission, LifecycleStep}` — Engineer (present)
- [x] `CommissionWrites` port + `UnitOfWork::commissions()` — Engineer (present)
- [x] pg: `commission` migration + `PgCommissionWrites` + uow wiring — Engineer (present)
- [x] `POST /commissions` route — Engineer (present)
- [ ] **`adapter-mem`: `MemCommissionWrites` + `MemUnitOfWork::commissions()` + staged commissions map + test helpers + tests** ← this session
- [ ] Build + full test suite green

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| domain element + port + accessor | 3 | P0 | 🧑 Engineer | — | ✅ `domain/src/elements/commission.rs`, `ports.rs:397` |
| pg adapter (migration, writes, uow) | 3 | P0 | 🧑 Engineer | — | ✅ `adapter-pg/src/commission.rs`, `uow.rs:70` |
| `POST /commissions` route | 3 | P0 | 🧑 Engineer | — | ✅ `api/src/routes/commissions.rs` |
| **adapter-mem `CommissionWrites` + staging + tests** | 2 — mirror of the account fake | P0 | 🤖 Claude | **Sonnet-tier work, executed here** — mechanical parity, non-security | ⬜ missing (build breaks: `E0046 missing commissions`) |

- The mem piece is Claude's mechanical lane: it re-uses the exact `StoredAccount`/`stage`/`apply`/rollback pattern already established; no domain judgment, no security surface.

## ✅ 6. Test checklist (TDD)
- **Unit (mem)** — _asserts that_ a commission written through `uow.commissions().create()` + `commit()` is readable back with `Draft` lifecycle and the creating user as owner → AC1, AC2, AC3.
- **Unit (mem)** — _asserts that_ dropping the unit before `commit()` discards the commission (rollback fidelity, DD 24150017) → mirrors pg.
- **Unit (mem)** — _asserts that_ an uncommitted commission is invisible until `commit()`.
- **Integration (api, Engineer's harness)** — _asserts that_ `POST /commissions` with a Title as a signed-in User → `201`, owner = caller, no Account required → AC1, AC2, AC4.

## 🚀 8. Next steps
1. Add the `adapter-mem` commission fake (staged map, `MemCommissionWrites`, `commissions()` accessor, inspect helpers) + tests → green build.
2. `/critique` the mem diff against the established account/invitation fake patterns.
3. `/security-review` — low surface (no auth/boundary/DID/session change in the mem diff), but run per the DoD trigger check.
4. `/document` the changed mem signatures.
5. Hand back to the Engineer for the route/pg gate + PR.

**Notes / minor observations (not blockers):**
- ⚠️ `domain/src/ports.rs:14` imports `CommissionId` but never uses it → unused-import warning (Engineer's WIP; a one-line mechanical fix — will surface in `/critique`).
- The pg table is named `commission` (singular) while other tables are plural (`accounts`, `account_members`). Naming is the Engineer's call; noting for consistency, not changing.
