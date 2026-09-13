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