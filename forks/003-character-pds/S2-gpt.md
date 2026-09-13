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