---
path: backend/crates
charted: 2026-08-21
fs:
  - name: domain/
    role: pure core: elements + ports, no I/O
    node: true
  - name: api/
    role: composition root + axum HTTP surface + generated contract types
    node: true
  - name: adapter-pg/
    role: private boundary: Postgres port impls, migrations, per-statement SQL
    node: true
  - name: adapter-atproto/
    role: public boundary: OAuth, PDS records, did:plc minting
    node: true
  - name: adapter-mem/
    role: in-process fakes of every port
    node: true
  - name: test-support/
    role: shared test rig: throwaway PDS + stub PLC, shared Postgres harness, PublicRecords conformance suite
    node: true
  - name: contract-gen/
    role: protox+prost+pbjson generator (lib + `just gen-contract` bin) → api/src/generated
    node: false
  - name: query-codegen/
    role: sqlx-rust-codegen config + `just gen-queries` runner: boots a container, migrates, rewrites both adapters' src/queries.rs
    node: false
---
**Is:** Eight workspace crates arranged as a hexagon: one I/O-free core, three adapters implementing its ports, one composition-root binary, and three support/codegen crates.

**Conventions:** read/write split per aggregate — `*Store` is pool-backed and non-transactional, `*Writes` is reachable only on an open `UnitOfWork`. Codegen crates are libraries first so the runner binary and the staleness-gate test share one generation body. `test-support` dev-depends on `adapter-pg` while adapters dev-depend on `test-support` — a cargo dev-cycle, not a runtime edge.

**Entry points:** each crate's `src/lib.rs` — the module-level docs are the real map.

**Refs:** `domain/README.md` · `adapter-pg/CLAUDE.md` · `adapter-atproto/CLAUDE.md`.
