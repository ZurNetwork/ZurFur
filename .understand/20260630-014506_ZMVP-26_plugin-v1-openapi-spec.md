# 🔎 Understanding ZMVP-26 — Author the OpenAPI spec for the Plugin REST surface (/plugin/v1)

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-26 · **Generated:** 2026-06-30 01:45 (refresh) · **Snapshot:** `.understand/20260630-014506_ZMVP-26_plugin-v1-openapi-spec.md`
> **Parent epic:** ZMVP-25 "API Contract (OpenAPI / AsyncAPI)" · **Priority:** Medium · **Prior:** `.understand/20260628-232209_ZMVP-26_plugin-v1-openapi-spec.md`

## 📊 Since last snapshot
- **Status:** To Do → **In Progress** (this unit of work, uow `b722f9`, parallel with ZMVP-38).
- **Prior §8 "DEFER — triple-blocked" is SUPERSEDED.** The three blockers (Commission schema pin, Plugin-security DD, REST-vs-XRPC fork) were stale; the **Engineer has settled the remaining forks** for an MVP-slice spec. Authoring proceeds under those decisions (below), with Commission/changelog schemas and the scope vocabulary explicitly **stubbed/provisional**.
- **Code reality re-checked, consistent with the plan:** no `openapi/` dir (this is the first such file), no `/plugin/v1` routes, no `app_key` in code, **no OpenAPI tooling on this branch** (branched fresh off `origin/main`; `feature/openapi-infra` deliberately NOT a dependency). `problem.rs` intact (ZMVP-35).
- **Net movement:** unblocked for the Claude-owned spec slice; Commission field-list + scope vocab remain owned upstream (ZMVP-19 / scope-vocab ticket) and are referenced as stubs, not authored here.

## 🧭 1. Context
Spec-only artifact: a standalone, hand-authored OpenAPI 3.x document for the **plugin → core** REST surface `/plugin/v1`. No Rust route handlers. Pairs with AsyncAPI (ZMVP-27, events core→plugin) and runs under the versioning/deprecation contract (ZMVP-28).

## 🗺️ 2. Domain (pinned facts the spec encodes)
- **Auth:** HTTP bearer — a short-lived, scoped, delegation-tagged token **exchanged from the per-(plugin, account) `app_key`** install binding. Never a long-lived browser secret; plugins never see the session cookie or the PDS credential. `/plugin/v1` is **CSRF-exempt by construction** and CORS-open (no ambient cookie). (DD-24543244 / ZMVP-23.)
- **Production model:** plugins gate **production** (write to the core-sovereign **changelog** in a core-renderable format); core gates reading/visibility. The changelog write is an **invoked** action — principal = the clicking user.
- **Commission** (DESIGN 3276807) is **Private** (Index/export JSON Schema, not an atproto lexicon; Lexicon §B). Field list not pinned → **stub** pending ZMVP-19.
- **Scopes:** `resource:action` families (`read:commission`, `commission:write`, `post:publish`, …); **vocabulary is PROVISIONAL** pending the DD-24543244 scope-vocab ticket.
- **Errors:** RFC 9457 `application/problem+json`, fields `type` (`urn:zurfur:error:*` URN), `code`, `title`, `detail`, `status`. Registry lives in `backend/crates/api/src/problem.rs`.

## 🎯 3. Goal & scope
Author `openapi/plugin-v1.yaml`: `info` + `servers`, the bearer security scheme (with CSRF/CORS notes + version contract), a reusable RFC 9457 `Problem` error component aligned to `problem.rs`, the **two MVP operations** (GET read commission, POST write changelog artifact) with per-op scope annotations, and **stub** `$ref` component schemas for Commission + changelog entry. Validate it parses as OpenAPI 3.x and lints clean.

**Out of scope:** UI surfaces, reactive/Golem-principal actions (post-MVP); events/webhooks (ZMVP-27); the full Commission field list (ZMVP-19); any Rust endpoint; the versioning *rules* themselves (ZMVP-28, only referenced).

## 📦 4. Deliverables
- [ ] `openapi/plugin-v1.yaml` — valid OpenAPI 3.x.
- [ ] Bearer (`app_key`-exchanged token) security scheme, applied globally; description covers CSRF-exempt + CORS-open + version contract.
- [ ] `Problem` reusable component (RFC 9457) + named error responses drawn from the `problem.rs` registry.
- [ ] `GET` read-commission + `POST` write-changelog operations, each with a scope annotation (`x-required-scopes`) and provisional-scope note.
- [ ] Stub Commission + ChangelogEntry component schemas, clearly marked pending ZMVP-19.
- [ ] Validation: `npx @redocly/cli lint` clean (report exact result).

## 🧩 5. Work breakdown
| Piece | Difficulty | Owner | Done |
|---|---|---|---|
| OpenAPI scaffolding (info/servers/security/error component) | 2 | 🤖 Claude | — |
| Two MVP operations + scope annotations | 3 | 🤖 Claude (per settled ops) | — |
| Stub Commission/changelog schemas | 2 | 🤖 Claude (stub only; ZMVP-19 fills) | — |
| Validate + optional CI wiring | 2 | 🤖 Claude | — |

## ✅ 6. Test checklist
- Document parses as valid OpenAPI 3.x and lints clean (`redocly lint`). 
- Every operation declares the bearer scheme + its required scope(s).
- Error responses are `application/problem+json` referencing the `Problem` component; codes/URNs drawn from `problem.rs`.
- Only the two MVP-slice operations appear (no UI/reactive endpoints, no events).
- Commission/changelog schemas are present but clearly stubbed (ZMVP-19).

## 🧠 7. Shape
`openapi/plugin-v1.yaml` → components: `securitySchemes.pluginBearer`, `schemas.{Commission(stub), ChangelogEntry(stub), Problem}`, `responses.{Unauthorized, Forbidden, UnprocessableEntity, InternalError, ServiceUnavailable}`; paths: `GET /commissions/{commissionId}`, `POST /commissions/{commissionId}/changelog`.

## 🚀 8. Next steps
Author the spec per settled decisions → validate with redocly → STOP at "built" and report to orchestrator (no /critique, /document, /prepare-pr; cross-set /close-gaps --post is the orchestrator's).
