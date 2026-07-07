# 🔎 Understanding ZMVP-104 — The Lexicon field lists are final (repo-canonical, Registry mirrors)

> **Status:** To Do (Medium) · **Owner:** 🧑 **Engineer** (pure design/authoring — shapes public record schemas) · **Epic:** ZMVP-101 "The Twenty One" (Wave 1) · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-104 · **Generated:** 2026-07-04 14:50 · **Snapshot:** `.understand/20260704-145017_ZMVP-104_lexicon-field-lists-final.md` · **recommended_model:** **Opus 4.8** (private↔public boundary + DID-correlation surface forces Opus; but every field-list *decision* is the Engineer's — Claude supports, never decides)

## 🧭 1. Context (cold-start)

Zurfur splits every fact across two data boundaries. **Class B** (Index-canonical, PostgreSQL): anything that *can* be private — the whole Commission family — never crosses as itself; it has **no lexicon**. **Class A** (atproto-native, PDS-canonical): things public-by-construction — gallery posts + their blobs, public profiles, follows, account identity — are born in the owner's repo. In atproto a public record's shape is a **lexicon**: a JSON schema under a reverse-DNS authority (`app.zurfur.*`, since we control `zurfur.app`). Lexicon fields are **one-way doors** — once a lexicon is published and third parties adopt it, the shape freezes; fields can only be *added*, never removed or tightened; a breaking change needs a new NSID.

ZMVP-104 closes the **last open row** of "Blocking Gaps for v1" (9994307): the atproto record schemas have NSIDs + rough shapes scaffolded on the Lexicon page, but **not complete field lists** (names, types, required/optional, size limits, blob refs). This ticket finalizes them as **lexicon JSON in the repo**.

**The 2026-07-04 ruling (in the ticket):** the repo JSON files are the **source of truth**; the Confluence **Lexicon Registry** page (29818896) becomes a **mirror**. This inverts the prior arrangement where Confluence was canonical. Also new on 2026-07-04: **blobs are in scope for v1** (image-bearing records declare blob fields).

**Cold-start reality of the repo:** exactly **two** Zurfur lexicon JSON files exist today — `lexicons/app.zurfur.graph.collection.json` + `lexicons/app.zurfur.graph.defs.json` (a `#lens` stub) — authored by ZMVP-38 (uow b722f9). They established the convention: one JSON per NSID under `lexicons/`, **JSON-only, no Rust codegen**, `memberType`-style open `knownValues` over closed enums, validated against the real `@atproto/lexicon` meta-schema. **No `feed.*`, `actor.*`, `seal.*`, `labeler.*` JSON exists yet.** `app.zurfur.*` appears nowhere in `backend/` Rust; the only atproto records the code touches are Bluesky's read-only `app.bsky.actor.profile` in `adapter-atproto/src/profile.rs`.

**Where 104 sits in the epic:** ZMVP-101 makes the *record write path* real against a **local, wipeable PDS**. 104 is one of three parallel Wave-1 tickets (102 = local PDS in dev loop; 103 = throwaway PDS in integration tests; 104 = this). 104 needs no PDS and no code — it is pure authoring. It **blocks ZMVP-105** (adapter-atproto record CRUD, which validates against these lexicons) and **ZMVP-106** (capstone: a post+blob survives wipe-and-replay). **Post-first ordering matters:** 105/106 only need `app.zurfur.feed.post` final, so finalizing the post lexicon first unblocks them before the rest of the set is authored.

## 🗺️ 2. Domain — the settled rulings this authoring must obey

The Boundary Contract programme (Units 1–6, all DECIDED 2026-07-02) is the ground truth. **⚠️ None of these pages are in the local `docs/confluence-design-index.md` — the index is stale and must be updated** (see §8):

- **Boundary Contract — Class A/B & the Public-Node Test** (29622283, Unit 1): the class test; lexicon fields are one-way doors; the network never guarantees forgetting; assertions-*about*-someone cross only as Provider-signed attestations.
- **Publish Consent & OAuth Scopes** (29687820, Unit 2): identity-only sign-in; publish authority requested at first use via permission sets — `app.zurfur.authPublish` (`repo:app.zurfur.*` + `blob:*/*`) now, `app.zurfur.authGraph` (friends, peer Seals) later. **These permission sets are themselves lexicons** but are *not records* — different shape.
- **Gallery Posts, the Product Snapshot & Index-Side Tagging** (29949954, Unit 3): **the `app.zurfur.feed.post` field list is RULED here** (its §9 "the one-way door" table — see §7 below). Product = Class B, no lexicon; Gallery Post = the Class A record. **Descriptive tags are 100% Index-side — struck from the record.** Maturity self-labels **stay on the record** (safety travels with blobs). Blobs CID-deduped across posters. commissionRef = **opaque token only** (never references private facts).
- **Maturity Vocabulary — Adopting atproto Self-Labels** (29982722, Unit 4): axis is now **Safe / Suggestive / Nudity / Adult + orthogonal Graphic flag** — **supersedes the old Safe/Questionable/Explicit** token that the Lexicon page scaffold still shows.
- **Asks as Tags** (29622362, Unit 5): Asks are Index-side tags, **no lexicon, no record** — confirms nothing new to author for asks.
- **The Lexicon Registry — Publish-Late, Additive-Only** (29818896, Unit 6): one Zurfur DID holds schema records, resolved via `_lexicon.zurfur.app` DNS TXT; **additive-only evolution**; **publish-late doctrine — a lexicon publishes only when its feature ships; pre-ship shapes are Index-internal and freely mutable.** Fixes the **namespace map** and a **queue-&-gates table** (both reproduced in §3/§7).
- **Seals — Attestations as Labels & Peer Grants** (29622321) + **Seal** entity page (30507148, edited ~14h ago): `seal.grant` = the user-to-user peer-grant transport record; institutional Seals ride atproto Labelers; `labeler.declaration` declares Zurfur (and third parties) as a labeler/Seal-minter.
- **Lexicon** (10354710): the scaffold page. **Status still says "scaffold; field lists are drafts."** Its `feed.post` row and `feed.defs#rating` token are now **STALE** vs Units 3/4 (drift — see §7).

**Load-bearing invariants the field lists must not violate:** `feed.post` must **not** embed or reference the private commission (opaque token only, no client identity / price / brief); credits are **DIDs** (a correlation surface — see §7 security note); maturity self-label is **required** on image-bearing records; every published field must be justified because it can never be removed.

## 🎯 3. Goal & scope

**Goal:** produce complete, final lexicon **JSON in the repo** for every record type the MVP write path actually writes — field names, types, required/optional, size limits, blob references — faithful to the Units 1–6 rulings; make the Lexicon Registry page mirror the repo and declare the repo canonical; close the last Blocking-Gaps row.

**The namespace map (Registry §4, fixed) — the candidate write-set:**

| NSID | What it is | Registry gate (§5) |
|---|---|---|
| `app.zurfur.feed.post` | Gallery Post (Class A) — **capstone subject of ZMVP-105/106** | **Fields ruled (Unit 3)**; draft at ship time |
| `app.zurfur.actor.account` | Account public profile record | Draft at publish time from entity page |
| `app.zurfur.actor.character` | Character record (two-tier: in User's PDS / own DID) | Draft at publish time from entity page |
| `app.zurfur.graph.friend` | Friendship / relationship record | DD decided; draft when feature nears ship |
| `app.zurfur.seal.grant` | Peer Seal grant (user-to-user attestation transport) | DD decided; draft when feature nears ship |
| `app.zurfur.labeler.declaration` | Labeler/Seal-minter declaration | DD decided; draft when feature nears ship |
| `app.zurfur.feed.comment` | Comment on a public record | **Blocked on Comments DD (does not exist yet)** |
| `app.zurfur.authPublish`, `app.zurfur.authGraph` | Permission-set lexicons (**not records**) | With first publish flow (Unit 2) |
| *(authored)* `app.zurfur.graph.collection` + `graph.defs#lens` | Already final in repo (ZMVP-38) | Done |

**In scope (bounded by the Engineer's scope call — see §8 Q1):**
- Finalize `app.zurfur.feed.post` **first** (it is ruled and it unblocks 105/106) + the shared defs it references.
- Author blob field(s) on image-bearing records (2026-07-04 blobs-in-scope ruling).
- Whichever *other* NSIDs the Engineer decides belong to the v1 write-set (see the publish-late tension in §8 Q1).
- Update the Lexicon Registry (mirror + "repo canonical" statement), reconcile the Lexicon scaffold page, close the Blocking-Gaps row.

**Out of scope:**
- Publication tooling (the `_lexicon.zurfur.app` DNS record, schema-push to Zurfur's PDS) — "ticket-level work at first ship" (Registry §7), not this ticket.
- `adapter-atproto` record CRUD / validation code — that's ZMVP-105.
- The private Commission/export schema (Group B) — deliberately not a lexicon.
- The Lens spec shape — still a stub, its own future DD (unchanged by this ticket).
- Anything the tag system needs (Index-side, Unit 5) — no record.

## 📦 4. Deliverables

- [ ] `lexicons/app.zurfur.feed.post.json` — complete, final field list per Gallery Posts DD §9 reconciled with Unit 4 maturity vocab; declares its blob field(s).
- [ ] Shared-def JSON the post references: `app.zurfur.embed.media` (blob embed) and `app.zurfur.feed.defs` (rating/kind or medium-type tokens) — **updated to the Unit 4 vocabulary**, not the stale Safe/Questionable/Explicit.
- [ ] JSON for each additional NSID the Engineer scopes into v1 (candidates: `actor.account`, `actor.character`, `graph.friend`, `seal.grant`, `labeler.declaration`) — each a complete field list, blob fields declared where image-bearing (`actor.account.avatar`, `actor.character.avatar`).
- [ ] Every authored lexicon validates against the real atproto `@atproto/lexicon` meta-schema + cross-ref resolution (the ZMVP-38 validation harness pattern).
- [ ] Lexicon Registry page (29818896) updated: mirrors the repo field lists + states "repo JSON is canonical, this page mirrors."
- [ ] Lexicon scaffold page (10354710) reconciled: `feed.post` row + `feed.defs#rating` token brought in line (drift fix), any new rows, "scaffold" status lifted for finalized entries.
- [ ] Blocking Gaps for v1 (9994307): the atproto-Lexicon checkbox **closed**.
- [ ] `docs/confluence-design-index.md` updated to add the six Boundary-Contract-programme pages + two Seal pages (currently absent).

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done-with-evidence |
|---|---|---|---|---|
| **Q1 — Scope:** which NSIDs finalize now (publish-late `feed.post`-only vs whole map)? | 4 — pure judgment; publish-late doctrine vs ticket's "then the rest" | P1 | 🧑 **Engineer** | ⬜ Registry §5 gates most at "draft when features near ship"; `feed.comment` hard-blocked |
| **Q2 — `feed.post` field encoding** (title/description/blobs/credits/commissionRef/maturity/medium-type/createdAt: required?, size limits?, exact shapes) | 6 — one-way doors; each field justified-before-shipped | P1 | 🧑 **Engineer** | ⬜ DD §9 rules the *fields*; the *encoding* is unmade |
| **Q3 — Maturity self-label encoding** (custom token vs atproto `com.atproto.label.defs#selfLabels`; Safe/Suggestive/Nudity/Adult + Graphic) | 5 — protocol-fidelity fork; safety metadata | P1 | 🧑 **Engineer** | ⬜ Unit 4 fixed the *vocabulary*, not the record encoding |
| **Q4 — medium/type** closed enum vs open string | 3 — Gallery Posts open/follow-up explicitly defers this "to lexicon time (Unit 6)" | P2 | 🧑 **Engineer** | ⬜ open ruling, this is lexicon time |
| **Q5 — blob field convention** (`embed.media`: mimeType allow-list, maxSize, alt text, aspect ratio) | 4 — sets the convention for all image-bearing records | P2 | 🧑 **Engineer** | ⬜ no blob def exists; 2026-07-04 put blobs in scope |
| Author the JSON once Q1–Q5 pinned (feed.post + defs first) | 3 — mechanical, ZMVP-38 pattern | P2 | 🤖 Claude (Engineer's lane) | ⬜ nothing authored beyond graph.collection |
| Validate against `@atproto/lexicon` meta-schema | 2 — reuse ZMVP-38 harness | P3 | 🤖 Claude | ⬜ |
| Registry mirror + "repo canonical" + scaffold reconcile + close Blocking-Gaps row (/design-sync) | 3 — doc edits, Engineer-approved | P3 | 🤖 Claude | ⬜ pages still say "scaffold" |
| Update `docs/confluence-design-index.md` (8 missing pages) | 1 — index maintenance | P3 | 🤖 Claude | ⬜ index snapshot predates Units 1–6 |

Difficulty is driven by **decision-gating and one-way-door irreversibility**, not effort — the JSON is small once the forks are settled. **This is Engineer's-lane implementation** (it encodes entity/invariant rules); Claude supports mechanically and reviews the boundary, but does not choose field lists.

## ✅ 6. Test checklist (validation, not TDD — a lexicon is a schema)

Reusing the ZMVP-38 approach (validate with the real `@atproto/lexicon` `Lexicons.add()`), each finalized lexicon asserts:
- **Schema validity** — the document parses as a valid atproto lexicon and cross-refs resolve → AC "complete field list."
- **Blob presence** — `feed.post` (and any image-bearing record) declares a blob field of the atproto `blob` type with its constraints → AC "image-bearing records declare blob fields."
- **Boundary invariant** — `feed.post` carries **no** private-commission reference beyond an opaque `commissionRef` string; **no** `tags` field (struck, Index-side) → Boundary Contract + DD §5/§9.
- **Maturity required** — the self-label field is present and required on the post → DD §6.
- **Additive-tolerant** — an unknown optional field deserializes without error → publish-late/additive-only doctrine.
- **Registry ↔ repo parity** — every field in the repo JSON appears in the Registry mirror table (a doc-review assertion, not a cargo test).

(No `cargo test` unless the Engineer reverses the ZMVP-38 "JSON-only, no Rust derive" convention — see §8 interaction with ZMVP-105.)

## 🧠 7. Logic & shape — the `feed.post` field list (ruled) and its drift

**Gallery Posts DD §9 "the one-way door" (the authoritative field list, captured from the ADF table):**

| Field | Note (from the ruling) |
|---|---|
| `title`, `description` | Poster's speech |
| `blobs` | The work itself; CID-deduped across sources |
| `credits` | DIDs; opt-out at compose; owner-editable; Index-suppressible |
| commission ref | **Opaque token only** |
| maturity self-labels | **Required**; values per Unit 4 (Safe/Suggestive/Nudity/Adult + Graphic) |
| medium/type | Sticker, fursuit, story, illustration, … |
| created-at | — |
| ~~tags~~ | **Struck — Index-side only** |

**⚠️ DRIFT to flag to the Engineer:** the **Lexicon scaffold page (10354710)** still lists `feed.post` as `text?, embed(→embed.media), kind, rating, tags[], createdAt` and defines `feed.defs#rating = Safe/Questionable/Explicit`. Both are **superseded**: the Unit 3 ruling replaces `text`→`title`+`description`, adds `credits`/`commissionRef`/`medium-type`, **removes `tags`**, and Unit 4 replaces the rating vocabulary. The final JSON follows the **rulings**, and the scaffold page must be reconciled (design-sync). This drift is exactly what "repo canonical, Registry mirrors" is meant to end.

**🔐 Security/boundary note (Designer lane):** `credits` is an array of **DIDs** published in a public record — a deliberate cross-persona correlation surface (it links a poster's DID to collaborators' DIDs, permanently, network-wide, un-forgettable). This is *intended* (opt-out at compose, Index-suppressible for render) but the Engineer should consciously affirm it at encoding time: once shipped it is a one-way door. Likewise `commissionRef` must be a truly opaque token that leaks nothing about the private commission (no sequential IDs that correlate, no embedded client DID). These are the two places a private↔public leak could hide.

**Proposed silhouette (a PROPOSAL to the Engineer, not a decision):**
```
app.zurfur.feed.post            (record, key=tid)
  title?           string   (maxGraphemes/maxLength — Engineer)
  description?     string   (Description-ceiling bound — Engineer)
  blobs            array<app.zurfur.embed.media>   (required? min1? — Engineer)
  credits?         array<{ did }>                  (empty-ok? role? — Engineer)
  commissionRef?   string   (opaque token; format — Engineer)
  labels           com.atproto.label.defs#selfLabels?  ← OR custom token (Q3)
  mediumType       token|string   (closed enum vs open — Q4)
  createdAt        datetime (required)
  // NO tags field (struck)

shared defs:
  app.zurfur.embed.media   { blob(blobref, accept[], maxSize), alt?, aspectRatio? }   (Q5)
  app.zurfur.feed.defs#maturity / #mediumType tokens   (Unit 4 vocab)
```
```
      ┌──────── DECISION GATES (Engineer) ────────┐
      │ Q1 scope · Q2 field encoding · Q3 maturity │
      │ encoding · Q4 medium/type · Q5 blob def    │
      └────────────────────┬──────────────────────┘
                           ▼
   Author feed.post + defs FIRST ─► validate ─► unblocks 105/106
                           ▼
   (Engineer-scoped remainder) ─► Registry mirror ─► close Blocking-Gaps row
```

## 🚀 8. Next steps & open questions

**Open design questions — the Engineer decides; Claude must NOT answer these:**

1. **Q1 — Scope of "the MVP write-set."** Publish-late doctrine (Registry §3) + the epic's pull rule ("work enters only if the record round-trip needs it") argue for finalizing **`feed.post` only** now, drafting the rest when their features near ship. But the ticket says "then the rest of the Class A/B MVP set (comments, friendship, relationship records, etc.)." These conflict. Note constraints that narrow it regardless: `feed.comment` is **hard-blocked** (no Comments DD exists — cannot be finalized); `graph.friend`/`seal.grant`/`labeler.declaration` are "draft when features near ship" and none ship in this epic; `authPublish`/`authGraph` are permission sets, not records, tied to the publish flow (Unit 2). **Recommendation to weigh (not a decision):** finalize `feed.post` + its shared defs now (unblocks 105/106), author `actor.account`/`actor.character` if the Engineer wants profiles in v1's write path, and leave friend/seal/labeler/comment as documented drafts. **This scope call is the Engineer's and gates everything else.**
2. **Q2 — `feed.post` field encoding** (per-field required/optional, size limits, exact shapes for `blobs`/`credits`/`commissionRef`). One-way doors — each field justified before it ships.
3. **Q3 — Maturity self-label encoding:** adopt atproto's `com.atproto.label.defs#selfLabels` shape wholesale, or a custom Zurfur token, for the Unit-4 vocabulary? (Protocol fidelity vs control; safety metadata must travel with blobs to other appviews.)
4. **Q4 — medium/type:** closed enum vs open string — Gallery Posts explicitly defers this "small ruling to lexicon time." This is lexicon time.
5. **Q5 — blob/`embed.media` convention:** accepted mimeTypes, max blob size, required alt text, aspect ratio — sets the convention for every image-bearing record.
6. **Codegen interaction with ZMVP-105:** ZMVP-38 fixed "JSON-only, no Rust derive." ZMVP-105 must *validate records against these lexicons* — confirm whether 105 validates structurally against the JSON or wants derived Rust types (an implementation call inside 105, but the Engineer may want to reaffirm the convention here).

**Ordered execution once Q1–Q5 are settled:**
1. 🧑 **Engineer decision session** (Q1–Q5). Offer to capture the field-list finalization as a DD amendment / design-sync since it hardens one-way doors — Engineer confirms.
2. 🤖 Author `feed.post` + `embed.media` + `feed.defs` JSON **first**, validate → this is the seam that unblocks 105/106.
3. 🤖 Author the Engineer-scoped remainder; validate each.
4. 🤖 `/design-sync`: mirror into the Registry (29818896) + "repo canonical" statement; reconcile the scaffold page (10354710) drift; **close the Blocking-Gaps row (9994307)**; update `docs/confluence-design-index.md` (add pages 29622283, 29687820, 29949954, 29982722, 29622362, 29818896, 29622321, 30507148).

**The seam handed to ZMVP-105:** the final `lexicons/app.zurfur.feed.post.json` (+ its shared defs) is exactly what `adapter-atproto` validates written/read records against in ZMVP-105, and what the ZMVP-106 capstone asserts "no draft fields." Post-first ordering means this single file unblocks the downstream pipeline before the rest of the set is authored — so **finalize `feed.post` before anything else**.

**Owner confirmation:** 🧑 **The Engineer owns this ticket.** It shapes public record schemas (entity/invariant decisions, irreversible one-way doors, the private↔public boundary) — squarely the Engineer's lane per `feedback_engineer_owns_all_decisions` + `feedback_engineer_implements_domain_work`. Claude proposes options, authors JSON once decided, runs validation and design-sync, and reviews the boundary/correlation surface — but decides **none** of Q1–Q5.

---
*Snapshot generated by /understand. Grounded in: ZMVP-104/101/102/103/105/106/107 (Jira); Confluence 29818896, 10354710, 9994307, 29949954, 29622283, 29687820 (fetched), 29982722/29622362/29622321/30507148 (search summaries); repo `lexicons/*.json`, ZMVP-38 snapshot; memory MEMORY.md. **Index staleness flagged:** the six Boundary-Contract + two Seal pages are newer than the `docs/confluence-design-index.md` snapshot.*
