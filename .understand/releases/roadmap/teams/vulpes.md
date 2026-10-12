# Roadmap: vulpes

Lead: Reynard · QA: Rook · Stages covered: all four, as Vulpes milestones mapped onto Zurfur's slots

Vulpes doesn't plan by Zurfur version. It keeps building its own roadmap in parallel (your ruling on pausing Vulpes and Wolven, option (b)). This file says which Vulpes milestones Zurfur needs, by which Zurfur slot. Sources: Vulpes `docs/ROADMAP.md` and `docs/CONTINUE-HERE.md` (both 2026-10-01), `FORKS.md`, `docs/acp.md`, `docs/ccs.md`, the repo as it stands on 2026-10-10, and the corpus pages named below. The Vulpes repo was only read.

## In one screen

- **Zurfur needs nothing from Vulpes until it writes its first Character ownership claim**, or, if you rule yes on question 1, until it mints DIDs through Vulpes. Both are identity's alpha cards ("claims", "did-ops"); neither is in 0.1.
- **End of pre-alpha:** Vulpes is a pure library on `main`. Its claim record matches the four-shapes ruling, a paired ownership edge verifies with a report, and the ownership kind is published under got-paws.net. If you rule yes on question 1, its minter does handle-less mints, takes the owner's key from the client and attaches a PDS. All of it is ready by the end of P4, so any Zurfur slot from P5 can use it.
- **End of alpha:** Zurfur's real claims (and DIDs, if swapped) run on a pinned Vulpes tag. From the first real claim, the claim record, the edge id and the ownership kind are frozen: atproto allows only additive changes to a published lexicon. Vulpes ships nothing breaking after that.
- **End of beta:** an attestor toolkit (signing, status lists, the linked-account kind) is ready if you place Zurfur's first attestor there. Wolven's six requests are settled if Wolven has resumed.
- **Release:** ACP 1.0 and the public announcement come only after real data has run through it, plus a patch line for security fixes. The private lane stays unplaced.
- **Two problems found while mapping, both blocking the claims slot:** (1) the ruled scope "Zurfur's publish permission set carries `repo:net.got-paws.acp.*`" can't work. An atproto permission set covers only its own namespace, and partial wildcards aren't allowed (atproto permission spec, checked 2026-10-10). (2) The ownership claim names the other party as `did` in two documents and as `subject` in two others.
- **The biggest open fork is question 1: does Zurfur mint through Vulpes?** Proposed: yes, swapped in pre-alpha, before any real DID or sealed key exists.

## Where Vulpes stands today (2026-10-10, read-only checks)

- **Done on `main`:** the lexicons; the record types with canonical bytes; signing and verifying; the seven-step attestation verifier with the in-memory kill test; dropping the old relationship path; the custody helpers; the typed claim kind.
- **Ruled, not on `main`:**
  - The pure-library ruling (2026-10-01, confirmed by you on 2026-10-08) is written up on PR #33.
  - Its three code cuts (shells, Postgres, caller-supplied randomness) are committed in local worktrees and are on no remote. `git ls-remote` lists only `main` and four other branches.
- **Stale on `main`:**
  - `docs/characters-atproto.md` still says "post-alpha", "co-ownership = … a claim from each co-owner" and "layout D with `vulpes_recovery`". That is the file the Characters on ATProto decision (54427650) names as its source of record. PR #30 fixes it and is open, unrebased.
  - The roadmap still lists "the first attestor" as a Vulpes item, though the pure-library ruling moved it to Zurfur.
- **Not started:**
  - the claim record catch-up (the lexicon already has the four-shapes fields; the Rust code doesn't);
  - the paired verifier and its report;
  - kind definitions as schema records;
  - the minter changes;
  - DNS publication.

## What Zurfur needs, by slot

| Vulpes milestone | Zurfur card that needs it | Latest Vulpes slot | Why then |
|---|---|---|---|
| Pure library on `main`, docs reconciled, a tag | any card that takes Vulpes as a dependency | P2 | Zurfur pins a tag, and the tag must not carry OAuth, Postgres or a reqwest client |
| Claim record catches up (four shapes; one name for the other party) | claims | P2 | It changes canonical bytes. That's free now and a one-way door at the first real claim |
| Minting ready (only if question 1 is yes) | did-ops, account-pds, character-presence | P3 | Accounts attach a PDS before any public Character exists |
| Claims ready: paired verifier, report, kinds published, JSON boundary | claims | P4 | Identity's claims slot needs all four |
| Freeze: additive-only from the first real claim | claims, from the alpha on | the alpha's first claim | atproto lexicon evolution (below) |
| Attestor toolkit | none on the board; attaching outside accounts (unplaced) | beta or later | No board card needs it before then |

If identity builds claims before hosted repos, swap P3 and P4. The consolidator should tell me which slots identity picks.

## What Zurfur consumes, and when

| What | From Vulpes | Used in Zurfur by | From |
|---|---|---|---|
| Nothing | — | — | P1 (0.1): no dependency, no ACP record, every fixture character Private (already a line in the 0.1 security review) |
| The minter, its policy, sealed custody keys, PLC operation types; Zurfur implements the key-store and operation-log traits over PostgreSQL and the directory trait over its guarded client | minter, policy, keys, vault, plc | `adapter-atproto` and `adapter-pg`, replacing Zurfur's own minter | identity's swap slot, pre-alpha, only if question 1 is yes |
| Claim record, edge id, kind parsing, the paired verifier and its report | `acp` | `adapter-atproto`, behind Zurfur's own ports | identity's claims slot |
| The three read ports (repo reader, DID resolver, status source); Zurfur implements them over the hardened resolver's guarded client | `acp` ports | `adapter-atproto` (the resolver design's "vulpes ports" row) | claims slot |
| The JSON ↔ canonical-bytes boundary for records read over XRPC | `acp` | `adapter-atproto` | claims slot |
| The custody due-diligence helper | `acp::custody` | Character transfer or sale, which no board card has yet | unplaced |
| Attestation signing and status-list artifacts | `acp` sign and status | Zurfur's first attestor | beta or later |

Recommendation, an implementation choice for identity: keep Vulpes types inside the adapters, behind Zurfur's ports, with std `From`/`TryFrom` conversions at the seam. The domain then never learns which library speaks the protocol.

## Pre-alpha

Vulpes builds these while Zurfur builds its own P2 to P4. None waits on Zurfur.

- **P2: Vulpes is a pure library on `main`.** Features: proposal: the pure-library ruling's three cuts merged (shells, Postgres, caller randomness); PR #33 (the write-up) and PR #30 (the 2026-09-12 Characters amendments, rebased) merged; the docs brought into line with "library only". That means `ccs.md`'s reference instance and its `[vulpes, zurfur]` floor, the identity model's Holder store, and the roadmap's Holder and first-attestor lines. Then a tag Zurfur can pin. Depends on: none (your reviews). Done when: `git -C ~/code/vulpes show origin/main:Cargo.toml | rg -w 'sqlx|jacquard|reqwest|ssi|testcontainers'` prints nothing; `gh pr list -R ZurNetwork/vulpes` lists neither #30 nor #33; `git -C ~/code/vulpes show origin/main:docs/characters-atproto.md | rg -i 'post-alpha roadmap|minted layout is d|claim from each co-owner'` prints nothing (it prints three lines today).
- **P2: the claim record catches up with the four-shapes ruling.** Features: proposal (Vulpes roadmap "CCS as attestations"): the claim gains `id`, `nonce`, `claimant`, `expiresAt`, `status` and `witnesses` in code; the edge id; witness signing; the fixtures bump. The other party gets one name everywhere (question 2). Depends on: your answer to question 2. Done when: `cargo test` passes on Vulpes `main`, and a pinned fixture holds an ownership claim carrying `id`, `nonce`, `claimant` and `expiresAt`.
- **P3: minting ready (only if question 1 is yes).** Features: proposal (Vulpes roadmap "Mint layout D", re-scoped):
  - a handle-less mint;
  - the owner's public key, made on the user's device, as rotation key 0 (Vulpes never holds it);
  - Zurfur's list, `[owner, zurfur]` at mint and `[owner, pds, zurfur]` after one "attach a PDS" update that adds the PDS service, its signing key and its rotation key, described in the policy so the strict update guard accepts it;
  - layout D's Vulpes slot renamed for library-only (question 5).

  If the security card's "key custody / KMS" means keys never leave a KMS, the minter must sign PLC operations through a signer the caller supplies, not a sealed secret. Depends on: question 1; identity's answer on KMS. Done when: a Vulpes test mints a handle-less DID whose rotation key 0 is a key the test supplied, attaches a PDS with one update, and `cargo test minter` passes.
- **P4: claims ready.** Features: proposal (Vulpes roadmap "CCS as attestations" and "Talk to a PDS"):
  - `verify_pair` and the report replacing the bare verdict, with the kill test re-run in memory;
  - the ownership kind's definition as a schema record (roles, one edge per Character, default lifetime);
  - the DNS records that make it resolvable;
  - a JSON ↔ canonical-bytes helper for records read over XRPC.

  No repo-writer port unless identity asks for one, because Zurfur's own public-records port writes. Depends on: you, for the DNS records on got-paws.net (question 6). Done when: `cargo test --all-features` on Vulpes `main` runs the four-shapes tests green; `dig +short TXT _lexicon.relationship.acp.got-paws.net` and `dig +short TXT _lexicon.acp.got-paws.net` each print a TXT record naming the schema repo's DID.
- **P5 and later: on call.** Vulpes fixes what identity's real-PDS test bed finds while Zurfur builds the swap and the claims. Breaking changes are still allowed until the alpha's first real claim.

Exit: Zurfur's `Cargo.lock` pins a Vulpes tag holding everything identity's alpha slots use. No breaking Vulpes change is queued. Questions 1 to 6 are answered. The Vulpes code Zurfur takes (the minter if swapped, the claim verifier) has passed Zurfur's own security reviews, the security card's included.

## Alpha

- **A1: frozen at the first real claim.** Features: proposal. From the alpha's first claim written into a real person's repo, these are additive-only: the claim record, the edge id formula, canonical bytes, and the ownership kind's fields, roles and lifetime. The rule is atproto's: "Any new fields must be optional", "Fields can not be renamed", and a breaking change needs "a new Lexicon name" (atproto lexicon spec, "Lexicon Evolution", checked 2026-10-10). Vulpes records the freeze and the tag Zurfur pins. Fixes ship as patch tags on that line. Depends on: identity's claims slot. Done when: Vulpes's roadmap records the freeze, and `rg -n vulpes ~/code/zurfur/Cargo.lock` shows a tag-pinned git source.
- **A2: the watcher helper (proposal, if you place the PLC-log watcher in the alpha).** A pure function that says which rotation key signed each operation in a DID's PLC log. Zurfur's watcher uses it to warn an owner about operations they didn't make, inside the 72-hour window in which their senior key can still undo them. Depends on: question 9. Done when: a Vulpes test labels an operation signed by the operator's key "not the owner".
- **A3: expiry and renewal (Vulpes roadmap item).** Lifetimes per kind, "human vouches never auto-renew", and the test for subjects stranded when an attestor dies. No alpha card needs it: the ownership pair uses a far-future lifetime (question 3). It's groundwork for the attestor toolkit. Depends on: none. Done when: `cargo test` runs the stranded-subject test green.

Exit: every alpha card that touches Vulpes runs on a pinned Vulpes tag, with no breaking change since the freeze. Those cards are claims, characters, character-presence and account-pds, plus did-ops if swapped.

## Beta

- **B: the attestor toolkit, when Zurfur builds its first attestor.** Features: proposal; no board card. It covers:
  - issuing attestations and status-list artifacts;
  - the linked-account kind's payload ("this Discord or Telegram account is this DID");
  - the status list's `list` identifier, decided with Zurfur because it is baked into every signed attestation.

  The auto-renew loop and the checking itself are Zurfur's. Depends on: your placement of attaching outside accounts (the unified-identity study: "attach to an existing User, then sign in with it", after the Bluesky-only phase), and your rulings on who attests for Wolven and who may run the lookup (that study, question 5 (d) and (e)). Done when: a Zurfur test issues an attestation, kills the attestor and still verifies it.
- **B: Wolven's six requests, if Wolven resumes.** Additive kinds only (membership grant, parent, partnership), never a change to the claim record. Depends on: Wolven resuming, and your rulings on how Wolven mints. Done when: Vulpes's "wolven consumes the acp" issue is closed.
- **Beta cards that need nothing from Vulpes:** Seals and achievements are ruled onto atproto labels and Zurfur's own grant record (Seals decision, 29622321). Gallery posts may raise the deferred `consent` category for pieces showing several characters. Ask at that slot; proposed answer there: takedown requests suffice for the beta, as the ACP spec says.

Exit: if the first attestor shipped, the `list` identifier is fixed and the kill test passes against a real attestor.

## Release

- **R1: ACP 1.0 and the announcement.** Features: proposal: the spec goes from "v0.1 (draft)" to 1.0, and the manifesto ships publicly, only after Zurfur has run claims on real data (question 7). Add a supported-versions note, so Zurfur can take security fixes as patch tags without other changes. Depends on: none. Done when: the Vulpes repo has a 1.0 spec tag and a security-fix policy in its README.
- **Unplaced, after release:** the private lane (holder-held credentials) and the active vouch mode. No board card needs either.

## One-way doors, and the slot that closes each

| Door | Closes at |
|---|---|
| The claim record's fields (`net.got-paws.acp.claim`) | the alpha's first claim in a real person's repo |
| The edge id formula and the canonical bytes | same: changing either orphans every written pair, because ids stop recomputing |
| The ownership kind: the other party's field name, roles, one-edge rule, default lifetime | same |
| The authority `net.got-paws.acp` and its DNS on got-paws.net | same; the domain must stay registered while anyone resolves the kinds |
| Which minter Zurfur uses, and its sealed custody format (Vulpes `keys.rs`: "part of the on-disk format") | the first real DID minted when the alpha opens; a later swap means re-sealing real users' keys |
| The rotation layout of real DIDs | the same first mint (identity's door; Vulpes only has to express it) |
| The status list's `list` identifier | the first attestor's first attestation (beta or later) |
| The linked-account kind's payload | the first published linked-account claim |
| The membership kind's `grant` | only if Account repos publish membership (what an Account repo holds is still open) |

## What Vulpes must not decide before Zurfur's alpha

1. **It must not freeze the claim record before identity has written pairs against a real PDS, nor change it after the alpha's first real claim.** Pre-alpha is when the shape gets tested.
2. **It must not pick the status list's `list` identifier alone.** The four-shapes and two-lanes rulings defer it to "the first attestor", which is Zurfur's.
3. **It must not publish the linked-account kind's schema** before you rule who attests for Wolven and who may run the lookup.
4. **It must not bring back a hosted Vulpes, a key Vulpes holds, or a private store.** That would contradict your library-only rulings (2026-10-01, 2026-10-08) and the Characters on ATProto decision's "vulpes holds no keys".
5. **It must not make a default policy require witnesses, a status or the custody check for ownership.** Zurfur's alpha halves carry none (question 3).
6. **Its minting rules must not refuse Zurfur's `[owner, pds, zurfur]` list.**
7. **It must not announce ACP publicly as final** (question 7).
8. **It must not let Wolven's requests change the claim record itself.** New kinds are additive; record changes are not.

## Cross-team dependencies (what I need, from whom, by which slot)

| Need | From | By |
|---|---|---|
| The slots identity picks for did-ops, account-pds, character-presence and claims | identity (Fox), through the consolidator | P2 planning |
| The three Vulpes read ports, implemented in `adapter-atproto` over the guarded client; a real-PDS test bed; the kill test re-run against it | identity | the claims slot |
| The ownership half's OAuth scope fixed (question 4), with the corpus corrected | identity, then a design-sync | the claims slot |
| The minter swap: Zurfur's key-store and operation-log traits over PostgreSQL, and Zurfur's own minter retired | identity | the swap slot, after Vulpes's P3, only if question 1 is yes |
| Whether "key custody / KMS" keeps keys inside a KMS | identity, with the security card | P3 planning |
| The owner's key made on the user's device for Accounts and Characters (Vulpes takes only the public key) | identity and web | did-ops and characters slots |
| A security review of the Vulpes code Zurfur takes, inside Zurfur's own reviews | tech lead, Designer lane | before the alpha opens |
| DNS records on got-paws.net; Vulpes PRs in your one queue, below blocking 0.1 items | the Engineer | P4 for DNS |
| Nothing until Wolven resumes; then its six requests | wolven (Lobo) | beta, if at all |

## Not on the board (proposals, each with why)

- **A PLC-log watcher (Zurfur, identity) plus Vulpes's helper (A2), by the end of the alpha.** The owner's senior key protects only if the owner notices a hostile operation within 72 hours. Vulpes `ccs.md`: "The layout is only as stable as the user's ability to notice — which is what the PLC-log watcher is for." In the alpha, both custodial keys below the owner are Zurfur's, so one breach reaches both (Characters on ATProto decision, "alpha caveat").
- **Vulpes's docs reconciled with "library only" (P2).** The Characters on ATProto decision names those docs as its source of record, and they still describe a reference instance and a Vulpes-held recovery key.
- **Retire the Holder and aggregation layer from Vulpes's roadmap.** Its private store contradicts the library-only ruling, and you gave the private list of a person's accounts to Zurfur on 2026-10-08.
- **User-linking (an alpha card) as a paired ACP kind.** Its shape is undecided (User-Profiles decision, 21594113: "proves control of two of their handles"). A pair of claims with a shared edge id, one in each handle's repo, is exactly that proof, and the claims slot already brings in the machinery. Ask at the user-linking slot; proposed answer there: yes, a generic relationship kind defined by Vulpes.
- **An ACP-published permission set, later and optional.** It reads better on the consent screen than a raw collection name. But every sign-in that requests it would depend on got-paws.net resolving: "if a permission set can not be resolved (and is not already in a local cache), the auth request will fail" (atproto permission spec). That's why question 4 proposes the plain scope for the alpha.

## Questions for the Engineer (each pre-answered)

1. **Does Zurfur mint DIDs through Vulpes, and when?** *Proposed: yes, swapped in a pre-alpha slot after Vulpes's P3, before the alpha mints any real DID.*
   - For: your pure-library ruling keeps the minter in Vulpes because "Handing minting to each consumer would make the kill test depend on every consumer re-implementing the senior-key rule correctly" (Vulpes FORKS, the pure-library ruling); two PLC write paths means two to secure; the sealed-key format gets fixed before real keys exist.
   - Against: a dependency on identity's path; Zurfur's minter works today and already mints handle-less.

   This was raised on 2026-10-07 as its own question and never ruled.
2. **What is the other party called in an ownership claim: `did` or `subject`?** *Proposed: `did`.* It matches the ACP spec's category table and the Characters on ATProto decision. `subject` (Vulpes `ccs.md` and the four-shapes ruling) collides with the spec's own word "Subject", the repo owner. Settle it before the claim record catch-up.
3. **In the alpha, what do the two ownership halves carry?** *Proposed: a far-future `expiresAt` taken from the kind's definition, and no `status` and no witnesses; deleting your half is the lever, as already ruled.* This keeps status lists, renewal and a Zurfur witness DID out of the alpha.
4. **How does Zurfur get write access to the `owner` half in a user's own repo?** *Proposed: at the first claim, request the plain scope `repo:net.got-paws.acp.claim?action=create&action=delete` next to Zurfur's own permission set.* The ruled "`app.zurfur.authPublish` carries `repo:net.got-paws.acp.*`" can't work. atproto: "Permission sets are limited to expressing permissions that reference resources under the same NSID namespace", and "Partial wildcards are not supported" (permission spec, checked 2026-10-10). The same rule touches that set's own `repo:app.zurfur.*`, which is identity's to fix. Offer a design-sync for the Publish Consent and Characters on ATProto pages.
5. **With no hosted Vulpes, what fills layout D's "vulpes_recovery" slot?** *Proposed: name the slot by role, "an independent recovery custodian", and keep D as Vulpes's default shape.* Zurfur's `[owner, pds, zurfur]` then conforms as written, with the PDS in the recovery slot. Your 2026-09-13 ruling keeps the name "layout D" for the three-party list; this changes only who may hold the middle key.
6. **Publish the ACP lexicons' DNS records on got-paws.net before the claims slot?** *Proposed: yes, three records: `_lexicon.acp`, `_lexicon.relationship.acp` and `_lexicon.identity.acp`.* atproto resolution "is not hierarchical", so Vulpes's owed note of one record on `acp.got-paws.net` resolves the record types but not the kinds. A PDS stores records of a lexicon it doesn't know unless asked to validate (`createRecord`: "validate only for known Lexicons"), so writes don't block on this. I'll send you a worksheet.
7. **Hold the public ACP announcement until Zurfur has run claims on real data?** *Proposed: yes, until at least the alpha's claims slot.* After an announcement, changes cost other implementers too.
8. **Do Vulpes's tags use Zurfur's stage names?** *Proposed: no.* Vulpes uses plain semver, the ACP spec keeps its own version, and Zurfur pins exact Vulpes tags. Vulpes's `v0.1.0` and its "ACP v0.1" are not Zurfur's `pre-alpha-v0.1.0`.
9. **Place a PLC-log watcher in the alpha?** *Proposed: yes, built by identity, with Vulpes's helper (A2).* See "Not on the board".
10. **Push the three cut branches now, so the pure-library work isn't on one disk?** *Proposed: yes, without a PR, then open the PRs in P2.* The 2026-10-10 handoff already asks you to confirm.
