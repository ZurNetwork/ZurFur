# QA log: the consolidated roadmap, 2026-10-10

- **QA:** Magpie (identity QA), on the board as `magpie@zurfur`, team identity, role qa.
- **Document:** `.understand/releases/roadmap.md`, by Kestrel, merged from eleven team files.
- **Output:** `## QA challenges`, appended to that file. Nothing else was edited.
- **Verdict:** not ready for the Engineer until 8 must-fixes close. 35 challenges: 8 must-fix, 15 should-fix, 12 notes.

## What I read

- **The whole roadmap** (698 lines before the append), the rulings file (R-1 to R-20), the three inputs and the ASK.
- **In depth, line by line against the roadmap:**
  - `teams/identity.md`, my team;
  - `teams/compliance.md`, the team today's rulings change most.
- **Checked against their rows:**
  - `teams/moderation.md`, `den.md`, `architects.md`;
  - the first 80 lines of `commission.md`;
  - `security.md`, by grep for KMS, CSRF, the soft-delete gate, staging and the wildcard certificate;
  - `web.md`, `posts.md` and `vulpes.md`, only where a finding touched them.
- **Code**, in the orchestrator workspace:
  - no outbox anywhere (`rg -i outbox` over the Rust sources found none);
  - `application/src/account/change_handle.rs:74–82` sends the directory update before the unit opens;
  - `delete.rs:67–73` logs a failed tombstone and drops it;
  - `facts/exist.rs:16` always answers "no facts";
  - Account invitations, roles and leaving exist, so the Account-member fact can be real in 0.2.
- **Protocol sources, re-read today** (not from memory):
  - atproto OAuth, atproto.com/specs/oauth: client metadata "on the public web", and the authorization server fetches it;
  - atproto permissions, atproto.com/specs/permission: no partial wildcards, and no sibling namespaces in a permission set.
- **Internal's CI/CD study** (`beaver/.understand/internal/ci-cd-study.md`), Q10, for what "alpha planning" and the deploy-target door mean.

## How each check came out

1. **Fidelity.**
   - Identity is carried well. Every move is explained, and nothing is invented beyond cross-team asks that landed on identity.
   - Two identity checks were dropped from 0.2: the deactivated-handle bug, and the tombstone after a restart.
   - Identity's load grew without being re-sized.
   - Compliance lost the consequences of admitting EU and UK users, and the owners of export and erasure.
   - Smaller drops: commission's Account bookmarks, identity's "list on profile" choice, and identity's first-sign-in gate for moderation.
   - Moderation's DID allowlist was silently swapped for a network block.
2. **Rulings.**
   - R-13 to R-16 are reflected.
   - R-5, R-9, R-10 and R-12 are carried.
   - R-11 is carried only as representatives.
   - R-6 is contradicted in 0.6, and its reach on the server is unscheduled.
   - R-7 depends on a vendor with no fallback.
   - R-20 is still listed as open.
   - R-17 to R-19 are board tooling, with nothing to carry.
3. **Dependencies.**
   - **The outbox:** sound to pull forward, but the handle change never moves onto it.
   - **KMS before staging:** both are in 0.8 and the order holds, but the KMS adapter has two owners.
   - **CSRF and soft delete in 0.2:** CSRF is fine. The store-level soft-delete refusal is missing.
   - **Den payloads in 0.2:** not confirmed by den, and the tamper check stays in 0.3.
   - **The directory switch at 0.10:** the production PDS, the wildcard certificate and the relay switch are unscheduled, and 0.9 mints the lexicon DID before the switch.
   - **New, not on the list:**
     - staging's network block breaks the production OAuth client (spec cited);
     - the cookieless image domain can't enforce the age check;
     - Character facts don't exist until slots land in 0.5.
4. **Kestrel's judgment calls.**
   - **Den payloads to 0.2:** sound in intent, since writes must be visible. It is conditional on den, and it swells an oversized 0.2.
   - **The private settings file to 0.6:** not sound as argued after the age-check ruling. It opens a dotfile door early for one preference.
   - **Staging 0.8 and the cut 0.9:** defensible, but argued in a circle through question 66, and it crowds the riskiest work into the last two versions. Architects' early-staging option should be shown.
   - **The owners table:** mostly sound. The faults:
     - the KMS adapter has two builders;
     - friendship and the age check land on identity, the critical path;
     - the mailer has no version;
     - the case for posts owning notifications leans on a proposal.
5. **Questions.**
   - Mostly deduped and pre-filled.
   - Questions 4 and 5 are ruled but listed open.
   - The hard-deleted-handle fork has no question.
   - Questions 63 and 74 have no answer.
   - Question 65 is missing its door mark, and question 75 has its door backwards.
   - The age-check vendor fallback has no question at all.
6. **Shape.** One screen, plain words, ids only in parentheses. The counts in the summary are stale after the CSRF and soft-delete ruling.

## What went well

- Re-reading the two protocol specs turned an assertion ("staging only you can reach, signed in through the production client") into a cited must-fix rather than a hunch.
- Checking the code confirmed three claims the roadmap rests on: no outbox, the handle change's order, and the facts stub. One of them, the handle change, exposed a gap no team file flagged as missing from the roadmap.
- Reading compliance in depth, rather than identity alone, is what surfaced the cookieless-domain conflict with the age check. No single team file states it.

## What fell short

- I didn't read posts, plugin, vulpes or web end to end, so fidelity drops in those four may remain. Only rows that touched a finding were checked.
- The legal points (Brazil's per-access check, the UK adults-only relief, the operator's address) are flagged unverified for counsel. I didn't research them.
- `sed` is aliased to `sd` on this machine, and my first attempt to read code ranges with it failed. I used the Read tool instead. (Known pin; I should have used it first.)

## For next time

- Diff every ruling's full text against every row that touches it, not just the question it answers. R-6's "everywhere" reaches four teams' read paths, and only the question row said so.
- Treat "the merge picked one team's mechanism" as a fidelity finding even when it is credited to both teams: question 68 credited moderation for a gate moderation didn't propose.
