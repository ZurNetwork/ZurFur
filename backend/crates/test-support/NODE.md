---
path: backend/crates/test-support
charted: 2026-08-21
fs:
  - name: Cargo.toml
    role: testcontainers + postgres module, axum for the PLC stub, adapter-pg for migrations, atrium-crypto
    node: false
  - name: src/lib.rs
    role: crate docs + usage recipe; pins the ZURFUR_PDS_IMAGE literal
    node: false
  - name: src/pds.rs
    role: ThrowawayPds::boot — container lifecycle, teardown on drop
    node: false
  - name: src/plc_stub.rs
    role: in-process stub PLC directory on loopback, reached via docker host-gateway
    node: false
  - name: src/fixture.rs
    role: FixtureAccount / ActingCredential (PDS session JWT)
    node: false
  - name: src/pg.rs
    role: shared Postgres harness: one container per process, migrated template, TEMPLATE clone per test
    node: false
  - name: src/contract.rs
    role: the generic PublicRecords conformance suite (mem + atproto)
    node: false
  - name: tests/throwaway_pds.rs
    role: self-test incl. the image/.env.example drift assertion
    node: false
---
**Is:** The shared test rig: a hermetic throwaway PDS with an in-process stub PLC directory, a one-container/template-clone Postgres harness, and the cross-adapter PublicRecords conformance suite.

**Conventions:** zero requests to the public atproto network — minting lands at the local stub only. Needs a container runtime socket (`DOCKER_HOST` honored) and Tokio. The default PDS image literal must equal `.env.example`'s `ZURFUR_PDS_IMAGE`. Container reuse is an escape hatch, not the default.

**Refs:** ZMVP-103 / 105 / 134 · `.env.example`.
