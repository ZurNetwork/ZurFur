---
path: contract
charted: 2026-08-21
fs:
  - name: README.md
    role: orientation: authority claim, layout, the rules in one breath
    node: false
  - name: VERSIONING.md
    role: the binding versioning/deprecation contract (ZMVP-28): rulings R1–R11, breaking-change list, 12-month notice
    node: false
  - name: buf.yaml
    role: v2 module config — lint STANDARD, breaking gate WIRE_JSON
    node: false
  - name: buf.gen.yaml
    role: frontend codegen (protoc-gen-es, target=ts) into frontend/web/src/lib/server/api/generated
    node: false
  - name: lifecycle.toml
    role: per-major lifecycle manifest ([major.v1] live|deprecated|sunset); CI routes follow it
    node: false
  - name: google/
    role: vendored googleapis annotations (api/annotations.proto, api/http.proto) — verbatim upstream, never edited
    node: false
  - name: zurfur/
    role: the Zurfur corpus; pass-through to api/v1
    node: true
---
**Is:** The API contract — protobuf as the independent IDL, authoritative over BOTH tiers (axum via prost+pbjson, SvelteKit via protobuf-es), owned by neither and generated into both.

**Conventions:** JSON over HTTP is the v1 transport — nothing here implies gRPC; transports are adapters. `service`/`rpc` blocks exist only to host `google.api.http` annotations; ONLY MESSAGES are generated, handlers stay hand-written but return the generated types. Path-major is welded to the package: `zurfur.api.v1` ⇒ `/api/v1/…`. Additive-only within a major; `buf breaking` at WIRE_JSON, the skip label is banned (R9). R1 canonical lowerCamelCase keys (`json_name` rejected). R4 absence = "not set" only. R6 ids opaque (≤64-char strings). R7 listings wrapped (`{"accounts": […]}`). R8 vocabularies are documented extensible `string`s, never enums. Deletions reserve number AND name. Serializer options are contract text pinned by `golden_wire.rs`.

**Entry points:** `README.md` → `VERSIONING.md` · `just gen-contract` · CI job `contract` (lint + breaking + clean-slate generate diff) · `backend/crates/api/tests/contract_routes.rs`.

**Refs:** DD "The API Contract — Protobuf as the Independent IDL" (40992770) · DD 23592962 · "Plugin — API Stability & Versioning" (41189413).
