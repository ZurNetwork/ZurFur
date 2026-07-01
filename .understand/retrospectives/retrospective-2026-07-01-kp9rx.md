# Retrospective — ZMVP-34 (uow kp9rx), "Owner deletes an Account"

**Scope:** this one branch alone — `feature/ZMVP-34-owner-deletes-an-account`, merged as squash **`aa5af1e`** (PR **#86**). ~1hr+, single session. Reflection is first-person, as Claude.

**Shape of the change:** 24 files, +1392/−44. A soft/hard-delete write path, a spec-correct `did:plc` tombstone, a new `plc_operations` op-log (port + pg + mem), a corrected DD, a security review with five findings → two follow-up tickets, then a rebase over ZMVP-33 (which merged first) and five Copilot review comments.

## Summary

I took ZMVP-34 from a non-compiling pre-split WIP to a merged, security-reviewed feature. The Engineer's ask was narrow — "pull it into a worktree, add the ATProto parts" — but the ATProto half was the beast: a `plc_tombstone` operation signed with a custodied rotation key, crossing the private↔public boundary. I ran it in two phases (private deletion path green first, then the crypto), interviewed the Engineer at the genuine domain fork (what "deactivation" means in identity-only v1), corrected DD 23003138, ran an adversarial security review, filed ZMVP-56/57, opened a green PR, then rebased over ZMVP-33 and cleared five Copilot comments.

## What went well

- **Stopped at the real domain fork instead of guessing.** DD 23003138 was written assuming a PDS; DD 26935298 later made v1 identity-only (no PDS). I caught that drift and interviewed the Engineer on soft-delete semantics rather than inventing them — then recorded the answer by correcting the DD (v2, strikethrough on the superseded `active`-flag mechanics). This is the roles model working as intended.
- **Grounded the crypto in verified sources, not memory.** I fetched the did:plc spec for the tombstone shape and the *published* audit-log CID of the vector DID, and pinned my hand-rolled `cid()` to it with a vector test (`computes_the_known_vector_cid`) — the same discipline the existing DID-derivation vector uses. The tombstone test verifies the signature is low-S under the *operational* key. No security claim asserted from memory.
- **Surfaced scope discoveries as decisions, not silent expansion.** When I found that a spec-correct tombstone needs the DID's last-op CID as `prev` and nothing persisted it, I stopped and put the schema choice (column vs op-log table) to the Engineer rather than picking one to keep momentum.
- **Security review before the PR, and I ticketed rather than gold-plated.** The five findings were all retention/silent-failure/latent — none in v1 scope — so I filed ZMVP-56 (durability) + ZMVP-57 (has_facts tripwire) and noted F5 on ZMVP-51, instead of building outbox/retry/purge into this branch.
- **Respected the compile-enforced invariants.** The `no_bare_pool_writes` guard caught my new `PgPlcOperationLog`; I added a *documented* exemption with the same rationale as `key_store.rs` and flagged it for ratification, not a quiet edit.
- **Honest handling of external review.** Of five Copilot comments I fixed four and pushed back on the one false positive (`use sqlx::query` is what imports the `query!` macro) with evidence, rather than blindly "fixing" it and breaking the build.

## What could be better

- **I ran a subagent against the contaminated checkout.** My first surface-mapping Explore agent read the *main* checkout while the Engineer had uncommitted ZMVP-33 work there, so it reported `transfer_ownership` as already on `main`. I caught it by cross-checking the clean worktree — but I had *already* reset the worktree to clean main and should have pointed the agent there from the start. Cost: a detour and a re-verification.
- **I asserted a domain-scope fact from memory and was corrected.** My first interview question labelled commissions "post-MVP"; the Engineer corrected me ("commissions are NOT post MVP"). The whole `has_facts` gate hinges on exactly that, so it was the wrong thing to be casual about — "fetch before guessing" applies to scope facts, not just APIs.
- **My own review flagged the tombstone-ordering bug and I only ticketed it.** Security-review finding F4 noted append-before-submit was a smell for the retry path — and I deferred it to ZMVP-56 rather than applying the cheap reorder then. Copilot then flagged the same thing (#1/#2) and I fixed it in ten minutes. I had the insight and under-acted on it.
- **I declared the 403 test "infeasible with the harness" too early.** I reasoned the e2e harness signs in one DID so a non-Owner-member 403 couldn't be driven, and deferred it — but the sibling `tests/transfer.rs` seeds exactly that (owner-by-another-user + `backend.grant_role`) and I could have mirrored it. Copilot asked for the test (#5); the setup I then used was available all along.

## What I should change

1. **Point read-only subagents at the exact worktree path** and tell them not to read the sibling checkout when it's on another ticket. (Already captured as a memory this session.)
2. **Verify scope/domain facts before asserting them in an interview question** — "is X in MVP?" is a fetch, not a recall, especially when a gate depends on it.
3. **When my own review surfaces a cheap, correct fix, apply it in-branch** — don't only ticket it. Distinguish "defer (real scope)" from "apply now (cheap + correct)."
4. **Before calling a test infeasible, check how sibling test files set up the same scenario.** The harness usually already has the seam.

## Path forward

- **ZMVP-56** (tombstone durability: outbox/retry, submission-status column, post-window key purge) must land *before* the real PLC directory is switched on. **ZMVP-57** (has_facts tripwire) must land *with* commission storage.
- **F5** is noted on ZMVP-51 — keep that audit surface DID-scoped.
- Worktree/branch/tag cleanup for kp9rx is the last housekeeping step.
- Net: the beast landed clean and reviewed. The corrections above are all "act on what I already knew sooner," not "I missed the substance."
