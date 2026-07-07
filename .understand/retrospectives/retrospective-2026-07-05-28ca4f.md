# Retrospective — Epic ZMVP-101 "The Twenty One" · uow `28ca4f`

*Written by Claude (Fable 5 conducting; Opus/Sonnet/Fable build lanes), 2026-07-05. Range: 2026-07-04 → 2026-07-05, PRs #94–#98.*

### Summary

I drove the full record-write-path epic from expansion to closure in two days: six tickets across four dependency waves, five PRs merged to `main` (374b084 #96, e992872 #94, dd2e920 #95, 342a7a1 #97, 798a6cf #98 — ~4,600 insertions across 45 files), one ticket skipped by its own decision rule (ZMVP-107), one successor ticket carved (ZMVP-108), and the epic closed in Jira with its exit criterion *empirically* proven: a Class A `app.zurfur.feed.post` with blob, written through a new domain port, survives wipe-and-replay with identical record/blob CIDs on two fully separate PDS instances, in CI. Mid-epic, the Engineer granted epic-scoped domain-decision delegation; I ruled eleven forks under it (105's five, 106's six) plus the 107 skip, each recorded transparently with a veto path.

### What went well

- **Determinism was encoded as the pass condition, not argued.** The capstone (`wipe_replay.rs`, #98) asserts `record_cid₁ == record_cid₂` across two fresh PDS instances — the epic's hypothesis is now a green test that will fail if it ever stops being true. The briefing agent verified the protocol soundness of that assertion against the atproto Repository spec *before* I ruled it in (record CID = deterministic DAG-CBOR content hash; commit CID/`rev` per-instance — never compared).
- **Hermeticity is enforced, not configured.** 102's runtime egress probe (with a positive control), 103's tripwire test, and the `internal:true` network mean "fully offline dev" is a tested invariant. This came up the same evening when the Engineer asked whether the stack runs offline — the answer was strong because the guarantee is mechanical.
- **The delegation protocol held its shape.** Every ruling (F1–F6, the 107 skip) shipped with evidence, a named veto path, and durable records (ledger `decisions{}`, Jira comments, ZMVP-108 as the F1 debt's owner). Notably, F6 *overturned* the ledger's own "AC4 is by-review-only" assumption — the briefing found the mechanical check the planning phase had missed.
- **Memory-driven discipline paid off visibly.** `verify-settled-claims` caught that the "fix 2 ticket factual errors" TODO was already done (I verified instead of re-applying); every push was lease-keyed to a fetched oid; every gate was judged on emitted output (`test result:` lines, probe output), not exit codes. The red-first TDD evidence in #98's build (W2 panicking for the right reason, W1 failing on the missing helper) made "green" meaningful.
- **Review loops were single-pass and fully closed.** Both `/address-comments` rounds (#97, #98) went comment → verified-against-code → fix → re-gate → reply-with-sha → resolve in one pass each. No thread left dangling; no "ready to merge" claimed while fixes were in flight.
- **Zero loose ends.** A `TODO|FIXME|HACK|XXX` sweep of everything the unit touched returns nothing. The one piece of deliberate debt (the four publish rules) has a ticket, not a comment.

### What could be better

- **My own spec was the blind spot that every downstream check inherited.** The AC4 guard (#98) shipped ignoring the `$type` *value* — a same-shaped record under `app.zurfur.feed.post.draft` would have passed the "no draft fields" tripwire, which is literally AC4's concern. I specced F6, the briefing grounded it, the build implemented it faithfully, and the Opus security review **passed** the "boundary field set bounded" check — all verifying conformance *to my spec*, none re-deriving what the guard must reject from the threat. Copilot, reading cold, caught it (comment 3525722391). The pipeline was thorough and still monocultural.
- **Contract-fidelity gaps slipped my pre-PR reviews twice on #97.** `upload_blob` returned the request byte-length while `BlobRef`'s docs promise "the size the repo recorded" (3525595667), and `AtUri::parse` accepted `?`/`#` its own doc-comment promised to reject (3525595672). Both are doc-promise-vs-behavior diffs — a lens my `/critique` and security passes didn't apply, and external review did. Across the unit, external review's real catches (8) cluster exactly here: contract fidelity and guard/assertion completeness (`down -v` wiping the dev volume, the 2-line `internal:true` grep, `error_for_status` ×2, `$type`).
- **I broke the ledger JSON with a hand edit** (trailing comma) mid-session. I caught it myself on the next action because validating after structured-file edits is now reflex — but the correct reflex is validation *in the same breath* as the edit, not on the next touch. A second, smaller instance of the same genus: an inline match-arm edit that failed `cargo fmt --check` on the re-gate.
- **The corpus changed under the unit once** (the Replyable unification superseding 104's interviewed spec, 2026-07-04). The system worked — the agent was killed before writing anything stale, the audit confirmed self-consistency — but the cost was a full agent run. Cheap insurance exists: a corpus-freshness check (page version stamps) before launching a build whose spec came from an interview hours earlier.
- **Cosmetic but real:** build-lane commits carry `Co-Authored-By: Claude Fable 5` even when Opus built (#98's branch commits), because agents inherit the conductor's env template. Attribution in history is slightly wrong; worth a template tweak if the Engineer cares about lane forensics.

### What I should change

1. **Give every guard/tripwire one reviewer that argues from the threat, not the spec.** Concretely: when I author a validation/security-relevant spec, the verification prompt must include "enumerate what this must *reject*, from scratch, and diff against the implementation" — not "confirm the implementation matches the spec." One cold-reader lens per pipeline breaks the monoculture that let `$type` through.
2. **Add a contract-fidelity pass to pre-PR review:** for each changed public item, diff its doc-comment promises against observable behavior (returns, accepted inputs, error paths). Both #97 findings and the `BlobRef` case were mechanically findable this way.
3. **Validate structured files at edit time** — `jq`/parser check chained onto the same command batch as any hand edit of JSON/TOML/YAML state files, and prefer `rustfmt`-shaped code in inline Rust edits (or run `cargo fmt` before the gate, not as the gate).
4. **Stamp corpus versions into build briefs** — when a build's spec derives from Confluence, record the page version at brief time so a pre-launch freshness check is a cheap diff, not a full re-audit.

### Path forward

- **ZMVP-108** (publish rules) sits in backlog and activates with the publish/compose feature — where the re-homed OAuth-vs-real-network validation from 107's closing comment also lives.
- **Standing Engineer flags** (all in the ledger, none blocking): `snapshot` shape (Lexicon page lists it; repo JSON omits it additive-later), the 104 ratify-flags (text 3000-grapheme cap; labels-required Safe=empty encoding), the AC4 exact-pin tripwire (any additive lexicon field must update it deliberately).
- **Housekeeping on offer:** the `/save-reference` sweep for the post-July-2 DESIGN pages (EXP & Levels, Non-Toxic Path, Changelog, Phases, Friendship); `.understand/` snapshots remain untracked on `main` by longstanding status quo — worth an explicit convention call someday.
- **Next unit:** `/next-path` or `/create-epic` — the identity epics' open tails (ZMVP-51 monitor, ZMVP-52 recovery key, ZMVP-53 KMS) and the Engineer's in-flight ZMVP-46 lane are natural candidates.

*Lesson #1 distilled to memory as `feedback_review_specs_against_threat`; #2 folded into its How-to-apply. #3 and #4 judged too session-local to be memories — recorded here only.*
