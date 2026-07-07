# Confluence DESIGN — local page index

Canonical design lives in the **zurnetwork** Confluence, space **DESIGN** — the single source of truth for Zurfur design decisions. This file is a local *pointer index*, NOT a copy of the content: when a topic below sounds familiar, **fetch the real page before asking or asserting from memory.**

## Fetch coordinates

- cloudId: `cafe5eef-9c51-4800-85df-ef42187f9414`
- DESIGN space id: `98310`
- Fetch a page: `getConfluencePage` with the `pageId` below. Web URL: `https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/{id}`
- New DD pages should be created via `/design-decision` (writes the Confluence page + Jira tickets). Index snapshot built 2026-06-30; incrementally updated 2026-07-02 (added DD 27852802, relabeled 24870914) and 2026-07-04 (ZMVP-104: added the AT Protocol Boundary Contract programme — 29622283 / 29687820 / 29949954 / 29982722 / 29622362 / 29622321 / 29818896 — and the Replyable DD 30572573) and 2026-07-06 (PR #100 review round: Eventlog DD 32178178 + Linked Channel DD 32112642; Changelog DD 30408741 amended [narrowed to explicit facts], External Chat Tracking 6848513 placement row decided [plugin], Plugin 3047451 production doctrine re-pointed to eventlog source streams) — a full `getPagesInConfluenceSpace` re-list of space `98310` is due at the next larger sweep to catch anything else since 2026-06-30.

## Settled-decision quick facts

Know these without fetching; fetch the linked DD page for detail.

- `did:plc` everywhere · Character has its own DID day one · ratings anchored to commission membership (2-dim Creator/Commissioner, 1–10★ positive-only) · fact-anchored deletion · payments = pre-alpha manual mark-as-paid.

## Page index

### Meta / structure
- `98422` — Design (space homepage)
- `2490388` — Design decision (DD page template/convention)
- `10125333` — The Index
- `786450` — Project Philosophy
- `9895947` — Product
- `589826` — Project MVP (current MVP doc)
- `3670017` — MVP & Roadmap — SUPERSEDED, merged → Project MVP (`589826`); known to be in flux/contradictory, don't treat as ground truth
- `9994307` — Blocking Gaps for v1

### Core entities / glossary
- `786439` — User
- `1966081` — Account
- `2162692` — Roles (Owner/Admin/Manager/Member hierarchy)
- `3276807` — Commission
- `5668866` — Character
- `5931025` — Slots
- `8912899` — Collections
- `9961473` — Batch
- `8978433` — Portfolio
- `8978492` — Post
- `10190849` — Gallery
- `9895957` — Workflow
- `2949165` — Tags
- `3047451` — Plugin
- `3244063` — First-party plugins
- `1933322` — Achievement
- `10453000` — Lens

### Design decisions (DDs)
- `4358151` — DID:PLC vs DID:Web
- `4882433` — Character Ownership Model
- `2490393` — User Reviews & Comments
- `3014657` — Deletion of Commissions
- `6422530` — Payments & Billing Model
- `6848513` — External Chat Tracking (core vs plugin)
- `21594113` — User-Profiles, the Handle Swap & Content Maturity (§6 emergent-type & §7 first-account-on-login SUPERSEDED by 26247170)
- `26247170` — User as Actor & On-Demand Accounts (User is first-class actor; no default account; Accounts created on demand via POST /accounts; Personal/Studio retired)
- `23003138` — Account Deletion, Tombstoning & Handle Reuse
- `23101442` — Notification Service, Fan-out-on-Read & the Seen Cursor
- `23592962` — API Response Shape & Error Model (RFC 9457)
- `24150017` — Transactions as a capability — compile-enforced Unit of Work in the private store
- `24182787` — Collection as a Generic Referenceable Membership Primitive
- `24182820` — Invitation Validity & Issuer Departure
- `24543244` — Auth Surfaces, the Plugin Trust Boundary & CSRF
- `24870914` — The Account Handle (initial handle choice at account creation; the post-onboarding *change* flow is DD 27852802)
- `26050561` — Confusable Handles & the Punycode Policy (block `xn--` IDN labels in v1; UTS #39 allow-with-checks is the documented upgrade path)
- `26607618` — Handle Resolution for *.zurfur.app — HTTPS well-known (DNS-reversible) (DECIDED; serve handle→DID via Host-routed `/.well-known/atproto-did` reading Postgres, behind one wildcard DNS+TLS cert; reversible to DNS TXT later; key-custody/minter still open; ZMVP-44)
- `26804226` — did:plc Identity Custody, Minting & Credible Exit (rotation-key custody + minter lifecycle; key-storage/KMS follow-ups; ZMVP-49/53)
- `26935298` — Zurfur Public Presence & PDS — Identity-Only for v1 (v1 mints identity-only did:plc for Accounts/Characters: valid identity+handle, no PDS/atproto repo — feed-generator pattern; entity public presence deferred reversibly; records-hosting fork open)
- `27852802` — Account Handle Change Flow (DECIDED 2026-07-01; Owner-only post-onboarding rename, light Bluesky-style rate-limit, QUARANTINE the vacated `*.zurfur.app` handle only, REPLACE alsoKnownAs, all BYO transitions with bidirectional verify-before-commit, DID-doc-first outbox ordering; bounded by credible-exit; closes the open row on The Account Handle 24870914; ZMVP-50 built the reusable update-op [DONE #89], ZMVP-46 consumes it)
- `19431425` — Authenticators
- `29622283` — The AT Protocol Boundary Contract — Class A / Class B & the Public-Node Test (DECIDED 2026-07-02; the public-node test governs what crosses Index→atproto: Class A = atproto-native, born in owner's repo, PDS canonical, Index a derived cache; Class B = anything private-capable [commissions + subtree], Index-canonical, only server-side derived projections cross by explicit publish; lexicon fields are one-way doors; assertions-about-someone cross only as Provider-signed attestations; network never guarantees forgetting; yardstick for Units 2–6)
- `29687820` — Publish Consent & OAuth Scopes — Identity-Only Sign-In, Scopes at First Use (DECIDED 2026-07-02, Boundary Unit 2; identity-only login, repo-write authority requested at first publish via granular atproto scopes bundled into few broad permission sets; first set `app.zurfur.authPublish` [`repo:app.zurfur.*` + `blob:*/*`]; `transition:generic` fallback with downgrade duty; revocation lives at the PDS)
- `29949954` — Gallery Posts, the Product Snapshot & Index-Side Tagging (DECIDED 2026-07-02, updated 2026-07-04, Boundary Unit 3; Product = Class B, Index-canonical, no lexicon vs Gallery Post = Class A `app.zurfur.feed.post`; **§9 = the FINAL `feed.post` field list** [required createdAt + labels; optional text/embed/reply/credits/snapshot]; credits `{role,did}` opt-out at compose; descriptive tags 100% Index-side; blob-CID dedupe; reply unification [feed.comment deleted]; ZMVP-104 authored the JSON)
- `29982722` — Maturity Vocabulary — Adopting atproto Self-Labels (DECIDED 2026-07-02, Boundary Unit 4, amends Content Maturity 21594113; adopt atproto self-labels wholesale — axis Safe / Suggestive / Nudity / Adult + orthogonal Graphic; ratings ARE the label values, no mapping layer; required per work, server-side, blocking at publish; labels travel in the snapshot)
- `29622362` — Asks as Tags — Status Tags & Tag Ownership Domains (DECIDED 2026-07-02, Boundary Unit 5; asks are Index-side status tags, not entities [ask feed = tag query]; tag ownership domains [gallery tags community-editable; user/account/commission tags owner-only]; tags ≠ labels; no atproto ask record [tombstoned])
- `29622321` — Seals — Attestations as Labels & Peer Grants (DECIDED 2026-07-02, supersedes the Provider stub; a Seal = attested mark in profile slots; atproto Labelers wholesale for institutional sources + peer grant records `app.zurfur.seal.grant` [Class A, grantor's repo]; the definition carries all authority/presentation, checked at render; shelf = accept, decline silent; Zurfur Seals render as Achievements, third-party as Community Seals; five-layer defense stack)
- `29818896` — The Lexicon Registry — Publish-Late, Additive-Only (DECIDED 2026-07-02, Boundary Unit 6 [closes the boundary programme]; one Zurfur-owned DID holds all `com.atproto.lexicon.schema` records, resolved via `_lexicon.zurfur.app` DNS TXT; additive-only evolution, breaking change = new NSID; **publish-late doctrine** — a lexicon publishes only when its feature ships, mutable Index-internal until then; fixed namespace map)
- `30572573` — Comments — The Replyable Trait (DECIDED 2026-07-02, updated 2026-07-04; Replyable = render-side trait [which subjects the Index materializes reply feeds for]; **there is no comment lexicon** — a comment/shout is an `app.zurfur.feed.post` with `reply {root,parent}` set, each arm a union `strongRef | did`; `feed.comment` deleted before publication; Class A [commenter's repo]; commissions are never Replyable)
- `32178178` — The Eventlog — the Derived Timeline & Source Streams (DECIDED 2026-07-06; splits the commission timeline: **changelog** = explicit existence/state facts only [closed Zurfur vocabulary, atomic with domain writes, per-commission hash chain h_i = H(h_{i-1} ‖ entry_i) + asymmetric-signed checkpoints]; **eventlog** = derived, core-rendered view over core-persisted append-only SOURCE STREAMS [changelog projection + comments + native output incl. markup + plugin output]; entry contract = envelope {id, time, source, type, refs[]} + mandatory core-renderable fallback; presentation = stored declarative templates, survives uninstall; append-only + targeted/authorized supersession, changelog projections supersedable by NO ONE; no plugin filters/reorders/restyles the canonical view; ⚠️ several follow-ups [taxonomy partition, consumers, cursors/ordering, dispute freeze, notes placement] marked "later today" — check the page's Open table before building anything timeline-shaped)
- `32112642` — The Linked Channel — Pointer Custody & Chat-Tracking Placement (DECIDED 2026-07-06; pointer STAYS core as an **opaque slot** — Zurfur owns the bytes, the setter owns the meaning; core stores/audits/renders raw, never parses; Changelog DD D2 reaffirmed, **ZMVP-87 AC3 stands / PR #100 merges as built**; chat ingestion = a plugin **Golem receiver writing its own eventlog source** [per Eventlog DD 32178178], never the changelog — closes External Chat Tracking 6848513's placement row as the Option-5 mechanism; the pointer is the SEAM: plugins read it visibility-bounded, may set it invoked/principal-bounded, **pointer ≠ binding** [authoritative binding self-hosts, no plugin depends on the pointer to operate]; single pointer per commission; open: pointer scope names at Plugin-API time)
- `8978453` — Where should Portfolios, Batches, Commissions and Collections live? (in progress)
- `8978501` — Portfolio Live vs Static
- `9863207` — Arrangement — SUPERSEDED, folded into Workflow (`9895957`)

### Data layer / infrastructure
- `9994298` — Where does Data live?
- `10354698` — Data Boundaries
- `9207877` — The Index & Data Boundaries — SUPERSEDED, moved → Data Layer
- `9207856` — Platform Authority
- `10125341` — Blobs, PDS & Private Storage
- `9994275` — Blob
- `10354710` — Lexicon
- `11763713` — Domains and Applications
- `12451841` — Golem
