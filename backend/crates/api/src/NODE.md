---
path: backend/crates/api/src
charted: 2026-09-06
fs:
  - name: lib.rs
    role: app() composition + no-store header layer; ports/Config now shared via the composition crate
    node: false
  - name: main.rs
    role: boot: .env, config, tracing, pool, migrations, session layer, live pg/atproto adapters, serve
    node: false
  - name: extract.rs
    role: CallingUser extractor — session → Did → UserId, 401 on absence
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
    role: canonical ProtoJSON Timestamp (Z-normalized, 0/3/6/9 digits) both directions; fallible TryFrom<WireTimestamp> for DateTimeUtc (OutOfWireRange) for use-case Commands
    node: false
  - name: sweep.rs
    role: deadline sweeper background task; single-writer via pg_try_advisory_xact_lock
    node: false
---
**Is:** Composition (lib/main) plus the cross-cutting HTTP concerns (extractors, problem, wire_time, sweep) around the route groups and the generated contract module.

**Conventions:** handlers translate HTTP ↔ ports only; no domain logic here. The sweeper is the only place the system (not a participant) acts on a commission.

**Refs:** ZMVP-86 (sweeper) · ZMVP-151 (origin split) · DD 55836674 (application layer).
