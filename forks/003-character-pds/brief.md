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
