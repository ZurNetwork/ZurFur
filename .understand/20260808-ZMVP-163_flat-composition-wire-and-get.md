# ZMVP-163 — GET /commissions/{id}: flat composition corpus messages + projection

**Snapshot:** 2026-08-08 · base `8d92eb17` (main, ZMVP-166 merge) · workspace
`/home/zuri/code/zurfur-backend` · bookmark `feature/zmvp-163-flat-composition-wire`
· lane: **Opus / security** (ticket's own Routing section).

---

## 1. Cold start

The commission read path. ZMVP-166 (merged, PR #173) landed the whole **server-side**
half of the flat composition — storage, domain types, both adapters, the write routes.
Nothing of it can leave the server: `CommissionComposition`, `ElementRow` and
`ElementPayload` deliberately carry **no `serde::Serialize`**, pinned by a
compile-probe test. This ticket builds the **only exit**: the contract messages, the
server-side projection that fills them, and `GET /api/v1/commissions/{id}`.

Governing decisions, fetched not recalled:

- **DD 45514754** *Commission Composition — Surfaces as Extension Points, Flat
  Elements and Tabs* (DECIDED 2026-08-04, amended same day). Flat Elements into
  code-declared Surfaces grouped by Tabs; `effective = min(tab, surface, element)`
  under the commission's own visibility; Total default everywhere; projection
  server-side at serialization; **no recursive message in the corpus**.
- **DD 42762241** *Surface Tree on the Wire* — superseded in part. **Survives:** D4
  `oneof payload { string opaque_json }` and its 2⁵³ rationale, D5 `created_by` off
  v1, **D6 withheld-envelope-at-birth (R4)**. **Dies:** SurfaceTree/SurfaceNode,
  nesting, the ≤63 depth cap, the surface-vs-component `oneof`.
- **DD 40992770** *The API Contract* — proto is authoritative over both tiers,
  messages only, handlers hand-written returning the generated type, `buf breaking`
  at WIRE_JSON gates CI.
- **`contract/VERSIONING.md`** R1 (lowerCamelCase), R3 (**sequential storage keys
  never cross the wire; wire ordering is dense per-projection renumbering computed
  AFTER the visibility filter**), R4 (absence may only mean "not set"), R6 (ids
  opaque), R7 (listings wrapped), R8 (**all v1 vocabularies are strings, never proto
  enums**), R11 (typed where Zurfur owns the vocabulary, opaque where a plugin does).
- **DD 46596098** (The Dialect) — `kind` is the client-facing discriminant standard.

The Jira comment of 2026-07-30 (serde_json `remaining_depth: 128` bracket budget) is
**void** — the Engineer voided it on 2026-08-04; no depth concept survives.

## 2. What already exists (so this ticket does not rebuild it)

| Thing | Where | State |
| --- | --- | --- |
| `commission_tab` / `commission_element` / `commission_surface_mode` | `backend/crates/adapter-pg/migrations/20260805234354_flat_commission_composition.sql` | merged; composite FK `(tab_id, commission_id)` makes a cross-commission tab **unrepresentable** |
| `VisibilityMode` (Total < Presentation < Description, **declaration order IS the ladder**) | `backend/crates/domain/src/elements/commission/element.rs` | merged |
| `effective_visibility(tab, surface, element)` = `min` | same file | merged, exhaustively tested |
| `CommissionComposition::effective_visibility_of` (fail-closed on missing tab) | same file | merged |
| `SKELETON` (code-declared tabs→surfaces; placeholder `main` / `content`), `declares_surface` | same file | merged, **placeholder — ZMVP-171 owns the real one** |
| the non-`Serialize` guard + `SerializeProbe` | same file | merged |
| `CommissionStore::load_composition(id) -> Option<CommissionComposition>` | `backend/crates/domain/src/ports/commission.rs` | merged, both adapters |
| `require_participant` (uniform 404, never 403 — no existence oracle) | `backend/crates/api/src/routes/commissions/mod.rs` | merged |
| handler pattern returning generated types | `backend/crates/api/src/routes/commissions/list.rs` | merged |
| both-tier codegen | `just gen-contract` → `cargo run -p contract-gen` + `cd contract && buf generate` | merged |
| weld/golden tests | `backend/crates/api/tests/golden_wire.rs`, `contract_routes.rs`, `contract_current.rs`; TS `frontend/web/src/lib/server/api/contract-tolerant-reader.spec.ts` | merged |

## 3. The real goal

Mint the flat composition's **first and final** wire shape, and the server-side
projection that is the only way composition content leaves the process. Every field
minted here is a one-way door (`/api/v1` is GA'd, `buf breaking` gates CI) — so the
discipline is: **omit what is doubtful, state what is withheld, never emit a stored
ordinal.**

## 4. Scope

**In:** corpus messages for the flat composition · `GET /api/v1/commissions/{id}` ·
the projected exit type (the only `Serialize` path) · both-tier weld tests · the R11
extension text in `contract/VERSIONING.md`.

**Out:** the non-participant tier mapping (**ZMVP-75** — 42762241 reserved it
explicitly: *"Non-owner semantics: ZMVP-75's lane entirely; this DD only reserves the
withheld discriminant"*) · the write gate (**ZMVP-167**) · the type catalog
(**ZMVP-171**) · the renderer (**ZMVP-170**) · seat ceilings (**ZMVP-96**).

## 5. The wire shape as built (every field a ruling)

```proto
message CommissionTab     { string id = 1; string tab = 2; string mode = 3; }
message CommissionSurface { string surface = 1; string tab_id = 2; string mode = 3; }
message CommissionElement {
  string id = 1; string tab_id = 2; string surface = 3;
  string kind = 4; string mode = 5;
  oneof payload { string opaque_json = 6; }
}
message GetCommissionResponse {
  // the commission envelope, FLAT (the corpus's per-endpoint-message style)
  … id, title, lifecycle, visibility, deadline?, maturity?, …, created_at
  bool composition_withheld = 11;   // D6, minted at birth
  repeated CommissionTab tabs = 12;
  repeated CommissionSurface surfaces = 13;
  repeated CommissionElement elements = 14;
}
```

- **No `position`, no `band` on the wire.** R3 is explicit: stored ordinals never
  cross, wire ordering is dense per-projection order computed *after* the filter.
  Emitting the stored `position` would hand a later non-participant a **gap oracle**
  (count the holes, learn how many elements are hidden). Served order is the order.
  `band` additionally is **placeholder vocabulary** (Engineer, 2026-08-04) whose fork
  is still open on ZMVP-171 — omitting keeps it open, emitting closes it forever.
- **No `created_by`** (DD D9 / 42762241 D5) and **no element `created_at`** — same
  doctrine, objective 5: omission is additive later, exposure is forever.
- **Three modes, one per level** — the Engineer's 2026-08-04 comment ("the wire's
  mode fields should reflect the three-level grain").
- **`composition_withheld`** is D6's discriminant, minted now, always `false` on this
  ticket's participant path. It is the field "ship owner-only, extend later" would
  quietly have failed on.
- Vocabularies are **strings** (R8), not enums.

## 6. Test checklist (TDD order)

1. **Domain/projection unit** — `min` clamp already proven; new: an element whose
   effective mode is below the viewer tier is **absent from the projection**, and a
   tab/surface below the tier is absent with it.
2. **The exit-type guard** — the projection type is the only `Serialize`; the raw
   composition still is not (extend the existing `SerializeProbe` assertions).
3. **Route** — owner gets 200 with the skeleton + elements; **non-participant gets
   the uniform 404**, same body as a nonexistent id; unauthenticated gets 401.
4. **Payload fidelity** — opaque JSON round-trips verbatim through `opaque_json`,
   including an integer above 2⁵³ (the 42762241 D4 regression guard).
5. **Golden wire** — lowerCamelCase keys (R1), absent optionals omit (R4), no
   `position`/`band`/`createdBy` key anywhere in the element object.
6. **Both-tier weld** — the same JSON decodes through protobuf-es `fromJson`.
7. **Contract gates** — `contract_routes` sees the new declared route; `buf lint`;
   `contract_current` (regenerated output committed).

## 7. Forks the Engineer must rule (built the reversible way, flagged for pre-merge)

Recorded in §"Engineer forks" of the final report. Nothing here was decided to keep
momentum; each is the option that keeps the door open.

## 8. Next steps

1. Mint the proto, regenerate both tiers.
2. Build the projected exit type + projection in the api crate.
3. The handler + route.
4. Weld/golden/route tests.
5. VERSIONING.md R11 extension.
6. Gate; report; **do not push, do not open a PR**.
