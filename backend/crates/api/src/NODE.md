---
path: backend/crates/api/src
charted: 2026-08-21
fs:
  - name: lib.rs
    role: Config (figment), Environment, AppState bag of port trait objects, app() composition + no-store header layer
    node: false
  - name: main.rs
    role: boot: .env, config, tracing, pool, migrations, session layer, live pg/atproto adapters, serve
    node: false
  - name: routes/
    role: per-domain route groups, each a *_router() builder
    node: true
  - name: generated/
    role: @generated prost structs + pbjson canonical-ProtoJSON serde; drift fails contract_current
    node: false
  - name: problem.rs
    role: the single RFC 9457 error type and its code→URN mapping
    node: false
  - name: wire_time.rs
    role: canonical ProtoJSON Timestamp (Z-normalized, 0/3/6/9 digits) both directions
    node: false
  - name: sweep.rs
    role: deadline sweeper background task; single-writer via pg_try_advisory_xact_lock
    node: false
---
**Is:** Composition (lib/main) plus the cross-cutting HTTP concerns (problem, wire_time, sweep) around the route groups and the generated contract module.

**Conventions:** handlers translate HTTP ↔ ports only; no domain logic here. The sweeper is the only place the system (not a participant) acts on a commission.

**Refs:** ZMVP-86 (sweeper) · ZMVP-151 (origin split).
