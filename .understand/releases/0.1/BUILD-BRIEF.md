# Build brief: release 0.1, Phase 1

From the Orchestrator (`claude`), 2026-10-10. The Engineer signed the PDD, TDD and plan today (`signoff.md`), so Phase 1 starts. Every builder reads this first, then their own work package.

## Where things are

- **The signed documents.** All are read-only for you. They live in `/home/zuri/code/zurfur-workspaces/orchestrator/.understand/releases/0.1/`:
  - `plan.typ`: find your work package's row (`[WP-n]`);
  - `tdd.typ`: your feature's sections, "One-way doors in 0.1" and "Parked: not in 0.1";
  - `pdd.typ`;
  - `rulings-2026-10-09.md`: every D-n ruling;
  - `objectives/<team>.md` and `objectives/join-<team>.md`;
  - `v2-carryover.md`: rows the plan cites.
- **Your workspace:** `~/code/zurfur-workspaces/<your name>`, a jj workspace whose working copy sits on `feature/my-den`, which was cut at `main`'s tip.
- **The rules:** `CLAUDE.md` in your workspace, especially "Definition of Done", "Code style (semantic)" (read the corpus page it names), "Branch Strategy" and "Parallel work". Then read `just nodes chain <path>` for every directory you touch.

## How to build

1. **Register on the board first:** `register` with your name, your team and role (`programmer`, or `lead` for a lead). Talk to the Orchestrator (`claude`) by board mail; bodies ≤2 KB, so put detail in a file.
2. **Tests first,** from the plan row and the TDD. Never fake a pass.
3. **One slice is one jj change** on `feature/my-den`. Describe it `[my-den][<slice#>] <Name>`, using the slice number your brief gives you. Bookmark it `feature/my-den-<slice#>-<slug>`.
4. **The gate is `just gate`.** It runs fmt, clippy, tests, deny, typos and the design-index check. Check each step's exit code, not a piped summary. Integration tests need a container runtime socket.
5. **Migrations** are made with `just migrate-add <name>` only.
6. **Push** with `jj git push -b <bookmark>`. Open the PR into `feature/my-den` **as a draft**, titled like the change. Request Copilot:
   `gh api --method POST repos/ZurNetwork/ZurFur/pulls/<N>/requested_reviewers -f 'reviewers[]=copilot-pull-request-reviewer[bot]'`
   End the PR body with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`. End commit messages with `Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>`.
7. **Don't merge, don't mark the PR ready, and don't restack anyone else's branch.** The team's QA, then Heron's security review where your row says `S`, then the Engineer come after you.

## Stop and ask

- **A domain fork** (how something is modeled, a name, an invariant, a stored shape, a contract): stop that piece. Write the question, carrying your answer, in the PR body under "Questions for the Engineer", and board-mail `claude`. Keep building the rest.
- **One-way doors in 0.1** (the TDD lists them) wait for the Engineer's explicit review before they merge. Say so in the PR body.
- **Something contradicts the signed documents:** don't pick a side. Board-mail `claude` with the file and line. The Engineer re-checks any change to a signed document.

## Report (your last message)

- the PR number(s) and change id(s);
- `just gate`'s exit code, and each step's exit code if it failed;
- the plan row's done-when, ticked or not;
- questions for the Engineer;
- anything you didn't build, and why.
