# 🔎 Understanding ZMVP-152 — A user creates and deletes an Account from the browser

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-152 · **Generated:** 2026-07-28T05:32:45Z · **Snapshot:** `/home/zuri/code/zurfur/.understand/20260727-233245_ZMVP-152_account-create-delete-browser.md`
>
> **Grounded at:** `main` tip `876e94d` ("[ZMVP-158] Typed response bodies"). Epic **ZMVP-149 "The Other Side"** · blocked-by **ZMVP-151 ✅ Done** (#153) and **ZMVP-157 ✅ Done** (#155) — **both blockers cleared; this ticket is unblocked.**

```
        ┌──────────────── The Other Side (ZMVP-149) ────────────────┐
        │  150 ✅ boot+split → 151 ✅ sign in/out → 152 ⬜ THIS      │
        │                                        → 153 ⬜ sibling   │
        └───────────────────────────────────────────────────────────┘
                     enablement landed underneath:
       ZMVP-157 ✅ (GET /accounts, GET /commissions)   #155
       ZMVP-25  ✅ epic: contract corpus + both codegens + /api/v1 mint
```

## 🧭 1. Context (cold-start)

Zurfur's browser client is a SvelteKit app (`frontend/web`) that reached "a visitor can sign in and sign out" in ZMVP-151. It has a session gate, an Effect-based server-side API port, a generated protobuf boundary decoder, and exactly one screen worth of chrome. It has **no `/accounts` route at all** — `frontend/web/src/routes/` today holds only `+layout`, `+page`, `login/`, and `(session)/logout/`.

This ticket makes the **Account** clickable: list the accounts you hold a role in, found a new one (which mints a sovereign `did:plc` behind the scenes), open one, delete it, and see the outcome told honestly.

Three things changed under this ticket since it was written on 2026-07-19, and they change its shape materially:

1. **ZMVP-157 (#155)** added `GET /accounts` — wrapped `{ "accounts": [...] }`, each row carrying **the caller's own `role`** (`backend/crates/api/src/routes/accounts.rs:253-285`). Without it AC1 was unbuildable.
2. **Epic ZMVP-25** minted `/api/v1` over an independent protobuf contract (DD 40992770). `contract/zurfur/api/v1/account.proto` is now authoritative for both tiers; Rust handlers return generated types, and the frontend has generated TS at `frontend/web/src/lib/server/api/generated/zurfur/api/v1/account_pb.ts` plus a generated boundary decoder (`decodeContract`, `zurfur-api.ts:35-40`).
3. **An Engineer ruling on 2026-07-25 changed `DELETE /accounts/{id}` specifically for this ticket**: it no longer answers a bodiless `204` — it answers **`200 { "outcome": "soft" | "hard" }`** (`accounts.rs:463-527`; pinned in `backend/crates/api/tests/golden_wire.rs:277-283`). The rationale is quoted in the contract: *the frontend is an interface — it renders what the program tells it, and cannot infer an outcome it was never told.* AC2 was written before that existed; it is now satisfiable without guessing.

Net: **the ticket's premise "the backend never changes for this ticket" now holds — with one exception (see ⚠️ F1).** Everything AC1–AC4 need is on the wire, except an account *detail* read.

## 🗺️ 2. Domain

- **Account** (DESIGN `1966081`) — a sovereign entity with its own `did:plc` + handle, founded on demand by a User. Not a default; a User is a first-class actor without one (DD `26247170`). Founding mints keys and an identity-only genesis op before any row is written (`accounts.rs:352-361`), which is why create is "the heavy one" behind a plain form.
- **Roles** (DESIGN `2162692`) — Owner/Admin/Manager/Member. Load-bearing here: `DELETE /accounts/{id}` is **Owner-only** and `403`s a non-Owner member (`accounts.rs:475-477`). ZMVP-157 put `role` on every listing row precisely so the UI doesn't render a delete button that fails.
- **Account deletion** (DD `23003138`) — *soft by default, hard only when empty*. Soft = private-store deactivation (row kept, handle stays reserved and still resolves, `did:plc` stays live, never escalates to hard). Hard = rows removed + handle freed + a **separate retryable** `plc_tombstone` (never a cross-store transaction). The DD's decision 6 explicitly defers the two frontend states to "the frontend era" — that era is this ticket.
- **Handle** (DD `24870914` / `26050561` / `27852802`) — user-chosen at founding, BYO domain or `*.zurfur.app`, punycode-banned, reserved labels rejected, recently-vacated handles quarantined.
- **Error model** (DD `23592962`) — RFC 9457 `application/problem+json`; clients branch on `code`, never on `type`.
- **Frontend stack** (DD `39944194`, D4 as amended by `40992770`) — Effect lives **only** under `src/lib/server/**`; runes never see a fiber; the seam is `runApi`; untrusted boundaries decode through the **generated** protobuf-es schema; no `Option`, strict-null `T | undefined`.
- **Contract versioning** (`contract/VERSIONING.md`, **binding**, R1–R11) — R6 ids are opaque (never parse, never order by), R7 listings are wrapped objects, **R8 vocabularies are extensible strings and clients MUST tolerate unknown values with a *defined fallback*** (this bites `role` *and* `outcome`).

## 🎯 3. Goal & scope

**Goal:** give the Account lifecycle a browser surface that tells the truth — the caller's real memberships with their real authority, a founding form that surfaces the domain's own rejections verbatim, and a delete that renders *which* deletion happened rather than assuming one.

**In scope**
- Two SvelteKit routes under the `(session)` group: `/accounts` (list + create) and `/accounts/{id}` (detail + delete), addressed by opaque id (ruling 8y).
- Extending the `ZurfurApi` port with `listAccounts` / `createAccount` / `deleteAccount`, decoding through the generated schemas.
- Rendering `handle_taken` (409) and `invalid_request` (422) inline through the existing `ProblemNote` seam.
- Rendering the delete `outcome` honestly, including a defined fallback for an unknown value (R8).
- A nav path from the app to `/accounts`.

**Explicitly out**
- **Any backend change** — ZMVP-157 was carved so this ticket wouldn't need one. (⚠️ F1 tests that promise.)
- Membership / invitation / role-management UI — ruling 6a. Note: rendering *the caller's own* authority to decide whether a delete button exists is **not** membership management; it's honest self-authority rendering, and ZMVP-157's AC explicitly justified `role` for it.
- Handle *change* (`PATCH /accounts/{id}/handle`) — exists on the wire, not in the URL map, not an AC.
- The `no creator?` onboarding prompt — ZMVP-30 stays orphaned by ruling 5b. **Plain create.**
- Design/visual identity — "no design work rides this epic."
- Anything commissions — ZMVP-153.

## 📦 4. Deliverables

- [ ] `frontend/web/src/lib/server/api/zurfur-api.ts` — `ZurfurApiShape` gains `listAccounts`, `createAccount(name, handle)`, `deleteAccount(id)`; live implementations decoding via `decodeContract(ListAccountsResponseSchema | CreateAccountResponseSchema | DeleteAccountResponseSchema, …)`; matching entries in `anonymousDefaults` so `zurfurApiTest` keeps its adapter-mem parity.
- [ ] `frontend/web/src/lib/api/account.ts` — the **plain component-facing** types (`AccountMembership`, `CreatedAccount`, `DeleteOutcome`) mapped down from the generated messages, mirroring how `Session` is handled (`zurfur-api.ts:155-172`). Generated types must not cross the seam.
- [ ] `frontend/web/src/lib/server/accounts.ts` — Effect programs + outcome unions in the shape of `session.ts` (`{accounts}` / `{problem}`, `{account}` / `{problem}`, `{outcome}` / `{problem}`).
- [ ] `frontend/web/src/routes/(session)/accounts/+page.server.ts` — `load` (list) + `default` action (create).
- [ ] `frontend/web/src/routes/(session)/accounts/+page.svelte` — the list, the create form, empty state, inline problem.
- [ ] `frontend/web/src/routes/(session)/accounts/[id]/+page.server.ts` — `load` (the one account) + `delete` action.
- [ ] `frontend/web/src/routes/(session)/accounts/[id]/+page.svelte` — detail + delete affordance + outcome rendering.
- [ ] A navigation entry reaching `/accounts` (root layout or `/`).
- [ ] Server specs: `accounts.spec.ts`, `zurfur-api.spec.ts` additions, `page.server.spec.ts` × 2 (node project).
- [ ] Component specs: `page.svelte.spec.ts` × 2 (browser project).
- [ ] Green `just gate` — including `yarn --cwd frontend/web run check | lint | test | build` (`Justfile:147-150`, CI `web` job `.github/workflows/ci.yml:127-147`).
- [ ] `/security-review` pass (first authenticated *writes* from the browser surface; the screen puts a `did:plc` next to a session identity).

## 🧩 5. Work breakdown

Owner split follows the Engineer's 2026-07-27 correction: **the Engineer implements the UI; Claude implements the infrastructure beneath it.** The seam between the two lanes is the module `$lib/server/accounts.ts` — everything *below* it (port methods, generated-type decoding, outcome unions, their specs) is Claude's; everything at or above `+page.server.ts` is the Engineer's, with `+page.server.ts` marked 👥 because it is where the two meet.

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| **A. `ZurfurApi` gains the three account calls** — live impls + generated decode + `anonymousDefaults` entries | 3 — blast radius: the one boundary every screen inherits, and it carries `did` + session | P0 | 🤖 Claude | **Opus** — the decode boundary carries DID/session data across the private↔public line; security-nature by the `/security-review` trigger list | ⬜ `zurfur-api.ts:43-64` has only `me`/`startSignin`/`signout` |
| **B. `$lib/api/account.ts` plain types** — generated message → plain data, keeping generated types below the seam | 1 — mechanical, `Session` is the worked example | P1 | 🤖 Claude | Haiku | ⬜ no such file |
| **C. `$lib/server/accounts.ts` Effect programs** — outcome unions mirroring `session.ts:39-79` | 3 — uncertainty is in the outcome-union shape, not the code | P0 | 🤖 Claude | Sonnet | ⬜ no such file |
| **D. Server specs for A + C** — decode, problem classification, R8 unknown-value tolerance | 2 — pattern set by `zurfur-api.spec.ts` + `testing/http.ts` | P1 | 🤖 Claude | Sonnet | ⬜ |
| **E. `/accounts/+page.server.ts`** — load + create action | 3 | P0 | 👥 **split** — Claude supplies the `$lib/server/accounts.ts` programs and the `runApi` plumbing pattern; the **Engineer owns what the page returns and what the action does after success** (redirect target, `fail()` shape), because those are UI decisions | — | ⬜ |
| **F. `/accounts/+page.svelte`** — list, create form, empty state | 3 — UI | P0 | 🧑 Engineer | — | ⬜ |
| **G. `/accounts/[id]/+page.server.ts`** — load one + delete action | 3 | P0 | 👥 **split** — same boundary as E; additionally blocked on ⚠️ F1 (where the detail read comes from) | — | ⬜ |
| **H. `/accounts/[id]/+page.svelte`** — detail, delete affordance, outcome rendering | 4 — UI + the soft/hard honesty + the `role`-gated affordance | P0 | 🧑 Engineer | — | ⬜ |
| **I. Nav entry to `/accounts`** | 1 — UI | P2 | 🧑 Engineer | — | ⬜ `+layout.svelte` has no nav |
| **J. Component specs for F + H** | 2 — UI, rides with the screens | P1 | 🧑 Engineer | — | ⬜ |
| **K. ⚠️ F1 decision — where `/accounts/{id}` reads from** | 2 (a decision, not effort) | **P0 — blocks G/H** | 🧑 Engineer | — | ⬜ open |
| **L. `/security-review` before the PR** | 3 | P1 | 🤖 Claude (Designer lane) | **Opus** — mandatory, all security is Opus | ⬜ |

**Ticket build model (Claude's lane) = Opus** — piece A is security-nature, and per policy one security piece pulls the whole build up.

### Already delivered — do **not** rebuild
| Thing the ticket implies | Where it already is |
|---|---|
| `GET /accounts` wrapped + per-row `role` | `accounts.rs:253-285`; tests `account_listing.rs:133-223` |
| `POST /accounts` → `201 {id,did,handle,name}` (carries `id` so create-then-navigate works) | `accounts.rs:308-396`; `golden_wire.rs:236-244` |
| `DELETE /accounts/{id}` → `200 {"outcome"}` | `accounts.rs:463-527`; `golden_wire.rs:277-283` |
| Generated TS messages + schemas | `frontend/web/src/lib/server/api/generated/zurfur/api/v1/account_pb.ts` |
| Tolerant generated decoder (`ignoreUnknownFields`) | `zurfur-api.ts:35-40`, pinned by `contract-tolerant-reader.spec.ts` |
| RFC 9457 → tagged error union | `src/lib/server/api/errors.ts` |
| Readable problem rendering ("never raw JSON" — **AC3's second half**) | `src/lib/components/ProblemNote.svelte` |
| Session gate for `/accounts*` | `src/routes/(session)/+layout.server.ts` |
| `/api/v1` prefix + SSR rewrite + session-cookie forwarding | `src/lib/api/client.ts:11`, `src/lib/server/api-proxy.ts`, `src/hooks.server.ts` |
| Effect runtime seam | `src/lib/server/runtime.ts` |
| Test harness (client/server vitest projects, fetch + problem stubs) | `vite.config.ts:33-60`, `src/lib/testing/http.ts` |

**Re-measured verdict:** every AC's *backend* half is done. What remains is entirely `frontend/web` — roughly one port extension, one server module, two routes, and their tests. AC3's "problem responses render readably" is ~90% pre-solved by `ProblemNote`.

## ✅ 6. Test checklist (TDD)

**Server project** (`environment: node`, excludes `*.svelte.spec.ts`)
- `listAccounts` decodes a wrapped `{accounts:[…]}` body into plain rows carrying `id, did, handle, name, role` → **AC1**
- `listAccounts` **tolerates an unknown `role` value** and an unknown extra field without throwing (R8 + §6 tolerant reader) → **AC1**
- `listAccounts` on a non-problem non-JSON body fails `ContractViolation` naming `/accounts` → **AC3**
- `createAccount` posts `{name, handle}` to `/api/v1/accounts` and decodes the `201` into `{id, did, handle, name}` → **AC1**
- `createAccount` maps a `409 handle_taken` problem body to `ApiProblem` carrying that `code` → **AC1**
- `createAccount` maps a `422 invalid_request` (the code a **reserved label and a punycode handle both land on** — there is no distinct `reserved` code) to `ApiProblem` → **AC1**
- `deleteAccount` decodes `200 {"outcome":"hard"}` → `'hard'`, and `{"outcome":"soft"}` → `'soft'` → **AC2**
- `deleteAccount` maps `403 forbidden` (non-Owner) and `404 account_not_found` — note the code is **`account_not_found`, not `not_found`** → **AC2**
- `deleteAccount` on an **unknown `outcome` string** resolves to the Engineer-chosen fallback (⚠️ F3), never throws → **AC2**, R8
- `/accounts` `load` returns the rows a stubbed `zurfurApiTest` hands it → **AC1**
- `/accounts` create action `fail()`s with the problem on `ApiProblem`, and on success does whatever the Engineer rules in E → **AC1**
- `/accounts/[id]` `load` produces the account, and produces a 404-ish outcome for an id the caller holds no role in → **AC2**, ⚠️ F1
- `/accounts/[id]` delete action redirects to `/accounts` carrying the outcome → **AC3**

**Client project** (browser, `vitest-browser-svelte`)
- The list renders one row per account with its handle and its `role` → **AC1**
- The list renders an empty state with a create affordance when the user holds no accounts → **AC1**, AC4
- The create form renders **no creator/onboarding prompt** — plain create → **AC4**
- A `handle_taken` problem renders through `[data-testid="problem"]` with the backend's `detail`, not raw JSON → **AC1, AC3**
- The detail screen renders the delete affordance **only when the caller's `role` is `owner`** (⚠️ F4) → **AC2**
- Given `outcome: 'hard'`, the screen says the account is gone; given `'soft'`, it says deactivated-and-retained — two distinguishable renderings, per DD `23003138` decision 6 → **AC2**
- After a delete, the list no longer contains the account → **AC3**

**Walkthrough** — the epic's exit criterion (sign-in → account create → commission create → tree → delete both → sign out, zero curl) is carried by **ZMVP-153**, not here. This ticket owes it the two account beats in working order.

⚠️ **Honest-testing note:** `account_has_facts` is a hard-coded `Ok(false)` (`accounts.rs:441-443`, guarded by a compile assert on an empty `ACCOUNT_FACT_TABLES`). **Every real delete answers `"hard"` today.** The `'soft'` rendering is therefore only reachable through a stubbed API Layer — it cannot be exercised end-to-end, and no test should pretend otherwise. See ⚠️ F2.

## 🧠 7. Logic & shape

The lane boundary, and where each deliverable sits:

```
  ┌─ 🧑 ENGINEER (UI) ────────────────────────────────────────────┐
  │  +page.svelte  ·  components  ·  runes  ·  nav  ·  styling    │
  │  component specs                                              │
  ├─ 👥 SPLIT ────────────────────────────────────────────────────┤
  │  +page.server.ts   load() / actions{}                         │
  │    Claude: runApi plumbing pattern + the programs it calls    │
  │    Engineer: what the page needs back, what happens after     │
  ├─ 🤖 CLAUDE (infrastructure) ──────── the seam: ───────────────┤
  │  $lib/server/accounts.ts      ← outcome unions                │
  │  $lib/server/api/zurfur-api.ts ← port methods + decodeContract│
  │  $lib/api/account.ts          ← plain component-facing types  │
  │  server specs                                                 │
  └───────────────────────────────────────────────────────────────┘
              │ generated types stop HERE (eslint guard,
              │ eslint.config.js:47-62 — `effect` is server-only)
              ▼
        /api/v1/accounts  →  Caddy or handleFetch strips /api/v1
                          →  axum `/accounts` (unprefixed)
```

Delete, end to end — the honesty AC2 asks for:

```
  [Delete] ──► action ──► DELETE /api/v1/accounts/{id}
                             │
              200 {"outcome":"soft"} ──► "deactivated · handle still reserved · DID live"
              200 {"outcome":"hard"} ──► "gone · handle freed · DID tombstoned"
              200 {"outcome":"???"}  ──► ⚠️ F3: the R8 defined fallback (Engineer's call)
              403 forbidden          ──► not the Owner
              404 account_not_found  ──► already gone
```

## 🚀 8. Next steps

1. **Settle ⚠️ F1 first — it blocks pieces G and H.** Everything else can start in parallel.
2. Claude opens the infrastructure lane: **B → A → C → D** (plain types, then the port, then the programs, then their specs). None of it depends on F1.
3. Engineer takes the UI lane: **F → H → I → J**, meeting Claude at **E** and **G**.
4. Ship gates as one Workflow (`/code-review` ∥ `/security-review` ∥ `/critique` ∥ `/document`), then `/prepare-pr`. **`/security-review` is not optional here** — this adds the first authenticated *writes* driven from the browser, and the list screen places a sovereign `did:plc` beside a session identity.

### ⚠️ Decisions needed — the Engineer's, not Claude's

- **⚠️ F1 — `/accounts/{id}` has no backend read. Where does the detail screen get its account?**
  There is **no `GET /accounts/{id}` route** (`accounts.rs:54-74` registers `GET`/`POST` on the *collection* only; `contract_routes.rs:146-151` pins the corpus at exactly 9 endpoints). Two options:
  - **(a) Derive from the listing** — `load` calls `listAccounts` and finds the row. Every field AC2 needs (`id, did, handle, name, role`) is already there. Keeps the ticket's "the backend never changes" promise intact, and "not in your list" is a clean, correct 404 (it means *you hold no role in it*, which is exactly the authorization answer). Costs one list fetch per detail view — irrelevant at v1 volumes, which is the same stance R7/ZMVP-157 took on pagination.
  - **(b) Add `GetAccount`** — a new rpc in `account.proto`, a new axum route, regenerated types on both tiers, and updates to `contract_routes.rs`'s endpoint-count assertion. This breaks the ticket's premise and re-does what ZMVP-157 was carved to prevent.
  - **Recommendation: (a).** Evidence: the listing row is field-complete for AC2, and option (b)'s cost is now structural (the contract weld test counts endpoints). If the Engineer prefers (b) on principle — a detail resource deserving its own read — it should be carved as its own enablement ticket in ZMVP-157's mould, not smuggled in here.

- **⚠️ F2 — is a stub-only `'soft'` rendering acceptable evidence for AC2?**
  `account_has_facts` returns a constant `false`, so no live delete can produce `'soft'`. The rendering can be built and unit-tested against a stubbed Layer, but never walked in a browser. Recommendation: yes, accept it, and put a comment on the soft branch pointing at `accounts.rs:433-443` so the next reader knows the branch is real-but-currently-unreachable rather than dead. The Engineer should confirm, because it means AC2 ships half-proven by construction.

- **⚠️ F3 — R8 demands a *defined fallback* for an unknown `outcome`. Which is it?**
  VERSIONING.md R8 requires clients to tolerate unknown vocabulary values with a defined fallback, and the same obligation applies to `role`. Recommendation: an unknown `outcome` renders the **conservative** reading — "removed from your accounts; it may still exist" — never "permanently gone", because claiming permanence we weren't told is the failure that can't be walked back. For `role`, an unknown value should render **no delete affordance** (fail closed). Engineer's call on both.

- **⚠️ F4 — what does a non-Owner member see on `/accounts/{id}`?**
  `DELETE` is Owner-only; the listing hands the UI the caller's role for exactly this. Absent affordance, or visible-but-disabled with a reason? Recommendation: **absent** — a disabled control that explains authority edges toward the membership UI ruling 6a excluded. Engineer's call.

- **F5 (resolved, noted for the record)** — create lives *inline on `/accounts`*, not at `/accounts/new`: the epic's URL map lists exactly seven routes and `/accounts/new` is not among them. No decision needed unless the Engineer wants to amend the map.

### 🔀 Collision check vs ZMVP-153 (understood in parallel)

Both tickets are the same shape against the same seams; they are parallelizable **only if these are sequenced or split by owner**.

| Surface | Risk | Detail |
|---|---|---|
| `frontend/web/src/lib/server/api/zurfur-api.ts` | 🔴 **hard** | Both add methods at the **same three sites**: `ZurfurApiShape`, `anonymousDefaults`, and the `ZurfurApi.of({…})` literal in `ZurfurApiLive`. Guaranteed conflict. **Mitigation: one ticket lands the port extension for both**, or 153 rebases on 152's slice. |
| `contract/zurfur/api/v1/*.proto` + **both** generated trees | 🔴 **hard, if triggered** | 152 needs no contract change **iff F1 resolves to (a)**. **153 very likely does**: `commission.proto` declares only `ListCommissions` + `CreateCommission` — there is **no message and no rpc for a commission detail read (its AC2 surface tree) or for delete (its AC3)**. If either ticket edits the corpus, both tiers' generated output and `contract_routes.rs`'s endpoint count move under both tickets at once. **Flag this to 153's briefing.** |
| `frontend/web/src/routes/+page.svelte` | 🟡 medium | Line 16 is literally `Your commissions will appear here (ZMVP-153)` — 153 replaces it; 152 may add an accounts link nearby. |
| `frontend/web/src/routes/+layout.svelte` | 🟡 medium | Whichever ticket introduces nav first owns the file; the second edits it. Cheap to resolve, certain to happen. |
| `frontend/web/src/lib/components/**` | 🟡 medium | Both want an empty state, a list shell, and a destructive-action confirm. **Decide up front who mints the shared components** or both will invent divergent ones (a `/critique` finding waiting to happen). |
| `frontend/web/src/lib/testing/http.ts` | 🟢 low | Both may add stub helpers; additive, small. |
| `frontend/web/src/lib/server/api/errors.ts` | 🟢 low | The existing tagged union covers both; neither should need a new tag. |
| `src/routes/(session)/accounts/**` vs `commissions/**` | ⚪ none | Disjoint. |
| `$lib/server/accounts.ts` vs `commissions.ts`; `$lib/api/account.ts` vs `commission.ts` | ⚪ none | Disjoint by design — keep them that way. |

**Recommended sequencing:** land 152's piece **A** (the port extension) as its own early slice, and have 153 build on top of it. Owner-wise the two tickets do **not** collide in the Engineer's lane (different screens) — the collision is almost entirely in Claude's infrastructure lane, which is a single-writer lane and therefore easy to serialize.
