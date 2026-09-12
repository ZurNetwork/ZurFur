---
path: backend/crates/composition
charted: 2026-09-06
fs:
  - name: Cargo.toml
    role: deps — domain, application, adapter-pg, adapter-atproto, figment, serde, anyhow, tracing, base64, fluent-uri
    node: false
  - name: src/lib.rs
    role: crate doc + re-exports of Config and Runtime; runtime is pub(crate), ports is pub
    node: false
  - name: src/config.rs
    role: Config struct, figment loader (Config::load/load_from), Environment enum, boot-time custody guard ensure_custody_hardened
    node: false
  - name: src/ports.rs
    role: From<&Runtime> impls building application's per-module Ports bags (AccountPorts, CommissionPorts)
    node: false
  - name: src/runtime.rs
    role: Runtime bag of Arc<dyn Port> (AccountStore, Authenticator, ChangelogStore, CommissionStore, Database, DidMinter, FileStore, ProfileCache, ProfileSource, UserStore), Runtime::connect wiring pg+atproto adapters, transaction convenience
    node: false
  - name: tests/no_http_deps.rs
    role: cargo-tree guard — composition and cli must never link an HTTP-server crate (axum, axum-core, tower-sessions)
    node: false
---
**Is:** The HTTP-free composition root shared by every driving adapter (api, cli): loads Config and wires the live Runtime bag of domain ports.

**Conventions:** HTTP-free by construction, enforced by `tests/no_http_deps.rs` — never add axum/tower-sessions here or to `cli`. This is the one crate that knows which adapters are live; `api` and `cli` only choose how to drive `Runtime`. Migrations, background tasks, sessions, cookies are explicitly NOT this crate's concern — left to the driver.

The Runtime→`application::*Ports` conversion lives once in `src/ports.rs` (one `From<&Runtime>` per application module); drivers never hand-assemble a Ports struct.

**Entry points:** `Config::load` · `Runtime::connect` · `Runtime::transaction`.

**Refs:** ZMVP-200 (crate extracted from api::AppState) · ZMVP-3 (original composition root) · DESIGN "Domains and Applications" (11763713) · memory `config-and-runtime` · CLAUDE.md "Configuration & database".
