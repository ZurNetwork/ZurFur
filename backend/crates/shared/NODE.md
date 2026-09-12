---
path: backend/crates/shared
charted: 2026-09-12
fs:
  - name: Cargo.toml
    role: crate manifest — no intra-workspace deps; only external dep is chrono
    node: false
  - name: src/lib.rs
    role: crate root doc + `pub mod settings`
    node: false
  - name: src/settings.rs
    role: compile-time constants — handle-change limit/window, handle-quarantine window
    node: false
---
**Is:** The bottom leaf crate: build-time configuration constants that every other backend layer, `domain` included, is allowed to consume.

**Conventions:** no intra-workspace dependency may ever be added here (only external crates, pinned at the workspace root — today just `chrono`), so consuming `shared` can never pull an adapter or driver into a lower layer. Holds build-time constants only (windows/limits/ceilings a design decision leaves to implementation); runtime config (env, profiles, ports) is `composition::Config`.

**Entry points:** `src/lib.rs` · `src/settings.rs`.

**Refs:** DD 27852802 — Account Handle Change Flow (src/settings.rs: governs `HANDLE_CHANGE_LIMIT`, `HANDLE_CHANGE_WINDOW`, `HANDLE_QUARANTINE_WINDOW`) · DD 55836674 — The Application Layer (Cargo.toml: source of the "shared = the one leaf crate domain may consume" ruling, dated 2026-08-26) · memory `project_application_layer_convention.md`.
