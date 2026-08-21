---
path: backend/crates/domain
charted: 2026-08-21
fs:
  - name: Cargo.toml
    role: value-object-only deps: chrono, uuid, cid, serde, zeroize, async-trait
    node: false
  - name: README.md
    role: architecture overview: elements/ports/datetime, principles, usage
    node: false
  - name: src/
    role: elements + ports + the clock + the string builder
    node: true
  - name: tests/
    role: fact_contract.rs — the Fact contract (commission-anchored evidence blocks hard deletion, ZMVP-67)
    node: false
---
**Is:** Zurfur's I/O-free core — the platform's nouns and the trait seams every adapter implements; deliberately transitional, to split into per-namespace crates (identity, gallery, workflow, plugin).

**Conventions:** every element mirrors a DESIGN glossary page; module docs carry the ZMVP ticket and DD id. Stub modules are allowed and documented honestly as stubs. Errors surface as `anyhow::Result` with context.

**Entry points:** `src/lib.rs`.

**Refs:** DESIGN glossary pages · DD 34013187 (Actor super-table) · DD 45514754 (flat composition).
