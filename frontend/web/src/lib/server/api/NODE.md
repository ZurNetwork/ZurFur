---
path: frontend/web/src/lib/server/api
charted: 2026-08-21
fs:
  - name: generated/
    role: protobuf-es output from contract/ (messages only, no clients); buf generate, prettier-ignored, CI drift-gated — never hand-edit
    node: false
  - name: zurfur-api.ts
    role: the port: every backend call as one service, decodeContract (tolerant reader), RequestFetch, ZurfurApiLive + zurfurApiTest
    node: false
  - name: errors.ts
    role: the tagged failure union (NotAuthenticated, ApiProblem, ContractViolation, NetworkFailure, …)
    node: false
  - name: *.spec.ts
    role: port specs + two infrastructure pins (tolerant-reader decode option; undici redirect:'manual' against a live local server)
    node: false
---

**Is:** The backend boundary: one `ZurfurApi` service (live HTTP Layer + in-memory Layer), a tagged error union, and the protobuf-es messages generated from `contract/`.

**Conventions:** decode only through `decodeContract` / the generated schemas — hand-rolled `fromJson` and hand-written narrowing are banned (ZMVP-162). Generated types stop at this directory. Failures are values in the tagged union; the RFC 9457 Problem rides inside the tags that carry one.

**Refs:** `contract/VERSIONING.md` §6 (tolerant reader).
