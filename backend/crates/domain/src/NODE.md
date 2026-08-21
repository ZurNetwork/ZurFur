---
path: backend/crates/domain/src
charted: 2026-08-21
fs:
  - name: lib.rs
    role: crate root; the ports-and-adapters position and the eventual split
    node: false
  - name: elements.rs
    role: module index for elements/, live entities vs stubs
    node: false
  - name: elements/
    role: one entity/value-object cluster per file
    node: true
  - name: ports/
    role: trait seams: mod.rs = Database/UnitOfWork/Account/User/Profile/Did/Key/Plc ports; actor_identity, changelog, commission, file re-exported flat
    node: false
  - name: datetime.rs
    role: DateTimeUtc — the single injected clock type
    node: false
  - name: string_builder.rs
    role: StringBuilder: the one trimmed/capped-string gate every newtype validates through (ZMVP-113)
    node: false
---
**Is:** Four public modules: the nouns (elements), the seams (ports), the one clock type, and the shared string-validation builder.

**Conventions:** a new port area adds a file under `ports/` rather than growing `mod.rs`. Time is injected, never read from the ambient clock.

**Entry points:** `ports/mod.rs` (Database + UnitOfWork traits).
