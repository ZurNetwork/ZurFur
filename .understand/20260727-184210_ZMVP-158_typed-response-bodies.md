# 🔎 Understanding ZMVP-158 — Typed response bodies: replace `json!` literals with named Serialize types

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-158 · **Generated:** 2026-07-27 18:42 MDT · **Snapshot:** `.understand/20260727-184210_ZMVP-158_typed-response-bodies.md`
> **Unit of work:** `7g6vt` (lane: 🤖 Claude build) · **Epic:** ZMVP-25 (API Contract) · **Base:** `main` @ `9fd9afab`

## 🧭 1. Context (cold-start)

`crates/api` is a hand-written axum HTTP layer. Historically every success body was
built with `serde_json::json!` — an ad-hoc `serde_json::Value` with no type behind
it. **Nothing can be checked against a `json!` literal**: not a schema, not a
compiler, not a doc generator. DD `40992770` (The API Contract) made that fatal for
the epic, because the property that makes the Protobuf contract *binding* is
"a handler must return the generated type, so a wrong shape fails `cargo build`."

**This ticket is a remainder, not a fresh start.** Slice [1] (PR #156, merged inside
epic PR #162) restructured the **nine contract endpoints**; ZMVP-160's prost adoption
then replaced even those hand-written structs with **generated** types from
`src/generated/zurfur.api.v1.rs`. What is left is every response body on the
surfaces the `.proto` corpus does **not yet name** — account membership, invitations,
transfer, and the commission tree/file/seat sub-routes.

```
crates/api response bodies
├── /api/v1 corpus (7 routes) ──────► GENERATED types            ✅ done (#156 → ZMVP-160)
│   GET /me · GET,POST /accounts · DELETE /accounts/{id}          lowerCamelCase,
│   PATCH /accounts/{id}/handle · GET,POST /commissions           WireTimestamp,
│                                                                  golden_wire pinned
└── everything else (15 routes) ────► json!(…) / serde_json::Value   ⬅️ THIS TICKET
                                      snake_case, chrono-default instants, unpinned
```

The ticket's own argument is IDL-independent and still stands: an untyped response
body is untestable and undocumentable regardless of any contract.

## 🗺️ 2. Domain

Barely any — and that is the point. This is an **API-surface** ticket, not a domain
ticket. The domain concepts appear only as *what the bodies carry*:

- **Account / Roles** (DESIGN `1966081`, `2162692`) — membership bodies echo
  `{account, user, role}`; `role` is `Owner|Admin|Manager|Member`.
- **Invitation** (DD `24182820`) — invitation bodies carry `{id, account, user, role, state}`
  (accounts) and `{id, commission, seat, user, state}` (seats).
- **Commission tree** (DD `28409880`, `28246028`) — surfaces/components/slots/seats
  bodies return the newly-minted `NodeId`(s).
- **Changelog** (DD `30408741`) — **44% of `json!` in this crate is a changelog
  *payload*, not a response body** (§7). Those are jsonb column values, out of scope
  for AC1 — but see the cross-cutting finding in §7, they *do* reach the wire later.

Live design refs: **DD `40992770`** (the contract), **DD `23592962`** (bare success
bodies, RFC 9457 errors — no `{data,error}` envelope). Style is bound by the living
page **`37519361`**, whose ruling 3 explicitly covers response bodies ("name the
construction into a `let` first" — already the idiom at `invitations.rs:127/168`),
plus ruling 1 (newtypes) and ruling 6 (std-trait-first).

## 🎯 3. Goal & scope

**Goal:** make every remaining HTTP success body in `crates/api` a *named, typed*
value, so the shape is compiler-checked and documentable — **without moving a single
byte on the wire.**

### In scope
- The **18 response-position `json!` sites** across **15 endpoints** (§7 table).
- The **2 `serde_json::Value` values in response position**: `ok_json`
  (`accounts.rs:238`) and `health`'s `Json<serde_json::Value>` return type (`health.rs:34`).
- **Flagging** (doc comment, no shape change) the three unschematized passthroughs.
- **Documenting** the timestamp serialization format on response types carrying one.

### Explicitly OUT
- ❌ **The 14 changelog/event-payload `json!` sites.** They are jsonb column values
  passed to `NewChangelogEntry::event(...)`. AC1 scopes to "a **response position**" —
  these are not one. Retyping them is a *different* ticket (it would touch stored data
  shape). Their handlers mostly return `204`/bare `201` with no body at all.
- ❌ The seven contract routes — already generated types.
- ❌ **Request** bodies, including `Json<Markup>` (`markup.rs:58`) — flag only.
- ❌ Any wire change: no field renames, no camelCase, no added/removed fields, no
  timestamp reformatting. **A test that changes is a regression, not a fixup.**
- ❌ Domain, adapter, or migration changes.

## 📦 4. Deliverables

- [ ] A named `#[derive(Serialize)]` struct for each of the 15 endpoints' bodies, each with a `///` doc comment stating the endpoint and outcome shape.
- [ ] `ok_json` (`accounts.rs:238`) **removed** — its 6 call sites return their own typed body.
- [ ] `health` returns a named type; `Json<serde_json::Value>` gone from its signature.
- [ ] The two "named `let` then respond" invitation sites (`invitations.rs:127/168`, `accounts.rs:1062/1112`) build a struct instead of a `json!`, keeping the existing 200-vs-201 status branching untouched.
- [ ] The two `revoked` closures (`accounts.rs:1174`, `invitations.rs:222`) return a named struct built once.
- [ ] Zero `json!` and zero `serde_json::Value` in any **response** position in `crates/api/src` (the 14 payload sites remain, deliberately).
- [ ] Three `⚠️ contract-decision-needed` doc comments: `AddComponentBody.payload` (`components.rs:41`), `ChangelogEntryBody.payload` (`changelog.rs:26`), `Json<Markup>` request (`markup.rs:58`).
- [ ] Timestamp format recorded in the doc comment of every response type carrying an instant (today: `ChangelogEntryBody.created_at`, `changelog.rs:28`).
- [ ] Full gate green: `cargo fmt`, `clippy`, and the **227** test functions across 36 test files — **all unchanged**.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| **A.** `accounts.rs` — 8 sites / 7 endpoints, + delete `ok_json` | 3 — surface area (1508-line file; DID-carrying bodies; 200/201 branching at 1062+1112) | P0 | 🤖 Claude | **Sonnet** — mechanical, wire pinned by tests; widening risk in §8.4 | ⬜ `ok_json` live at `:238` w/ 6 callers; sites 745/894/969/1062/1112/1174/1260/1354 |
| **B.** `commissions/invitations.rs` — 3 sites / 2 endpoints | 2 — 127+168 share a shape across a status branch | P0 | 🤖 Claude | **Sonnet** | ⬜ `:127`, `:168`, `:222` |
| **C.** Commission "created-id" bodies (`surfaces:84`, `components:101`, `files:130`, `seats:133`, `slots:126`) | 2 — trivial shapes, one newtype-serialization trap | P1 | 🤖 Claude | **Sonnet** — must verify `NodeId`/`FileKey` serialize identically to the current `*deref` | ⬜ all five are `Json(json!({...}))` |
| **D.** `health.rs` — named body + drop `Json<serde_json::Value>` | 1 | P2 | 🤖 Claude | **Haiku** — 46-line file, two fixed variants | ⬜ `:34`, `:38`, `:43` |
| **E.** Flag the 3 unschematized passthroughs (doc comments only) | 1 | P1 | 🤖 Claude | **Haiku** — comments, no logic | ⬜ none flagged today |
| **F.** Timestamp-format documentation (AC5) | 2 — needs the §8.2 scope answer first | P2 | 🤖 Claude | **Sonnet** | 🟡 `WireTimestamp` (`wire_time.rs`) already solves the *contract* half; remainder undocumented |
| **G.** ⚠️ **Shared-vs-per-endpoint body shape** (`{id}` recurs 4×, `{account,user}` 3×) | — | P0 (blocks A + C) | 🧑 **Engineer** | — | ⬜ genuine contract fork — §8.1 |
| **H.** *(optional)* Guard test: no `json!` in a response position | 3 — precedent exists (`tests/contract_routes.rs` is a text-level weld test) | P3 | 🧑 Engineer to approve scope | — | ⬜ not in the ACs; proposed, not assumed |

**Ticket build model = Sonnet** (max over Claude-owned pieces). No piece's *logic* is
security logic — this re-expresses already-public bodies and the wire is pinned — but
**`/security-review` is still required before the PR** (DoD trigger: these bodies carry
DIDs; the failure mode is accidental field widening, §8.4).

## ✅ 6. Test checklist (TDD)

The safety net is **inverted** from a normal ticket: the assertion is that *nothing
changes*. The 227 existing test functions are the spec.

- **Integration (regression net)** — _asserts that_ every existing test in `tests/accounts.rs`, `tests/invitations.rs`, `tests/transfer.rs`, `tests/leave.rs`, `tests/commission_seat_invitations.rs`, `tests/commission_{surfaces,components,slots,seats,files}.rs`, `tests/health.rs` passes **byte-identically without edit** → AC3
- **Unit** — _asserts that_ each new response struct serializes to exactly the JSON its `json!` predecessor produced, including **snake_case keys** (`previous_owner`, not `previousOwner`) → AC1, AC3
- **Unit** — _asserts that_ `NodeId`/`FileKey`/`SeatKind` embedded as struct fields serialize to the same scalar the current `*key` / `.as_str()` deref emits → AC3
- **Integration** — _asserts that_ the idempotent re-invite paths still answer **200** with the stored offer and the minted path **201** (`accounts.rs:1062/1112`, `invitations.rs:127/168`) — the typed body must not collapse the status branch → AC3
- **Integration** — _asserts that_ `GET /health` still returns `{"status":"ok","database":"up"}` at 200 and `{"status":"degraded","database":"down"}` at 503 → AC1, AC3
- **Compile-time / grep** — _asserts that_ `json!` and `serde_json::Value` survive **only** at the 14 payload sites and the 3 flagged passthroughs → AC1, AC2
- **Unit** — _asserts that_ a response type carrying an instant emits the documented format (whatever §8.2 decides) → AC5
- **Doc** — _asserts that_ each of the three passthroughs carries a `⚠️` contract-decision comment → AC4

## 🧠 7. Logic & shape

### The measurement that reframes the ticket

The Jira comment estimated "~26 `json!` sites remain". **Re-measured on `9fd9afab`:
32 total — 18 response bodies, 14 changelog payloads.** The comment conflated the two.

```
32 json! in crates/api/src
│
├── 18  RESPONSE POSITION ─────────────────────────── IN SCOPE (AC1) — 15 endpoints
│   accounts.rs (7 endpoints, 8 sites)
│     745  accept_invitation              {account,user,role}
│     894  grant_role                     {account,user,role}
│     969  revoke_role                    {account,user}
│    1062  invite_user_to_account  (200)  {id,account,user,role,state}  ┐ one endpoint,
│    1112  invite_user_to_account (200/201) {id,account,user,role,state}┘ two sites
│    1174  revoke_invitation_to_account   {account,user}     (closure, all paths)
│    1260  decline_invitation             {account,user}
│    1354  transfer_ownership             {account,owner,previous_owner}
│   commissions/invitations.rs (2 endpoints, 3 sites)
│     127  invite_to_seat          (200)  {id,commission,seat,user,state} ┐ one endpoint,
│     168  invite_to_seat      (200/201)  {id,commission,seat,user,state} ┘ two sites
│     222  revoke_seat_invitation         {commission,seat,user}  (closure, all paths)
│   commission tree (5 endpoints, 5 sites)  — all POST → 201
│      84  surfaces    {id}   ·  101  components {id}  ·  130  files {id}
│     133  seats       {id}   ·  126  slots      {ids}
│   health.rs (1 endpoint, 2 sites)
│      38  200 {status,database}  ·  43  503 {status,database}
│
└── 14  CHANGELOG / EVENT PAYLOAD ───────────────── OUT OF SCOPE (jsonb, not a response)
    sweep 78 · channel 62,107 · files 109 · create 103 · deadline 154,269
    markup 81 · positioning 142,192 · archive 58,96 · status 121 · seats 113
```

**Worked example — `files.rs`, both categories in one handler:**
`:109` builds the `file_added` changelog payload (stored) → stays.
`:130` returns `Json(json!({ "id": *key }))` (the wire) → becomes a named struct.

> **Classification is not guessable from the variable name.** `let existing_offer = json!({…})`
> at `invitations.rs:127` *looks* like a payload but is a response body (`Json(existing_offer)`
> at `:134`) — that's style ruling 3, not a payload idiom. Every site must be read to its
> `Json(...)` / `NewChangelogEntry::event(...)` consumer. Two independent passes this session
> disagreed on exactly these three sites (`invitations.rs:127/168`, `accounts.rs:1112`) before
> reading the consumer settled it.

### ⚠️ Cross-cutting finding: the payloads *do* reach the wire — just not here

Every one of the 14 payload values is later re-serialized to Participants over
`GET /commissions/{id}/changelog`, riding `ChangelogEntryBody.payload: serde_json::Value`
verbatim (`changelog.rs:26,54,60`). So "out of scope" means *out of scope for this
ticket's response-position criterion* — **not** "never public".

```
 json!(payload) ──► jsonb column ──► ChangelogEntryBody.payload (Value) ──► client
   [14 sites]        (changelog)         ⚠️ AC4 flag #2                    (untyped!)
```

This makes AC4's `ChangelogEntryBody.payload` flag the **most load-bearing of the
three**: it is the single hole through which 14 untyped shapes reach clients. It also
means `markup.rs` puts a domain type on the wire in **both** directions — `Markup`
in on the request (`:58`), and back out inside the changelog payload (`:81`).
Flag both facets; fix neither (AC4).

### The newtype trap (piece C)

`json!({ "id": *key })` emits the **deref'd inner** `Uuid`. A typed
`struct AddFileBody { id: FileKey }` emits `FileKey`'s own `Serialize`. serde
serializes a *newtype struct* as its inner value, so these coincide — **but only if
`FileKey` is a newtype struct with a derived `Serialize`.** Verify per type; a custom
`Serialize` or a non-newtype shape silently changes the wire.

### Naming stays snake_case

DD `40992770` R1 mints **lowerCamelCase** at `/api/v1`. These endpoints are **not**
under `/api/v1`, and AC3 forbids a wire change — so the new structs must **not** carry
`#[serde(rename_all = "camelCase")]`. (When these surfaces later enter the corpus, the
rename happens at the mint, which is where a rename is free.)

## 🚀 8. Next steps

1. ⚠️ **BLOCKING — Engineer decision (piece G): one shared body type per shape, or one per endpoint?**
   Three shapes repeat across *different* endpoints: `{id}` at **4** endpoints
   (surfaces/components/files/seats), `{account,user}` at **3** (revoke_role,
   revoke_invitation, decline_invitation), `{account,user,role}` at **2** (accept,
   grant). Options: **(a)** one shared struct per shape — DRY, and echoes DD `40992770`'s
   "Problem declared ONCE" lesson (it was declared 3× and had already drifted on
   `detail`); **(b)** one struct per endpoint — endpoints stay independently evolvable.
   This is a **contract-vocabulary decision with lasting consequences**: once these
   surfaces enter the `.proto` corpus it is additive-only, and merging or splitting a
   message afterwards is a breaking change.
   *(Claude's reasoning if asked: **(b)** per-endpoint — these are only coincidentally
   identical; `revoke_role` and `decline_invitation` have different futures, and a
   shared `{id}` across four unrelated tree endpoints would couple them for no gain.
   The Problem precedent cut the other way because Problem is genuinely one concept.
   But this is the Engineer's call, not Claude's.)*

2. ⚠️ **Engineer scope call (piece F): what does AC5 "note their serialization format explicitly" mean?**
   **(a)** documentation only; or **(b)** adopt `WireTimestamp` (`src/wire_time.rs`) on
   the remainder routes too.
   **Verified empirically this session:** chrono's default `DateTime<Utc>` serde emits
   `"2025-07-25T12:00:00Z"` / `"…T12:00:00.123456Z"` — **Z-normalized, byte-identical to
   `WireTimestamp`** for every in-range instant. So (b) is wire-compatible in practice
   and would additionally range-validate (0001–9999). Cheap hardening — but it is scope
   beyond "note the format", so it is the Engineer's to grant.
   The safety net is **thin here**: `tests/commission_changelog.rs:191` asserts only
   `entry["created_at"].is_string()`, so it would **not** catch a format change.
   (`tests/golden_wire.rs:105` pins the contract half properly.)

3. **Then build, in order:** E + D (trivial, independent) → C (five near-identical
   sites) → B → A (largest; needs decision 1) → F (needs decision 2).

4. **Before the PR:** run `/security-review` — **required**, not optional. The DoD
   trigger is met (these bodies carry DIDs; `accounts.rs` responses are the
   DID/handle-correlation surface). The specific threat this refactor introduces is
   **accidental field widening**: swapping a hand-picked `json!` for a struct that
   derives `Serialize` over a *domain* type would silently publish fields the literal
   never emitted. `tests/cross_persona_unlinkability.rs` is a partial net only.

5. **Ticket hygiene:** ZMVP-158's description still cites the 2026-07-25 measurements
   (36 `json!`, `created_json`, `commission_json`). **Two of AC2's three targets are
   already gone** — `created_json` no longer exists (there never was a `created_json`
   helper on `main`; `201`s are built inline), and `commission_json` is now
   `wire_commission` (`list.rs:30`) returning the generated `Commission`. Offer the
   Engineer a description refresh so the ACs match reality before the PR closes it.

6. **Dangling / not blocking:** piece H (the anti-regression guard test) is *not* in
   the ACs. `tests/contract_routes.rs` is a working precedent for a text-level
   structural weld. Propose it; don't build it unasked.
