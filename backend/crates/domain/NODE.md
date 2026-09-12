---
path: backend/crates/domain
charted: 2026-09-12
fs:
  - name: Cargo.toml
    role: value-object deps: chrono, uuid, cid, serde, zeroize, async-trait; tokio io-util only to name AsyncRead for the streaming FileStore port
    node: false
  - name: README.md
    role: architecture overview: elements/ports/datetime, principles, usage
    node: false
  - name: src/
    role: elements + ports + the clock + the string builder
    node: true
  - name: tests/
    role: fact_contract.rs — the Fact contract (commission-anchored evidence blocks hard deletion)
    node: false
---
**Is:** Zurfur's I/O-free core — the platform's nouns and the trait seams every adapter implements; deliberately transitional, to split into per-namespace crates (identity, gallery, workflow, plugin).

**Conventions:** every element mirrors a DESIGN glossary page. Stub modules are allowed, and documented honestly as stubs. Errors surface as `anyhow::Result` with context.

**Entry points:** `src/lib.rs`.

**Refs:** DESIGN glossary pages · DD 34013187 (Actor super-table) · DD 45514754 (flat composition) · ZMVP-67 (tests/fact_contract.rs: the Fact contract's origin ticket) · DD 3014657 "Deletion of Commissions" (tests/fact_contract.rs: why no production Fact implementor exists yet).
