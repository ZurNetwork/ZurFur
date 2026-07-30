# 🔎 Understanding ZMVP-153 — A user creates and deletes a Commission from the browser; the surface tree renders

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-153 · **Generated:** 2026-07-27T23:34:30-06:00 · **Snapshot:** `.understand/20260727-233430_ZMVP-153_commission-browser-tree.md`
> **Epic:** ZMVP-149 *The Other Side* · **Blocked by:** ZMVP-151 ✅ Done, ZMVP-157 ✅ Done · **Parallel sibling:** ZMVP-152 (accounts)
> **Grounded at:** `main` @ `876e94d`

```
   ┌──────────────────────────── The Other Side (ZMVP-149) ────────────────────────────┐
   │  150 ✅ app boots   151 ✅ sign in/out   152 ⬜ accounts   153 ⬜ COMMISSIONS ←── │
   └───────────────────────────────────────────────────────────────────────────────────┘
        exit criterion: one browser session · sign-in → account → commission →
                        the tree renders → delete both → sign out · ZERO CURL
```

## 🧭 1. Context (cold-start)

Zurfur's backend has shipped ~47 HTTP routes over four epics. Until three weeks ago it had **no client** — the only UI was a bare-HTML sign-in page. Epic ZMVP-149 pulls the frontend toolchain in: a SvelteKit app at `frontend/web`, same-origin behind Caddy, which strips `/api/v1/*` and reverse-proxies to axum (axum's own route table is **unprefixed** — there is not one `.nest()` in the composition root).

ZMVP-153 is the epic's last and heaviest ticket. It builds two screens — `/commissions` (list + create) and `/commissions/{id}` (detail: read-only surface tree + delete) — and its walkthrough **is** the epic's exit criterion.

Three things happened *after* this ticket was written (2026-07-19) that change how it must be built:

1. **Epic ZMVP-25 landed the protobuf contract corpus.** `contract/zurfur/api/v1/*.proto` is now authoritative over both tiers (DD 40992770). Rust generates via prost+pbjson into `backend/crates/api/src/generated/`; TypeScript generates via protobuf-es into `frontend/web/src/lib/server/api/generated/` — **server-side only, below the runes seam**.
2. **`contract/VERSIONING.md` is BINDING** (ratified 2026-07-27, rules R1–R11). It constrains every new field this ticket adds.
3. **The boundary decoder is generated.** `decodeContract()` in `frontend/web/src/lib/server/api/zurfur-api.ts:35` runs protobuf-es `fromJson` with `ignoreUnknownFields: true`. Any endpoint the frontend consumes should arrive through a generated schema — which means **the endpoint must be in the `.proto` corpus first**.

The load-bearing discovery of this briefing: **the corpus declares 9 endpoints; the router serves 47.** ZMVP-153 needs endpoints from both sides of that gap — and one endpoint that exists on *neither*.

## 🗺️ 2. Domain

- **Commission** (DESIGN `3276807`) — the Reason's aggregate. User-scoped; no Account required (DD 26247170). Entirely Index-side, never atproto (closed door).
- **Surfaces & Components** (DD `28246028`) — a commission's content is a recursive tree. A **Surface** is an interior node carrying a visibility **mode** (Presentation / Description / Total); a **Component** is a leaf that *inherits* its parent's mode. Every commission has exactly one root surface, and **the root's mode IS the commission's visibility** (Private = root Total, Listed = Presentation, Public = Description). Effective visibility is **min-of-ancestors**, computed server-side at serialization. **Participants always receive Total** — brief access *is* participation.
- **Tree Storage** (DD `28409880`) — `commission_node` adjacency rows: envelope as real columns, payload as opaque `jsonb`, integer `position` for sibling order. Read model is **whole-tree load**: one indexed query, assembly + projection **in Rust**.
- **Deletion of Commissions** (DD `3014657`) — hard delete while **fact-free**; once any fact exists (Product, rating, EXP, achievement, payment) the commission is immutable and only **Archive** (soft delete) is permitted. DD `28246028` D9 fixes the wording: *Delete = hard delete, Archived = soft delete.*
- **The Fact contract** (ZMVP-67) — a `Fact` marker trait plus `commission_has_facts(id) -> bool`, deliberately placed on the **transactional write view** so the gate runs inside the same transaction as the delete (no TOCTOU).
- **The closed door** — whether a commission *exists* is participant-only knowledge. Every commission handler answers a non-participant *and* a truly absent id with the same uniform `404`, never `403` (a `403` is an existence oracle).
- **Frontend stack** (DD `39944194`) — Effect lives only under `src/lib/server/**`; the seam is `ManagedRuntime.runPromise` at hooks/loads/actions; **runes never see a fiber**; no `Option` (strict-null `T | undefined`); RFC 9457 → a tagged error union. D4 amended by DD `40992770`: the API boundary decodes via the **generated** protobuf-es decoder.

## 🎯 3. Goal & scope

**The goal:** make the Reason clickable — and, in doing so, force the first serialization of the commission content tree that has ever left the server.

That second clause is the real work. The ticket reads like UI, but AC2 cannot be satisfied by any endpoint that exists.

**In scope**
- `/commissions` — list the signed-in user's owned commissions, offer create.
- `/commissions/{id}` — render the composed surface/component tree, read-only, owner-POV.
- Delete honoring the Fact contract as the API exposes it; the list reflects the outcome.
- The backend + contract work that makes the above possible (see §5 — this is most of the ticket).
- The epic's exit walkthrough, end to end.

**Explicitly out**
- Tree **writes** — no add-surface / add-component / remove-node affordance anywhere in the UI (AC4 is a *negative* criterion and must be tested as one).
- The non-participant projection view (**ZMVP-75**, backend-open, excluded from the epic).
- Changelog, uploads, status/lifecycle, seats/invitations, phases, invoices (epic ruling 6a).
- Visual design of any kind (epic: "crude-but-real", "no design work rides this epic").
- The `creator?` onboarding prompt (ZMVP-30 stays orphaned, ruling 5b).
- Vanity/handle routes — addressing is UUIDv7 (ruling 8y).

## 📦 4. Deliverables

**Contract (`contract/zurfur/api/v1/commission.proto`)**
- [ ] `GetCommission` rpc — `get: "/api/v1/commissions/{id}"` — with its request/response messages.
- [ ] The tree wire messages (node envelope + payload + children) — **shape is an Engineer decision, see §8 ⚠️D1**.
- [ ] ⚠️D3 — `DeleteCommission`, `ArchiveCommission`, `UnarchiveCommission` rpcs declaring the three endpoints already served but never declared.
- [ ] Regenerated artifacts committed on both tiers (`just gen-contract`): `backend/crates/api/src/generated/**`, `frontend/web/src/lib/server/api/generated/zurfur/api/v1/commission_pb.ts`.

**Backend (`backend/crates/api/`)**
- [ ] `routes/commissions/get.rs` — the `GET /commissions/{id}` handler, owner-POV, behind `require_owner`/`require_participant`.
- [ ] A **projected**, serializable tree type (the raw `CommissionTree` is deliberately not `Serialize`).
- [ ] `routes/commissions/mod.rs` — `.route("/commissions/{id}", get(...).delete(...))`.
- [ ] `tests/contract_routes.rs` — the `served` set transcription extended and the `declared.len()` count bumped from 9.

**Frontend infrastructure (`frontend/web/src/lib/server/`)**
- [ ] `api/zurfur-api.ts` — `listCommissions`, `getCommission`, `createCommission`, `deleteCommission` (+ archive) on `ZurfurApiShape`, `ZurfurApiLive`, `anonymousDefaults`.
- [ ] `api/errors.ts` — the arm that carries the `409 commission_has_facts` refusal to the caller.

**Frontend UI (`frontend/web/src/routes/`)** — Engineer's lane
- [ ] `(session)/commissions/+page.svelte` + `+page.server.ts` — list + create form.
- [ ] `(session)/commissions/[id]/+page.svelte` + `+page.server.ts` — detail, tree render, delete.
- [ ] A recursive tree-render component (surface → children, component → leaf).
- [ ] `/` signed-in landing shows the user's commissions (epic URL map).

**Tests**
- [ ] `backend/crates/api/tests/commission_read.rs` (new) — the single-commission read, closed door, tree shape.
- [ ] Vitest specs beside every new `.server.ts` and `.svelte`.
- [ ] The exit walkthrough.

## 🧩 5. Work breakdown

> **Owner split per the Engineer's directive (2026-07-27):** *"I want to implement the UI. I care little if you implement infrastructure."* All UI is the Engineer's to build; infrastructure beneath it is Claude's; `+page.server.ts` loads/actions straddle and are marked 👥 with the boundary stated. **Ownership of the *build* is not ownership of the *decisions*** — every ⚠️ in §8 remains the Engineer's call regardless of who types the code.

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| **D1** Decide the tree's wire shape (first-ever tree serialization; sets the precedent ZMVP-75 inherits) | — (decision) | P0 | 🧑 Engineer | — | ⬜ **BLOCKS everything below.** No prior art: `CommissionTree` is deliberately non-`Serialize` (`domain/src/elements/commission/node.rs:398-413`) |
| **D2** Decide the delete UX contract (try-DELETE→409→archive: automatic or confirmed?) | — (decision) | P1 | 🧑 Engineer | — | ⬜ API dictates via `409 commission_has_facts` (`api/src/problem.rs:255-263`); the *client* picks the branch |
| **D3** Decide whether the served-but-undeclared delete/archive endpoints enter the corpus now | — (decision) | P1 | 🧑 Engineer | — | ⬜ 3 endpoints served, 0 declared (`routes/commissions/mod.rs:140-160`) |
| **1** Contract: `GetCommission` + tree messages in `commission.proto` | 3 — precedent-setting, R4/R8/R11-constrained | P0 | 🤖 Claude *(Engineer directive: infra)* | **Opus** — contract precedent + the visibility boundary | ⬜ corpus declares 9 rpcs; no `GetCommission` (`contract/zurfur/api/v1/commission.proto:98-116`) |
| **2** Backend: projected tree type + `GET /commissions/{id}` handler | 6 — **security-nature**: the non-`Serialize` guard exists precisely to stop this | P0 | 🤖 Claude *(infra)* | **Opus** — mandatory; `/security-review` required | ⬜ route is `delete`-only (`routes/commissions/mod.rs:140-143`); `load_tree` port exists (`domain/src/ports/commission.rs:116`) and has **zero** callers in `api/src` |
| **3** Contract: declare `DELETE`/`archive`/`unarchive` (pending D3) | 2 — mechanical once decided | P1 | 🤖 Claude *(infra)* | Sonnet | ⬜ served, undeclared |
| **4** Weld-test upkeep: `served` transcription + `declared.len()` 9 → n | 1 | P0 | 🤖 Claude *(infra)* | Haiku — ships inside 1/3 | ⬜ `tests/contract_routes.rs:115-151` |
| **5** Frontend infra: `ZurfurApi` commission methods + error arm + generated-type adoption | 3 — follows the `liveMe` precedent exactly | P0 | 🤖 Claude *(infra)* | Sonnet | ⬜ `zurfur-api.ts` exposes only `me`/`startSignin`/`signout`; `commission_pb.ts` is generated and **unused** |
| **6** UI: `/commissions` list + create form, inline validation | 4 | P0 | 🧑 **Engineer** *(directive)* | — | ⬜ no `routes/(session)/commissions/` exists |
| **7** UI: `/commissions/{id}` detail + **recursive tree renderer** + delete affordance | 6 — the recursive render is the epic's showpiece | P0 | 🧑 **Engineer** *(directive)* | — | ⬜ |
| **8** `+page.server.ts` loads/actions for both routes | 3 | 👥 **Split** | — | ⬜ **Boundary:** Claude supplies the `ZurfurApi` methods + the `ManagedRuntime.runPromise` seam pattern (precedent: `routes/login/+page.server.ts`); the Engineer owns form actions, redirects, and what each load returns to the component |
| **9** UI: `/` signed-in landing lists commissions (epic URL map) | 2 | P2 | 🧑 **Engineer** *(directive)* | — | ⬜ `routes/+page.svelte` is the sign-in CTA only |
| **10** AC4 negative test: no tree-write affordance anywhere in the UI | 2 | P1 | 👥 **Split** | — | ⬜ Engineer writes the assertion against their own markup; Claude can supply the sweep pattern |
| **11** The epic exit walkthrough, end to end | 4 — integration, not logic | P1 | 👥 **Group** | — | ⬜ `scripts/web-smoke.sh` exists as the harness seed |

**Ticket build model for Claude-owned pieces: Opus** — piece 2 is squarely security-nature (the private↔public visibility boundary), and the ticket's model is the max over its pieces.

## ✅ 6. Test checklist (TDD)

**Backend — integration** (`backend/crates/api/tests/commission_read.rs`, new)
- _asserts that_ `GET /commissions/{id}` returns the commission envelope **and** its assembled tree for the owner → **AC2**
- _asserts that_ the tree arrives rooted at the root surface with children in stored sibling order → **AC2**
- _asserts that_ a non-participant receives the uniform `404` `commission_not_found` — byte-identical to the body for a wholly absent id (no existence oracle) → **AC2 / closed door**
- _asserts that_ an anonymous caller receives `401`, never a redirect → **AC2**
- _asserts that_ the archived commission is still readable by id (it vanishes from the list, not from existence) → **AC3**

**Backend — contract weld**
- _asserts that_ every declared route is served and the declared count matches the new corpus size (`contract_routes.rs`) → **AC1–AC3**
- _asserts that_ the regenerated Rust artifacts match the corpus (`contract_current.rs`) → **AC1–AC3**
- _asserts that_ the tree response's JSON keys are canonical lowerCamelCase (R1) and that timestamps are Z-normalized (§7.3) → **R1/R7**

**Backend — the guard that must survive**
- _asserts that_ `CommissionTree`/`CommissionNode` still do **not** implement `Serialize` — the projection is the only exit (a compile-fail or a doc-pinned test; do not delete the guard to make piece 2 easier) → **AC2 / DD 28246028 D3**

**Frontend — unit** (`zurfur-api.spec.ts`)
- _asserts that_ `getCommission` decodes a well-formed body through the generated schema and tolerates unknown fields → **AC2 / §6 tolerant reader**
- _asserts that_ `deleteCommission` maps `204` to success and `409 commission_has_facts` to the dedicated tagged arm, not a generic `ApiProblem` → **AC3**
- _asserts that_ a malformed tree body fails `ContractViolation` naming the endpoint → **AC2**

**Frontend — component**
- _asserts that_ a two-level tree renders parent before children, in order → **AC2**
- _asserts that_ the detail page exposes **no** add-surface / add-component / remove-node control → **AC4**
- _asserts that_ an API problem renders readably via `ProblemNote`, never as raw JSON → **AC3**

**E2E**
- _asserts that_ one browser session walks sign-in → account create → commission create → tree renders → delete both → sign out, with zero curl → **epic exit criterion**

## 🧠 7. Logic & shape

**The endpoint inventory this ticket runs into** — three tiers, and ZMVP-153 needs all three:

```
 ┌── DECLARED in contract/zurfur/api/v1/*.proto ── 9 endpoints ──────────┐
 │  GET /me · POST /signin · POST /logout                               │
 │  GET|POST /accounts · PATCH /accounts/{id}/handle · DELETE /accounts/{id}
 │  GET /commissions          ← 153 consumes (AC1)  ✅ ready            │
 │  POST /commissions         ← 153 consumes (AC1)  ✅ ready            │
 └──────────────────────────────────────────────────────────────────────┘
 ┌── SERVED but NOT declared ── 38 endpoints ────────────────────────────┐
 │  DELETE /commissions/{id}          ← 153 consumes (AC3)  ⚠️ D3       │
 │  POST /commissions/{id}/archive    ← 153 consumes (AC3)  ⚠️ D3       │
 │  POST /commissions/{id}/unarchive                                     │
 │  POST .../surfaces · .../components · DELETE .../nodes/{node} · …     │
 └──────────────────────────────────────────────────────────────────────┘
 ┌── NEITHER — does not exist at all ────────────────────────────────────┐
 │  GET /commissions/{id}             ← 153 REQUIRES (AC2)  🛑 THE GAP   │
 └──────────────────────────────────────────────────────────────────────┘
```

The weld test is **one-directional by design** (`contract_routes.rs:105-108`): declared-but-not-served fails; served-but-not-declared passes. So the 38-endpoint drift is silent and will stay silent.

**Why the gap is real, not an oversight** — the domain refuses to serialize the tree, on purpose:

```rust
// backend/crates/domain/src/elements/commission/node.rs:398-413
/// **This type must never serialize.** It holds everything — `Total`-tier
/// content included — so an impl of `serde::Serialize` here … would put "the
/// whole tree leaves the server" one `Json(tree)` away. Neither type implements
/// it, so serializing the raw tree is a **compile error** today; serialization
/// exists only on the viewer-projected shape ZMVP-75 introduces (min-of-
/// ancestors, computed server-side). Do not add a `Serialize` derive here —
/// project first, always.
pub struct CommissionTree { pub root: CommissionNode }
```

And the gap is **already known in-tree**, pinned to the excluded ticket:

```
backend/crates/api/tests/commission_seats.rs:460
  #[ignore = "ZMVP-76 AC4 is distributed: needs the ZMVP-75 projection
              (post-rebase arm) — do not count as green"]
  … GET {base}/commissions/{id} …   // "the ZMVP-75 projection endpoint serves the ask"
```

**The narrow, honest way through.** For the **owner**, effective visibility is Total at every node — min-of-ancestors is the identity function. So an owner-POV read needs a *projected* wire type but not ZMVP-75's viewer-class machinery. That keeps ZMVP-153 inside the epic's fence while introducing the type ZMVP-75 will later extend. **That framing is a proposal, not a decision** — see ⚠️D1.

**The delete flow AC3 describes** (the API dictates; the client picks the branch):

```
      user clicks Delete
             │
             ▼
  DELETE /api/v1/commissions/{id}
             │
      ┌──────┴───────────────────────────┐
      ▼                                  ▼
   204 No Content              409 commission_has_facts
   row + tree + seats gone     "…Archive it instead."
      │                                  │
      │                          ⚠️D2: auto-archive, or
      │                          surface the choice?
      ▼                                  ▼
   list no longer shows it     POST .../archive → 204
                               list no longer shows it (filtered)
```

Note the asymmetry the UI must render **honestly** (AC3): both outcomes remove the row from `GET /commissions`, but they mean different things. The frontend knows which happened only because it made the calls — no response field distinguishes them, and the `Commission` wire message carries no `archivedAt`.

## 🚀 8. Next steps

**Ordered**

1. **Settle ⚠️D1 with the Engineer** — nothing else can start. Consider `/design-decision`: this is the first serialization of the commission tree, and whatever shape ships becomes the precedent ZMVP-75 and every future viewer inherit (exactly as the `Commission` listing row became the precedent for commission serialization).
2. Settle ⚠️D2 and ⚠️D3 (cheap; they're scope calls, not research).
3. Claude: pieces 1 → 3 → 4 (contract + weld), then 2 (handler + projection) with `/security-review` on the diff.
4. Claude: piece 5 (`ZurfurApi` methods) — **coordinate with ZMVP-152 first, see the collision table.**
5. Engineer: pieces 6, 7, 9 (the UI); piece 8 jointly.
6. Piece 10 (AC4 negative test), then piece 11 (the walkthrough) — the epic closes on it.

---

### ⚠️ Decisions needed — Engineer's call, not resolved here

**⚠️D1 — What shape does the commission tree take on the wire? (BLOCKING)**
No answer exists anywhere in the corpus. Sub-questions, each with a real fork:

- **(a) Whose shape is it?** Is this ZMVP-75's projected type arriving early (and ZMVP-153 uses it at owner-POV, where the projection is the identity), or a deliberately temporary owner-only shape that ZMVP-75 later replaces? *Recommendation: the former* — a second shape would be two precedents, and DD 40992770 makes wire shapes one-way doors (additive-only; removing a field mints v2). But it imports a slice of an excluded ticket into the epic, which is exactly the pull-rule call that is yours.
- **(b) Surface vs Component discrimination.** The domain already models this correctly — `NodeKind` carries `mode` on surfaces and nothing on components. **R4 forbids** mirroring that as an `optional mode` where absence means "this is a component": *"Absence that would carry positive meaning MUST be an explicit discriminant."* So the wire form needs a `oneof` or an explicit `kind` field. *Recommendation: mirror `NodeKind` as an explicit discriminant.*
- **(c) Recursion.** Nested `repeated Node children` (protobuf permits recursive messages; protobuf-es and prost both handle it, though prost needs `Box` on the recursive arm) vs a flat node list the client assembles. *Recommendation: nested* — the DD's read model is already "assemble in Rust", so the server has the tree in hand, and a flat list would push assembly into the runes layer.
- **(d) The payload.** `serde_json::Value`, opaque to core, round-tripping unmodified (ZMVP-72 shipped the generic untyped contract; the type catalog is deferred). **R11 rules payload typing by vocabulary ownership** — typed where Zurfur owns it, opaque JSON `string` where a plugin does — but R11 was decided for *changelog* payloads and does not name node payloads. Node payloads are mixed-ownership by design (core components today, plugin subtrees later). *Recommendation: the opaque-string form*, per R11's plugin arm — `google.protobuf.Struct` would silently degrade int64 precision through JSON numbers, which is the exact 2⁵³ hazard R11 designed around. **But this is a genuine extension of R11 and is yours to rule.**
- **(e) `created_by`.** The envelope carries a `UserId`. Exposing it is a **DID/handle correlation surface** — even owner-POV, it will be inherited by the non-owner projection unless the field is scoped now. *Recommendation: omit it from the v1 wire shape* (R4: adding it later is additive; removing it later mints v2).

**⚠️D2 — Delete UX: automatic fallback, or surfaced choice?**
The API refuses a fact-bearing delete with `409 commission_has_facts` naming Archive. Does the UI transparently follow with `POST .../archive`, or stop and tell the user "this can no longer be deleted — archive instead?" — AC3 says the outcome must render "honestly", which reads against a silent fallback. *Note:* every commission answers `has_facts = false` today (no fact-minter exists), so **the archive branch is unreachable in a real browser walkthrough** — the exit criterion will always take the hard-delete path, and the archive arm is only testable with the `FactBearingUow` double already used in `commission_delete.rs:265-336`.

**⚠️D3 — Do the served-but-undeclared endpoints enter the corpus in this ticket?**
`DELETE /commissions/{id}` and `POST .../archive|unarchive` work today but are outside the corpus, outside `buf breaking`, and outside the generated decoder. ZMVP-153 makes the frontend the first real consumer of two of them. *Recommendation: declare them* — DD 40992770 makes the contract authoritative over both tiers, and consuming an undeclared endpoint means hand-writing a decode the amended DD 39944194 D4 says should be generated. The cost is ~3 rpcs plus a count bump; the alternative is a hand-written seam that drifts. **Scope call is yours** — declaring all 38 is emphatically *not* proposed here.

### Dangling notes (non-blocking, worth knowing)

- **`contract_routes.rs:113-114` overclaims.** The comment calls the pinned set "the cookie-surface routes axum serves today … transcribed from `routes/commissions/mod.rs`" — but it lists 2 of that module's 29 routes. The test is correct (it only checks declared → served); the comment is not. Worth a one-line fix while piece 4 is in there.
- **A client cannot distinguish archived from deleted.** `GET /commissions` filters archived rows out, the `Commission` message has no `archivedAt`, and there is no single-commission read — so an archived commission is invisible over HTTP except via its changelog, which requires already knowing the id. Un-archive is reachable but nothing tells a client *what* to un-archive. Not this ticket's problem, but the detail page is the natural future home for it.
- **`commission_pb.ts` is already generated and entirely unused.** Piece 5 is its first consumer.
- **ZMVP-74** (set a surface's visibility mode) is To Do. It does not block AC2 — the owner sees Total regardless — but it means the tree the UI renders will have every non-root surface at its `Total` default.
- **`frontend/auth`** (the legacy React app) still starts under `just dev` and is outside the `gate` recipe. Untouched by this ticket; noted so nobody mistakes it for the surface under test.

### 🔀 Collision check vs ZMVP-152 (understood in parallel)

Both tickets are frontend-first and land in the same small tree. Sequence or co-slice the HIGH rows.

| File | Risk | Why |
|---|---|---|
| `frontend/web/src/lib/server/api/zurfur-api.ts` | 🔴 **HIGH** | Both add methods to the **same four** structures: `ZurfurApiShape` (:43), `ZurfurApi` tag, `ZurfurApiLive` (:214), `anonymousDefaults` (:236). Guaranteed conflict. |
| `frontend/web/src/lib/server/api/zurfur-api.spec.ts` | 🔴 **HIGH** | Same file, parallel new suites. |
| `frontend/web/src/lib/server/api/errors.ts` | 🟡 MED | Both add tagged arms (152: soft/hard account-delete semantics; 153: `commission_has_facts`). |
| `frontend/web/src/routes/+page.svelte` (+ `.server.ts`) | 🟡 MED | Epic URL map: `/` signed-in shows **commissions** (153's piece 9) while 152 adds `/accounts` navigation. |
| `frontend/web/src/lib/components/SessionHeader.svelte` | 🟡 MED | Nav links to both `/accounts` and `/commissions`. |
| `frontend/web/src/lib/server/api/generated/**` | 🟡 MED | Regenerated wholesale by `just gen-contract`; 153 changes `commission_pb.ts`, and a stale regeneration in either branch clobbers the other. Regenerate after merge, never resolve by hand. |
| `frontend/web/src/lib/components/ProblemNote.svelte` | 🟢 LOW | Both render problems; likely no edit needed. |
| `backend/crates/api/tests/contract_routes.rs` | 🟢 LOW | **153 only.** All four of 152's account endpoints are already declared and served. |
| `contract/zurfur/api/v1/*.proto` | 🟢 LOW | **153 only** (`commission.proto`). 152 needs no contract change. |
| `backend/crates/api/src/routes/commissions/**` | 🟢 LOW | 153 only. |

**Recommended ordering:** land ZMVP-152 first, *or* carve the `ZurfurApi` extension for **both** tickets as one shared slice before either UI branch starts. ZMVP-153 carries all the backend and contract work, so its infra lane can run in parallel with 152's UI lane without touching a shared file — the collision is confined to piece 5.
