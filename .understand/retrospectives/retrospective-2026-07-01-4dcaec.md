# Retrospective — unit of work `4dcaec` (2026-07-01)

*First person, as Claude. Scope: the single unit `4dcaec` — named set ZMVP-44 / ZMVP-46 / ZMVP-47. Evidence from git, GitHub (#88), Jira, and Confluence (DD 27852802).*

### Summary

I drove one `/unit-of-work` pass over a three-ticket named set. It resolved into three very different shapes:
- **ZMVP-44** — I caught that it was *already merged* (#82) despite the ledger calling the unit "COMPLETE" while Jira still read "In Review"; dropped it from the build after the Engineer moved it to Done.
- **ZMVP-46** — a domain-heavy design fork. I ran it through `/design-decision` (Engineer-driven), landing **DD "Account Handle Change Flow" (27852802)**, rescoping ZMVP-50/ZMVP-46 in Jira with a blocked-by link, filing ZMVP-64, and syncing the glossary. No code — the build is a future unit gated behind ZMVP-50.
- **ZMVP-47** — the one build. Shipped as **PR #88 → `main` `91b5114`**: an `AccountRole` `FromRequestParts` extractor lifting the ad-hoc membership floor into one compile-enforced seam, 6 handlers retrofitted behavior-preserving, security-reviewed clean, one Copilot comment closed, Jira Done.

This was a **driver-heavy, code-light** unit: most of my value was orchestration, verification, and holding decision authority with the Engineer — not lines written (602 across 2 files).

### What went well

- **I verified the "settled" claim instead of trusting it.** The ledger's `previous_unit` note said ZMVP-44 shipped and the unit was complete, but live Jira showed "In Review." I checked PR #82's actual merge state before treating ZMVP-44 as either done or new work — exactly `feedback_verify_settled_claims_on_resume`. That one check reshaped the unit correctly at the start.
- **Evidence before assumption.** I fanned `/understand` across the named set in parallel rather than eyeballing it. That surfaced two things I would have gotten wrong from memory: ZMVP-46 is *doubly* blocked (no `did:plc` UPDATE op exists, only genesis+tombstone), and ZMVP-47's membership floor *already essentially existed* — so the ticket was "lift it into one seam," not "add enforcement everywhere." The build's commit message reflects that framing (`91b5114`).
- **I argued with researched evidence, then deferred.** When the Engineer leaned toward a 30-day rename cooldown, I didn't accept it to keep momentum, nor assert from memory — I pulled Bluesky's actual `updateHandle` rate limit (~10/5min, a burst throttle) and the atproto handle spec, and made the case that a long cooldown burdens the user (Philosophy tenet 1) for a platform-hygiene problem quarantine already covers. I also flagged the **credible-exit invariant** against the Philosophy page. The Engineer adopted both refinements. This is `feedback_argue_then_defer` + `feedback_verify_security_claims` working as intended.
- **I never took a domain decision.** Every fork — all 8 of ZMVP-46's, both of ZMVP-47's — went to the Engineer via modal/interview; I proposed with recommendations and deferred. Ownership bands held: the domain classification and the check *signature* were theirs; the extractor plumbing and tests were the agents'.
- **I closed my own loop before calling it done.** `/security-review` ran on Opus *before* the PR; then `/address-comments` triaged Copilot's one finding, I had it fixed in `b905a06`, **verified there was no provisioning-order bug behind it** (the gate rejects before `provision`), replied on the thread with the fix commit, and resolved it. Reconciled the force-push to the fetched oid (clean replay) per `feedback_reconcile_before_force_push`. No "ready while pushing" hazard.
- **Convention followed:** first branch commit carried `[ZMVP-47][uow:4dcaec]`, later commit just `[ZMVP-47]`; Co-Authored-By present; loose-ends grep on the merged files is clean.

### What could be better

- **I mis-designed the delegation and had to undo it.** I spawned the ZMVP-46 design-decision agent *for the Engineer to drive directly* — but they couldn't see it ("I only see ZMVP-47"). A **stopped** background agent drops off their active list, so the interactive session I'd delegated was unreachable, and I had to become the relay anyway. That partly defeated the token-saving purpose. I should have confirmed reachability *before* handing interactive work to a spawned agent.
- **I spawned agents that immediately parked.** Both Engineer-driven agents launched and then stopped on the first decision only the Engineer could make. For the design-decision especially, the interview *had* to route through me regardless — so the up-front spawn front-loaded work that couldn't move. Interviewing (or confirming the decisions) first, *then* spawning agents to **execute**, would have been cleaner.
- **I left model choice implicit until asked — and then codified an unverified, inverted claim.** I spawned both agents on Opus without surfacing the model/cost tradeoff; the Engineer had to prompt it ("what model are you going to use?"). Worse, when they said Fable "is cheaper," I **took that at face value and wrote it into a policy memory** (`feedback_model_split_fable_impl_opus_design`) — "Fable implements because cheaper." A same-day deep-investigation showed it was **factually inverted**: Fable 5 is the *most expensive* model ($10/$50, 2× Opus) *and* hard-refuses security-adjacent code, so my policy would have routed ZMVP-46/47 — both security-nature — to a model that would have **refused them**. The correct policy is now `feedback_model_assignment_policy` (Sonnet builds, Opus owns all security + judgment + conductor, Haiku glue, Fable opt-in only). This is a direct violation of my own `feedback_verify_security_claims` (never assert a model/security fact from hearsay). *Mitigating fact:* the actual execution this unit was still correct — I kept ZMVP-47 on Opus for the auth boundary — so the bad memory never mis-routed anything before it was caught. But it *would have* on the next unit.
- **I fought the modal-timeout instead of adapting fast.** `AskUserQuestion` kept timing out at 60s while the Engineer stepped away; I re-fired it and flip-flopped between prose and modal across several turns. I should have recognized the away pattern immediately, batched all pending decisions into one return-clears-everything set, and stopped re-firing modals into an empty room.

### What I should change

1. **Before delegating an *interactive* session to a spawned agent, confirm the user can actually reach it.** If uncertain, relay through the main thread from the start, or make reachability explicit — don't assume a spawned agent is a channel the user has.
2. **Sequence delegation by dependency:** interview/confirm the blocking decisions *first*, then spawn agents to **execute** the settled work. Don't spawn agents that can only park on the user.
3. **Surface model/cost at spawn**, proactively — name the model and the tradeoff when launching an agent (especially long/expensive ones), rather than waiting to be asked. **And never codify a model's cost/capability from hearsay** — verify against `reference_claude_model_catalog` / `feedback_model_assignment_policy` before writing it into a policy or a decision.
4. **Adapt to an away user quickly:** when a modal times out once, offer the format choice once and then batch every pending decision so a single return clears them; stop re-firing into the void.

### Path forward

- **ZMVP-50 is the next natural unit** — it's now the hard unblocker for ZMVP-46's build (the reusable `did:plc` UPDATE-op + retryable outbox). Note: it's crypto/identity-critical, so under the new model policy it likely stays on **Opus**, not Fable — the policy's escalation clause applies.
- **Housekeeping:** stale worktrees from prior merged units (`zmvp-44`, `zmvp-49`, `uow-transaction`) are still on disk, pending the Engineer's go to remove.
- **Support the team better** by tightening the delegation mechanics above — this unit proved the *orchestration* is where I add or lose value on a code-light unit, more than the diff.

*Lessons indexed to memory: new `feedback` on delegating interactive agents (reachability + sequencing + proactive model surfacing). Reinforced (not new): verify-settled-claims, argue-then-defer, close-review-loop-before-pr, reconcile-before-force-push. Sharply reinforced by a self-inflicted miss: `feedback_verify_security_claims` — I codified an inverted "Fable is cheaper" from hearsay; corrected same-day by deep-investigation into `feedback_model_assignment_policy` (which supersedes my deleted `feedback_model_split_fable_impl_opus_design`).*
