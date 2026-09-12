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

**Refs:** ZMVP-200 (crate extracted from api::AppState) · ZMVP-3 (original composition root) · DESIGN "Domains and Applications" (11763713) · DD 24150017 (unit of work) · DD 26607618 (handle resolution) · memory `config-and-runtime` · CLAUDE.md "Configuration & database".

## Notes
- **Live adapter per port** (`Runtime::wire`): `auth`/`profile_source`/`did_minter` → adapter-atproto (`AtprotoAuthenticator`, `AtprotoProfileSource`, `RealDidMinter` over `PgKeyStore` + `PgPlcOperationLog` + the directory); every other port → adapter-pg (`PgUserStore`, `PgAccountStore`, `PgCommissionStore`, `PgChangelogStore`, `PgWorkflowStore`, `PgColumnStore`, `PgFileStore`, `PgProfileCache` at a one-hour TTL, `PgDatabase`). Tests swap in the mem fakes. Adding a capability = a field on `Runtime` plus a line in `wire`.
- **Reads are pool-side, writes go through `database.begin()`** — that is the whole rule (DD 24150017). Two documented exceptions are pool-backed on purpose: the `profile_cache` fill (a read-path write with no transactional invariant) and `files` (blob bytes cannot ride a transaction).
- `Runtime::connect` connects the pool but deliberately does NOT run migrations — the driver calls `adapter_pg::migrate` explicitly.
- Config layering, lowest first: `config/{profile}.toml` → the unprefixed `DATABASE_URL` (the name sqlx tooling expects) → `ZURFUR_*` env. `ZURFUR_ENV` both picks the profile file and deserializes into `Config::env`, so the two spellings must agree; it is validated as a bare name so it cannot walk out of the config dir. The config dir is anchored to `CARGO_MANIFEST_DIR`, never the CWD.
- `handle_domain` is parsed into a `HandleDomain` ONCE at load, so the claim checks and the `/.well-known/atproto-did` resolver cannot disagree about the namespace (DD 26607618).
- `ensure_custody_hardened` is a boot-time refusal, not documentation: `PROD`/`STG` are refused outright while custody is config/env-root-backed, and PLC submission is refused under the shipped `EXAMPLE_DEV_ROOT_KEY` in any environment.
- `Config::DEFAULT_MAX_UPLOAD_BYTES` (50 MiB) is the one home for the upload cap. Raising it past `i32::MAX` bytes is a wire break — `byte_size` must then become an int64 decimal string (`contract/VERSIONING.md` §7.2).
