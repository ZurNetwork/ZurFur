---
path: backend/crates/contract-gen
charted: 2026-09-12
fs:
  - name: Cargo.toml
    role: crate manifest (protox/prost-build/pbjson-build deps)
    node: false
  - name: src/
    role: generator library (lib.rs) + `just gen-contract` binary (main.rs)
    node: false
---
**Is:** the code generator that turns the repo-root protobuf contract corpus into the `api` crate's committed `src/generated/` Rust module (prost structs + canonical-ProtoJSON serde).

**Conventions:** the two callers of `contract_gen::generate` are the `main.rs` binary and the `api` crate's `contract_current` test, which regenerates into a temp dir and diffs against the committed output. `NoServices` guarantees `service` blocks in the `.proto` files never emit gRPC stubs — routes stay declarations. `google.api.rs` is generated then deleted (route-metadata only, not a runtime message).

**Entry points:** `src/lib.rs::generate` (the generation body); `src/main.rs` (the CLI entry, resolves `../../../contract` and `../api/src/generated` relative to `CARGO_MANIFEST_DIR`).

**Refs:**
- DD 40992770 — The API Contract (Protobuf as the Independent IDL) (src/lib.rs, src/main.rs: governs why generation is pure-Rust/protox, the committed-output + drift-test pattern, and why `service` blocks generate nothing)
- `contract/VERSIONING.md` §7.3/§7.7 — Timestamp wire-format rules (src/lib.rs: why `.google.protobuf.Timestamp` is extern-path'd to `crate::wire_time::WireTimestamp` in the `api` crate instead of using pbjson_types' own Serialize)
