# Unit of work `h0rnz` — ZMVP-39 (solo)

**Set:** ZMVP-39 (split the flat api router into nested sub-routers). Single ticket — fully Claude-owned, mechanical, behaviour-preserving refactor of `api/src/lib.rs`.
**Minted:** 2026-06-30 by `/parallelize`. **Ledger:** `.understand/parallel-set.json` · **Prior unit:** `b722f9` (ZMVP-26 ∥ ZMVP-38, COMPLETE — #74/#73 merged).

## Why solo (from /next-path 2026-06-30)
The backlog hit a **decision wall**: the cheap settled tickets are done; the next tier each needs one Engineer fork resolved before it parallelizes. ZMVP-39 is the only build-ready, fully-Claude-takeable ticket, and it owns `api/src/lib.rs` wholesale (restructures the route table) — so nothing parallelizes against it on the code side without a guaranteed collision. Padding the set was declined.

## Open threads (what's not yet sound / loose ends)
- [x] **Build ZMVP-39** — DONE: extracted into `src/routes/{health,session,accounts,mod}.rs`; `app()` pure composition; 13-test suite green; signature frozen. Merged → main 5743f0b (PR #76).
- [x] **CSRF layer placement** — RESOLVED: Engineer chose scope-onto-cookie-surface (option b), not keep-global. Rationale: future read/bearer namespace (/plugin/v1) exempt-by-construction. Implemented + security-reviewed.
- [ ] Other worktrees in flight (NOT this unit, watch for merge-order collisions later): `feature/openapi-infra` (code-first OpenAPI tooling — may touch api crate; note: now that the router is split, a code-first OpenAPI change touching routes will land on the new `src/routes/` layout) and `feature/characters-domain` (domain). Disjoint from the router restructure today.
- [ ] **Residual (carried to backlog):** scoped CSRF layer is fail-open for a *future* cookie sub-router that forgets to mount under `cookie_surface` — mitigated by app()/routes-mod doc rule + the requirement that a new cookie router add its own csrf.rs coverage. Revisit if/when the next cookie router lands.

## Backlog notes carried from /next-path (the decision wall — for the NEXT unit)
- **ZMVP-27** (AsyncAPI spec) — blocked: v1 event catalogue/taxonomy unpinned (3 naming styles; "Blocking Gaps for v1" lists it open). Needs `/design-decision "v1 core→plugin event taxonomy"`; then Claude-authorable like ZMVP-26. Para 3/5. CI note: redocly doesn't lint AsyncAPI — needs `@asyncapi/cli validate`.
- **ZMVP-44** (handle issuance/resolution) — blocked: open forks (DNS-TXT vs `/.well-known`; PLC rotation-key custody [DD 4358151 infra DD "in review"]; whether 44 absorbs a real did:plc minter, today only a synthetic stub). Engineer/Group-heavy (diff-8 crypto/custody). Foundation for the handle cluster.
- **ZMVP-48** (punycode reject) — blocked-soft: no handle-validation code exists; the shared path is what ZMVP-44 creates. Rule trivial (DD 26050561 settled). Fold into/behind ZMVP-44, never beside it.
- **ZMVP-47** (capability write-gate) — blocked-soft + premature: target routes (Workflows/Portfolios/plugins) don't exist; 2 forks (scope/sequencing + extractor-vs-guard). Engineer owns bulk. Low overlap w/ handle cluster (it's the lib.rs route-auth seam).
- Snapshots cached this round: `.understand/20260630-1354*_ZMVP-{39,27,48,47}…` + `…135627_ZMVP-44…`.

## Trail
- 2026-06-30 — Minted uow `h0rnz`. /next-path recommended ZMVP-39 solo (decision wall; no clean 2+ code set). Engineer approved → `/parallelize ZMVP-39`.
- 2026-06-30 — `/start`: worktree `~/code/zurfur-zmvp-39-nested-sub-routers` (pg :22247, backend :22248), branch `feature/zmvp-39-nested-sub-routers`, Jira → In Progress. Cached snapshot reused (fresh today, same base) — no re-/understand.
- 2026-06-30 — **Engineer decided item 6 (the one fork): CSRF scoped onto the cookie surface** (option b), not kept global. Rationale: lets a future read-based/bearer namespace (/plugin/v1) mount top-level exempt-by-construction; realises arch 11763713 "namespace boundary = policy boundary". Behaviour-identical today (GET safe-methods pass either way; all current routes are cookie routes).
- 2026-06-30 — BUILT. Extracted `src/routes/{mod,health,session,accounts}.rs`; `app()` now pure composition (`merge(health) + merge(session.merge(accounts).layer(csrf))`). fmt+clippy clean; **all 13 api test files green (44 tests)** — csrf(4) + cross_persona_unlinkability(3) the load-bearing regressions. `app()` signature frozen, zero behaviour delta. CLOSED item 6 open thread.
- 2026-06-30 — `/security-review`: clean. Only security-relevant change is CSRF-layer placement; verified by construction (every state-changing route enumerated under cookie_surface) + byte-identical guard logic + passing csrf suite. Residual (scoped layer fail-open for a *future* unmounted cookie router) is the Engineer-chosen defense-in-depth trade-off, documented in app()/routes-mod docs — not a finding.
- 2026-06-30 — SHIPPED → **PR #76** (paused for merge). Single uow-stamped commit `32c2ac5 [ZMVP-39][uow:h0rnz]`. Gates green locally (fmt + clippy --workspace -Dwarnings + test --workspace, SQLX_OFFLINE). Copilot review requested. Worktree `~/code/zurfur-zmvp-39-nested-sub-routers` (pg :22247) awaits removal post-merge.

## ✅ UNIT COMPLETE — 2026-06-30
PR #76 squash-merged → main `5743f0b` (uow tag survived into the squash subject). Jira ZMVP-39 → Done; worktree + branch removed; main synced. One-ticket solo unit, fully Claude-owned, zero behaviour delta. Backlog decision-wall items (ZMVP-27/44/48/47) carry to the next unit — each gated on one Engineer fork.
