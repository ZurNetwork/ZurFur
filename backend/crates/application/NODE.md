---
path: backend/crates/application
charted: 2026-09-06
fs:
  - name: Cargo.toml
    role: deps shared+domain+anyhow+tracing+serde_json+tokio(io-util, upload streaming); dev-deps test-support+composition+tokio+uuid+chrono+async-trait
    node: false
  - name: src/lib.rs
    role: crate doc (the use-case shape contract) + module declarations, re-exports transaction()
    node: false
  - name: src/errors.rs
    role: pub(crate) NotFound<T> — a generic not-found constructor per error enum (CommissionError so far)
    node: false
  - name: src/ports.rs
    role: pub(crate) WithPort<P> — borrow one port off a Ports struct generically; not yet consumed
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

**WIP (2026-09-06, uncommitted, Engineer's ZMVP-205 stack):** the crate does not build — `commission/getters.rs` was deleted while ~15 commission use cases still call it (the new `errors::NotFound<T>` looks like the intended replacement), `commission.rs` declares a `role` submodule with no backing file, and `tests/account.rs` targets the old flat API.

**Entry points:** `src/lib.rs` · `src/account.rs` · `src/commission.rs` · `src/user.rs` · `src/transaction.rs`.

**Refs:** DD 55836674 — The Application Layer, Use Cases, DTOs and Ports · DD 24150017 — Transactions as a capability · DD 23003138 — Account Deletion, Tombstoning & Handle Reuse · memory `project_application_layer_convention` · memory `project_transaction_unit_of_work` · memory `feedback_work_by_module`.
