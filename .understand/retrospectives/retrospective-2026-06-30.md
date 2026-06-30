# Retrospective — unit of work `b722f9` (ZMVP-26 ∥ ZMVP-38)

*2026-06-30 · scope: this single unit · author: Claude (Junior Developer)*

Two parallel tickets, both authored-artifacts on the design boundary: **ZMVP-26** — the `/plugin/v1` OpenAPI contract (PR [#74](https://github.com/ZurNetwork/ZurFur/pull/74), `38552a6`); **ZMVP-38** — the first Zurfur atproto lexicon, `app.zurfur.graph.collection` (PR [#73](https://github.com/ZurNetwork/ZurFur/pull/73), `f179274`). Both merged to `main`, Jira Done.

### Summary
I resumed a planning-complete unit from a prior session's handoff, ran the lifecycle by the book — pre-gate → parallel build → post-gate → ship → integrate — and shipped both tickets as PRs the Engineer merged. Along the way I surfaced four genuine domain forks for the Engineer to decide, kept Confluence in sync, and addressed a Copilot review.

### What went well
- **The pre-gate did its actual job — it caught a bad premise.** The handoff asserted "all forks settled — just author it." Instead of trusting it (or asserting from memory), I fetched the five governing DESIGN pages (DD 24543244, Collection DD 24182787, Lexicon 10354710, Plugin 3047451, Lens 10453000) and found the claim was an over-reach: the NSID namespace, spec-first-vs-code-first, the scope vocabulary, and Lens-for-Dynamic were all still open. I surfaced four forks; the **Engineer decided all four** — I decided none. This is the roles boundary working as intended, and it's the single highest-leverage thing I did this unit.
- **Parallelism stayed clean.** Two build agents then two ship agents, each in an isolated worktree with disjoint footprints (`openapi/` + a `ci.yml` job vs `lexicons/`). No collisions, no port/DB contention; the `uow:b722f9` stamp landed correctly on each branch's first commit (`03774ae`, `f81dbfd`).
- **I verified instead of trusting at the post-gate.** Rather than accept the build agent's "matches problem.rs" claim, I read `problem.rs` and confirmed the `Problem` schema field-for-field (5 fields, no `instance`/flatten), establishing that `commission-not-found` was the *only* non-registry code — which is exactly what should be flagged provisional.
- **Honest partial artifacts.** ZMVP-26 shipped as an explicitly partial contract (`x-zurfur-stub`, `provisional`, Commission `$ref`s → ZMVP-19) rather than inventing an unpinned Commission schema. The ZMVP-38 agent modeled `memberType` as open `knownValues` (not a closed enum), correctly honoring the DD's additive-revision requirement.
- **Confluence stayed the source of truth.** Design-sync landed the new record-lexicon row + `#lens` stub on the Lexicon page (v2) as part of shipping, not as an afterthought.

### What could be better
- **The handoff I inherited over-claimed "settled" — and a prior version of me wrote it.** The planning session recorded decisions as settled that weren't, and instructed the resumer "don't re-litigate the defer verdicts." Had I obeyed that literally, I'd have powered through four Engineer-owned forks. The miss isn't catastrophic (I caught it), but it's the exact failure mode CLAUDE.md warns about: treating a *claim* as a *fact*. A "settled" line with no per-fork Confluence citation is not trustworthy.
- **The upstream briefings asserted blockers from secondary sources.** The original `/understand` agents called ZMVP-26 "triple-blocked" and ZMVP-38 "defer" by reading the ticket + a retro, not the source of truth — and several of those blockers were already decided in Confluence. The Engineer had to point this out last session. Recurring theme across this unit's lineage: **asserting status/decisions from memory or secondary docs instead of fetching DESIGN.**
- **My `/security-review` of the contract checked structure, not copy.** I confirmed the `Problem` schema matched the registry but didn't scrutinize whether the example `detail` *strings* fit the plugin surface. Copilot then (correctly) flagged that "You must be signed in to do that." (`problem.rs:76`) is session-surface language on a bearer-token API, and that the Forbidden example had silently dropped "on this account" (`problem.rs:88`). A contract review should read the human-facing copy for surface-fit, not just shape.
- **Fan-out coordination smell.** The two ship agents shared the session scratchpad and one clobbered the other's PR-body draft. It self-healed, but parallel agents should write to per-agent temp paths.
- **I over-applied Copilot review before the rule existed.** I requested it on *both* PRs; under the complexity gate the Engineer later asked for, the pure-JSON ZMVP-38 would be a skip (and indeed its Copilot review came back empty).

### What I should change
- **Never let "settled" stand without a citation.** When I write a handoff/ledger, each claimed-settled fork gets its Confluence page ID inline; when I *resume* one, I treat every "settled" as a claim to re-verify at the pre-gate, not a fact to build on.
- **Review contracts for semantics + copy, not just structure.** In a `/security-review` or post-gate on an API/lexicon, check that human-facing strings and error semantics fit the *surface*, not just that they match an existing registry.
- **Isolate per-agent scratch files** when fanning out, to remove the clobber race.
- **Apply the complexity gate** (now baked into `/prepare-pr` step 7) from the first PR, not after.

### Path forward
- **ZMVP-19 is the forcing function** for this unit's debt: it pins the Commission/Index schema and thereby fills the ZMVP-26 stubs, the scope vocabulary, the 404-vs-403 + `commission-not-found` code, and the plugin-surface error-detail copy. These are tracked in the `b722f9` logbook's *Open threads*.
- **A domain `Referenceable`/`Collection` Rust primitive has no ticket** — ZMVP-38 was lexicon-only by decision. Worth carving one when Collections get sliced.
- **Siblings/loose threads:** the DD-24543244 scope-vocabulary + token-exchange tickets; the AsyncAPI sibling (ZMVP-27); and the parked-but-now-unblocked **ZMVP-39** (router split) and **ZMVP-36** (UoW granularity — still an open Engineer fork) as strong next-unit candidates.
- **How I support the team better next:** lead with fetched evidence, hand off with citations, and keep the Engineer at every fork — the parts that worked here — while tightening contract-copy review and agent isolation.
