---
path: backend/crates/api
charted: 2026-09-06
fs:
  - name: Cargo.toml
    role: wires domain + adapter-pg + composition + application + shared, tokio-util/futures-util for upload streaming glue (adapter-mem/test-support/contract-gen/cli as dev-deps); sqlx only for the sweeper's advisory lock
    node: false
  - name: src/
    role: lib/main, extractors, routes, problem, sweep, generated contract types
    node: true
  - name: tests/
    role: ~40 flat integration suites over reqwest (accounts, commissions/*, csrf, session_fixation, cross_persona_unlinkability, golden_wire, contract_current, e2e, plus CLI/API parity tests create_account_parity/delete_account_parity/whoami_parity) + tests/common/mod.rs assert_problem
    node: false
---
**Is:** The composition root and the HTTP surface: the only crate that knows which adapters are live, owning the router, the RFC 9457 error shape, the generated wire contract, and the one background task — handlers are thin drivers over the shared `application::*` use cases (ZMVP-205; `commissions/channel` + `commissions/elements` still await a use case, and `session`'s sign-in provisioning still opens its own unit of work), with CLI/API parity pinned by dedicated tests.

**Conventions:** two endpoint shapes coexist — the browser sign-in flow redirects; every JSON API returns status codes (401, never a redirect). Errors are `application/problem+json` with a `urn:zurfur:error:*` type and terse code. A namespace boundary is a policy boundary: cookie-surface routers sit under the first-party-Origin CSRF layer; `/health` and a future bearer `/plugin/v1` mount outside it by construction. `src/generated` is `@generated` — regenerate with `just gen-contract`. Session→identity resolution is one axum extractor, `extract::CallingUser`, never a per-module helper. HTTP drivers (this crate) and the CLI both call the same `application::*` use cases (DD 55836674, ZMVP-205); parity tests (`create_account_parity.rs`, `delete_account_parity.rs`, `whoami_parity.rs`) pin the two drivers' projections identical until the contract moves to a leaf crate (DD 40992770 D11).

**Entry points:** `src/main.rs` · `src/lib.rs::app`.

**Refs:** DD "The API Contract" (40992770) · `contract/VERSIONING.md` · DD "Auth Surfaces, the Plugin Trust Boundary & CSRF" (24543244) · DD "API Response Shape & Error Model" (23592962) · DD "The Application Layer — Use Cases, DTOs and Ports" (55836674).
