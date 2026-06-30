# 🔎 Understanding ZMVP-26 — Author the OpenAPI spec for the Plugin REST surface (/plugin/v1)

> **Status:** To Do (floor stub) · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-26 · **Generated:** 2026-06-28 23:22 · **Snapshot:** `.understand/20260628-232209_ZMVP-26_plugin-v1-openapi-spec.md`
> **Parent epic:** ZMVP-25 "API Contract (OpenAPI / AsyncAPI)" (To Do) · **Priority:** Medium · **Labels:** none · **ACs:** none authored

## 🧭 1. Context (cold-start)
Zurfur lets external **plugins** extend the **core** app. The `/plugin/v1` REST surface is the **plugin → core** contract: the HTTP API a (first-party, in v1) plugin calls to read commission data and push commission-relevant output into core. This ticket is to **author the OpenAPI document** describing that surface — a design/spec artifact, **not** application code.

It is one of three sibling tasks under epic **ZMVP-25**:
- **ZMVP-26** (this) — **OpenAPI**, REST, plugin → core.
- **ZMVP-27** — **AsyncAPI**, events/webhooks, core → plugin.
- **ZMVP-28** — the **versioning & deprecation contract** both specs run under.

The ticket is an explicit **"floor stub"** ("dress when the Commission/Plugin API surface is sliced") and is **"Blocked on schema finalization"** — it must not be authored against an unpinned schema.

Current code reality (evidence-checked): **the `/plugin/v1` surface does not exist in code.** The api router (`backend/crates/api/src/lib.rs:261-282`) registers only `/health`, `/`, `/signin`, `/signin-callback`, `/me`, `/accounts*`, `/logout`, invitation routes — **no plugin namespace, no `app_key`** (grep for `app_key` across `backend/crates` returns nothing). On **`main` there is no OpenAPI tooling** (no `utoipa`/`aide`/`openapi`/`ToSchema`). **However**, an in-flight branch **`feature/openapi-infra`** stages the tooling: `utoipa` 5 + `utoipa-scalar` 0.3, an `ApiDoc` struct + bearer security scheme + `ErrorBody` schema in `backend/crates/api/src/openapi.rs`, and `/api/docs` (Scalar UI) + `/api/docs/openapi.json` routes — **but no `/plugin/v1` path annotations** (deferred "Phase 3"). The only "plugin" references in the api crate are CSRF-exemption **comments** (`lib.rs:285,300-302`, `problem.rs:94`). The RFC 9457 error model the spec must reference **does** exist: `backend/crates/api/src/problem.rs` (from ZMVP-35, Done).

⚠️ **A third, protocol-level open fork:** the ZMVP-42 retrospective (`.understand/retrospectives/retrospective-2026-06-29.md`) records that **REST vs XRPC for `/plugin/v1`** and the **token-exchange shape** are *still-open design decisions*. The ticket's title presumes "OpenAPI" (REST/XRPC-over-OpenAPI), but the surface style itself isn't settled — an Engineer call that precedes authoring.

## 🗺️ 2. Domain
- **Plugin** (DESIGN, page 3047451) — the governing glossary page, rich and current. Key invariants this spec must encode:
  - **Transport / auth:** plugins are issued an **`app_key` per `(plugin, account)` pair** (the install binding), **exchanged for short-lived, scoped, delegation-tagged tokens** — *never used as a long-lived bearer secret*. A plugin **never** receives the session cookie or the user's atproto credential. Server-side plugins are **rate-limited per `app_key`**.
  - **Authority rule (non-negotiable):** `plugin may do = granted scopes ∩ principal's authority ∩ domain validation`. Principal = the clicking user (invoked) or a Golem (reactive; reactive is **post-MVP**).
  - **Production vs reading:** plugins gate **production** (write commission-relevant output to the **changelog**, core-renderable); **core gates reading/visibility**. No private side-channels.
  - **Scopes:** `resource:action`, three families (event / surface / action), declared in manifest, granted at install.
  - **MVP delivery slice:** Reactor (react/notify) + **invoked actions** (writes via the API, principal = clicker). UI surfaces and reactive/Golem actions are **post-MVP** — they bound what `/plugin/v1` needs to cover in v1.
- **Commission** (DESIGN, page 3276807) — "the **Commission resource at the centre of the schema**." This is **Private** data. Per the **Lexicon** page (10354710, §B), the Commission family is **deliberately NOT an atproto lexicon** — it lives in **The Index** and travels as an internal/export JSON Schema (the same contract the `zurfur-trello-export` tool emits). So "schema" here = the **Index / commission-export schema**, not `app.zurfur.*` records.
- **Auth Surfaces, the Plugin Trust Boundary & CSRF DD** (DESIGN, DECIDED; via ZMVP-23) — `/plugin/v1` is **bearer `app_key`, CSRF-exempt by construction**, served off the cookie surface. This much of the security posture is pinned.
- **Error model** — RFC 9457 `application/problem+json` with `type` (`urn:zurfur:error:*`), `code`, `title`, `detail`, `status` (DD via ZMVP-35, Done; `problem.rs` exists). The spec's error responses must reference this shape.

## 🎯 3. Goal & scope
**Goal:** produce a single **OpenAPI document** that is the authoritative, reviewable contract for the v1 plugin → core REST surface, with the **Commission resource as the central schema**, the **`app_key` bearer** security scheme, **problem+json** errors, and operations matching the **MVP plugin slice** (read commission data; produce changelog output via invoked actions).

**In scope (when unblocked):**
- The OpenAPI document itself (paths, the Commission/Participant/changelog component schemas, security scheme, error responses, version pinning).
- Alignment with the existing `problem.rs` error registry and the CSRF-exempt bearer posture.

**Out of scope:**
- AsyncAPI events/webhooks (**ZMVP-27**) and the versioning-contract *rules* themselves (**ZMVP-28**) — this spec *consumes* those, doesn't define them.
- Implementing any `/plugin/v1` route, handler, `app_key` issuance, or token exchange in Rust (spec-only; no code endpoints).
- UI surfaces, reactive/Golem-principal actions (post-MVP).
- Pinning the Commission schema (**ZMVP-19**'s job) or writing the Plugin-security DD — those are upstream blockers, not this ticket's deliverables.

## 📦 4. Deliverables
- [ ] An **OpenAPI 3.x document** for `/plugin/v1` (location TBD — no `docs/`, `spec/`, or `openapi.*` exists today; ⚠️ Engineer picks the convention).
- [ ] **Security scheme:** bearer (`app_key`-exchanged token), documented as CSRF-exempt by construction.
- [ ] **Central component schema:** Commission (+ Participant / Lifecycle-Status / changelog entry), sourced from the pinned Index/export schema.
- [ ] **Operation set** for the MVP slice (e.g. read a commission; write a changelog artifact) with **scope requirements** annotated per operation.
- [ ] **Error responses** referencing the RFC 9457 problem+json shape and the existing `urn:zurfur:error:*` code registry.
- [ ] **Version pinning** mechanism reflected (per ZMVP-28's contract).
- [ ] *(optional)* CI lint/validation step (e.g. spectral/redocly) — no such step exists in `.github/workflows/ci.yml` today.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| OpenAPI scaffolding (info, servers, `app_key` bearer securityScheme, problem+json error component referencing `problem.rs` registry, version header) | 2 — boilerplate once shapes are fixed | P2 | 🤖 Claude | 🟡 — tooling staged on `feature/openapi-infra` (`utoipa`/`utoipa-scalar`, `openapi.rs`, `/api/docs`) but **no `/plugin/v1` annotations**; nothing on `main` |
| ⚠️ **REST vs XRPC + token-exchange shape** for `/plugin/v1` | 5 — protocol fork, lasting consequences | P2 | 🧑 Engineer | ⬜ — flagged open in the ZMVP-42 retro; presupposed by "OpenAPI" but not decided |
| **Commission central component schema** (fields, Participant/Lifecycle/changelog) | 7 — *uncertainty + domain*; blocked on ZMVP-19 | P2 | 👥 Group | ⬜ — Commission Index/export schema not pinned (Lexicon §B "field lists to finish"; Blocking Gaps still open) |
| **Endpoint/operation set + scope→operation mapping** (which ops exist; action-scope vocabulary; principal model) | 6 — domain forks throughout | P2 | 👥 Group | ⬜ — no `/plugin/v1` routes in `lib.rs:261-282`; scope vocab not finalized |
| **Auth/security details** (`app_key`→token exchange semantics, rate-limit headers) | 5 — needs the Plugin-security DD | P2 | 🧑 Engineer | ⬜ — Plugin-security DD still pending (Blocking Gaps: "full DD still pending") |
| Spec location convention + optional CI lint wiring | 2 | P3 | 🤖 Claude | ⬜ — no `docs/`/`spec/` dir; CI has no spec-lint step |

- **Owner bands:** 0–3 → 🤖 Claude · 3–6 → 🧑 Engineer · 6+ → 👥 Group. The two heaviest pieces (Commission schema, operation/scope design) are the *substance* of the ticket and are squarely Group/Engineer domain work — and **externally blocked**.

## ✅ 6. Test checklist (TDD)
*No acceptance criteria are authored on the ticket — these are derived; the Engineer should ratify the bar.* For a spec artifact, "tests" = validation/lint assertions:
- **Lint/structural** — *asserts that* the document parses as valid OpenAPI 3.x (spectral/redocly clean). → (derived)
- **Contract** — *asserts that* every operation declares the `app_key` bearer security scheme and the required scope(s). → Plugin authority rule
- **Contract** — *asserts that* error responses are `application/problem+json` with `type`/`code`/`title`/`status`, codes drawn from the `problem.rs` registry. → ZMVP-35 alignment
- **Consistency** — *asserts that* the Commission component schema matches the pinned Index/export schema field-for-field (cross-check vs ZMVP-19 output). → schema-finalization gate
- **Scope** — *asserts that* only MVP-slice operations appear (no UI-surface, no reactive/Golem-principal endpoints). → Plugin delivery phasing

## 🧠 7. Logic & shape
```
        BLOCKERS (must close first)                 THIS TICKET (ZMVP-26)
  ┌───────────────────────────────────┐      ┌──────────────────────────────┐
  │ ZMVP-19  Commission/Index schema  │─────▶│  OpenAPI doc /plugin/v1      │
  │          (Lexicon §B field lists) │      │  • Commission @ centre       │
  ├───────────────────────────────────┤      │  • app_key bearer (CSRF-exem)│
  │ Plugin-security DD  (still pending)│─────▶│  • problem+json errors       │
  │  → app_key/token-exchange, scopes │      │  • scopes per operation      │
  └───────────────────────────────────┘      └──────────────┬───────────────┘
                                                             │ runs under
                                                ┌────────────▼───────────┐
                                                │ ZMVP-28 versioning ctr.│
                                                └────────────────────────┘
   ZMVP-35 error model ✅ DONE (problem.rs)  ·  ZMVP-27 AsyncAPI = sibling (core→plugin)
```
The spec is the **dependent** node: it cannot be authored faithfully until both upstream gaps close. Authoring against an unpinned schema is what the ticket explicitly forbids.

## 🚀 8. Next steps
1. ⚠️ **DEFER — triple-blocked.** Do **not** start authoring. Three upstream gaps must close first: **(a)** the Commission Index/export schema must be pinned (**ZMVP-19**, To Do — the forcing function, deadline-driven by the July DeviantArt sunset); **(b)** the **Plugin-security DD** must be written (drafted only in the Plugin page; Blocking Gaps lists it as still pending); **(c)** the **REST-vs-XRPC + token-exchange** fork must be decided (ZMVP-42 retro). All three are **Engineer/Group domain decisions** — not Claude's to resolve.
2. ⚠️ **Decisions needed before this is workable** (all Engineer-owned): the spec file's **home/convention** (no `docs/`-`spec/`-`openapi.*` exists); the **MVP operation set** for `/plugin/v1`; the **action-scope vocabulary**; the `app_key`→token **exchange** shape; whether to add a **CI spec-lint** gate.
3. When unblocked, the Claude-ownable slice is the **mechanical scaffolding** (info/servers/securityScheme/error component/version header) wired to the existing `problem.rs` registry — but only *after* the schema + operation/scope decisions land.
4. **Sequencing note:** naturally pairs with **ZMVP-27** (shared Commission schema) and depends on **ZMVP-28** (versioning); consider scheduling the three together once ZMVP-19 lands.
