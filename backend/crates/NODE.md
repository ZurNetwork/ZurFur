---
path: backend/crates
charted: 2026-09-12
fs:
  - name: domain/
    role: I/O-free core: elements (the nouns), ports (trait seams), the injected clock, the string-validation builder
    node: true
  - name: application/
    role: use cases — one plain async fn per use case + the shared transaction() orchestrator, called by every driver
    node: true
  - name: composition/
    role: HTTP-free composition root shared by api + cli — loads Config, wires the live Runtime port bag
    node: true
  - name: shared/
    role: bottom leaf crate of build-time constants that even domain may consume; no intra-workspace deps
    node: true
  - name: api/
    role: HTTP driver: axum router, RFC 9457 problem shape, generated wire contract, background sweeper
    node: true
  - name: cli/
    role: the zurfur terminal driver: clap subcommands over the same application use cases, in-process via Runtime
    node: true
  - name: adapter-pg/
    role: private boundary: Postgres port impls, migrations, per-statement SQL files, generated typed accessors
    node: true
  - name: adapter-atproto/
    role: public boundary: OAuth sign-in, PDS records/blobs, public profile reads, did:plc minting + key custody
    node: true
  - name: adapter-mem/
    role: in-process fakes of every port, transactional rollback included
    node: true
  - name: test-support/
    role: shared test rig: throwaway PDS + stub PLC, one-container/template-clone Postgres, PublicRecords conformance suite
    node: true
  - name: contract-gen/
    role: protox+prost+pbjson generator (lib + `just gen-contract` bin) → api/src/generated
    node: true
  - name: query-codegen/
    role: sqlx-rust-codegen runner (`just gen-queries`): boots a container, migrates, rewrites both adapters' src/queries.rs
    node: true
---
**Is:** Twelve workspace crates arranged as a hexagon: one I/O-free core, one application layer of use cases, one HTTP-free composition root, two driving adapters (api, cli) over it, three data-boundary adapters implementing its ports, a leaf config crate, and three support/codegen crates.

**Conventions:** read/write split per aggregate — `*Store` is pool-backed and non-transactional, `*Writes` is reachable only on an open `UnitOfWork`. Closed-vocabulary enums implement std `Display`/`FromStr`, never bespoke `as_str`/`parse` pairs; id newtypes may derive serde, loaded entities never do. Codegen crates are libraries first so the runner binary and the staleness-gate test share one generation body. `test-support` dev-depends on `adapter-pg` while adapters dev-depend on `test-support` — a cargo dev-cycle, not a runtime edge. `application` depends only on `domain` + `shared`, never an adapter or `composition`.

**Entry points:** each crate's `src/lib.rs` — the module-level docs are the real map.

**Refs:** `domain/README.md` · `adapter-pg/CLAUDE.md` · `adapter-atproto/CLAUDE.md` · DD "The Application Layer" (55836674).
