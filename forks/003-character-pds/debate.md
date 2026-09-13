# Fork 003 — Character public presence: does a public Character get its own PDS repo? (SCRUTINIZE)

Run 2026-09-12. Judge: Claude (odd fork). Seats: GPT `gpt-5.6-sol` (Responses API + web_search), Gemini `gemini-3.1-pro-preview` (google_search). Sealed ranking appears only in the S4 section.

---

# Brief (seat-facing; sealed content stripped)

# Proposal brief — fork 003: does a public Character get its own PDS repo? (SCRUTINIZE)

Zurfur is an AT Protocol-native art-commission platform in Rust (ports-and-adapters: `domain` → `application` →
adapters `adapter-pg` [private store, PostgreSQL] and `adapter-atproto` [public boundary: PLC directory, PDS,
OAuth] → `composition`). A **Character** is a sona/OC — a persistent creative-identity object kept by one or more
**Keepers** (Users, each with their own `did:plc` and repo), depicted in commissions by filling **Slots**, taggable,
with its own profile. The Engineer runs a sibling library, **vulpes** (AT Protocol identity for Rust servers and the
reference implementation of ACP, the Attested Claims Protocol); Zurfur is a consumer of it.

## The fork (one question)

**Should a public Character be a first-class ATProto subject with its own PDS repo — and, if not, what should a
Character be on the network?**

## Framing

Every Character already mints a `did:plc` at creation (DD 57081857, 2026-08-30, decided and built: `CharacterId(Did)`,
handle-less, per-Character rotation-key material). The landed decision under scrutiny (DD 54427650, 2026-08-23) adds,
for **public** Characters, a **full PDS account**: a real repo holding ref sheets, bio and art records via custom
`app.zurfur.*` lexicons, Zurfur-hosted by default and migratable; ownership expressed as attested claims (owner's repo
claim + the character's attestation + rotation-key seniority); transfer = key rotation + claim swap, final after the
did:plc ~72 h recovery window; de-publicize = teardown of the repo. Private Characters keep the DID, publish nothing,
and live in the Index. On 2026-09-12 the Release Board (Project MVP 589826 v14) pulled this PDS half **into the alpha**.
The Engineer's stated purpose for the on-PDS claim: so a Character can publicly say *"this person owns me"* on the
network rather than that fact living only in keys; control itself lives in the rotation keys. Co-ownership is a
Zurfur-side (product/keys) concern, not something the claims layer handles.

The board's job is not to re-litigate whether Characters have DIDs (settled, built) but whether the **repo** — a hosted
PDS account per public Character, in the alpha — is the right shape, and to attack the framing if the option space is
wrong.

## Options (unranked, neutrally worded)

- **A. Own DID + own full PDS repo.** Public Characters get a Zurfur-hosted (migratable) PDS account with their own
  records (ref sheets, bio, art records under custom lexicons); ownership by attested claims in both repos; transfer by
  key rotation + claim swap; de-publicize = repo teardown. In the alpha.
- **B. Own DID, no repo.** Public Characters remain DID-only identities; their content lives in Zurfur's Index and is
  served by Zurfur; ownership is an Index fact plus, at most, a claim record in the Keeper's own repo; no per-Character
  hosting. (The posture of DD 26935298 "identity-only for v1".)
- **C. No own DID; records in the Keeper's repo.** A Character is a record set (`app.zurfur.*`) inside its Keeper's
  repo; the Keeper's DID is the identity; transfer = re-publication under the new Keeper (provenance by reference);
  would require reversing DD 57081857 for Characters.
- **D. B now, A later.** DID-only in the alpha; the repo is added at publicize once the alpha ships (the pre-2026-09-12
  timing of DD 54427650, i.e. "post-alpha").

## Enumerated factual claims

Verify each. Claims 1–5, 7 and 9 are external (atproto/PLC/PDS behaviour) — use your own web search, at most 5 fetches,
cite the URL per verdict. Claims 6, 8, 10 and 11 are verifiable against the context block below only.

1. `did:plc` supports multiple rotation keys in priority order, and a ~72 hour recovery window during which a
   higher-priority (more senior) rotation key can undo a later rotation.
2. A `did:plc` identity can exist with no `atproto_pds` service entry (no repo hosted anywhere); adding a PDS later is
   a DID document update operation, not a new identity.
3. The reference Bluesky PDS hosts many accounts per instance, each with its own repo and CAR export; account
   migration between PDSes is supported (deactivate on the old host, import the repo on the new, activate).
4. An atproto handle is an `alsoKnownAs` entry verified by DNS TXT or an HTTPS well-known; an account whose handle
   cannot be verified is rendered by AppViews as `handle.invalid`.
5. Custom lexicons under `app.zurfur.*` can be published as `com.atproto.lexicon.schema` records and resolved through a
   `_lexicon.<domain>` DNS TXT record naming the schema-holding DID.
6. Per-Character rotation-key material (never per-Keeper) prevents correlating a Keeper's Characters through the
   public PLC log.
7. The PLC directory log is append-only and public; a minted DID's creation timestamp is world-readable forever, even
   after the identity is tombstoned.
8. Zurfur's alpha ships no AppView and no production PDS of its own today (the dev loop runs a reference PDS container
   only); a Character repo would render in third-party AppViews or in Zurfur's own catered Gallery view.
9. Hosting cost and operational surface scale with (Keepers × public Characters) accounts on Zurfur's PDS; the
   reference PDS implementation has practical per-instance account limits or sizing guidance.
10. ACP/CCS ownership claims are custom records under a domain authority (kind
    `net.got-paws.acp.relationship.ownership`, roles `owner`/`owned`), not an atproto-standard mechanism; a third-party
    verifier needs the vulpes docs to interpret them.
11. DD 54427650 models co-ownership as priority-ordered rotation keys plus one ownership claim per co-owner (each
    attested by the character); vulpes's CCS rule is one in-force ownership edge per character, with N humans standing
    behind an *account*-owned character. The two are different models.

## Doctrine floor (always applies)

Anti-domination — exit and voice preserved by construction (a Keeper can always leave with their Character; Zurfur
vanishing must be an inconvenience, never a breaking factor). The non-toxic path outranks optimization: any mechanic
failing "does this hurt anyone's sanity?" is disqualified, not discounted. Zurfur never holds funds absent an explicit
ruling. Consensus is not evidence. The alpha has just been cut to the spine: security items first, then the minimum a
working artist and client need.

## Context block (excerpts; Confluence DESIGN pages as of 2026-09-12 unless noted)

### DD 54427650 "Characters on ATProto — Public DIDs, PDS Accounts & ACP Ownership" (DECIDED 2026-08-23; amended 2026-08-30 and 2026-09-12)

> **TL;DR.** Characters become first-class ATProto subjects. *Every* Character has its own `did:plc` from creation. A
> **public** Character additionally gets a **full PDS account** — a real repo (ref sheets, bio, art records via custom
> lexicons), Zurfur-hosted by default and **migratable** (no lock-in). A **private** Character lives entirely in the
> Index with no repo and no published records — the Class A/B boundary applied literally to *content*; its identity
> still exists in the PLC directory. Ownership is **atproto-native** through the Attested Claims Protocol (ACP) /
> Consensual Claims System (CCS): no verifiable-credential machinery needed for the public case.
>
> **The rulings.** One did:plc per every Character, minted at creation, handle-less (empty `alsoKnownAs`; Character
> handles are a roadmap item). Default PUBLIC; private Characters are Index-only in content but still carry a DID;
> unlinkability preserved by per-Character rotation-key material. **Ownership = three agreeing facts** (no VC): (1) the
> owner's repo carries an `ownership` claim (`net.got-paws.acp.relationship.ownership`, role `owner`, `did` = the
> character); (2) the character's **attestation** of that claim, signed with the character's key, stored beside it;
> (3) the owner holds a rotation key on the character **senior** to every custodian's. **Keeper = owner.** **Transfer /
> sale = PLC key rotation + claim swap**; 72 h finality rule — sales final only after the window closes (escrow or hold).
> **Co-ownership** = multiple priority-ordered rotation keys + one `ownership` claim per co-owner, each attested by the
> character. **Publicize = consent-gated one-way door** (Index → create repo → publish → link); **de-publicize =
> teardown, not rollback** (delete repo records, deactivate/tombstone the account, revoke ownership records, NULL the
> Index's repo pointers; the character continues internally on its unchanged DID). **Hard requirements** for every
> Zurfur-hosted account: the owner holds an equal-or-senior rotation key; routine CAR export/mirroring makes
> restore-elsewhere real. Key layout D — `[user_cold, vulpes_recovery, zurfur_operational]`, user key generated
> client-side.
>
> **Kill tests (both pass).** vulpes vanishes → every claim still verifies. Zurfur vanishes → DIDs persist in the PLC
> directory, repos survive on other PDSes and render in any AppView, Zurfur-hosted accounts re-point via the senior key
> and restore from CAR; a private Character's content dies with the Index but its DID persists as an empty husk.
>
> **Open items.** Custody: layout D + client-side user key + CAR export vs DD 26804226 custodial-by-default + opt-in
> recovery key — OPEN. Character handles — OPEN, roadmap (57081857 D3). Source of record = vulpes
> `docs/characters-atproto.md` (another repository; not edited by the 2026-09-12 timing amendment).

### Character page 5668866 — `Claims` section (added 2026-09-12)

> **Ownership is a symmetrical claim.** A Character **claims** a User as its owner, and the User **claims** the
> Character. The ownership edge exists only when **both** assertions exist — neither side can bind the other
> unilaterally. Same double-sided shape as Friendship's two records (DD 29884417). Because every Character mints its
> own DID, each side's claim is an assertion by a DID about a DID. **Scope: alpha.** **Open — not a decision:** the
> record shape (Index rows vs atproto records in each repo; how withdrawal severs; what a half-claim renders as).

### vulpes CCS — as relayed by the vulpes session on 2026-09-12 (docs/ccs.md rules 1+4, docs/acp.md kind naming; not fetched by this board)

> A claim is a record in the subject's *own* repo; consent is the counterpart's signature. A kind is a five-segment NSID
> `<tld>.<domain>.<protocol>.<category>.<name>`; authority = whoever controls the domain; the closed part is
> `<category>`, and a `relationship` payload must carry `did` (the counterpart) and `role` (this side), attested by the
> DID named in `did`. `net.got-paws.acp.relationship.ownership` is seeded as a generic (roles `owner`/`owned`).
> One kind, two roles — the owner writes {role: owner, subject: character} in its repo; the character writes
> {role: owned, subject: owner} in its own. Both halves share an edge `id` computed from the two DIDs, `expiresAt` and a
> shared nonce. The edge exists iff both halves are in force; a half alone renders "claimed, unconfirmed". Severed
> (half deleted from a reachable repo) is distinct from not-checkable (repo unreachable). **One in-force ownership edge
> per character** — more is a conflict state, never co-ownership; N humans behind an account-owned character = N
> `owner` halves against the account. Transfer = sever + new term, new `id`. Withdrawal = delete your half; `expiresAt`
> required as a backstop. The vulpes MVP is paired-only (no status lists).

### DD 57081857 "Actor Addressing — DID as the Only Identifier, Everywhere" (DECIDED 2026-08-30) — excerpt

> `did:plc` is the ONLY actor identifier on wire, in the domain and in the store — `UserId(Did)` / `AccountId(Did)` /
> `CharacterId(Did)`; actor tables keyed by `did` NOT NULL; every actor mints a DID at creation, Characters included.
> D2: per-Character rotation-key material, never per-Keeper. D3: Characters mint **handle-less**; a Character handle
> scheme is a roadmap item.

### DD 26935298 "Zurfur Public Presence & PDS — Identity-Only for v1" (DECIDED 2026-06-30; partly superseded) — sentences now in tension

> Title: "Identity-Only for v1". TL;DR: "For v1, Zurfur mints identity-only `did:plc` for Accounts and Characters …
> with no PDS and no atproto repo." D1: "Genesis op omits `atproto_pds`; no repo is hosted." D3: "Deferred, reversibly:
> … Account/Character public profiles, entity-authored Posts + public Blobs." Accepted tradeoff: "an Account/Character
> has an identity + handle but no network-visible atproto profile/post feed in v1." (The Character-minting portion is
> superseded by 54427650/57081857; the **Account** identity-only stance is unchanged.)

### Project MVP 589826 (v14, 2026-09-12) — the alpha, excerpt

> Alpha (34): login/logout · DID operations and minting · user pages · Accounts · handle change · Workflows (one board
> per account) · board ordering · Commission core · Slots · Seats + application handshake · Markup review loop ·
> Changelog as a human-readable record · Surfaces rudimentary and frozen · view grants + visibility · client-visible
> progress · Characters · **Claims** · Tags · Invoice generation · one Request for Approval action · Gallery as a minimal
> catered view (no `feed.post` records, no AppView) · maturity-labels UI · **Character public presence** · Ask-for-Art ·
> Blocks · Friendship · two-dimensional reputation · Portfolios surface · User-linking · frontend for exactly what the
> backend supports · Deployment · Security-first · closed-door privacy · fact-anchored deletion. Beta: Gallery Posts,
> Phases, trees, hash chain, comments, replies, Achievements, Plugin API, YCH, Kafka, Seals. Release: capacity guards,
> Collections, Notifications, Lens feed, disputes, marketplace, Telegram, Trello, webhook signing, subscriptions.

### DD 29818896 "The Lexicon Registry — Publish-Late, Additive-Only" — excerpt

> One Zurfur-owned DID holds all `com.atproto.lexicon.schema` records, resolved via `_lexicon.zurfur.app` DNS TXT;
> additive-only evolution; **publish-late** — a lexicon publishes only when its feature ships.

### Codebase facts (repo `main` = 49bb50f3, 2026-09-12)

> `domain/src/elements/character.rs` is a documented stub. `adapter-atproto` implements real did:plc minting with
> envelope-encrypted key custody, the signed UPDATE-op + outbox, OAuth sign-in and PDS record/blob writes against a
> PDS endpoint; `.env.example` §"Local PDS + PLC (dev loop)": `just up` brings up a digest-pinned reference PDS
> container at `localhost:2583`, "dev-only, never deployed" — there is **no production PDS today**. Nothing in the
> backend creates PDS accounts for Characters; no `app.zurfur.*` character lexicon is published (publish-late).

---

# S0 — Verification

## GPT

1. **CONFIRMED** — Rotation keys are priority-ordered by descending authority; within 72 hours, a higher-authority key can nullify operations signed by a lower-authority key. https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md))
2. **CONFIRMED** — A regular PLC operation may carry an empty services map; later operations replace DID state, allowing `atproto_pds` to be added without changing the DID. https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md))
3. **CONFIRMED** — A PDS may host many accounts, each with a repository; migration supports CAR export/import, DID/PDS reassignment, activation on the new host, and deactivation on the old. https://atproto.com/guides/account-migration ([atproto.com](https://atproto.com/guides/account-migration?utm_source=openai))
4. **WRONG** — Handles are DID `alsoKnownAs` claims verified bidirectionally through DNS TXT or HTTPS, but AppViews are not required to “render” `handle.invalid`; the specification says invalid handles should be marked invalid and API responses **can** use that sentinel. https://atproto.com/specs/handle ([atproto.com](https://atproto.com/specs/handle?hl=en-US))
5. **CONFIRMED** — Lexicons can be published as `com.atproto.lexicon.schema` repository records; `app.zurfur.*` resolves through `_lexicon.zurfur.app` to the publishing DID. https://atproto.com/specs/lexicon ([atproto.com](https://atproto.com/specs/lexicon?utm_source=openai))
6. **CONFIRMED** — The context explicitly mandates unique per-Character rotation-key material and states that this preserves unlinkability against key reuse in the PLC log.
7. **CONFIRMED** — PLC permanently exposes operation history, including nullified operations and timestamps, after deactivation; this exposes the identity-creation date. https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md ([github.com](https://github.com/did-method-plc/did-method-plc/blob/main/website/spec/v0.1/did-plc.md))
8. **UNVERIFIABLE** — The absence of a production PDS and AppView is confirmed by the context, but actual rendering of custom Character records by unspecified third-party AppViews is not evidenced.
9. **WRONG** — Account count does not scale as `Keepers × public Characters`; each hosted public Character adds one account, so hosted accounts scale roughly with hosted Keepers **plus** public Characters. The reference PDS does provide sizing guidance—1 GB RAM, one core, 20 GB SSD for 1–20 users—but not a protocol hard limit. https://github.com/bluesky-social/pds/blob/main/README.md ([github.com](https://github.com/bluesky-social/pds/blob/main/README.md))
10. **CONFIRMED** — The context identifies ACP/CCS ownership as custom `net.got-paws.*` records governed by that domain, not an ATProto-standard ownership mechanism; interpretation therefore requires the custom protocol specification.
11. **CONFIRMED** — The context directly describes incompatible models: DD 54427650 permits multiple co-owner edges, while the relayed CCS rules permit only one in-force ownership edge per Character and place multiple humans behind an owning account.
## Gemini

1. CONFIRMED. `did:plc` relies on priority-ordered rotation keys, and standard recovery mechanisms specify a 72-hour window during which a senior key can override and undo rotations.
2. CONFIRMED. The `atproto_pds` service is merely an endpoint entry within the DID document; a DID can exist without it, and adding it later is a DID update operation, not a new identity.
3. CONFIRMED. The reference Bluesky PDS handles multiple accounts, and account migration natively requires authenticating, exporting a CAR file from the old PDS, importing it to the new PDS (in a deactivated state), updating identity, and activating.
4. CONFIRMED. Handles are `alsoKnownAs` URIs verified via DNS TXT or HTTPS well-known routes; if resolution fails on the network, AppViews and standard APIs resolve it to the sentinel `handle.invalid` value.
5. CONFIRMED. Custom lexicons publish as `com.atproto.lexicon.schema` records and are discovered via the AT Protocol lexicon resolution mechanism by querying the `_lexicon.<domain>` DNS TXT record for the schema-holding DID.
6. CONFIRMED. Context explicitly states DD 57081857 and DD 54427650 implement strictly per-Character rotation-key material to preserve unlinkability. 
7. CONFIRMED. The PLC directory log is a public, append-only JSON line log; the `createdAt` timestamp of the genesis operation persists in the export log indefinitely, even after the DID undergoes a tombstone operation.
8. CONFIRMED. Context verifies the alpha ships no production AppView and only runs a local dev-loop reference PDS container, relying on third-party AppViews or Zurfur's own rudimentary Gallery view.
9. CONFIRMED. The Bluesky reference PDS enforces practical rate limits and operational constraints (especially concerning bulk CAR imports), and assigning a full PDS account per Character actively multiplies the hosting/operational surface.
10. CONFIRMED. Context verifies `net.got-paws.acp.relationship.ownership` is a custom domain schema; its bespoke pairing logic, edge ID nonce computation, and specific semantics for severed vs. non-checkable states require the vulpes specification to properly parse.
11. CONFIRMED. Context reveals a direct architectural conflict: DD 54427650 models co-ownership as multiple claim halves (one per owner), whereas vulpes strictly dictates that more than one edge per character results in a conflict state rather than valid co-ownership.
## Judge consolidation

# S0 — consolidated verdicts (judge: Claude; fork 003 is odd)

Seats: GPT (web search), Gemini (Google grounding). Where they split, the judge resolved with one primary-source fetch and records it here. **Corrected claims replace the originals for S1–S4; a claim may not be argued from in its original form.**

| # | Verdict | Resolution / corrected claim |
|---|---|---|
| 1 | **CONFIRMED** (both) | did:plc rotation keys are priority-ordered; within ~72 h a higher-authority key can nullify operations signed by a lower-authority key (did-plc spec v0.1). |
| 2 | **CONFIRMED** (both) | A PLC operation may carry an empty services map; `atproto_pds` can be added later by a regular update op without changing the DID. |
| 3 | **CONFIRMED** (both) | A PDS hosts many accounts, each with a repo + CAR export; migration = export → import on the new host (deactivated) → identity update → activate → deactivate old (atproto.com/guides/account-migration). |
| 4 | **CORRECTED** (GPT WRONG vs Gemini CONFIRMED → GPT's nuance upheld) | Handles are `alsoKnownAs` claims verified bidirectionally via DNS TXT or HTTPS well-known. The spec says: *"In API responses, the special handle value `handle.invalid` can be used to indicate that there is no bi-directionally valid handle for the given DID"* and that such a handle *"can not be used in most situations (search queries, API requests, etc)"*. **Display is not mandated** — AppViews commonly show it, but the claim's "rendered by AppViews as `handle.invalid`" is an observation about current clients, not a protocol guarantee. Source: atproto.com/specs/handle (fetched by the judge). |
| 5 | **CONFIRMED** (both) | Lexicons publish as `com.atproto.lexicon.schema` records; `app.zurfur.*` resolves via `_lexicon.zurfur.app` DNS TXT to the schema-holding DID. |
| 6 | **CONFIRMED** (both, context) | Per-Character rotation-key material, never per-Keeper, prevents correlation through the PLC log (57081857 D2 / 54427650). |
| 7 | **CONFIRMED** (both) | The PLC log is public and append-only; the genesis `createdAt` persists indefinitely, tombstone included. |
| 8 | **CORRECTED** (GPT UNVERIFIABLE vs Gemini CONFIRMED → split the claim) | First half CONFIRMED from context: the alpha ships no AppView and no production PDS (dev-loop container only). Second half REPLACED: a Character repo's **custom `app.zurfur.*` records render only in Zurfur's own views or in generic record browsers**; Bluesky-class AppViews render only the lexicons they implement (a standard `app.bsky.actor.profile` record in the Character's repo would render as a profile; the ref-sheet/art records would not). No third-party AppView rendering of Zurfur's custom records is evidenced. |
| 9 | **CORRECTED** (GPT WRONG vs Gemini CONFIRMED → GPT's arithmetic upheld and tightened) | Accounts hosted on Zurfur's PDS = **the number of public Characters only** — Users bring their own PDS (they sign in with existing DIDs) and Accounts are identity-only (26935298, unchanged), so no Keeper account is hosted; the "Keepers × Characters" product was wrong. The reference PDS publishes sizing guidance (README: ~1 GB RAM, 1 core, 20 GB SSD for 1–20 users) and rate limits, **no protocol hard limit** on accounts per instance. Source: github.com/bluesky-social/pds README (GPT). |
| 10 | **CONFIRMED** (both, context) | ACP/CCS ownership claims are custom `net.got-paws.*` records under a domain authority; not an atproto-standard mechanism; a third-party verifier needs the vulpes spec. |
| 11 | **CONFIRMED** (both, context) | DD 54427650 (priority-ordered keys + one claim per co-owner) and CCS (one in-force ownership edge per character; N humans behind an account-owned character) are different co-ownership models. |

Struck as argument material: nothing entirely; the original wording of 4, 8 and 9 may not be argued from.

---

# S1 — Attack (isolated)

## GPT

## Preliminary defect in the instruction

There is **no claim 13** in the brief or consolidated verdicts. I treat that reference as **UNVERIFIABLE**, likely intended to mean corrected claim 11. I use corrected claims 10 and 11 below; I do not invent a claim 13.

## A — Own DID + own full PDS repo

**Strongest case AGAINST**

This bundles three separable needs—network identity, ownership assertion, and content hosting—into one expensive account lifecycle. For alpha, it requires production PDS operations, account creation/migration/export/teardown, unpublished character lexicons, and application code that does not exist. The repo also does not buy broad rendering: custom `app.zurfur.*` records appear only in Zurfur-aware views or generic record browsers.

Worse, its ownership design is internally inconsistent. ACP/CCS is custom rather than an ATProto standard, so portability depends on third parties implementing vulpes semantics. DD 54427650’s multiple co-owner edges directly contradict CCS’s one-in-force-edge rule. “Atproto-native ownership” therefore overstates both interoperability and specification coherence.

**Strongest case FOR**

A repo gives the Character an autonomous publication and consent boundary. It can make its own half of a symmetrical claim, publish records under its DID, migrate between hosts, and survive Zurfur’s disappearance if exports and user-held rotation keys actually work. Of the listed options, A most strongly satisfies exit by construction rather than by promise.

**Concrete failure scenario**

Two Keepers co-own Character C. Zurfur writes and character-attests two ownership edges. A conforming CCS verifier sees multiple in-force edges and marks C conflicted rather than co-owned. Zurfur displays “two owners”; an external verifier displays “invalid ownership state” → a transfer or commission authorization proceeds under contradictory ownership results.

---

## B — Own DID, no repo

**Strongest case AGAINST**

This makes “public Character” mostly a Zurfur database concept. If Zurfur disappears, the DID remains but its profile, references, art records, and character-side ownership assertion disappear. Under the supplied CCS rules, a claim belongs in the claimant’s own repo; without a Character repo, genuine symmetrical ownership cannot be represented as specified. Calling a Keeper-side record plus an Index fact “network ownership” would be misleading.

**Strongest case FOR**

It is the honest alpha shape. DID-only identity is supported, adding a PDS later preserves identity, and it avoids introducing an unimplemented production hosting system into an already cut-down alpha. It also avoids pretending custom records have ecosystem reach they do not have. Security work can focus on PLC keys and existing commission functionality.

**Concrete failure scenario**

C is public with its ref sheet and ownership state only in the Index. Zurfur suffers permanent data loss. C’s DID remains in PLC, but external commission records resolve to no profile or references and no character-side claim can be checked → nominal identity persistence produces practical identity loss.

---

## C — No own DID; records in Keeper’s repo

**Strongest case AGAINST**

This violates the settled identifier model and collapses Character identity into current custody. Transfer changes repository authority and record URIs, making provenance, backlinks, blocks, tags, and historical commission references harder to preserve. It also lets a departing Keeper retain or alter the old apparent canonical representation. Reversing built DID work is unjustified merely to avoid hosting.

**Strongest case FOR**

It follows ATProto’s normal account/repository economy: people hold accounts and publish records about things. No separate account lifecycle, recovery key hierarchy, or PDS slot is required for each fictional entity. Existing Keeper hosting provides immediate network publication.

**Concrete failure scenario**

Keeper K1 transfers C to K2. K2 republishes C in a new repo, but K1 leaves the old records online and labels them canonical. Existing links continue resolving to K1 while new links resolve to K2 → the network has two representations with no stable Character subject capable of authoritatively choosing between them.

---

## D — B now, A later

**Strongest case AGAINST**

“Later” hides the hardest product migration. Alpha users will establish Index URLs, ownership semantics, privacy expectations, and deletion behavior that may not map cleanly to repos. It also allows “Character public presence” to ship under one architecture and silently acquire a materially different permanence and exposure model later.

**Strongest case FOR**

This is the strongest listed alpha decision. It preserves the already-minted DID, keeps the upgrade path open, and delays PDS operations until the ownership model, lexicons, recovery custody, exports, and rendering contract are coherent. Unlike A, it does not put speculative infrastructure ahead of minimum artist/client workflow.

**Concrete failure scenario**

C is publicized during alpha and shares stable Index links. Later, A is introduced and records receive AT URIs while old Index identifiers remain embedded in commissions and portfolios. No canonical mapping or export contract was reserved → duplicate records appear and clients disagree over which version is authoritative.

---

## Missing option

**E — Own DID + minimal claim/profile repo.** At publicize, create a Character repo containing only the Character’s CCS half, a minimal profile, and signed pointers/manifests; keep bulky art, commission records, and blobs elsewhere until independent portability justifies moving them.

This exposes the brief’s false choice between **full account-shaped content hosting** and **no repo**. A minimal repo could supply the one capability B fundamentally lacks—the Character speaking for itself—without dragging the entire Gallery/content model and blob-hosting surface into alpha.

However, E is still blocked until the co-ownership contradiction is resolved. No repository topology can make DD 54427650’s multiple ownership edges compatible with CCS’s one-edge rule.
## Gemini

**Option A: Own DID + own full PDS repo**
* **(a) Strongest case AGAINST:** Violates the "alpha cut to spine" doctrine and ships a broken ownership model. Zurfur currently has no production PDS (Claim 8). Forcing the deployment and operational scaling of a PDS for all public Characters (Claim 9) distracts from critical commission workflows. Worse, the brief's co-ownership model (multiple ownership claims per character) explicitly violates the vulpes CCS specification, which mandates exactly one in-force ownership edge per character (Claim 11).
* **(b) Strongest case FOR:** Purest adherence to the anti-domination floor. By providing a fully populated PDS repo and a CAR export (Claim 3), it guarantees that if Zurfur vanishes, a Character’s identity and data survive. Even if custom `app.zurfur.*` records only render in generic browsers (Claim 8), the cryptographic continuity and data ownership are undeniable.
* **(c) Failure scenario:** Alice and Bob co-own a public Character. Zurfur writes two `net.got-paws.acp.relationship.ownership` records into the Character's repo following DD 54427650. A third-party AppView, using the vulpes docs to interpret the custom records (Claim 10), encounters the multiple claims, flags a conflict state (Claim 11), and strips the Character of its owned status on the wider network.

**Option B: Own DID, no repo**
* **(a) Strongest case AGAINST:** Categorically fails the anti-domination doctrine. By locking all Character content (ref sheets, art, bio) in Zurfur's private PostgreSQL Index, a Keeper's right to exit is a mirage. If Zurfur perishes, the user is left with a handle-less DID that has an empty services map (Claim 2) and zero verifiable creative content.
* **(b) Strongest case FOR:** Strictly honors the "alpha cut to spine" directive. It completely removes the need to deploy and scale a production PDS for the alpha (Claim 8), slashing operational surface and allowing the team to focus strictly on artist-client commission features and security.
* **(c) Failure scenario:** Zurfur unexpectedly shuts down. A Keeper attempts to migrate their Character to another ATProto platform. Because the identity was minted with no `atproto_pds` and no repo, there is no CAR export to recover. The DID's genesis timestamp (Claim 7) survives, but the character's art and history are permanently destroyed. 

**Option C: No own DID; records in the Keeper's repo**
* **(a) Strongest case AGAINST:** Destroys privacy and unlinkability by construction. Cohabiting Character records within the Keeper's personal repo permanently correlates all of a Keeper's sonas/OCs together in the public, append-only PLC log (Claim 7), explicitly breaking the per-Character key rotation mandate (Claim 6). 
* **(b) Strongest case FOR:** Eliminates the necessity of Character DIDs entirely. It massively simplifies the architecture by piggybacking on the Keeper's existing DID and PDS, dodging the necessity of standing up a Zurfur-hosted PDS and negating the N-public-characters scaling footprint (Claim 9).
* **(c) Failure scenario:** A Keeper sells their Character to a buyer. Because the Character is merely a record set rather than a DID, "transfer" requires deleting the records from the seller's repo and re-publishing them in the buyer's repo. All external AT-URIs pointing to the original Character immediately 404, and the cryptographic provenance of the Character's creation is irrevocably severed.

**Option D: B now, A later**
* **(a) Strongest case AGAINST:** Postpones the structural failure rather than fixing it. It delays the deployment of the PDS but retains the fatally flawed DD 54427650 co-ownership model. When Phase A finally ships, it will immediately crash into the vulpes CCS single-edge limitation (Claim 11). Furthermore, it denies alpha users any genuine exit path, violating anti-domination for early adopters.
* **(b) Strongest case FOR:** A pragmatic compromise between operational reality and protocol idealism. It ships the core commission platform immediately ("alpha cut to spine") while securing the handle-less DIDs and per-Character keys (Claim 6), laying the identity foundation to fulfill the anti-domination floor post-alpha.
* **(c) Failure scenario:** The alpha gains traction. A Keeper attempts to leave Zurfur with their public Character due to a platform dispute. Finding themselves in the "B now" phase, they discover their Character has no PDS repo, no CAR export, and no data portability. They are held hostage by Zurfur's Index, effectively proving the platform's anti-domination guarantee is vaporware.

### Missing Options (Attack on Framing)

The brief's framing is fundamentally broken because it conflates *content hosting* (heavy blob storage for art) with *cryptographic ownership* (ACP claims), and entirely ignores the irresolvable conflict between DD 54427650 and the vulpes CCS rule (Claim 11). The option space is incomplete. 

* **Option E: Own DID, Claims-Only Repo.** Public Characters get a Zurfur-hosted PDS repo, but it ONLY contains `net.got-paws.acp.relationship.ownership` claims and a standard `app.bsky.actor.profile` stub. Heavy media (ref sheets, custom lexicons) remains Index-only for the alpha. This satisfies external vulpes verifiers and grants ATProto identity presence without forcing a high-volume blob storage deployment.
* **Option F: Own DID + Repo, Vulpes-Compliant Account Ownership.** Characters get a repo, but the co-ownership model is rewritten to obey Claim 11: Zurfur mints an Account DID that owns the Character DID (one in-force edge), and the N human Keepers co-own the Account DID.
---

# Board-added options (from S1, given to both seats for S3)

Board-added options from S1 (both seats independently proposed E; F is Gemini's):

- **E — Own DID + a minimal, claims-only repo.** At publicize the Character gets a Zurfur-hosted repo holding ONLY: its CCS ownership half (`net.got-paws.acp.relationship.ownership`, role `owned`), a standard `app.bsky.actor.profile` stub, and (GPT's variant) signed pointers/manifests to content held elsewhere. Ref sheets, art records, custom `app.zurfur.*` lexicons and blobs stay Index-only for the alpha; they move into the repo later only if independent portability justifies it. Rationale: it is the one capability B fundamentally lacks — the Character speaking for itself — without dragging the Gallery/content model and blob hosting into the alpha.
- **F — Own DID + repo, vulpes-compliant ownership via an Account.** The Character's single in-force ownership edge points at an Account DID; the N human Keepers co-own the Account. Satisfies CCS's one-edge rule (claim 11) instead of fighting it; co-ownership becomes an Account-membership concern on Zurfur's side.

Both seats state that no repo topology resolves the co-ownership contradiction (claim 11) by itself; E and F differ in whether the contradiction is deferred (E) or dissolved by re-pointing the edge (F).

---

# S2 — Rebuttal (one round; each seat saw the other's S1)

## GPT

## Rebuttal

### 1. **“Option A guarantees that identity and data survive” — false**

A repo plus a theoretical CAR-export policy does not **guarantee** survival. The brief says:

- no production PDS exists;
- no backend path creates Character accounts;
- no Character lexicons are published;
- routine CAR export/mirroring is a requirement in a design document, not landed behavior.

More importantly, repo CAR migration and blob migration are separate. Official migration guidance says blobs must be copied individually; if the old PDS, a mirror, and local backups are unavailable, old content is missing. A signed repo does not magically preserve its referenced artwork. ([atproto.com](https://atproto.com/specs/repository?utm_source=openai))

Thus “cryptographic continuity” is plausible; “data ownership is undeniable” is rhetoric. **Current implementation: UNVERIFIABLE as a survival mechanism, and contradicted by the codebase facts as an alpha-ready mechanism.**

### 2. **Option A’s AppView failure scenario is invented**

The CCS conflict is real **per the relayed vulpes rules**, and I concede that DD 54427650’s multiple simultaneous ownership edges conflict with its one-edge rule.

But:

> “A third-party AppView … flags a conflict state and strips the Character of its owned status”

is **UNVERIFIABLE**. There is no atproto-standard “owned status,” and Claim 10 says third parties must deliberately implement vulpes semantics. A generic AppView may ignore the custom records entirely. The defensible failure is: *a vulpes-compatible verifier reports conflicting ownership edges*. “Strips status on the wider network” invents both behavior and network consensus.

### 3. **Option C does not correlate Characters “through the PLC log”**

This is the clearest technical error. Under C, Characters have **no DIDs**, so there are no per-Character PLC operations to correlate. Character records would correlate through inspection of the Keeper’s **public repo**, not through the PLC log.

Claim 6 also cannot prove C “breaks the per-Character key rotation mandate”: C explicitly reverses the decision requiring per-Character identities and therefore has no per-Character rotation keys to reuse. C damages unlinkability, but the seat identified the wrong mechanism.

### 4. **Option C’s transfer failure is overstated**

“All external AT-URIs immediately 404” follows only if the old records are deleted. C explicitly permits “provenance by reference”; the seller could retain a transferred/tombstoned record pointing to the successor.

AT-URIs are not content-addressed strong references and records can be removed or changed, so C unquestionably has weak continuity. But provenance is not “irrevocably severed” if the design carries old CIDs, signatures, and predecessor references forward. Conversely, merely retaining an old AT-URI is not sufficient proof. ([atproto.com](https://atproto.com/specs/repository?utm_source=openai))

The correct criticism is **identity continuity becomes an application convention rather than DID-level continuity**, not inevitable total provenance loss.

### 5. **“B categorically fails anti-domination” — CONCEDED, but only for B as written**

B says all Character content lives in Zurfur’s Index with no independent hosting. DD 54427650 itself admits that private Index-only content dies with Zurfur. Under the doctrine that Zurfur disappearing must never be a breaking factor, B fails unless supplemented by portable signed exports, user-controlled mirrors, or equivalent escape infrastructure.

I also concede the analogous criticism of D during its B phase. “We will provide exit later” is not exit for alpha users.

### 6. **Option E does not remove the hard operational problem**

Claims-only repos reduce blob storage, but they do **not** eliminate:

- production PDS deployment;
- account creation and lifecycle;
- authentication/key custody;
- moderation and abuse controls;
- backups and migrations;
- relay signup/rate-limit coordination;
- one account per public Character.

Official production guidance explicitly identifies extra hardening, storage, Redis/horizontal-scaling, moderation, and relay signup constraints for multi-user hosting. ([atproto.com](https://atproto.com/guides/going-to-production?utm_source=openai))

Adding an `app.bsky.actor.profile` stub is also unjustified. The Character is deliberately handle-less, and Bluesky compatibility is not the same as ATProto-native interoperability. E may be a useful **claims-only scope reduction**, but it is not the cheap halfway house presented.

### 7. **Option F relocates rather than solves co-ownership**

I concede that one Account→Character edge matches the relayed vulpes single-edge model better than DD 54427650.

But “N human Keepers co-own the Account DID” is not specified by CCS in the supplied context. That ownership must still be represented through keys or some separate product model. F therefore moves the unresolved co-ownership semantics from Character to Account; it does not solve them. Whether existing Zurfur Accounts can safely serve this role is **UNVERIFIABLE** from the brief.

## Explicit concessions

- A offers the strongest potential protocol-level exit **if** account provisioning, user-held recovery, blob backup, and tested migration actually land.
- DD 54427650’s co-ownership model conflicts with the relayed vulpes one-edge CCS rule.
- B and D-as-B violate anti-domination as currently defined.
- C weakens stable identity and transfer continuity.
- The option space should separate claims publication from heavy Character media hosting.
## Gemini

Here is the S2 Rebuttal.

### CONCESSIONS

I explicitly concede the following points from the other seat’s S1:
1. **The co-ownership contradiction is real.** The other seat correctly identifies that DD 54427650’s model of multiple `ownership` claims (one per co-owner) directly violates the vulpes CCS rule of "one in-force ownership edge per character" (Context: vulpes CCS, Enum Claim 11). 
2. **Option C is a non-starter.** The other seat correctly notes that collapsing Character identity into the Keeper's repo violates the settled DID model (DD 57081857) and breaks provenance upon transfer.
3. **Option A is heavy.** The other seat accurately highlights that shipping Option A in the alpha introduces massive operational complexity (production PDS lifecycle, CAR exports, undocumented application code).

### REBUTTALS

#### Attack 1: The "Minimal Repo" Fallacy (Option E is a delusion)
**The Claim:** The other seat proposes a missing "Option E," arguing that spinning up a "minimal claim/profile repo" sidesteps the "full account-shaped content hosting" of Option A, providing network ownership without dragging the "blob-hosting surface into alpha." 

**Why it is wrong:** This demonstrates a total misunderstanding of what makes PDS infrastructure expensive. The engineering and operational burden of Option A is *not* disk space or blob hosting. The burden is the **account lifecycle**: PDS instantiation, envelope-encrypted key custody (Layout D), PLC update operations, OAuth flows, and CAR exports (Context: Codebase facts show "no production PDS today" and "nothing in the backend creates PDS accounts"). 

A PDS account is a PDS account. Enum Claim 9 explicitly states: *"Hosting cost and operational surface scale with (Keepers × public Characters) accounts on Zurfur's PDS."* Option E requires 100% of the PDS lifecycle infrastructure that Option A requires, just to host a 100-byte claim instead of a 2MB art blob. It imposes the exact same architectural blockers on the alpha while artificially restricting the payload. Option E is just Option A with less user value. 

#### Attack 2: Championing Option D Violates the Doctrine Floor
**The Claim:** The other seat declares Option D (DID-only now, repo later) the "strongest listed alpha decision" because it preserves the upgrade path and avoids speculative infrastructure. 

**Why it is wrong:** Option D violates the Doctrine Floor and is therefore disqualified, not discounted. 

The Doctrine Floor strictly demands: *"Anti-domination — exit and voice preserved by construction... Zurfur vanishing must be an inconvenience, never a breaking factor."* Under Option D (DID without a repo), all public character content (art, bio, references) lives exclusively in Zurfur's Index. If Zurfur vanishes during the alpha, the Character's DID survives but is an "empty husk" (DD 54427650 Kill tests), and the content is permanently lost. 

The other seat explicitly notes this exact failure mode in its attack on Option B, yet inexplicably blesses Option D for the alpha. You cannot delay "exit by construction" until post-alpha. If the alpha accepts real users, it must apply the doctrine floor. If the team cannot ship PDS repos in the alpha, they cannot ship public Characters in the alpha. 

#### Attack 3: Conflating Symmetrical Ownership with Network Publication
**The Claim:** In attacking Option B, the other seat argues that *"without a Character repo, genuine symmetrical ownership cannot be represented as specified."*

**Why it is wrong:** It is entirely possible to represent symmetrical ownership without a Character repo in the alpha. The other seat conflates the *concept* of symmetrical ownership with the *ATProto network transport* of that ownership. 

The brief explicitly leaves the medium of symmetrical claims unresolved for the alpha. Character page 5668866 states: *"Open — not a decision: the record shape (Index rows vs atproto records in each repo)."* Zurfur can enforce a symmetrical double-sided claim (Keeper claims Character, Character claims Keeper) purely using Zurfur-side database Index rows, fulfilling the domain logic without requiring a PDS record. Stating it "cannot be represented" is textually false; it just cannot be verified by third-party network actors until the PDS is shipped.
---

# S3 — Independent ranking (A–F)

## GPT

**None is alpha-ready unchanged; this ranks architectural shape, not implementation readiness.**

1. **F — Own DID + repo, ownership through an Account.** Best of a flawed set because it preserves portable Character data and a Character-authored claim while obeying CCS’s single-edge rule; however, Account governance, Keeper exit, and key control remain **UNVERIFIABLE** and must be specified before shipping.

2. **A — Own DID + full repo.** Strongest direct anti-domination design—stable identity, portable publication, no intermediary owner—but DD 54427650’s multi-owner claims knowingly produce a CCS conflict, and the production PDS, backup, blob migration, and account lifecycle do not yet exist.

3. **E — Own DID + claims-only repo.** Correctly separates Character speech from heavy content hosting and limits premature lexicon work, but it still incurs nearly the full per-account PDS security burden while leaving the actual creative identity trapped in Zurfur’s Index; omit the unjustified Bluesky profile stub.

4. **D — DID-only now, full repo later.** Operationally honest and reversible, but “exit later” violates the doctrine for alpha users, and the deferred A destination remains incoherent until ownership, custody, export, and migration semantics are repaired.

5. **B — Own DID, no repo.** Simplest implementable alpha shape, but the surviving DID is an empty husk if Zurfur disappears: neither Character-authored consent nor independently recoverable profile/content exists, so anti-domination fails outright.

6. **C — Records in Keeper’s repo, no Character DID.** Worst because it reverses built identity architecture and turns transfer continuity into an application convention; it also publicly correlates Characters through the Keeper’s repo and leaves competing old/new “canonical” records unresolved.

**Evidence that would flip my #1:** an authoritative vulpes specification plus interoperability test demonstrating that multiple simultaneous human→Character ownership edges are valid co-ownership—not a CCS conflict—would move **A above F**, because direct Keeper ownership is preferable to inserting an Account intermediary once both models are equally protocol-valid.
## Gemini

1. **Option F (Own DID + Repo, Vulpes-Compliant Account Ownership)**: The only proposal that fulfills the anti-domination floor by guaranteeing CAR-exportable exit for content, while structurally resolving the fatal single-edge CCS contradiction (Claim 11).
2. **Option A (Own DID + own full PDS repo)**: Secures the anti-domination floor via genuine network exit, but guarantees invalid network states because its multiple-claim co-ownership model explicitly breaks vulpes CCS rules.
3. **Option D (B now, A later)**: Disqualified for violating anti-domination during the alpha, though its sequential approach at least preserves the identity architecture without correlating users or minting invalid claims.
4. **Option E (Own DID + minimal claims-only repo)**: Disqualified for failing anti-domination (art remains trapped in the Index) while inexplicably still paying the full operational PDS lifecycle tax (Claim 9) for zero content portability.
5. **Option B (Own DID, no repo)**: Disqualified for flagrantly violating the anti-domination doctrine by locking all creative identity and content hostage inside Zurfur's private, non-exportable Index.
6. **Option C (No own DID; records in the Keeper's repo)**: Disqualified for irrevocably destroying user privacy by permanently grouping all of a Keeper's characters together on the public, append-only PLC log (Claim 7).

To flip my #1 ranking away from Option F, I would need codebase evidence proving that a Keeper can generate a valid, PDS-importable CAR export entirely client-side using their locally cached commission data and their client-held `user_cold` rotation key (Layout D). Because the Doctrine Floor dictates that "the non-toxic path outranks optimization," Option F currently wins by forcing the expensive PDS infrastructure into the alpha solely to guarantee "exit by construction." If client-side CAR generation is verifiably supported in the codebase today, it would prove that cryptographic exit does not require a Zurfur-hosted PDS in the alpha. That single piece of evidence would instantly elevate an Account-ownership variant of Option D (DID-only now, preserving the "alpha cut to spine" mandate without violating the anti-domination floor) to the #1 spot.
---

# S4 — judge reveal (judge: Claude; fork 003 is odd)

<sealed>
Engineer's ranking (given 2026-09-12, before any seat ran): **A > D > B > C.** Stated lean: "I lean A myself." No per-rank reasons given.
</sealed>

Board rankings (S3, independent, over the brief's A–D plus board-added E and F):

| Rank | GPT | Gemini |
|---|---|---|
| 1 | **F** | **F** |
| 2 | A | A |
| 3 | E | D |
| 4 | D | E |
| 5 | B | B |
| 6 | C | C |

## 1. Delta

**On the four options the Engineer ranked, the board agrees with the sealed ranking unanimously and exactly: A > D > B > C.** No seat moved any of the Engineer's options relative to each other, and the ordering survived S2 (GPT had called D "the strongest listed alpha decision" in S1, then conceded in S2 that D-as-B fails anti-domination for alpha users and dropped it below A in S3 — which is where the Engineer already had it).

**The entire divergence is additive: both seats rank a board-added option, F, above A.** The Engineer never considered E or F.

- **F — own DID + repo, the Character's single in-force ownership edge points at an Account DID; humans co-own the Account.** GPT #1, Gemini #1. Evidence driving it: S0 claim 11 CONFIRMED (DD 54427650's "one `ownership` claim per co-owner" vs CCS's "one in-force ownership edge per character — more is a conflict state, never co-ownership"), plus both S1s and both S2s conceding the contradiction is real and that no repo topology resolves it by itself. F is ranked first *only* because it re-points the edge so the contradiction dissolves; on every other axis both seats treat F as A. GPT's own flip condition says so: "an authoritative vulpes specification plus interoperability test demonstrating that multiple simultaneous human→Character ownership edges are valid co-ownership … would move A above F."
- **E — own DID + a claims-only repo (CCS half + profile stub; content Index-only).** GPT #3, Gemini #4. Both seats proposed it in S1 as the cheap middle; both demoted it in S2/S3 after agreeing the PDS cost is the *account lifecycle*, not blob storage (S0 claim 9 as corrected; GPT S2 §6, Gemini S2 attack 1). GPT keeps it above D for the one thing B/D lack — the Character speaking for itself on the network; Gemini calls it "Option A with less user value."

**On the lean ("I lean A myself"):** the board agrees A is the best of the listed shapes and the strongest direct anti-domination design, but both seats say A *as written in DD 54427650* ships an invalid network state under CCS as relayed — a vulpes-compatible verifier reports conflicting ownership edges the moment a Character has two co-owners. GPT struck Gemini's stronger version of that failure ("a third-party AppView strips the Character of its owned status") as invented; the defensible failure is the narrower one. Both also say A is not alpha-ready today on the codebase facts (no production PDS, no account-creation path, no CAR export landed, no lexicons published) — a cost the Engineer already accepted when pulling it forward, not a ranking argument.

**The judge's reading of what the delta actually turns on.** Claim 11 was verified against the *context block* only (S0: "CONFIRMED (both, context)") — the CCS rules were relayed by the vulpes session, not fetched, and the brief's Framing paragraph carries the Engineer's own gloss that "co-ownership is a Zurfur-side (product/keys) concern, not something the claims layer handles." That gloss and the DD text ("one `ownership` claim per co-owner, each attested by the character") are in tension *inside the Engineer's own record*. If the on-network claim names **one** owner (the holder of the senior rotation key) and co-ownership lives in keys and the Index only, then claim 11 does not bite A at all, and F's sole advantage over A disappears — the board's #1 collapses back into the Engineer's #1. If the DD text stands (one edge per co-owner), the board's objection to A stands with it. That is a ruling, not a finding; it is the first item in §4.

## 2. Divergence map (seat vs seat)

| Where they split | GPT | Gemini | Evidence | Held through S2? |
|---|---|---|---|---|
| **D — DID-only now, repo later** | Discounted: "operationally honest and reversible" but exit-later violates the doctrine for alpha users (#4) | Disqualified outright by the doctrine floor; "if the team cannot ship PDS repos in the alpha, they cannot ship public Characters in the alpha" (#3) | Doctrine floor sentence "Zurfur vanishing must be an inconvenience, never a breaking factor"; DD 54427650 kill test ("empty husk") | **Narrowed.** GPT moved from "strongest listed alpha decision" (S1) to conceding D-as-B fails anti-domination (S2). Residual split is disqualified-vs-discounted, and Gemini still ranks D above E. |
| **E — claims-only repo** | Kept above D: the Character-authored claim is the one capability B/D lack; drop the `app.bsky.actor.profile` stub (#3) | Disqualified: pays the full PDS lifecycle tax for zero content portability (#4) | S0 claim 9 corrected (cost = accounts, not blobs); Gemini S2 attack 1; GPT S2 §6 concedes E "is not the cheap halfway house presented" | **Narrowed** on cost (both now agree), **held** on whether a Character-authored claim alone is worth an account. |
| **Why C is last** | Reverses built identity architecture (57081857); transfer continuity becomes an application convention; correlation is through the **Keeper's public repo** | "Irrevocably destroying user privacy by permanently grouping all of a Keeper's characters together on the public, append-only **PLC log**" | GPT S2 §3 showed the PLC-log mechanism is impossible under C (no per-Character DIDs, so no per-Character PLC operations) | **Held — Gemini repeated the struck mechanism in S3.** The judge upholds GPT's correction: C's privacy cost is real but flows through the Keeper's repo, not the PLC log. C stays last on GPT's grounds. |
| **What A's failure is** | "A vulpes-compatible verifier reports conflicting ownership edges" — narrow | "Third-party AppView … strips the Character of its owned status" — broad | S0 claim 10 (third parties must implement vulpes semantics deliberately); GPT S2 §2 | **Held.** Gemini did not withdraw the broad version; the judge treats only the narrow version as established. |
| **Evidence that would flip the #1** | Authoritative vulpes spec + interop test that multi-edge co-ownership is valid → A over F | Codebase proof of *client-side* PDS-importable CAR export from cached data + the client-held `user_cold` key → an Account-ownership variant of D to #1 | — | n/a (S3 only). Judge note on Gemini's condition: as stated it names the *rotation* key; repo commits are signed by the `atproto` verification key, a different key, so the condition is not checkable in that form. |

## 3. Consensus check — with the red-team pass

Fast unanimity is a warning sign; each item below was attacked once before being reported.

**(a) F is #1 — both seats, independently, with near-identical reasoning.**
Red team: **this consensus does not survive the design corpus the brief omitted.**
- CCS as relayed says a claim lives in the *subject's own repo*. Under F the `owner` half is written by an **Account DID**. Accounts in the alpha are identity-only — no repo (S0 claim 9, corrected: "Accounts are identity-only (26935298, unchanged), so no Keeper account is hosted"). F's owner half therefore has nowhere to live unless Accounts also get Zurfur-hosted repos, which reintroduces the hosted-account scaling the S0 correction removed. Neither seat noticed; both cited the S0 correction for other purposes.
- Characters are **User-anchored**; a User need not have an Account at all (DD 26247170 "User as Actor & On-Demand Accounts" — not in the brief). F forces an Account into every public Character's ownership path and moves Keeper = User (54427650, 5668866) to Keeper = Account member.
- GPT itself flagged the hole in S2 §7: "N human Keepers co-own the Account DID is not specified by CCS in the supplied context … F relocates rather than solves co-ownership."
Verdict: **not reportable as consensus.** F is a genuine gap in the option space, but as specified by the board it conflicts with two settled decisions and with the board's own S0 record. What survives is the *question* F raises (§4, item 1), not F.

**(b) B and D, as written, fail anti-domination for alpha users.** Both seats; GPT conceded it in S2.
Red team: the argument proves too much. By the same reading the alpha already fails the floor: Accounts are identity-only (26935298, unchanged), and commissions are Class B, Index-canonical by design (29622283) — their private content dies with Zurfur, deliberately. The consensus holds **only if** public Character content is Class A, which is exactly what DD 54427650 asserts ("the Class A/B boundary applied literally to *content*"). So the consensus is consistent with the record but rests on a classification the Engineer can revisit for the alpha. **Survives, conditionally** — condition stated in §4, item 3.

**(c) C is last.** Both seats.
Red team: Gemini's mechanism is wrong (see divergence map); GPT's is sound (reverses DD 57081857, which is decided *and built*; transfer continuity becomes application convention; Keeper-repo correlation). **Survives** on GPT's grounds.

**(d) The cost of a Character repo is the account lifecycle, not storage — so E is not cheap.** Both seats by S3.
Red team: the reference PDS sizing (S0 claim 9) is per-instance, and the alpha's hosted-account count is "public Characters only." A claims-only repo still needs creation, key custody, PLC update at publicize, backup/migration, moderation and relay coordination per account — the seats' list is drawn from the atproto going-to-production guide (GPT S2 §6, sourced). **Survives.**

**(e) A is not alpha-ready today and DD 54427650's "kill tests" describe intended, not landed, behaviour.** Both seats.
Red team: true on the codebase facts and conceded by nobody's opponent; but this is a cost the Engineer priced when pulling the PDS half into the alpha on 2026-09-12. **Survives as a cost statement**, carries no ranking weight against the sealed order.

**(f) The co-ownership contradiction (claim 11) is real and no repo topology resolves it by itself.** Both seats, S1 through S3.
Red team: verified against relayed text only; and the Engineer's own framing sentence in the brief says the claims layer is not the co-ownership mechanism. If that sentence governs, the "contradiction" is between two sentences in the Engineer's record, not between Zurfur and vulpes. **Survives as a documented tension; not survives as "A is broken."** Ruling needed (§4, item 1).

## 4. Forks for ruling (stated, never resolved)

1. **What does the on-network ownership claim say when a Character has two Keepers?** (i) One in-force edge naming the senior-key holder; co-ownership in rotation keys + Index only (the Engineer's 2026-09-12 gloss) — amend DD 54427650's "one `ownership` claim per co-owner" and claim 11 stops biting A; or (ii) one claim per co-owner (the DD text) — accept that a CCS verifier reports a conflict state, or ask vulpes to admit multi-edge co-ownership; or (iii) an Account-owned Character (F) — which requires ruling item 2 first. This fork decides whether the board's #1 differs from the Engineer's #1 in substance.
2. **Do Accounts get repos?** F cannot exist without an Account `owner` half living somewhere; 26935298 says Accounts are identity-only and the S0 correction to claim 9 relied on it. If yes: hosted accounts = public Characters + Accounts. If no: F is off the table as specified.
3. **Is "exit by construction" binding on alpha-stage public Character content?** Yes ⇒ B and D are out for the alpha, as both seats say. No (the 26935298 identity-only posture extends to Characters for the alpha, as it does to Accounts) ⇒ D is a legitimate alpha shape and the sealed A > D gap is a scope choice, not a doctrine one.
4. **Does E survive as a scope, not a shape?** Even if A is the destination, the *first* repo content could be the CCS half alone (GPT's E without the profile stub), with ref sheets/art records following once lexicons publish (29818896 publish-late). Ruling: is that sequencing acceptable inside A, or must publicize create the full record set?
5. **Which failure is A's, for the security-review threat model?** Narrow (a vulpes-compatible verifier reports conflicting edges) or broad (any third party can strip owned status). The board established only the narrow one.
6. **Gemini's flip condition, restated correctly:** can a Keeper produce a PDS-importable CAR for their Character *without* Zurfur — i.e. who holds the Character's `atproto` signing key (not the rotation key) under layout D? This is the custody open item on DD 54427650 (layout D vs 26804226 custodial-by-default) and it decides whether "migratable" is a property of the design or of Zurfur's cooperation.
7. **DD 26935298's title and D1 ("Identity-Only for v1")** are now false for public Characters and need a DID update op at publicize (S0 claim 2). Edit, supersede-in-part, or leave with a banner.
8. **Second sitting:** the repo survived this one on the board's own ranking (A/F above every no-repo shape). Convene "do Character handles follow the repo into the alpha?" as a DEBATE, or defer.

The judge resolves nothing above and makes no recommendation.

---

# Token spend

| Seat | Tokens (cumulative, all phases) |
|---|---|
| gpt | 105,814 |
| gemini | 50,380 |
| Claude (judge) | this session — not metered separately |
