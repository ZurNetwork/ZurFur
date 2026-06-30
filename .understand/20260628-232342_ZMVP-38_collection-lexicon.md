# 🔎 Understanding ZMVP-38 — Author the Lexicon for the generic Collection record

> **Status:** To Do (Medium) · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-38 · **Generated:** 2026-06-28 23:23 · **Snapshot:** `.understand/20260628-232342_ZMVP-38_collection-lexicon.md`

## 🧭 1. Context (cold-start)

Zurfur is an AT Protocol-native commission platform split across two data boundaries: **Private** facts live in The Index (PostgreSQL); **Decentralized** facts live as public records in each user's PDS. In atproto, a public record's shape is fixed by a **lexicon** — a JSON schema document under a reverse-DNS authority (here `app.zurfur.*`). The DESIGN rule (Lexicon page): **"only Decentralized entities get a lexicon."**

A fresh Design Decision — **"Collection as a Generic Referenceable Membership Primitive"** (DECIDED 2026-06-28, DESIGN 24182787) — redefined a `Collection` from "a bag of `Post`s" to "a bag of **`Referenceable`**" (a new domain trait in the family of Taggable/Historical/Commentable/Rateable). Favorite / wishlist / will-not-commission **artist lists** are the motivating consumers: each a **Static, homogeneous, public** Collection holding a **typed reference** (a DID for an artist/Account/Character; an internal id for an Index-local Post). Because these Collections are **public atproto records**, the DD's own open table tickets one obligation: **author the lexicon** for the generalized Collection record. That is this ticket.

**Critical cold-start fact:** the repo has authored **zero** Zurfur lexicons so far. `app.zurfur.*` does not appear anywhere in code; there is no `lexicons/` directory, no NSID constants, no lexicon codegen. The only atproto records touched are *Bluesky's* (`app.bsky.actor.profile`, read-only, in `adapter-atproto/src/profile.rs`). So ZMVP-38 would author the **first** Zurfur lexicon and thereby set the convention (file location, NSID namespace, whether to derive Rust types).

## 🗺️ 2. Domain

Confluence DESIGN entities in play:
- **[Collection](.../8912899)** — curation primitive: a **reference, not a label**; holds **live** references (a member silently drops if privatized/deleted); **Static** (hand-curated explicit bag, manual order) vs **Dynamic** (a `Lens` query, order from a sort rule).
- **[Collection-as-Referenceable DD](.../24182787)** — the binding decisions: Referenceable is a trait; member type is the parameter; members **homogeneous** per collection; reference is **typed** (DID vs internal id); profile collections are **display-only** and **public atproto records** at v1; private variant deferred (→ ZMVP-37).
- **[Lexicon](.../10354710)** — the schema registry page. **Status: scaffold.** Its "A. Record lexicons" table currently lists `feed.post`, `actor.account`, `actor.character`, `feed.comment` — **no Collection NSID yet.** Conventions: authority `app.zurfur.*`, camelCase fields, **version in place** (additive only; new NSID only for a true break).
- **[Lens](.../10453000)** — a saved query `{ source, filter, sort }`, **data not code**, membership-only (visibility added per-viewer by the Engine). A Dynamic Collection *is* a Lens. The Lens is described **conceptually**; no concrete encodable schema is pinned anywhere.

The lexicon must encode three DD invariants: **homogeneous member kind**, **typed reference** (DID vs internal id), and **Static-or-Dynamic** (Dynamic carrying a Lens spec).

## 🎯 3. Goal & scope

**Goal:** produce the AT Protocol lexicon JSON for a public, generic Collection record — generic over `Referenceable` member type — that faithfully encodes the DD's invariants (homogeneous kind, typed reference, Static/Dynamic), and register it on the Lexicon DESIGN page.

**In scope:**
- The `app.zurfur.*.collection` lexicon JSON (NSID + field list), establishing the repo's lexicon convention (location, namespace).
- Schema encoding of: a `memberType` discriminator (homogeneity), a typed member reference (DID vs internal id), and a Static (explicit members) / Dynamic (embedded Lens spec) union.
- Registering the new NSID + field list on the Confluence Lexicon page (design-sync).

**Out of scope:**
- The **private** (Index-resident) Collection variant → **ZMVP-37** (post-v1).
- Any **behavioral enforcement** on the lists (display-only per DD).
- Building the **Lens Engine** / query compilation (Lens is its own primitive).
- The wider commission/post Lexicon field-list finalization that gates ZMVP-19/25/26/27 — independent of this record.

**Gating (from the ticket Notes + DD open table) — two unresolved Engineer forks:**
1. **`Referenceable` v1 coverage** — Account + User needed now; whether `Character` + `Post` are declared in from the start is **open**.
2. **The Lens spec shape** — needed to make Dynamic representable (AC3); not pinned. Plus: "author **alongside** the generic Collection primitive, not before it" — and that primitive does not exist in code or in a separate ticket.

## 📦 4. Deliverables

- [ ] An `app.zurfur.*.collection` **lexicon JSON** (NSID chosen; field list per the DD invariants) — committed at an agreed repo location.
- [ ] Schema construct for **homogeneous member kind** (a `memberType` token/enum).
- [ ] Schema construct for the **typed member reference** (a union: DID-form vs internal-id-form).
- [ ] Schema construct for **Static vs Dynamic** (a union: explicit `members[]` vs an embedded **Lens spec** def).
- [ ] Shared **Lens spec** lexicon def (or a referenced `app.zurfur.*.defs#lens`), pending the pinned Lens shape.
- [ ] Lexicon page (DESIGN 10354710) updated: new row in "A. Record lexicons" + any shared `defs`.
- [ ] (If the convention chosen is code-derived) jacquard-`derive` Rust types + a serde/validation round-trip test.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| Pin `Referenceable` v1 coverage (Account/User vs +Character/+Post) | 3 — pure judgment, low effort | P1 | 🧑 Engineer | ⬜ DD open table lists it unresolved (DESIGN 24182787 "Open / follow-up") |
| Pin the **Lens spec** encodable shape (for Dynamic) | 7 — cross-cutting fork; Lens Engine/Gallery/search/Workflows all consume it | P1 | 👥 Group | ⬜ Lens page is conceptual only (DESIGN 10453000); no concrete schema; arguably its own ticket/DD |
| Choose NSID + lexicon-file location + codegen approach (first-of-kind convention) | 4 — naming + structural, sets precedent | P2 | 🧑 Engineer | ⬜ no `lexicons/` dir, no `app.zurfur.*`, no NSID anywhere in repo |
| Author the Collection lexicon JSON (members, typed ref, Static/Dynamic union) | 3 — mechanical once decisions pinned | P2 | 🤖 Claude | ⬜ not started; nothing exists |
| Register NSID + field list on the Lexicon DESIGN page (design-sync) | 2 — doc edit, Engineer-approved | P3 | 🤖 Claude | ⬜ Lexicon table has no Collection row |
| (Optional) jacquard-derived Rust types + round-trip/validation test | 3 — boilerplate | P3 | 🤖 Claude | ⬜ no Collection/Referenceable types in `domain` (elements/ has none) |

Difficulty driver throughout is **uncertainty/decision-gating**, not effort: the JSON itself is small once the two forks are settled.

## ✅ 6. Test checklist (TDD)

A lexicon is a schema, so "tests" are validation/round-trip assertions, mostly meaningful only if the chosen convention derives Rust types:
- **Schema validity** — _asserts that_ the `app.zurfur.*.collection` document is a valid atproto lexicon (parses against the lexicon meta-schema / jacquard accepts it) → AC1.
- **Homogeneity** — _asserts that_ a record declaring `memberType = account` whose members carry a non-account reference is rejected/representable as invalid → AC2.
- **Typed reference** — _asserts that_ a DID-anchored member round-trips as a DID and an Index-local member (Post) round-trips as an internal id, via the typed-reference union → AC2.
- **Static vs Dynamic** — _asserts that_ a Static record (explicit `members[]`) and a Dynamic record (embedded Lens spec, no explicit members) both round-trip and are distinguishable → AC3.
- **Additive-evolution guard** — _asserts that_ an unknown optional field deserializes without error (lexicons version in place) → Lexicon-page convention.

(If the team chooses JSON-only with no Rust derive, ACs are met by the document + a lexicon-validator run rather than cargo tests.)

## 🧠 7. Logic & shape

Proposed schema silhouette (a **proposal to the Engineer**, not a decision — the unions are the load-bearing choices):

```
app.zurfur.graph.collection   (NSID TBD — graph? actor? new ns?)
  record:
    name           string
    description?    string
    static | dynamic   ← union (exactly one)
      static:  { memberType: token, members: [ ref ] }   ← ref is the typed-reference union
      dynamic: { memberType: token, lens: <app.zurfur.*.defs#lens> }
    createdAt      datetime

  typed reference (union):
    didRef       { did: string(format=did) }       ← Account / User / Character
    indexRef     { id:  string }                    ← Index-local Post (internal id / at-uri?)

  memberType (token): account | user | character | post   ← gated by Referenceable v1 coverage
```

```
        ┌──────────── DECISION GATES (Engineer) ────────────┐
        │  1. Referenceable v1 coverage  →  memberType enum  │
        │  2. Lens spec shape            →  dynamic.lens def │
        │  3. NSID + file location       →  convention       │
        └───────────────────────────────────────────────────┘
                              │  (all pinned)
                              ▼
        Author JSON  →  register on Lexicon page  →  (opt) derive + round-trip test
```

## 🚀 8. Next steps

1. ⚠️ **Decision session with the Engineer before any authoring** — this ticket's substance is gated on two unmade domain calls:
   - **Referenceable v1 coverage** (Account+User only, or +Character/+Post?). Drives the `memberType` enum.
   - **Lens spec shape** — Dynamic representability (AC3) needs a concrete encodable Lens schema; today the Lens page is conceptual. This is a sizeable, cross-cutting fork (Lens Engine, Gallery, search, Workflows all consume it) — **recommend carving it into its own DD/ticket** and either (a) blocking ZMVP-38 on it, or (b) descoping AC3 to "Dynamic is *representable* via a ref to a future `defs#lens`" so the Static path can ship now.
2. ⚠️ **Where does the generic Collection primitive live?** The ticket says author "alongside the generic Collection primitive, not before it," but no `Collection`/`Referenceable` type exists in `domain/src/elements/` and no separate ticket was found. Confirm whether the primitive is part of this ticket or a missing upstream one.
3. **Engineer:** choose the NSID namespace + the repo's lexicon-file convention (first one authored — sets precedent), and whether to jacquard-`derive` Rust types.
4. **Then Claude (mechanical):** author the JSON, register it on the Lexicon page (offer `/design-sync`), add the round-trip test if derived.
5. **Sequencing note:** ZMVP-38 is **upstream of ZMVP-37** (Private Collections), which is explicitly "pulled when the generic Collection primitive's public variant ships." No file collision between them.

**Open questions (blocking):** Referenceable v1 coverage · Lens spec shape (own DD?) · NSID + lexicon convention · home of the generic Collection primitive.

---

## ➕ Addendum (built 2026-06-30, uow b722f9) — forks settled, lexicon authored

§8's "decision session before authoring" verdict is **superseded** — the Engineer settled all four forks. Authored per those decisions:

- **NSID:** `app.zurfur.graph.collection` (new `graph` namespace, mirroring `app.bsky.graph.list`; precedent: membership/relationship records live under `app.zurfur.graph.*`).
- **Files:** `lexicons/app.zurfur.graph.collection.json` (record) + `lexicons/app.zurfur.graph.defs.json` (`#lens` **stub**). First Zurfur lexicons; created the `lexicons/` dir convention (one JSON per NSID).
- **JSON-only, no Rust codegen.** Generic `Collection`/`Referenceable` Rust primitive explicitly out of scope.
- **memberType** = open `knownValues: [account, user]` (NOT a closed `enum`) — faithful to the DD's "additive enum value adds without breaking." Character/Post deferred.
- **Typed-ref union** `#didRef {did: did}` | `#indexRef {id: string}`; **membership union** `#static {members[]}` | `#dynamic {lens → app.zurfur.graph.defs#lens}`.
- **Lens** is a placeholder stub (non-normative `{source, filter, sort}` strings), pending a future Lens-lexicon DD.

**Validated** with the real atproto `@atproto/lexicon` `Lexicons.add()` meta-schema + cross-ref resolution + DD-invariant assertions: ALL PASSED.

### design-sync owed (orchestrator / Engineer — do NOT edit Confluence here)
Lexicon page **10354710**, table "A. Record lexicons" needs a new row:

| NSID | Purpose | Key fields (draft) | Status |
| --- | --- | --- | --- |
| `app.zurfur.graph.collection` | Public, homogeneous Collection of Referenceable members (Static explicit / Dynamic Lens). See Collection-as-Referenceable DD (24182787). | `name`, `description?`, `memberType` (account\|user), `membership` (static\|dynamic), `createdAt` | Drafted (ZMVP-38) |

Plus a "Shared defs / tokens" entry: `app.zurfur.graph.defs#lens` — **stub**, Lens spec shape pending a dedicated Lens-lexicon DD.
