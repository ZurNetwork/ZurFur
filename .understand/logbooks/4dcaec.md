# uow 4dcaec — ZMVP-44 (close-out) · ZMVP-46 (handle change flow) · ZMVP-47 (capability-scoped write gating)

Ledger: `.understand/parallel-set.json` · uow `4dcaec` · tickets ZMVP-44 / ZMVP-46 / ZMVP-47.

Named set. ZMVP-44 was already merged (#82) and the Engineer moved it to Done — dropped from the build
set. ZMVP-46 & ZMVP-47 delegated to **Engineer-driven background agents** so the interactive
back-and-forth (design decisions, domain forks) doesn't burn the main-thread context; the driver reads
each agent's result on completion and updates the ledger.

## Open threads
- [x] **ZMVP-46 change-flow policy — DECIDED (all 8 forks).** ship v1 / Owner-only / **Bluesky-style burst
  throttle** (not a 30d cooldown — verified Bluesky updateHandle ≈10/5min) / **quarantine `*.zurfur.app`-only,
  bounded, reclaimable, EXIT-EXEMPT** (BYO never quarantined) / replace aka (+ future Steam-style name-history,
  Name≠Handle, native to did:plc log) / BYO allow-all verify-bidirectionally-before-commit / DID-doc-first
  ordering (worst transient = `handle.invalid`) / ZMVP-50 builds the reusable signed update-op+outbox, ZMVP-46
  consumes. DD being written by the design-decision agent.
- [x] **ZMVP-46 DD written + propagated** — DD "Account Handle Change Flow" **27852802** (DECIDED). Jira:
  ZMVP-50 rescoped (reusable did:plc UPDATE-op + outbox), ZMVP-46 rescoped (policy+endpoint) `is blocked by`
  ZMVP-50, **ZMVP-64** filed (name-history/PDS-export, Low). Memory `project_account_handle_change` +
  glossary synced (Account 1966081→v12, DD 24870914→v4 open row RESOLVED). Credible-exit invariant is a DD
  callout. **ZMVP-46 design DONE this unit; build is a future unit gated behind ZMVP-50.**
- [ ] **Future / non-MVP (record, don't build):** Steam-style name-history; exporting our own copy of the
  user's PDS data on request.
- [x] **ZMVP-47 role-semantics — DECIDED: (A) flat membership floor returning `Role`.** Classification
  confirmed as-is + explicit constraint: gate WRITES only, public account reads stay anonymous-readable
  (discovery). Seam = `FromRequestParts` extractor. Build now + retrofit the 6.
- [x] **ZMVP-47 needs /security-review** — done on Opus, no findings (fails strictly more closed).
- [ ] **Skills run in cwd, not the worktree** (memory `feedback_skills_run_in_cwd_not_worktree`) — the
  ZMVP-47 agent must drive /security-review & /prepare-pr against the primary checkout / its branch head,
  not assume they act on the worktree.

## Log
- 2026-07-01T16:02 — uow minted. ZMVP-44 confirmed merged (#82, commit 5cf10a9) + Engineer moved Jira to
  Done → dropped. Both /understand briefings cached (read-only, no branch/worktree/Jira change). Spawning
  two Engineer-driven agents: /design-decision on ZMVP-46; /start→interview→build split on ZMVP-47.
- 2026-07-01T16:15 — ZMVP-46 agent presented the 8-fork slate (recs: defer build / Owner-only / cooldown /
  quarantine old handle / replace alsoKnownAs / verify-BYO-before-commit / DID-doc-first ordering /
  ZMVP-50 builds the reusable update-op) and parked awaiting Engineer. Agent not visible in the Engineer's
  UI (stopped agents drop off the active list) → driver will interview + relay.
- 2026-07-01T16:45 — **Model policy set** (memory `feedback_model_split_fable_impl_opus_design`):
  implementation agents → Fable 5 (cheaper); design/interviews + /security-review + adversarial verify →
  Opus 4.8. In-flight ZMVP-47 build kept on Opus to finish (sunk-cost + auth boundary); Fable-implements
  applies to the NEXT builds (ZMVP-50, then ZMVP-46). Guardrail: security-review stays on Opus even for
  Fable-built code. — Engineer green-lit the ZMVP-46 DD write-plan (as written; future items = DD section
  + backlog ticket). ZMVP-46 agent executing: DD page + Jira rescope (ZMVP-50=update-op+outbox,
  ZMVP-46=policy+endpoint blocked-by ZMVP-50) + backlog ticket + memory + /design-sync.
  NOTE: sqlx "connection refused" LSP diagnostics in the primary checkout are environmental (Postgres
  down + DATABASE_URL masks the .sqlx offline cache) — no code impact, agents/CI unaffected.
- 2026-07-01T17:45 — **UNIT COMPLETE.** ZMVP-47 shipped: PR #88 CI-green, security-review (Opus) clean,
  Copilot's 1 test-hardening comment closed in b905a06 (thread replied + resolved via /address-comments;
  verified no provisioning-order bug), Engineer squash-merged → main **91b5114**. Close-out: ZMVP-47 → Done,
  local main fast-forwarded, worktree + branch removed. ZMVP-46 design landed as DD 27852802 (build deferred
  behind ZMVP-50). Ledger reset to no-active-unit. Next unblocker: **ZMVP-50** (gates ZMVP-46's build).
- 2026-07-01T16:20 — ZMVP-47 agent finished grounding: worktree up, branch
  `feature/zmvp-47-capability-scoped-write-gating`, pg :28780, Jira In Progress. **Code ground-truth
  correcting the briefing:** the `require_user→load_account→actor_role` triple is in exactly **6**
  handlers (delete_account, grant_role, revoke_role, invite_user_to_account, revoke_invitation_to_account,
  transfer_ownership) — the clean retrofit set. **3 deliberately differ, keep off the seam:** leave_account
  (non-member→404, Owner→409), accept/decline_invitation (keyed on the session user's own invite, no
  actor_role). create_account is founding (require_user only). Parked awaiting Engineer forks 1&2
  (blocking) + 3&4 (sign-off). Agent's rec: (A) flat membership floor returning `Role` (target-relative
  grant/revoke rules can't collapse into a min-rank param); build now; `FromRequestParts` extractor seam.
