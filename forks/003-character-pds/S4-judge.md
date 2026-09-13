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
