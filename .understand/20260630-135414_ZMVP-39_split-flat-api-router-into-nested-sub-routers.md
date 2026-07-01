# ZMVP-39 — Split the flat api router into nested sub-routers

- **Snapshot:** 2026-06-30 13:54:14 · `/understand`
- **Jira:** ZMVP-39 (Task, Medium, **To Do**, unassigned) — https://zurnetwork.atlassian.net/browse/ZMVP-39
- **Type:** mechanical, behaviour-preserving refactor of the `api` crate router. No new endpoints, no contract changes.
- **Branch base:** `main` (clean). The tickets that previously collided here (ZMVP-21/23/24/40) are all merged.

---

## 1. Cold-start context

`api::app(state) -> Router` lives in `backend/crates/api/src/lib.rs` (lines **260–292**) and is a **single flat `Router::new()`** — one `Router::new()`, twelve `.route(...)` calls, one global CSRF `.layer(...)`, one `.with_state(...)`. There is **no `routes.rs` / `handlers.rs` / per-resource module**: the router *and* all thirteen handlers *and* the shared helpers all live in one 1192-line `lib.rs`. `main.rs` (93 lines) is the live-adapter binary that wraps `app()` with the session layer; `problem.rs` (301 lines) is the RFC 9457 error type.

The crate already carries four route areas — health, the OAuth/HTML sign-in flow, `/me`, and the `/accounts/*` tree (members + invitations) — and the ticket's premise is that one flat table will become a merge-conflict hotspot as gallery/workflow/plugin land.

**This is implementing already-decided architecture, not designing it.** DESIGN page *"Domains and Applications"* (`11763713`) already prescribes the exact target: subdomain → namespace → router, each domain crate exposes a `router()`, and `api` is "pure composition" that `.nest(...)`s them. It even gives a worked `api`-crate example (`Router::new().nest("/login", …).nest("/accounts", …).nest("/plugin/v1", …).layer(cors()).layer(trace())`) and states the load-bearing rule: **a namespace boundary is also a policy (auth) boundary** — `/plugin/v1` is its own top-level namespace *because* it authenticates by `app_key`, not cookie, and is therefore CSRF-exempt.

Caveat: the per-domain crates (`identity`, `gallery`, `workflow`, `plugin`) **do not exist yet** — there is still a single transitional `domain` crate. So ZMVP-39 is the *intermediate* step: introduce sub-router builder functions **inside the `api` crate** (e.g. `mod routes { fn health_router()/session_router()/accounts_router() }`), grouped along those future namespace seams, so the eventual crate split is a move, not a redesign.

## 2. Domain & invariants in play

- **Cross-persona unlinkability (ZMVP-17).** The `app()` doc comment (lib.rs 244–253) is the canonical statement that "this table is the public surface" and no route may correlate one person's separate handles. Guarded by `tests/cross_persona_unlinkability.rs`, which includes a **structural negative-route guard** (lines 218–235): `GET /users`, `GET /accounts`, `GET /profiles`, `GET /members` must each return **404 or 405** (no enumeration surface). `GET /accounts` must keep returning **405** (path exists for POST only) — so the accounts sub-router must register *only* the write verbs and must not accidentally widen a method or flip 405→404.
- **CSRF / auth-surface boundary (DD 24543244, ZMVP-23).** `require_first_party_origin` is currently a **global** `.layer()` inside `app()` (lib.rs 287–290) wrapping *every* route incl. `/health`. The doc comments (283–302) already declare the intent the refactor should realise: the future bearer `/plugin/v1` surface must sit **outside** this layer. Whether to scope CSRF onto the cookie sub-router now (vs leave it global) is the one judgment call (see §7).
- **No glossary entity is reshaped.** This touches the *architecture* page (`11763713`), and only to *conform* to it; it does not change User/Account/Commission/etc.

## 3. The real goal & scope

**Goal:** `app()` stops declaring every route inline; routes are grouped into nested/merged sub-routers that mirror domain boundaries, so a future domain adds its own router without touching unrelated groups — **with every existing path resolving unchanged and every current test green.**

**In scope:**
- Extract per-area sub-router builders (health, auth/session HTML, accounts→members + accounts→invitations).
- `app()` becomes the composition point: `Router::new().merge(...)/.nest(...)` then attach CSRF `.layer()` + `.with_state()`.
- Decide where the shared auth/lookup helpers (`require_user`, `load_account`, `actor_role`, `ok_json`, `created_json`) and HTML helpers land so both members and invitations handlers still reach them.
- Keep the `app()` doc comment accurate after the move (privacy/correlation guidance + CSRF guidance).

**Out of scope (do NOT):** add/remove/rename any route or method; change handler behaviour or signatures; mount `/plugin/v1` (no Rust handlers exist — it's spec-only at `openapi/plugin-v1.yaml`); split the `domain` crate; change `app()`'s public signature (`pub fn app(AppState) -> Router`) — every test depends on it; add tracing layers.

## 4. Concrete deliverables

1. A routes module (or submodules) in the `api` crate with sub-router builder fns returning `Router<AppState>`, grouped by area.
2. `app()` rewritten as composition: merge/nest the sub-routers, then `.layer(CSRF)` + `.with_state(state)`.
3. Shared helpers relocated to a place reachable by all sub-routers (likely an internal `mod` kept `pub(crate)`), without behaviour change.
4. `app()` doc comment updated: route list, the unlinkability paragraph, and the CSRF-layer description kept truthful to the new structure.
5. Whole existing test suite (13 test files) green; `cargo fmt`/`clippy` clean.

## 5. Work-breakdown (difficulty × owner × evidence)

| # | Item | Diff (0–6+) | Owner | Done = |
|---|------|:---:|:---:|--------|
| 1 | Extract `session_router()` (`/`, `/signin`, `/signin-callback`, `/me`, `/logout`) | 1 | 🤖 Claude | e2e/session/logout/profile/session_fixation/session_persistence tests green |
| 2 | Extract `accounts_router()` with members + invitations sub-trees | 2 | 🤖 Claude | accounts/invitations/leave tests green; `GET /accounts`→405 preserved |
| 3 | Keep `/health` top-level; recompose `app()` via merge/nest | 1 | 🤖 Claude | health test green; all paths resolve unchanged |
| 4 | Relocate shared helpers reachable across sub-routers | 2 | 🤖 Claude | compiles; no behaviour delta |
| 5 | Update `app()` doc comment (routes + unlinkability + CSRF) | 1 | 🤖 Claude | comment matches new shape |
| 6 | **Scope decision: CSRF layer global vs cookie-surface-scoped now** | 3 | 🧑 Engineer (proposal ready) | Engineer picks; csrf.rs stays green either way |

**Bands:** all execution sits in the **0–3 (Claude)** band; the *bulk* (items 1–5) is mechanical and clearly-correct. The single **3 / Engineer-touch** item (6) is a small design fork, not a build task — and it has a low-risk default (leave CSRF global; `/plugin/v1` isn't mounted yet) that keeps the ticket fully Claude-executable if the Engineer defers it. **No 6+ / Group work.**

## 6. TDD / test checklist (mostly regression — the suite already exists)

The refactor preserves `app()`'s signature and path table, so the existing 13 files *are* the spec; the work is "keep them green," not "write new ones."

- **Routing unchanged:** health.rs (`GET /health`), session.rs (`GET /me`→redirect), e2e.rs (full sign-in flow), logout.rs, profile.rs, session_fixation.rs, session_persistence.rs (PgSessionStore).
- **Accounts tree:** accounts.rs (POST/DELETE members, 401/403/404/422), invitations.rs (invite/revoke/decline/accept), leave.rs (`DELETE /accounts/{id}/members/me`).
- **Most refactor-sensitive — must not regress:**
  - `csrf.rs` — asserts `require_first_party_origin` still wraps the cookie routes (cross-origin refused, same-origin/no-Origin/safe-method pass). If CSRF moves to a sub-router, this must still hold for every cookie route.
  - `cross_persona_unlinkability.rs` (218–235) — `GET /users|/accounts|/profiles|/members` each 404/405; `GET /accounts` specifically **405**.
- **Optional new guard:** a tiny test asserting `/health` is reachable without a session/Origin even after recomposition (cheap insurance that recomposition didn't accidentally pull it under a layer).

## 7. Open questions / decision to route

- **CSRF layer placement (item 6).** Today CSRF is global inside `app()`. Two options: (a) keep it global, scope-narrow later when `/plugin/v1` actually mounts; (b) scope it now onto the cookie sub-router(s) so the bearer surface is exempt *by construction* the day it lands. **Recommendation:** (b) is the better realisation of DESIGN `11763713` ("namespace boundary = policy boundary") and of the memory `make_unsoundness_unreachable` (one shared enforced path, not per-site drift) — but it's the Engineer's call since it shapes where a security boundary attaches. Low-risk fallback (a) keeps the ticket unblocked if the Engineer isn't available. **This is the only domain-flavoured fork; everything else is mechanical.**
- Naming of the sub-router builder fns / module layout — propose mirroring the future namespace names (`session`/`accounts`/`health`) per `11763713`; Engineer may prefer otherwise. Trivial, non-blocking.

## 8. Next steps

1. `/start ZMVP-39` → feature branch `feature/zmvp-39-nested-sub-routers` off clean `main`; transition Jira → In Progress.
2. Put item 6 (CSRF scoping) to the Engineer with the §7 recommendation; default to "keep global" if deferred.
3. Extract sub-router builders (items 1–4) refactor-first, running the suite after each extraction (red→green = unchanged tests stay green).
4. Recompose `app()`; update its doc comment (item 5).
5. `cargo fmt && cargo clippy && just test` (workspace). Then `/critique` → `/prepare-pr`.
6. **No `/design-sync` expected** — the change conforms to `11763713`, it doesn't alter it. If the CSRF placement is changed, note it on the ZMVP-23 DD only if it materially shifts the described boundary.

---

### Footprint
- **Crate:** `backend/crates/api` (only).
- **Files:** `src/lib.rs` (router + handlers + helpers — the whole edit), `src/main.rs` (likely untouched; session layer stays outside `app()`). Possibly a new `src/routes/` module (or `src/routes.rs`) if helpers/sub-routers are split out.
- **Tests:** all 13 files in `backend/crates/api/tests/` are regression coverage; none should need editing.
- **Migrations:** none.
- **Not touched:** `openapi/plugin-v1.yaml` (spec-only, not router-coupled), `domain`/`adapter-*` crates.
