# 🔎 Understanding ZMVP-166 — storage: flat commission composition

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-166 · **Generated:** 2026-08-04 17:01 MDT · **Snapshot:** `.understand/20260804-170104_ZMVP-166_flat-composition-storage.md`

## 🧭 1. Context (cold-start)
DD 45514754 (amended same-day) replaced the recursive Surface/Component tree with a **flattened, fixed-depth composition model**: typed **Elements** contributed into core-declared **Surfaces**, **Tabs** one level up, effective visibility = `min(tab, surface, element)` under the commission's own visibility field. This ticket executes **D8 — the storage half**: drop the `commission_node` adjacency tree (singular — the DD text says "commission_nodes" but the repo convention is singular and the code always was), create the flat tables, repoint the satellites. Pre-alpha: **drop-and-recreate, not a data migration** (Engineer confirmed). Everything downstream (163's GET, 167's write gate, 170's renderer) builds on this schema.

## 🗺️ 2. Domain
**Commission** content (Index-canonical, Class B — never atproto) · **Surface** = declared extension point (skeleton-fixed, global) · **Element** = contributed leaf (envelope columns + opaque `jsonb` payload) · **Tab** = a Surface of kind tab, per-commission mode row · **Slot/Seat** = typed positions riding elements as identity-sharing satellites (28311564; "Slot" ≠ the extension point). Visibility modes P/D/T, Total default, closed-door (28246028's surviving rulings).

## 🎯 3. Goal & scope
Make the flat model the *only* storage reality while keeping every commit gate-green. **In:** migration (new tables + recreated satellites + invitation FK repoint), domain re-model (element/tab vocabulary + the three-term projection fn + a deliberately-minimal placeholder skeleton), both adapters, element-CRUD routes (owner-only — Engineer ruling), test suite rework, `queries.rs` regeneration. **Out:** composition-rule engine / plugin writers / ceilings / per-plugin bound (ZMVP-167) · the wire mint + GET (163) · the type catalog (171 — the placeholder skeleton is scaffolding, not a domain decision) · renderer (170).

## 📦 4. Deliverables
- [x] Slice 1: Justfile dev-back fix + DD index/ledger propagation (committed, `cc3a8ac4`)
- [ ] Migration via `just migrate-add`: `commission_tab`, `commission_element` (with `band` — Engineer ruling 2026-08-04), `commission_surface_mode`; drop `commission_node`; recreate `commission_slot`/`commission_seat` on element ids; repoint `commission_invitation.seat_id`
- [ ] `domain/src/elements/commission/element.rs` replacing `node.rs`: `ElementId`, `TabId`, `VisibilityMode`, `NewElement`, `ElementRow`, skeleton catalog const, `effective_visibility(tab, surface, element)`
- [ ] Reworked `ports/commission.rs` (flat signatures; `ParentNotASurface`/`CannotRemoveRoot` retired, `UnknownSurface`/`ElementNotFound` in)
- [ ] adapter-pg: ~13 SQL files rewritten/deleted, `commission.rs` rework, `queries.rs` regenerated (the `codegen_current.rs` tripwire green)
- [ ] adapter-mem mirror (`StoredElement` maps; parity)
- [ ] api: element CRUD routes replacing surfaces/components/remove; slots/seats/invitations on element ids; problem entries updated (`parent_not_a_surface`/`cannot_remove_root` retired)
- [ ] Tests green across all layers; stale `.sqlx` cache entry (`query-43ff0f8e….json`) deleted

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| Slice 1 infra | 0 | P3 | 🤖 Claude | Haiku-grade, already done | ✅ `cc3a8ac4` |
| Migration + satellites + FK teeth | 3 — blast radius | P0 | 🤖 Claude | **Opus** — migration = security-nature per policy | ⬜ `migrations/` ends at `20260725…` |
| Domain re-model (element/tab/skeleton) | 3 — surface area, settled-DD execution | P0 | 🤖 Claude | Sonnet | ⬜ `node.rs:1-685` all tree |
| Projection fn `min(tab, surface, element)` | 2 — small but visibility-critical | P0 | 🤖 Claude | **Opus** — private↔public boundary | ⬜ no flat projection exists |
| Ports rework | 2 | P1 | 🤖 Claude | Sonnet | ⬜ `ports/commission.rs:116` `load_tree` |
| adapter-pg rewrite + regen | 3 — 13 SQL files + codegen | P1 | 🤖 Claude | Sonnet | ⬜ `commission.rs:143` parent gate |
| adapter-mem mirror | 2 — mechanical, large (3380 LOC) | P1 | 🤖 Claude | Sonnet | ⬜ `StoredNode` at `commission.rs:146` |
| api routes + problems | 2 | P1 | 🤖 Claude | Sonnet | ⬜ `surfaces.rs`/`components.rs`/`remove.rs` all parent-premised |
| Test rework (~4,800 LOC, TDD-first per layer) | 3 — volume | P1 | 🤖 Claude | Sonnet | ⬜ `commission_node.rs` 12 tests tree-bound |

**Ticket build model = Opus** (migration + projection pull it up; rest rides Sonnet per piece).

## ✅ 6. Test checklist (TDD)
The ticket carries scope bullets, not formal ACs — mapping to those (S1–S7 in ticket order):
- **Unit** — _asserts `effective_visibility` clamps an element mode above its surface/tab (over-claim inert)_ → S3/DD-D4
- **Unit** — _asserts skeleton lookup rejects an unknown surface id (fail-closed)_ → S2
- **Integration (pg)** — _asserts an element citing another commission's tab is **unrepresentable** (FK violation, not app check)_ → S3
- **Integration (pg)** — _asserts `(commission, tab, surface, band, position)` uniqueness + delete renumbering_ → S4
- **Integration (pg)** — _asserts slot/seat satellites ride element ids; invitation flow intact end-to-end_ → S6
- **Integration (pg)** — _asserts `codegen_current` regenerates clean against the migrated schema_ → S7
- **Integration (mem)** — _asserts adapter-mem parity on every above behavior_ → S1–S6
- **E2E (api)** — _asserts POST element → 201, unknown surface → 4xx problem, DELETE element → 204 with satellite cascade_ → S2/S6

## 🧠 7. Logic & shape
```
commission (visibility, maturity — the formal root)
└─ commission_tab (commission_id, tab, mode)          UNIQUE(commission_id, tab)
     ▲ composite FK (tab_id, commission_id) — teeth: cross-commission cite unrepresentable
   commission_element (id, commission_id, tab_id, surface, type, mode, band, position,
                       created_by, created_at, payload jsonb)
       UNIQUE(commission_id, tab_id, surface, band, position) DEFERRABLE
   commission_surface_mode (commission_id, surface, mode)   -- absent row = Total
   commission_slot (element_id PK→element, …) · commission_seat (id PK→element, …)
effective: min(commission gate; tab.mode, surface_mode, element.mode) — three lookups, zero recursion
```
Skeleton v1 = a placeholder const (one tab `main`, one permissive surface) — **scaffolding until ZMVP-171**, marked as such in code.

Rulings taken at plan checkpoint (Engineer, 2026-08-04): ordering = position **+ reserved `band` column** (closes the DD's ordering-bands open row — record closure on the DD at ship); 166 scope = **element CRUD, no rules engine** (167 layers the gate); surface-mode home = own table (forced: surfaces are code-declared, no rows to hang a column on).

## 🚀 8. Next steps
1. `/implement` — TDD, slices in dependency order: [2] migration+regen (Opus) → [3] domain+projection → [4] adapter-pg → [5] adapter-mem → [6] api → [7] docs + DD ordering-bands closure note.
2. Slice PRs into `feature/zmvp-166-flat-composition-storage` per the two-tier flow; ship-gates + `/security-review` (migration + projection) before the PR to main.
3. ⚠️ Open: none blocking — both plan forks were ruled. The placeholder skeleton is explicitly non-binding (ZMVP-171 owns the real catalog).

**Blast-radius appendix (Explore inventory, 2026-08-04):** ~30 files / ~9,500 LOC implicated. Schema: `commission_node` (86-LOC migration incl. root backfill), satellites `commission_slot.node_id` + `commission_seat.id` (identity-sharing, PK = node PK), `commission_invitation.seat_id` transitive. Domain: `node.rs` 685 + re-exports + `Visibility::as_root_mode` (retires). Ports: `load_tree`/`add_surface`/`add_component`/`remove_node`/`declare_slots`/`declare_seat` + 4 error newtypes. adapter-pg: `commission.rs` 1085 (incl. `COMMISSION_NON_FACT_TABLES` list + compile-time assert), 13 tree SQL files, generated `queries.rs` 1880 (tripwire `codegen_current.rs`). adapter-mem: `commission.rs` 3380 + `MemBackend.nodes/slots/seats` + snapshot plumbing. api: 6 route files + 3 problem entries (`node_not_found`, `parent_not_a_surface`, `cannot_remove_root`). Tests: 6 api files (~2,850 LOC/43 tests, incl. the `FactBearingCommissions` manual port impl in `commission_delete.rs:317`), 4 pg it-files (~1,980 LOC/29 tests), 36 mem tests, 20 domain tests. Contract clean (no Surface*/Tree message — only prose comments at `commission.proto:52,101` to update at the 163 mint). Frontend clean. Stale `.sqlx/query-43ff….json` to delete.
