---
path: backend/crates/api
charted: 2026-08-21
fs:
  - name: Cargo.toml
    role: wires domain + adapter-pg + adapter-atproto (adapter-mem/test-support/contract-gen as dev-deps); sqlx only for the sweeper's advisory lock
    node: false
  - name: src/
    role: lib/main, routes, problem, sweep, generated contract types
    node: true
  - name: tests/
    role: ~35 flat integration suites over reqwest (accounts, commissions/*, csrf, session_fixation, cross_persona_unlinkability, golden_wire, contract_current, e2e) + tests/common/mod.rs assert_problem
    node: false
---
**Is:** The composition root and the HTTP surface: the only crate that knows which adapters are live, owning Config/AppState/the router, the RFC 9457 error shape, the generated wire contract, and the one background task.

**Conventions:** two endpoint shapes coexist — the browser sign-in flow redirects; every JSON API returns status codes (401, never a redirect). Errors are `application/problem+json` with a `urn:zurfur:error:*` type and terse code. A namespace boundary is a policy boundary: cookie-surface routers sit under the first-party-Origin CSRF layer; `/health` and a future bearer `/plugin/v1` mount outside it by construction. `src/generated` is `@generated` — regenerate with `just gen-contract`.

**Entry points:** `src/main.rs` · `src/lib.rs::app`.

**Refs:** DD "The API Contract" (40992770) · `contract/VERSIONING.md` · DD "Auth Surfaces, the Plugin Trust Boundary & CSRF" (24543244) · DD "API Response Shape & Error Model" (23592962).
