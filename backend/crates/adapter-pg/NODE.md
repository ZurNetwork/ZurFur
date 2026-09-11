---
path: backend/crates/adapter-pg
charted: 2026-09-06
fs:
  - name: CLAUDE.md
    role: settled invariants: no cross-store transactions, transactions-as-capability, the five bare-pool exemptions
    node: false
  - name: Cargo.toml
    role: sqlx (derive, not macros), tower-sessions-core, chacha20poly1305/zeroize; dev-deps sqlx-rust-codegen + query-codegen + adapter-atproto + test-support
    node: false
  - name: build.rs
    role: embeds migrations/ into OUT_DIR as a sorted (version, description, sql) array — replaces sqlx::migrate!
    node: false
  - name: sqlx.toml
    role: date-time preferred-crate = chrono
    node: false
  - name: migrations/
    role: timestamp-versioned forward-only SQL (39 files, latest creates commission_markup), embedded and run on boot and by every test template
    node: false
  - name: queries/
    role: "one .sql file per statement, grouped by namespace directory: account/ (28 — create/find/list, memberships & roles, invitations, alias, handle change + rate audit, soft & hard delete, departure re-homing, ownership transfer, locking find), commission/ (41 — create/find/delete, elements & tabs & surface modes, seats & invitations, slots, placement & view grants, statuses, maturity, archive, files, markup add/read, linked channel, locking find), actor_identity/ (create, intern, find, find_by_did, cache_handle), changelog/ (append + ordered read), session/, user/, profile/, file/, key_store/, plc/, health/. Directory name = generated module, file name = generated function, a leading -- comment becomes its doc. Carries NO NODE.md of its own: sqlx-rust-codegen refuses any non-directory under queries/, so the chart for it lives here."
    node: false
  - name: src/
    role: port impls — uow (PgDatabase/PgUnitOfWork), account, user, commission (+markup), commission_changelog, actor_identity, profile, session_store, file_store (streaming, buffers internally), key_store, key_vault, plc_operation_log; @generated queries.rs; lib.rs pool/migrator
    node: false
  - name: tests/it/
    role: one integration binary sharing a Postgres container across ~22 modules — per-surface suites + the codegen_current, no_bare_pool_writes, single_binary_guard, schema_status gates
    node: false
---
**Is:** The private data boundary: app-owned UUIDv7 rows in PostgreSQL, SQL kept in per-statement files, typed accessors generated from a live migrated schema, writes reachable only through a transaction-owning Unit of Work.

**Conventions:** SQL never lives in Rust; `queries.rs` is `@generated` (`just gen-queries`) and its staleness is a CI diff. `query!`/`query_as!` macros are retired by construction (`macros` feature off). Writes only on tx-bound `UnitOfWork` views; the EXEMPT list in `tests/it/no_bare_pool_writes.rs` is authoritative — a new exemption is a design question. Migrations only via `just migrate-add <name>` (a hand-typed round-hour version is a latent collision); filename = `<version>_<name>.sql`, checksums must stay valid. Custody key material is envelope-encrypted (XChaCha20-Poly1305) under a root key before it touches disk. Table names singular from ZMVP-65 on. Commission-owned satellite rows that annotate other bookkeeping (e.g. `commission_markup`) are NON-FACT and carry a composite FK back onto their parent row, so a cross-commission reference is unrepresentable. `FileStore` streams at the port (`AsyncRead` in, `FileDownload` out) but this v1 adapter buffers in memory — a documented exception pending the blob architecture.

**Entry points:** `src/lib.rs` · `src/uow.rs` · `tests/it/main.rs`.

**Refs:** `CLAUDE.md` here · memory `project_sqlx_offline_cache` · DD "Blobs, PDS & Private Storage" (10125341).
