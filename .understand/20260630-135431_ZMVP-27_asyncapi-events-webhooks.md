# 🔎 Understanding ZMVP-27 — Author the AsyncAPI spec for core→plugin events & webhooks

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-27 · **Generated:** 2026-06-30 13:54 · **Snapshot:** `.understand/20260630-135431_ZMVP-27_asyncapi-events-webhooks.md`
> **Parent epic:** ZMVP-25 "API Contract (OpenAPI / AsyncAPI)" · **Priority:** Medium · **Sibling (landed):** ZMVP-26 OpenAPI `/plugin/v1` (PR #74, merged `38552a6`)

## 🧭 1. Context (cold-start)
Spec-only artifact: a standalone, hand-authored **AsyncAPI v3** document describing the **core → plugin** direction — the events Zurfur core fans out to **Reactor** plugins over **outbound webhooks**. It is the mirror of the landed `openapi/plugin-v1.yaml` (which is plugin → core, REST). No Rust delivery code sits behind it; this is a contract document, spec-first.

The pairing across epic ZMVP-25:
```
  plugin → core  (REST, request/response)   =  openapi/plugin-v1.yaml      [ZMVP-26 ✅ landed]
  core → plugin  (events, webhook push)     =  asyncapi  (this ticket)     [ZMVP-27 ← here]
  the versioning/deprecation contract both run under                        [ZMVP-28  To Do]
  the webhook delivery/signing/SSRF *implementation*                        [ZMVP-41  To Do]
```
The transport is **signed outbound webhooks** (`standard-webhooks`), and DD-24543244 decision 5 explicitly says the contract is to be **"described in AsyncAPI v3"** — so the format and direction are pinned by design, not chosen here.

## 🗺️ 2. Domain (the pinned facts the spec must encode)
- **Direction / form:** core → plugin, the **Reactor** form (Plugin page: "reacts to events it's subscribed to … Zurfur pushes events to it"). MVP delivery phase = Reactor (notify, no writes) + invoked actions. Reactive/Golem-principal is post-MVP and out.
- **Transport & envelope — SETTLED** (DD-24543244 dec.5; ZMVP-41 description): `standard-webhooks` — HMAC-SHA256 over the **raw serialized bytes**, per-`(plugin, account)` secret; **`{id}.{timestamp}.{body}`** signed; timestamp+id give replay-defense + idempotency; at-least-once with backoff+jitter + DLQ + N-active-secret rotation; SSRF resolve-validate-pin-dial. These delivery/signing *mechanics* are **owned by ZMVP-41** and are *referenced* here, not defined (exactly as ZMVP-26 referenced ZMVP-28's versioning rules).
- **Event scopes:** event subscription is permission-scoped — a plugin only receives events for the commissions/workflows its `app_key` was granted (Plugin "Capabilities & scopes" → **Event scopes**; Workflow "Events"). Scope family `events:*` — vocabulary still **provisional** (same DD-24543244 scope-vocab follow-up that left ZMVP-26's action scopes provisional).
- **The event catalogue (drafted, see §8 — NOT formally pinned):** the **Workflow** page (DESIGN 9895957) enumerates two subjects:
  - **Commission events** (subject = commission, global): `lifecycle.changed`, `status.changed`, `file.uploaded`, `markup.added`, `paid`, `completed`.
  - **Workflow events** (subject = workflow, board-local): `card.moved`, `card.reordered`, `card.added`/`removed`, `card.archived`/`restored`, `list.created`/`renamed`.
  - The **Telegram bot** (First-party plugins 3244063) names the consumer's MVP subscription slice: Status changes (`Changes Requested`, `Waiting for Approval`, `Waiting for Input`, `Late`), Lifecycle (`Active`, `Completed`, `Cancelled`, `Disputed`), new File entry, new Markup, dispute raised/mediated/resolved.
- **Payloads = STUB.** Every event references the **Commission** family (Status/Lifecycle enums, file entry, markup), which is **Private** (Index/export JSON Schema, not a lexicon) and **not yet pinned** — gated on ZMVP-19 / Lexicon field-lists, the same gate ZMVP-26 stubbed around (Blocking Gaps for v1: "atproto Lexicon / record schemas").
- **Trust boundary** (DD-24543244): plugins are core-only, never atproto clients; receive **scoped principal identifiers**, never the global DID; never the cookie or PDS credential. Webhooks SSL-verified + signed.

## 🎯 3. Goal & scope
Author one AsyncAPI v3 document for the **core → plugin** webhook event surface: `info` + the webhook server/channel binding, the **common message envelope** (id, type, timestamp, subject, delegation tag) wired to the `standard-webhooks` signature headers, the **v1 event channels/messages** (the commission + workflow event catalogue, scoped to the MVP slice), per-message **event-scope** annotations, and **stub** payload schemas (`$ref`) for the Commission-derived bodies — marked pending ZMVP-19. Validate it parses/lints clean and wire it into CI's `spec-lint`.

**In scope:** the AsyncAPI document; the message envelope + signature-header reference; the event channel catalogue (MVP slice); event-scope annotations; stub payload schemas; CI lint wiring for AsyncAPI.

**Out of scope:** the webhook *delivery/signing/SSRF implementation* (ZMVP-41 — referenced only); the versioning *rules* (ZMVP-28 — referenced only); the full Commission/Status/Lifecycle field lists (ZMVP-19 — stubbed); reactive/Golem-principal + UI-surface events (post-MVP); any Rust.

> ⚠️ **Scope is gated on one open Engineer decision — see §8.** Unlike ZMVP-26, whose MVP operation surface the Engineer had *already* settled before authoring, ZMVP-27's **event catalogue is drafted but not pinned** (Blocking Gaps lists "Event taxonomy" as an open should-have; three unreconciled naming styles exist). That selection is a domain fork and the Engineer's call.

## 📦 4. Deliverables
- [ ] An AsyncAPI **v3** document (e.g. `asyncapi/plugin-events-v1.yaml` — placement is a small naming call; `openapi/` is now a misnomer for an AsyncAPI file, see §8).
- [ ] `info` + webhook server/binding; a `x-zurfur-status` maturity/stub banner mirroring ZMVP-26.
- [ ] A common **event envelope** message (id, type, timestamp, subject/scoped-principal) + the `standard-webhooks` signature headers (`webhook-id`, `webhook-timestamp`, `webhook-signature`), referencing ZMVP-41 for the signing mechanics.
- [ ] **Channels/messages** for the v1 event catalogue (commission events + workflow events), each with an **event-scope** annotation (`x-required-scopes`, provisional) and `action: receive` (core sends).
- [ ] **Stub** payload schemas for the Commission-derived bodies, clearly marked pending ZMVP-19 (mirror ZMVP-26's `x-zurfur-stub`/`x-zurfur-pending`).
- [ ] **CI:** AsyncAPI validation in the `spec-lint` job (redocly does **not** lint AsyncAPI — needs `@asyncapi/cli validate` or Spectral asyncapi ruleset; the current `openapi/*.yaml` glob would mis-handle it).
- [ ] Validation run reported clean.

## 🧩 5. Work breakdown
| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Pin the v1 event catalogue & channel/naming taxonomy** (which events ship v1; reconcile Workflow vs Plugin-page naming; MVP-slice cut) | 4 — uncertainty/domain, not effort | P0 | 🧑 Engineer | ⬜ open fork — Blocking Gaps flags "Event taxonomy" as an open should-have; no DD/decision pins it (Workflow 9895957 drafts it; Plugin 3047451 & First-party 3244063 use different names) |
| AsyncAPI scaffolding (`info`, server, envelope message, signature headers, status banner) | 2 | P1 | 🤖 Claude | ⬜ — pattern set by `openapi/plugin-v1.yaml` |
| Event channels/messages from the pinned catalogue + event-scope annotations | 3 | P1 | 🤖 Claude (once catalogue pinned) | ⬜ — depends on row 1 |
| Stub Commission-derived payload schemas (pending ZMVP-19) | 2 | P2 | 🤖 Claude | ⬜ — mirror ZMVP-26 stubs |
| CI AsyncAPI lint wiring (asyncapi-cli/Spectral; fix `openapi/*.yaml` glob assumption) | 3 — redocly ≠ AsyncAPI | P1 | 🤖 Claude | ⬜ — `spec-lint` exists (`ci.yml`) but is redocly-only |

- Bulk of the *authoring* is Claude-owned boilerplate mirroring the landed OpenAPI sibling. The **one Engineer-owned piece is the gate**: pinning the event catalogue/taxonomy. It is small and well-fed (the Workflow draft + DD-settled envelope), not a research project — but it is a genuine domain fork, so it is the Engineer's.

## ✅ 6. Test checklist (TDD)
- **Validation** — _asserts that_ the document parses as valid **AsyncAPI 3.x** and lints clean (`asyncapi validate` / Spectral asyncapi ruleset) → AC: "spec is authored & valid".
- **CI** — _asserts that_ the `spec-lint` job actually validates the AsyncAPI file (not silently skipped by a redocly-only/OpenAPI-only path) → AC: "runs under CI like the OpenAPI sibling".
- **Envelope** — _asserts that_ every message carries the common envelope (id, type, timestamp, subject) and the `standard-webhooks` signature headers are documented, referencing ZMVP-41 → AC: "signed-webhook contract described".
- **Catalogue** — _asserts that_ exactly the **pinned v1 event slice** appears (commission + workflow events agreed in §5 row 1), each `action: receive`, each with its event-scope → AC: "core→plugin events enumerated".
- **Scoping** — _asserts that_ each channel/message declares its required `events:*` scope (provisional) → AC: "permission-scoped subscription expressed".
- **Stubs** — _asserts that_ Commission-derived payloads are present but clearly marked stub/pending ZMVP-19 → AC: "schema-gate respected, not faked".

## 🧠 7. Logic & shape
```
asyncapi: 3.0.0
info: { title, version 1.0.0, x-zurfur-status: { maturity: draft, stubbed:[Commission→ZMVP-19], consumes:[ZMVP-41, ZMVP-28] } }
servers:  plugin-webhook  (the plugin's own SSL endpoint; delivery owned by ZMVP-41)
channels:
  commission/{commissionId}   → messages: lifecycle.changed, status.changed, file.uploaded, markup.added, paid, completed
  workflow/{workflowId}       → messages: card.moved, card.reordered, card.added|removed, card.archived|restored, list.created|renamed
operations:  one per message, action: receive (core sends → plugin receives), x-required-scopes: [events:commission.*|events:workflow.*]  (provisional)
components:
  messages.EventEnvelope:  headers {webhook-id, webhook-timestamp, webhook-signature}; payload {id, type, occurredAt, subject, data:$ref(stub)}
  schemas.{Commission(STUB), …}  x-zurfur-stub / x-zurfur-pending: ZMVP-19
```
Mirror ZMVP-26's conventions verbatim where they transfer: the `x-zurfur-status` banner, `urn:zurfur:error:*` framing if errors appear, the "stub `$ref` pending ZMVP-19" pattern, and the versioning reference to ZMVP-28.

## 🚀 8. Next steps
1. ⚠️ **BLOCKING — Engineer decision (route via `/design-decision` if it rises to a DD):** pin the **v1 core→plugin event catalogue** before authoring channels:
   - **Which events** ship in v1 — adopt the Workflow page's two lists wholesale, or cut to the Telegram-bot MVP slice (status/lifecycle/file/markup/dispute)?
   - **Canonical naming/taxonomy** — reconcile the three styles now in the docs: Workflow `lifecycle.changed`/`card.moved` vs Plugin `events:commission.completed`/`events:workflow.card.*` vs First-party human labels. One channel/message naming scheme must be chosen.
   - **Envelope field names** — confirm the envelope follows `standard-webhooks` (id/timestamp/type/data) + the scoped-principal/delegation tag; the *semantics* are DD-settled, the field spelling is a small ratification.
   - This is the analogue of the decision the Engineer had *already made* for ZMVP-26's operation surface; here it is still open (Blocking Gaps "Event taxonomy" = open should-have).
2. Once pinned, author the AsyncAPI doc mirroring `openapi/plugin-v1.yaml` conventions; stub Commission payloads (ZMVP-19); reference ZMVP-41 (signing/delivery) and ZMVP-28 (versioning).
3. Wire **AsyncAPI** validation into CI `spec-lint` — redocly@2.35.1 lints OpenAPI only; add `@asyncapi/cli validate` (or Spectral asyncapi ruleset) and decide the file's home (likely a sibling `asyncapi/` dir, since the `openapi/*.yaml` glob and name don't fit AsyncAPI).
4. Coordinate the **ZMVP-41 seam**: ZMVP-41's description already claims the "AsyncAPI v3 contract → relates to ZMVP-27." Keep this doc the *contract* (channels/messages/envelope/signature-headers) and ZMVP-41 the *implementation* (HMAC, SSRF, outbox, DLQ, rotation) — avoid double-owning the envelope. Flag for `/close-gaps` if both run in one set.

**Dangling notes:** ZMVP-19/28/41 are all **To Do** (none built) — like ZMVP-26, they are stub/reference targets, not hard blockers. The ticket has no Jira issue-links (the ZMVP-25/26/28/41 relationships live only in prose); worth linking. Event-scope vocabulary inherits the same "provisional" status ZMVP-26 gave action scopes (DD-24543244 scope-vocab follow-up).
