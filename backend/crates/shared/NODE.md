---
path: backend/crates/shared
charted: 2026-08-29
fs:
  - name: Cargo.toml
    role: crate manifest — name `shared`, only dep is chrono (workspace)
    node: false
  - name: src/lib.rs
    role: crate root doc + `pub mod settings`
    node: false
  - name: src/settings.rs
    role: compile-time constants — HANDLE_CHANGE_LIMIT/WINDOW, HANDLE_QUARANTINE_WINDOW (DD 27852802)
    node: false
---
**Is:** Zero-dependency leaf crate holding build-time configuration constants that every other backend layer, including `domain`, is allowed to consume.

**Conventions:** no workspace dependencies allowed here — this crate must never pull an adapter or driver into a lower layer. Holds build-time configuration constants only (windows/limits/ceilings the DDs leave to implementation); runtime config (env, profiles, ports) is `composition::Config`, not this crate.

**Entry points:** `src/lib.rs` · `src/settings.rs`.

**Refs:** DD 27852802 — Account Handle Change Flow · DD 55836674 — The Application Layer (source of the "shared = the one leaf crate domain may consume" ruling) · memory `project_application_layer_convention.md`.
