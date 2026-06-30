# Epic authoring — ZMVP-13 "The Citizen (Accounts)"

- **Date:** 2026-06-30
- **Mode:** authoring an *existing* epic (not picking a new one) + gap check, at the Engineer's direction.
- **Cache:** none — this is the first `/create-epic` run; `.understand/epic-planning/` created here.
- **Source digests (4-way parallel fan-out):**
  - 🟦 **Confluence DESIGN:** 10 pages, 5 DDs (all DECIDED). Account (1966081), Roles (2162692), default-handle DD (24870914), deletion DD (23003138), handle-swap DD (21594113), invitation-departure DD (24182820), collection DD (24182787), Project MVP (589826), User (786439), auth/CSRF DD (24543244). Open items live in the DDs (verbatim, below).
  - 🟩 **Jira ZMVP-13:** already a well-formed epic — strong description, 10 children (7 Done), open: ZMVP-30/33/34. Orphans in scope: ZMVP-44/40/29.
  - 🟨 **Local UoW:** last unit `b722f9` COMPLETE (ZMVP-26 ∥ ZMVP-38 merged); nothing in flight. Account code exists for entity/roles/invitations/membership/leave/revoke; absent: provisioning, handle infra, deletion flow, write-gate.
  - 🟥 **Retros:** verify "settled" per-fork against DESIGN (don't trust the ledger); enumerate all call sites of a multi-site invariant; do ZMVP-39 router split before the next account-area batch.

## Decision

**ZMVP-13 was already substantially authored** (Context · Exit criterion · Pull rule · 10 Covers · Settled gaps · Excluded). No body rewrite needed. The Engineer disposed on the open forks; recorded everywhere:

**Confluence**
- **Roles page (2162692) → v8:** added rule 8 "Transferring ownership" — on transfer the chosen member becomes the new `Owner` (root, no parent); the **outgoing `Owner` becomes an `Admin`**, re-homed under the new `Owner`. Engineer's call; **no standalone DD** (their choice).

**Jira hygiene**
- Renamed ZMVP-13 → **"The Citizen (Accounts)"** (placeholder retired).
- Transitioned ZMVP-13 To Do → **In Progress** (7/10 children Done; epic had never moved).
- Reparented under ZMVP-13: **ZMVP-44** (handle infra), **ZMVP-40** (departure-invite revoke), **ZMVP-29** (Roles reconcile).
- Related (not reparented): **ZMVP-23**, **ZMVP-24** (CSRF / session-fixation — supporting auth family).

**New child tickets created (under ZMVP-13)** — from DD 24870914 open items:
- **ZMVP-45** — Reserved-label list for `*.zurfur.app` (extracted from ZMVP-44's open-items; relates ZMVP-44).
- **ZMVP-46** — Account-handle change flow (post-onboarding; ⚠️ change-flow design still open; relates ZMVP-44).
- **ZMVP-47** — Enforce the onboarding write-gate across all write routes (single shared enforcement point; relates ZMVP-30).
- ZMVP-44 description cleaned to point at ZMVP-45 + the pending punycode ticket.
- ZMVP-33 carries a comment recording the transfer rule + link to Roles rule 8.

**DoD of the epic** (unchanged, from its description): *A signed-in User can create an Account, staff it with consenting members under roles, transfer its ownership, and delete it.* Remaining build work: **ZMVP-30** (provisioning + handle choice), **ZMVP-33** (transfer — now design-complete via Roles rule 8), **ZMVP-34** (deletion/tombstoning), plus the new ZMVP-45/46/47 and infra ZMVP-44; punycode policy pending a DD.

## Candidate table (coverage matrix, frozen this run)

| Capability (Confluence) | Ticket | Status |
|---|---|---|
| Account entity + dual identity | ZMVP-14 | Done |
| Roles + parent-tree, rank-strict grant | ZMVP-15/16, ZMVP-29 | Done |
| Invitations (issue/accept) | ZMVP-20/32 | Done |
| Bulk-revoke invites on departure | ZMVP-40 | Done |
| Leave (self-service, re-home) | ZMVP-21 | Done |
| Cross-persona unlinkability | ZMVP-17 | Done |
| Default-account onboarding + handle | ZMVP-30 | To Do |
| `*.zurfur.app` resolution + alsoKnownAs | ZMVP-44 | To Do |
| Ownership transfer (old Owner→Admin) | ZMVP-33 | To Do (design now complete) |
| Account deletion / tombstoning / reuse | ZMVP-34 | To Do |
| Reserved-label list | ZMVP-45 | To Do (new) |
| Account-handle change flow | ZMVP-46 | To Do (new) |
| Write-gate enforcement | ZMVP-47 | To Do (new) |

## Cached for next time

- **Punycode / confusable policy for BYO domains** — ✅ **DONE this run.** Decided via `/design-decision`: reject any `xn--` label (both namespaces) for v1; UTS #39 allow-with-checks is the documented upgrade path. DD page **26050561** "Confusable Handles & the Punycode Policy" (DECIDED 2026-06-30); implementation **ZMVP-48** under ZMVP-13; ZMVP-44 pointer + DD 24870914 open item both updated; memory `project_punycode_handle_policy` filed. *(Residual future work: the UTS #39 upgrade — no ticket; raise when international demand appears.)*
- **ZMVP-46 change-flow design** — only the *initial* handle choice is specified; who/how-often/old-handle/BYO-rebind for *changing* a handle is open. **Trigger:** when ZMVP-46 is picked up, route the fork to `/design-decision` before building.
- **ZMVP-39 router split** — retro says do it before the next account-area batch (the flat `api/src/lib.rs` router is the collision chokepoint). **Trigger:** before parallelizing ZMVP-30/34/47, which all touch account routes.
- **Previous-participants roster scope** (client- vs creator-side, opt-out) — DD 21594113 open item; leans on the post-v1 Level system. Frontend-era; not ZMVP-13 build scope.
- **Published-product reference location vs account deactivation exemption** — DD 23003138 build-time detail; surfaces when ZMVP-34 is built.
- **Invitee notification / revoke-reason on auto-revoke** — DD 24182820; deferred to the notification service.

## Open threads

- `/design-decision` for punycode/confusable policy — **invoked at the end of this run** (interactive; creates the implementation ticket + updates ZMVP-44).
- Stale DESIGN links to fix when convenient: Account + User pages reference the SUPERSEDED "MVP & Roadmap" (3670017); User page (oldest, May 27) predates onboarding/write-gate and says "zero or more accounts" — reconcile with first-account-on-login (a `/design-sync` candidate, not done here).
- Worth fetching **Blocking Gaps for v1 (9994307)** when scoping the remaining ZMVP-30/34 build.
- ZMVP-30 onboarding is interactive handle-choice behind a hard write-gate (not a silent mint) — build accordingly; ZMVP-47 enforces the gate.
