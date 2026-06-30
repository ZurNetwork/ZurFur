# 🔎 Understanding ZMVP-35 — Adopt RFC 9457 problem+json as the API error model

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-35 · **Generated:** 2026-06-27 01:45 · **Snapshot:** `.understand/20260627-014501_ZMVP-35_problem-json-errors.md`

## 🧭 1. Context (cold-start)

The JSON API answers failures with an ad-hoc `{ "error": "<string>" }` body — a bare human string with no machine-readable identity, and a *different* top-level shape from success (which returns the bare resource). A recorded design decision ([API Response Shape & Error Model (RFC 9457)](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/23592962), DECIDED 2026-06-26) settled the fix: **keep success bare, standardize errors on RFC 9457** `application/problem+json`. This ticket implements the **error half** of that decision. Success bodies do not change.

RFC 9457 (IETF Standards Track; obsoletes 7807) defines a problem document with members `type` (a URI *identifier*), `title`, `status`, `detail`, `instance`, plus arbitrary extensions. Our decision pins two specifics: `type` is a **non-dereferenceable URN** (`urn:zurfur:error:<slug>` — stable identity, nothing to host), and we add a `code` **extension** (our own terse vocabulary, e.g. `already_member`) for machine branching. Served with `Content-Type: application/problem+json`.

```
  today                                   after ZMVP-35
  401 application/json                    401 application/problem+json
  { "error": "You must be signed in…" }   { "type":"urn:zurfur:error:not-authenticated",
                                            "code":"not_authenticated",
                                            "title":"…", "detail":"…", "status":401 }
```

## 🗺️ 2. Domain

Not a glossary-entity ticket — it's an HTTP-surface contract decision. The relevant design pages:
- **[API Response Shape & Error Model (RFC 9457)](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/23592962)** — the DD this implements; holds the *why*, the worked example, and the starter code registry.
- **[Domains and Applications](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/11763713)** — establishes the two audiences this contract serves: the **internal namespaces** (shipped with their frontends) and the versioned public **`/plugin/v1`** (third-party). The same problem+json shape serves both — *we are our own client*, so dogfooding the public-grade format buys debuggability everywhere.

The error layer lives entirely in the `api` crate (the composition root / HTTP surface). Adapter/domain code returns `anyhow::Result`/typed errors; handlers translate them to HTTP — so the migration is confined to one crate.

## 🎯 3. Goal & scope

**Goal:** every **JSON-API** error response is a valid RFC 9457 `application/problem+json` document carrying `type` (URN), `code`, `title`, `detail`, `status` — replacing the `{ "error": string }` shape — with a small, central registry of error types. Success bodies untouched.

**In scope**
- A `Problem` type (serializes to problem+json) + `IntoResponse` that sets `Content-Type: application/problem+json`.
- The **starter code registry** (the 8 rows from the DD) as `urn:zurfur:error:*` + `code` constants.
- Migrating the **7 centralized helpers** (`unauthorized`, `forbidden`, `not_found`, `member_not_found`, `unprocessable`, `conflict`, `internal_error`) **and the 2 hand-rolled JSON error sites** in `create_account` (the 503 DID-mint and 500 create paths) to emit `Problem`.
- The ZMVP-32 invitation endpoints inherit the change automatically (they use the same helpers).
- Tests asserting the problem+json shape (`type`/`code`/`title`/`status`) across accounts, members, invitations.

**Out of scope**
- **Success bodies** — stay bare resources; no `{ data, meta }` envelope (YAGNI).
- **The browser-facing sign-in/HTML flow** (`/`, `/signin`, `/signin-callback`, `/me`, `/logout`) — its errors render an HTML `sign_in_page(...)`, not JSON. Not an API contract; left alone.
- **The TypeScript `Result<T, Problem>` client wrapper** — deferred until the frontend consumes the JSON API (DD follow-up).
- A shared error crate for `/plugin/v1` — that namespace doesn't exist yet; keep the type in `api` for now.

## 📦 4. Deliverables

- [ ] A `Problem` struct (serde `Serialize`: `type`, `code`, `title`, `detail`, `status`) + `impl IntoResponse` that sets `Content-Type: application/problem+json` — new module `api/src/problem.rs` (or `error.rs`)
- [ ] The `urn:zurfur:error:*` + `code` registry (8 starter entries) as constants/an enum
- [ ] The 7 helpers rewritten to build a `Problem` (keeping their call signatures, so ~30 call-sites are untouched)
- [ ] The 2 inline `create_account` error sites routed through `internal_error` / a `service_unavailable` helper
- [ ] Tests asserting the problem+json body shape + `Content-Type` for representative statuses (401/403/404/409/422/500/503)
- [ ] No bare `{ "error": string }` remains in the JSON API

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| `Problem` type + `IntoResponse` + content-type | 3 — the content-type override gotcha | P0 | 🤖 Claude | ⬜ none exists; current errors are `json!({"error":…})` (`lib.rs:589-644`,`1106`) |
| `urn:zurfur:error:*` + `code` registry (8 entries) | 2 | P0 | 🤖 Claude | ⬜ no registry; mapping defined in the DD |
| Migrate the 7 helpers | 2 — mechanical, signatures unchanged | P0 | 🤖 Claude | ⬜ helpers emit `{error:string}` (`lib.rs:589,599,611,621,631,643,1106`) |
| Route the 2 hand-rolled `create_account` sites | 1 | P1 | 🤖 Claude | ⬜ inline `json!({"error":…})` (`lib.rs:553-559`,`567-573`) |
| Tests: problem+json shape + content-type | 3 | P0 | 🤖 Claude | ⬜ e2e assert status only — **no** `body["error"]` assertions today (low churn) |
| Decide mechanism (local type vs `problem_details` crate; helper-return vs `Result` path) | 1 — small fork | P0 | 🧑 Engineer | ⬜ see §8 |

*A tight, single-crate refactor with low blast radius (success unchanged; no client consumes the JSON API yet), so it's Claude-ownable end-to-end once the §8 mechanism call is made.*

## ✅ 6. Test checklist (TDD)

- **Unit** — _asserts that_ a `Problem` serializes to exactly `{ type, code, title, detail, status }` with the URN/`code` for its kind → AC1/AC3
- **Unit** — _asserts that_ `Problem`'s `IntoResponse` sets `Content-Type: application/problem+json` (not `application/json`) and the matching HTTP status → AC4
- **E2E** — _asserts that_ an unauthenticated write returns 401 with `type = urn:zurfur:error:not-authenticated`, `code = not_authenticated`, content-type problem+json → AC1/AC2
- **E2E** — _asserts that_ inviting an existing member returns 409 `code = already_member` problem+json (the ZMVP-32 endpoint inherits the shape) → AC2/AC6
- **E2E** — _asserts that_ a non-member actor gets 403 `forbidden`, a missing account 404 `account_not_found`, a blank account name 422 `invalid_request`/`name_required` → AC2/AC3
- **E2E** — _asserts that_ no JSON-API error response carries the old `{ "error": string }` shape → AC2
- **(regression)** — _asserts that_ success responses are byte-for-byte unchanged (bare resources) → AC5

## 🧠 7. Logic & shape

The `Problem` type and the content-type gotcha (axum's `Json` sets `application/json`, so the header must be overridden *after*):

```rust
#[derive(Serialize)]
struct Problem {
    r#type: &'static str,   // "urn:zurfur:error:already-member"
    code:   &'static str,   // "already_member"
    title:  &'static str,   // stable, per-code human summary
    detail: String,         // specific (may carry the caller's `reason`)
    status: u16,
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap();
        let mut res = (status, Json(&self)).into_response();
        // Json set application/json; override it — RFC 9457 requires problem+json.
        res.headers_mut().insert(
            CONTENT_TYPE, HeaderValue::from_static("application/problem+json"));
        res
    }
}
```

Helpers keep their signatures, so the ~30 existing call-sites don't move — only their *bodies* change:

```
forbidden()            -> Problem { type: …forbidden,        code: "forbidden",         title: "…", detail: "…", status: 403 }
unprocessable(reason)  -> Problem { type: …invalid-request,  code: "invalid_request",   title: "…", detail: reason, status: 422 }
conflict(reason)       -> Problem { type: …already-member?,  code: …,                   ... 409 }   // see §8: 422/409 sub-codes
```

Registry (from the DD): `not_authenticated` 401 · `forbidden` 403 · `account_not_found` / `member_not_found` 404 · `already_member` 409 · `invalid_request` (+`name_required`/`unknown_role`) 422 · `internal_error` 500 · `service_unavailable` 503.

```
JSON API errors ──► one of 7 helpers / 2 inline sites ──► Problem ──IntoResponse──► problem+json
HTML sign-in errors ──► sign_in_page(…) ──► HTML            (untouched, out of scope)
GET /health 503 ──► { status, database }                    (decision — see §8)
```

## 🚀 8. Next steps

1. **Settle the mechanism (small fork, do first):**
   - ⚠️ **Type source:** a **thin local `Problem`** type *(recommended — full control over the URN + `code` extension, no new dep, matches the repo's minimalism)* vs. the `problem_details` crate *(standard but generic; extension members for `code` need wiring)*.
   - ⚠️ **Call shape:** keep helpers returning a response and let handlers early-return as today *(recommended — smallest diff, the helpers already centralize everything)* vs. refactor handlers to `Result<T, Problem>` + `?` (the ticket's hinted "Result error path" — cleaner, but rewrites every handler body for no behavioural gain now).
2. Build `Problem` + `IntoResponse` + the registry (red tests first: serialize shape + content-type).
3. Migrate the 7 helpers, then the 2 `create_account` inline sites; run the e2e matrix.

**Decisions needed:**
- ⚠️ **`GET /health` 503** currently returns `{ "status": "degraded", "database": "down" }` — a liveness-probe contract, not an API error. **Recommend leaving it as-is** (monitoring reads that shape; it isn't a problem+json error), but confirm.
- **422 sub-codes:** the DD lists `invalid_request` with specifics `name_required` / `unknown_role`. Decide whether `unprocessable(reason)` picks a sub-`code` by call-site (more precise) or all 422s share `invalid_request` with the specifics only in `detail` (simpler). Lean: start with `invalid_request` + descriptive `detail`, add sub-codes where a caller benefits.
- **`title` vs `detail` split:** `title` is the stable, per-`code` summary; `detail` carries the specific/caller string (e.g. the `reason` args, the fixed forbidden message). Confirm wording per code.
- **`instance` member** (optional per-occurrence URI): **omit** for now (YAGNI) unless you want per-request correlation.
