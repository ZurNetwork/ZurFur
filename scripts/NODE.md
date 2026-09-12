---
path: scripts
charted: 2026-09-12
fs:
  - name: pds-provision.sh
    role: create/sign into the fixture test account on the local dev PDS, print session JSON
    node: false
  - name: pds-smoke-test.sh
    role: shell-level scripted proof the local PDS+PLC dev loop boots and is repeatable
    node: false
  - name: web-smoke.sh
    role: shell-level scripted proof the one-origin Caddy routing split works
    node: false
  - name: jj-push.sh
    role: pre-push gate for jj — fmt + lint before code leaves the machine
    node: false
  - name: worktree-init.sh
    role: give a git worktree its own isolated dev-loop ports/DB/compose project
    node: false
  - name: uow-status.sh
    role: SessionStart hook — print in-flight unit-of-work status from the ledger
    node: false
---
**Is:** Shell scripts backing the `just` dev-loop recipes and jj/session hooks — not cargo tests, but scripted proofs and glue for the manual dev loop.

**Conventions:** `ZURFUR_*` env convention (env wins over `.env.example` defaults); each smoke script proves the dev loop itself at the shell/`just` level, distinct from the automated per-test-throwaway integration harness.

**Entry points:** invoked via `just` recipes (`pds-provision`, `pds-smoke`, `web-smoke`, `worktree-init`) — see repo-root `Justfile`; `uow-status.sh` and `jj-push.sh` run as hooks, not `just` recipes.

**Refs:** none — the ticket references in these scripts are in-body `#` comments, not doc comments, and stay where they are.
