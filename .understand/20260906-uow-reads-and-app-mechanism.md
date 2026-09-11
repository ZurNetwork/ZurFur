# Plan — reads on the Unit of Work + the `App` factory (mechanism only)

**Date:** 2026-09-06 · **Owner of the decision:** the Engineer (rulings this session) · **Scope:** the mechanism, no call-site migration. The Engineer migrates one call site (`commission::deadline::set`) afterwards to measure friction; the DD (amending 24150017 + 55836674) is written as a reaction to that.

## Rulings this plan executes

- Application = the **Orchestrator**. API/CLI decode + receive; Orchestrator enforces rules and composes simple persistence operations; Domain = invariants; Persistence = granular, dumb, decides nothing.
- Reads become available **on the unit of work** (the transaction's connection), not only pool-side.
- No per-module `Ports` structs for new code: one bag, an `App` factory built at composition, drivers hold `App` and pass `now`.
- `Database::begin()` stays the factory. Drop without commit = rollback. `?` = rollback. Commit consumes the unit.
- Use cases become methods on per-namespace structs (`Commissions<'_>`), assembled across files with one `impl` block per use-case file. `impl` blocks hold use cases only; helpers stay free functions.
- `Commissions` exposes `new`, `From`, and `TryFrom` constructors.
- New port traits arise only from repetition (refactor step). This plan adds the minimum the first migration needs.
- sqlx stays. No query builder.

## Open assumption (redirect me if wrong)

`TryFrom` needs a failure. Assumed source: the bag's *optional* adapters. `Ports.files` and `Ports.did_minter` become `Option<Arc<dyn …>>` so a driver without a blob store or a minter (a CLI profile) can still build an `App`. `Commissions: TryFrom<&Ports>` fails with `MissingPort("files")`; `Accounts: TryFrom<&Ports>` fails with `MissingPort("did_minter")`; `Users: From<&Ports>` never fails. `new` takes the fully-resolved dependencies; `From<&App>` delegates to `TryFrom` and panics with the port name (the composition root guarantees completeness; a panic there is a boot-time misconfiguration, not a runtime path).

## Executed 2026-09-06 (scope narrowed by the Engineer: uow functions + factory only, no adapter migration)

Done, uncommitted on the primary checkout: slice 1 (domain: `CommissionReads`/`AccountReads`, `CommissionRepo`/`AccountRepo`, `UnitOfWork` vends the repos, `rollback` doc-marked legacy) and slice 4 (application: `app.rs` with `Ports`/`App`/`MissingPort`; `Commissions`/`Accounts`/`Users` namespaces with `new`/`From<&App>`/`TryFrom<&Ports>`). `cargo check -p domain` shows no error from the ports; the workspace stays red on the Engineer's in-flight ZMVP-205 edits (domain `Account { did }`, application `getters`). Slices 2, 3, 5, 6 deliberately NOT done: adapters do not yet implement the `*Reads` traits, so adapter-pg/adapter-mem will not compile until the Engineer's persistence migration lands.

## Slices (each gate-green: fmt, clippy, full tests)

### 1 · domain: the tx-bound read side
Files: `domain/src/ports/commission.rs`, `domain/src/ports/account.rs`, `domain/src/ports/mod.rs`.
- Add `CommissionReads` (`&mut self`): `find`, `find_for_update`, `is_participant`, `tab_for_update`. Add `AccountReads` (`&mut self`): `find`, `find_for_update`, `role_of`.
- Add `pub trait CommissionRepo: CommissionReads + CommissionWrites {}` + blanket impl; same for `AccountRepo`.
- `UnitOfWork::commissions()` / `accounts()` return `Box<dyn CommissionRepo + '_>` / `Box<dyn AccountRepo + '_>`. Existing call sites keep compiling (supertrait).
- New domain value `Tab { id, commission, name }` if not already loadable as one.
- `rollback()` stays (used by `transaction()`), doc-marked for removal after migration.
- **Done when:** workspace builds with no call-site edits.

### 2 · adapter-pg: reads on the transaction
Files: `adapter-pg/queries/commission/{find_for_update,tab_for_update}.sql`, `adapter-pg/queries/account/find_for_update.sql`, `adapter-pg/src/{commission,account,uow}.rs`, `src/queries.rs` (regenerated).
- SQL: `SELECT … FROM commission WHERE id = $1 FOR NO KEY UPDATE` (and the tab/account variants; tab keeps the `(id, commission_id)` pair). `just gen-queries`.
- Extract row→domain mapping and read bodies into helpers generic over `impl sqlx::PgExecutor<'_>` (the generated fns already take it). `PgCommissionStore` (pool) and the tx view call the same helpers — no duplicated read logic.
- Rename `PgCommissionWrites` → `PgCommissionRepo`; implement `CommissionReads` over `&mut *self.tx`. Same for accounts. `PgUnitOfWork` returns them.
- `no_bare_pool_writes` guard: unchanged (reads are not writes). `WRITE_QUERY_FNS` unchanged.
- **Done when:** `codegen_current` green; all `tests/it` green.

### 3 · adapter-mem: reads over the staged copy
Files: `adapter-mem/src/{commission,lib}.rs`, and the account fake.
- `MemCommissionWrites` → `MemCommissionRepo`; reads answer from the **staged** maps so a read inside the unit sees the unit's own writes. `find_for_update` is a plain read (single-threaded fake; locking is adapter-pg's to prove).
- **Done when:** `adapter-mem/tests/commission.rs` green plus the new mechanism tests (slice 6).

### 4 · application: `Ports`, `App`, the namespace structs
Files: `application/src/{lib,app}.rs`, `application/src/{commission,account,user}.rs`.
- `pub struct Ports { database, commissions, accounts, users, changelog: Arc<dyn …>, files: Option<Arc<dyn FileStore>>, did_minter: Option<Arc<dyn DidMinter>> }` at the crate root.
- `pub struct App { ports: Ports }`; `App::new(Ports)`; `commissions()`, `accounts()`, `users()` accessors returning the namespace structs (via the `From<&App>` impls).
- `pub struct Commissions<'a> { ports: &'a Ports, files: &'a dyn FileStore }` with `new(ports, files)`, `impl From<&'a App>`, `impl TryFrom<&'a Ports>` (`MissingPort`). `Accounts` likewise with `did_minter`. `Users` with `From<&'a Ports>` only.
- `pub enum MissingPort { Files, DidMinter }` (terse `Display`, per the error rule).
- `transaction()`, `UnitOfWorkFn`, `CommissionPorts`, `AccountPorts` **untouched**.
- `tests/dep_guard.rs` still passes (no new deps).
- **Done when:** `App::new(mem ports).commissions()` constructs in a unit test; `TryFrom` error path tested.

### 5 · composition + drivers + test-support: build the `App`
Files: `composition/src/runtime.rs`, `composition/src/ports.rs`, `api/src/lib.rs` (`AppState`), `cli/src/lib.rs`, `test-support/src/runtime.rs`.
- `Runtime::connect` also assembles `application::Ports` and stores `app: Arc<App>`; `From<&Runtime> for &App` (or a getter). Existing `From<&Runtime> for XPorts` stay.
- `AppState` exposes `app`; CLI runtime likewise. No handler changes.
- Mem fixture builds the same `App` from the mem adapters (`MemRuntime::app()`).
- **Done when:** api + cli boot paths construct `App`; parity tests unchanged and green.

### 6 · mechanism tests (the evidence)
Files: `adapter-pg/tests/it/uow.rs` (new module, registered in `main.rs` for `single_binary_guard`), `adapter-mem/tests/uow.rs`, one shared suite in `test-support` if the bodies are identical.
Against **both** adapters:
- drop without commit rolls back (extend `a_dropped_unit_of_work_discards_the_archive` to the new read path);
- `?` on the write path rolls back;
- a read inside the unit sees the unit's own uncommitted write;
- after commit, a fresh pool read sees it.
adapter-pg only:
- a second `find_for_update` on the same row **waits** until the first unit commits (two connections, `tokio::time::timeout`, assert the second completes only after the first's commit).
- **Done when:** all green in CI.

### 7 · docs + chart
- `///` on every changed public item (terse, per the docs rule). `NODE.md` for `application`, `adapter-pg`, `adapter-mem`, `composition` re-charted (`/chart --diff`).

## Order and lanes
1 → 2 → 3 → 4 → 5 → 6 → 7. Slices 2–3 are the bulk and mechanical → builder agent (Sonnet). Slices 1, 4, 6 → inline (Fable), small and contract-shaped. Slice 5 → builder. Each slice its own PR into the feature branch, per the two-tier PR rule.

## Not in scope
- Migrating any use case (the Engineer takes `deadline::set` first).
- Removing `transaction()`, `rollback()`, `CommissionPorts`, or adapter-side gates (`require_tab`, `require_address`) — those go as call sites migrate.
- The `Committed<T>` token (declined for now; silent rollback on a forgotten commit accepted).
- The DD.

## Needs before `/start`
A ZMVP ticket for the mechanism (suggested: "Unit of work: reads on the unit + the App factory"), a feature branch at `main`'s tip. The primary checkout currently carries the Engineer's uncommitted ZMVP-205 stack, so this lands on its own branch and rebases onto whatever the Engineer commits first.
