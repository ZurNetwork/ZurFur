# Retrospective — uow `t4bdk` (2026-06-30)

> First-person reflection on my own work this session. Unit: ZMVP-48 + ZMVP-45 (Handle validation newtype, PRs #77 + #80) ∥ ZMVP-36 (compile-enforced Unit of Work, PR #78), plus the parallel-lane DD **26607618** (ZMVP-44 resolution = HTTPS well-known) and the "Engineer implements domain work" workflow change. Ledger: `.understand/parallel-set.json`; logbook: `.understand/logbooks/t4bdk.md`.

### Summary

I ran one full unit of work end to end. I re-planned the backlog, built two settled-DD foundations in parallel isolated worktrees, resolved a real design fork *with* the Engineer as a concurrent domain lane, addressed a Copilot review on both PRs, and integrated everything to `main`. Landed: `53036cc` (#77, Handle newtype), `8d62757` (#78, Unit of Work), `4761a11` (#80, Handle doc cleanup) — all three tickets Jira-Done. Recorded DD 26607618. Changed the lifecycle itself (CLAUDE.md + `/next-path` + memory `feedback_engineer_implements_domain_work`).

### What went well

- **I refused to inherit a stale verdict.** The prior unit's ledger declared a "decision wall." Instead of accepting it, I re-checked live Jira (no hard blockers, only "relates to" links) and scouted the code — and found the wall was *partly an artifact* of ZMVP-39 hogging `api/src/lib.rs`, now merged. That re-examination is what surfaced two genuinely-workable foundations. This is exactly `feedback_verify_settled_claims_on_resume` paying off.
- **Separating validation from issuance unlocked the Handle work.** ZMVP-44's own briefing said it "consumes a validated handle" — so handle *validation* (ZMVP-48+45) was cleanly buildable now, independent of ZMVP-44's deep forks. Bundling 48+45 into one `Handle` newtype ("one gate, not three") matched the DD's own topology and shipped clean (#77, faithful `AccountName` mirror, no critique findings).
- **The parallel-lane model worked on its first real outing.** While my mechanical units built, the Engineer drove the ZMVP-44 resolution DD with me — a 3-agent cited investigation (spec + Bluesky prior art + ops/security) → DD 26607618. That fork was the *centerpiece* of the prior "wall," and it fell *during* the build wave instead of blocking the next one. I then encoded that as a standing rule so it isn't a one-off.
- **The UoW guard was built to actually bite.** ZMVP-36's `no_bare_pool_writes` guard was verified to *fail-loud* on planted violations, not just pass — and after hardening, on three of them (plain, whitespaced, cross-crate). That's `feedback_make_unsoundness_unreachable` and `feedback_verify_command_output_not_exit_status` honored in the artifact, not just the prose.
- **I verified recovery instead of trusting it.** When the ZMVP-36 fix agent reported a force-push regression "recovered," I independently checked PR #78's diff vs main (handle.rs untouched, only ZMVP-36 deliverables) before calling it ready. Good instinct given the stakes.

### What could be better

- **Two force-push incidents on live branches.** (1) The Handle ship-agent hit the already-merged-and-deleted #77 branch on `--force-with-lease` and stopped (correct). (2) The ZMVP-36 fix-agent's first push **clobbered `e4bd3c8`** — a main-merge that had externally integrated Handle #77 into the #78 branch — dropping it; it was caught from the push output and recovered (`reset --hard e4bd3c8` + cherry-pick + re-gate). Both ended clean, **but this is a real fragility pattern, not bad luck**: I dispatch fix-agents to *live, open PR branches* that the Engineer (or GitHub's "Update branch") may have moved underneath them, and the agents pushed without re-fetching. `--force-with-lease` only protects when the lease is keyed to the *current* remote — a stale local tracking ref defeats it.
- **A merge landed mid-review-fix and I didn't anticipate it.** #77 was squash-merged while I was still applying its Copilot cleanups, so commit `622ad7b` merge-missed and I had to re-land it as #80. I had told the Engineer "ready to merge whenever" *and* dispatched fix-agents to the same branch — those two facts were in tension and I didn't flag it.
- **Bookkeeping artifacts are still uncommitted on `main`.** CLAUDE.md (the new rule), `docs/confluence-design-index.md` (DD 26607618), the ledger, logbook, snapshots, and this retro are working-tree changes with no PR yet. A clean unit shouldn't leave its own paperwork dangling.

### What I should change

1. **Reconcile remote state before force-pushing to a live branch.** When resuming an agent to push to an *already-open PR branch*: `git fetch` first, rebase/replay onto the current remote tip, and key `--force-with-lease=<ref>:<fetched-oid>` to the freshly-fetched ref — never a stale local one. If the remote moved in a way that isn't a clean replay, **stop and report** rather than force. (→ new memory `feedback_reconcile_before_force_push`.)
2. **Don't say "ready to merge" and "I'm still pushing fixes" about the same branch.** Either hold the merge-ready signal until fix-pushes land, or explicitly tell the Engineer "don't merge yet — fixes incoming." Make the branch's mergeability state unambiguous.
3. **Close the unit's paperwork as part of integration**, via a small docs PR — not an afterthought.

### Path forward

- **Immediate:** land the bookkeeping (CLAUDE.md rule, DD index entry, ledger/logbook/snapshots, this retro) via one small docs PR to `main`.
- **Next unit — ZMVP-44**, opening with the Engineer's domain lane: the **rotation-key custody / real `did:plc` minter** DD (DD 4358151 "in review") — the last ZMVP-44 fork, gating the `alsoKnownAs` outbox write. Its serving half (well-known route + `Account.handle` field/migration + config) is now Claude-ownable thanks to DD 26607618 and the Handle newtype, and can build alongside.
- **Support the team better** by making the force-push safeguard a standing rule so the parallel-worktree flow stops risking externally-added commits.
