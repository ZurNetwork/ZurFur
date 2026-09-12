---
path: backend/crates/api
charted: 2026-09-12
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
**Is:** The HTTP driving adapter over `composition::Runtime`: the axum router, the RFC 9457 error shape, the generated wire contract, the session layer, and the one background task — handlers are thin drivers over the shared `application::*` use cases (`commissions/channel` + `commissions/elements` still await a use case, and `session`'s sign-in provisioning still opens its own unit of work).

**Conventions:** two endpoint shapes coexist — the browser sign-in flow redirects; every JSON API returns status codes (401, never a redirect). Errors are `application/problem+json` with a `urn:zurfur:error:*` type and terse code. A namespace boundary is a policy boundary: cookie-surface routers sit under the first-party-Origin CSRF layer; `/health` and a future bearer `/plugin/v1` mount outside it by construction. Session→identity resolution is one axum extractor, `extract::CallingUser`, never a per-module helper. Parity tests (`create_account_parity.rs`, `delete_account_parity.rs`, `whoami_parity.rs`) pin this driver's projections identical to the CLI's until the contract moves to a leaf crate.

**Entry points:** `src/main.rs` · `src/lib.rs::app`.

**Refs:**
- DD 40992770 — The API Contract (the generated wire types every handler returns) · `contract/VERSIONING.md`.
- DD 23592962 — API Response Shape & Error Model (`src/problem.rs`).
- DD 24543244 — Auth Surfaces, the Plugin Trust Boundary & CSRF (the cookie-surface policy zone).
- DD 55836674 — The Application Layer (D6/D7: authorization + transaction belong in `application`, so driver-side authz survives only in `routes/commissions/elements.rs`/`channel.rs`/`mod.rs`; D10: CLI/API projection parity, `routes/accounts.rs`) · ZMVP-205.
- DD 11763713 — Domains and Applications (`routes/mod.rs`).
- DD 26607618 — Handle Resolution for *.zurfur.app (`routes/wellknown.rs`).
- DD 26247170 — User as Actor & On-Demand Accounts (`routes/commissions/create.rs`, `tests/commissions.rs`, `tests/account_scope_gate.rs` — a user-scoped write needs no Account).
- DD 29130754 — Commission Ownership Separation (`routes/commissions/positioning.rs`, `tests/commission_positioning.rs` — grants are per-User, placement lives on the account board).
- DD 57081857 — Actor Addressing (`routes/accounts.rs`, `tests/commission_positioning.rs` — an Account id IS a DID, no surrogate).
- DD 27852802 — Account Handle Change Flow (`routes/accounts.rs`).
- DD 45514754 — Commission Composition (`routes/commissions/elements.rs`).
- DD 30408741 — The Changelog (`routes/commissions/create.rs`).
- DD 3014657 — Deletion of Commissions (`routes/commissions/delete.rs`/`archive.rs`, `tests/commission_delete.rs`, `tests/commission_listing.rs`).
- DD 29982722 — Maturity Vocabulary (`routes/commissions/maturity.rs`, `src/problem.rs`, `tests/commission_maturity.rs`).
- DD 34013187 — Identities, the Actor Super-Table (actor-kind uniqueness: `src/problem.rs`, `tests/commission_seat_invitations.rs`, `tests/invitations.rs`).
- DD 23003138 — Account Deletion, Tombstoning & Handle Reuse (`src/problem.rs`, `tests/account_listing.rs`, `tests/accounts.rs`, `tests/invitations.rs`).
- DD 28311564 — Referenceable, Slot & Seat (`routes/commissions/seats.rs`).
- DD 32112642 — The Linked Channel and DD 6848513 — External Chat Tracking (`routes/commissions/mod.rs`, `channel.rs`).

## Notes
- Cross-persona unlinkability: no route may join one person's separate handles/Users as the same human (holds by construction — separate handle → separate User → separate DID); guarded by `tests/cross_persona_unlinkability.rs`.
- The cookie surface also carries `Cache-Control: no-store` (beyond CSRF) so authenticated identity/PII JSON is never cached (CWE-525).
- `src/generated/*.rs` still carries DD pointers in its doc comments: it is generated from `contract/*.proto`, so the fix belongs on the `.proto` field comments, not here — a hand-edit would be regenerated away.
