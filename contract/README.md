# The Zurfur API contract

**This directory is the authority over both tiers** (DD [40992770](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/40992770)): the axum backend and the SvelteKit frontend are *generated from* these files, and neither may redefine what they declare. It is ports-and-adapters applied at the project level — the contract is a peer of both trees, owned by neither, hosted here.

**JSON over HTTP is the v1 transport. Nothing here implies gRPC.** The `service`/`rpc` blocks exist so `google.api.http` annotations have a home — they declare each endpoint's path and verb, checked against axum's real route table by `crates/api/tests/contract_routes.rs`. **Only messages are generated**; handlers stay hand-written but must return the generated types. Transports are adapters, plural by design: a second transport would consume these same files, unchanged.

## Layout

| Path | What |
| --- | --- |
| `zurfur/api/v1/*.proto` | The v1 corpus — `package zurfur.api.v1`, welded to the path-major `/api/v1` |
| `google/api/*.proto` | Vendored googleapis annotations (no registry dependency in CI) |
| `VERSIONING.md` | The versioning & deprecation contract (ZMVP-28) — how this corpus may evolve |
| `lifecycle.toml` | The per-major lifecycle manifest (`live`/`deprecated`/`sunset`) |
| `buf.yaml` | Lint (STANDARD) + breaking (`WIRE_JSON`, explicit and load-bearing) |

## The rules in one breath

Additive-only within a major; a breaking change mints a new major (new package, new path, one move — `FILE_SAME_PACKAGE` welds them). Field names are the wire identity: keys emit lowerCamelCase (R1), absence only ever means "not set" (R4), listings are wrapped objects (R7), vocabularies are documented strings the domain enforces (R8), ids are opaque (R6). Every deletion reserves both the field number **and name** — the `.proto` file is the retired-name register. The serializer option set is contract text, pinned by `crates/api/tests/golden_wire.rs`, because `buf breaking` diffs schemas and can never see an emitted-bytes change.

Full detail, with citations: [`VERSIONING.md`](./VERSIONING.md).
