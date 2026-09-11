---
path: backend/crates/application
charted: 2026-09-06
fs:
  - name: Cargo.toml
    role: deps shared+domain+anyhow+tracing+serde_json+tokio(io-util, upload streaming); dev-deps test-support+composition+tokio+uuid+chrono+async-trait
    node: false
  - name: src/lib.rs
    role: crate doc + module declarations, re-exports App/Ports/MissingPort and transaction()
    node: false
  - name: src/app.rs
    role: the Ports bag the composition root assembles, App over it, MissingPort
    node: false
  - name: src/ports.rs
    role: pub(crate) WithPorts<'a> — borrow the Ports bag off any use-case namespace
    node: false
  - name: src/transaction.rs
    role: the one transaction() begin/commit/rollback orchestrator (DD 24150017); mod, not pub mod
    node: false
  - name: src/user.rs
    role: user module — MeQuery/MeResult/MeProfile/MeError, the flat me use case
    node: false
  - name: src/account.rs + src/account/
    role: account module root (AccountError/AccountPorts/AccountResult) + one file per use case
    node: true
  - name: src/commission.rs + src/commission/
    role: commission module root (CommissionError/CommissionPorts, sweep_deadlines) + per-facet use-case trees
    node: true
  - name: tests/account.rs
    role: account use cases against adapter-mem — still imports the pre-restructure flat names (WIP)
    node: false
  - name: tests/commission.rs
    role: integration tests for sweep_deadlines
    node: false
  - name: tests/commission_files.rs
    role: file upload-then-download as a use case (streaming, auth order, reject-then-delete cleanup) via composition::Runtime over adapter-mem
    node: false
  - name: tests/user.rs
    role: integration tests for the me use case
    node: false
  - name: tests/dep_guard.rs
    role: cargo-tree witness — application must never link an adapter, composition, or an HTTP stack (normal edges only; the dev graph is deliberately cyclic)
    node: false
---
**Is:** The application layer (DD 55836674): one plain async fn per use case, called by every driver (api, cli), holding orchestration between routing and domain — organized as `account/` and `commission/` entity trees plus the flat `user` module and the shared `transaction()` orchestrator.

**Conventions:** one plain `pub async fn run` per use case, no mediator. Each use case is its own file exporting `Command` (or `Query`) + `Output` + `run`, so call sites read `commission::slots::declare::run(...)`; sub-facets (`account::invitation`, `commission::deadline::status`, …) nest as directories of such files under the parent module. The module root file keeps the per-entity `Ports` struct of `&dyn` ports, the terse-`Display` error enum (never interpolates the cause — it stays on `source()`) and the `Result` alias. Runtime config + `now: DateTimeUtc` are plain params. Output DTOs carry domain VALUES, never entities. Writes call `transaction()`; reads skip the unit of work. Depends on `domain` and `shared` only — never an adapter or `composition` — enforced by `tests/dep_guard.rs`. A use case need not have an actor: `commission::sweep_deadlines` is the system acting on an injected `now`.

**Entry points:** `src/lib.rs` · `src/app.rs` · `src/account.rs` · `src/commission.rs` · `src/user.rs` · `src/transaction.rs`.

## Notes
- The composition root assembles ONE `Ports` bag (`app.rs`); `App` vends the per-entity namespaces `Accounts`/`Commissions`/`Users` with the ports already bound, and drivers pass only `now`. Every `Ports` entry is required, so `MissingPort` is currently unreachable — it is the home for the first port a driver profile may omit.
- `transaction()` is the one `begin`/`commit`/`rollback` orchestrator: commits on `Ok`, rolls back on `Err`. It takes a bare `&dyn Database` so use cases, `composition::Runtime::transaction` and `api`'s deadline sweeper share it instead of each re-implementing commit/rollback. (DD 24150017)
- Its closure bound is `domain::ports::UnitOfWorkFn` plus explicit `F: Send`/`T: Send`, NOT std's `AsyncFnOnce` — higher-ranked `AsyncFnOnce` bounds do not hold the returned future `Send` (rust-lang/rust#110338).
- Each module's `From<anyhow::Error>` impl is the single place a typed store error becomes a use-case error; `?` is the only path a store error takes, so no call site can let a 404/409 degrade into a 500.
- Authorization posture across the crate: refusals never distinguish "absent" from "forbidden" for a caller with no standing. See the per-module NODE.md notes.

**Refs:** DD 55836674 — The Application Layer, Use Cases, DTOs and Ports · DD 24150017 — Transactions as a capability · DD 23003138 — Account Deletion, Tombstoning & Handle Reuse · memory `project_application_layer_convention` · memory `project_transaction_unit_of_work` · memory `feedback_work_by_module`.
