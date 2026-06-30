# Retrospective — Unit of Work `7ff9c7` (2026-06-29)

*First-person reflection by Claude on my own contributions to unit `7ff9c7`. Scoped to PRs #68–#71 plus the design/triage work around them.*

## Summary

I drove one unit of work from "what's next?" to four merged PRs:

- **#68 [ZMVP-24]** — rotate the session id on sign-in (`Session::cycle_id()` at the privilege change). `afcc194`.
- **#69 [ZMVP-21]** — User leaves an Account: `DELETE /accounts/{id}/members/me`, role-tree re-homing, invitation revocation, the `leave` port + pg/mem adapters. `2573d0f`.
- **#70 [ZMVP-23]** — defense-in-depth CSRF: a first-party `Origin` guard on the cookie surface. `5e23201`.
- **#71 [ZMVP-40]** — `revoke_role` completes the member-departure (shared `settle_member_departure` helper). `ad06321`.

Alongside the code: two Design Decisions (*Invitation Validity & Issuer Departure*; *Auth Surfaces, the Plugin Trust Boundary & CSRF* — the latter grounded in a five-thread deep investigation), Confluence sync (Roles, Account, Plugin), three spun-out tickets (ZMVP-41/42/43), and a full repo cleanup (17 merged branches + stale worktrees removed).

## What went well

- **I reshaped a colliding set before any branch existed.** The named set {21, 23, 36, 39} funneled through two chokepoints — the flat `api/src/lib.rs` router and the `adapter-pg` write layer. I surfaced that with evidence and proposed 21 + 23 + 24 (deferring the two cross-cutting refactors), which avoided a four-way merge fight. Triage with evidence, not vibes.
- **I caught real domain bugs in the engineer's ZMVP-21 draft before merge** (#69): re-parenting that wasn't scoped to the account (cross-account corruption), the invitation clause targeting `invited_user` instead of `inviter` and using `DELETE` instead of the DD-decided `Revoked`, a state-changing `GET /leave`, a missing transaction, and a missing `adapter-mem` impl (it didn't compile). These are cheap to fix now and expensive in production.
- **The ZMVP-40 refactor closed a bug I wasn't even asked about** (#71): extracting `settle_member_departure` revealed that `revoke_role` had *never* re-homed children (a rule-3 violation) — I flagged it, you chose the full fix, and the shared helper now keeps `leave`/`revoke_role` honest.
- **The deep investigation produced a better decision, not just a faster one.** Five parallel agents on primary sources caught the registrable-domain-vs-subdomain trap and the "plugins never need a PDS credential" insight that collapsed the whole question.
- **Clean execution end to end:** every gate green locally (fmt · clippy `-D warnings` · `test --workspace`, offline), **no PR review comments needed on any of the four**, and **no `TODO`/`FIXME` left behind** in the files I touched.
- **I corrected myself.** I had claimed DPoP non-extractable keys "neutralize" browser-token XSS risk; when the research refuted it, I walked it back explicitly rather than letting it stand.

## What could be better

- **Ceremony didn't match stakes.** I ran the full lifecycle (`/start` → `/understand` → `/implement` → `/critique` → `/document` → `/prepare-pr`, worktree + granular commits) around ~10-line changes (ZMVP-24's one `cycle_id()` call; ZMVP-23's one middleware). The process cost exceeded the diff for the trivially-mechanical tickets.
- **I under-parallelized the execution, and had to be told.** You explicitly nudged me to "maximize parallelization" — I'd read the initial four tickets inline and serially, and the Claude lane (24→23) ran sequentially. The genuine parallelism (the research agents) only showed up later. I should have defaulted to fan-out from the start.
- **I asserted before grounding.** The DPoP overstatement came from memory in the middle of a conversation that had *just* emphasized "look it up, don't assert from memory." I should have hedged until a source confirmed it.
- **I nearly shipped ZMVP-40 half-done.** Folding it into ZMVP-21, I implemented only the `leave` site even though the DD explicitly named *both* `leave` and `revoke_role`. I caught it in the wrap-up — but that should have been a checklist item when I folded it in, not a late save.
- **sqlx offline-cache friction.** I regenerated `.sqlx` reactively (twice) instead of anticipating the prepare step the moment I added `query!`s.

## What I should change

1. **Scale the lifecycle to the stakes.** For a typo-class / one-call mechanical ticket, collapse it — skip the full briefing, fewer gates, one commit. Reserve the whole machine for domain-touching or structural work. The DD work deserved *more* rigor; the `cycle_id()` call deserved less.
2. **Default to fan-out.** When ≥2 tickets need understanding, dispatch parallel `/understand` agents immediately (already saved as a memory) — and reach for the multi-agent investigation pattern earlier, since it was the clear source of leverage this unit.
3. **Hold researched claims, not remembered ones.** On any protocol/security/LLM claim, mark it uncertain until a source confirms — especially mid-conversation, where an unhedged assertion can steer a decision before it's checked.
4. **Enumerate folded-in scope up front.** When a decision is folded into another ticket, list *all* its call sites as an explicit checklist so a multi-site invariant doesn't ship partially.

## Path forward

- **Deferred, still open:** ZMVP-36 (compile-enforced Unit of Work) and ZMVP-39 (router split). ZMVP-39 is worth doing *before* the next account-area batch — splitting the flat router directly removes the `lib.rs` chokepoint that created this unit's collision pressure in the first place.
- **New, from the DD:** ZMVP-41 (webhook HMAC/SSRF), ZMVP-42 (plugin credential model — carries the open forks: REST vs XRPC for `/plugin/v1`, the token-exchange shape), ZMVP-43 (embedded-UI sandbox, post-MVP).
- **How I can support the team better:** lean into the parallel-investigation pattern for the hard forks (it paid off clearly here), lighten the per-ticket ceremony for mechanical work, and keep every domain fork yours — which held this unit (every decision that shaped an entity, an invariant, or a contract was yours, with my role to research, propose, and execute).
