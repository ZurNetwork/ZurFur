# Retrospective — uow `h0rnz` (ZMVP-39) + the `/next-path` round before it

*2026-06-30 · scope: one unit of work (PR #76) and the planning round that chose it · author: Claude*

### Summary
I planned the next parallel path off a freshly-completed unit (b722f9), then built and shipped the one ticket it recommended. `/next-path` fanned 5 `/understand` agents in parallel over a shortlist (ZMVP-39/27/48/47/44); the briefings revealed a **decision wall** — every candidate except ZMVP-39 was blocked on an open Engineer fork or unbuilt substrate — so I recommended **ZMVP-39 solo**. Built it as a behaviour-preserving refactor: split the flat 1192-line `api/src/lib.rs` route table into per-domain route groups (`routes/{health,session,accounts,mod}.rs`), with the CSRF Origin guard scoped onto the cookie surface per an Engineer decision. Merged → `5743f0b` (PR #76), Jira Done, worktree cleaned up.

### What went well
- **Planning bought real information.** The parallel `/understand` fan-out turned "which ticket next" into evidence-based triage *before* any branch was cut. It caught that ZMVP-27's event taxonomy is unpinned, ZMVP-48 has no handle-validation substrate (it rides behind ZMVP-44), and ZMVP-47's target routes don't exist — none visible from the Jira summaries. I didn't start a blocked ticket.
- **Didn't blindly resume a stale "in-flight" flag.** The SessionStart hook flagged uow b722f9 as in-flight (ZMVP-44 `planned`), but reading the ledger showed b722f9 was complete and ZMVP-44 was a parked record. I surfaced that instead of resuming a finished unit.
- **Read the *why*, not just the AC.** The ticket's goal was "stop the merge-conflict hotspot," so builder-fns-inside-`lib.rs` would have satisfied the literal AC while failing the intent. I did the real module split so a future domain adds a file without touching the others.
- **Respected decision authority on the one fork.** CSRF-layer placement shapes a security boundary → I asked rather than defaulted. The Engineer's "this allows read-based things too, right?" confirmed it was a genuine decision, not ceremony.
- **Verified the security claim concretely.** Rather than assert CSRF coverage from confidence, I enumerated every state-changing route and confirmed each sits under the layer, and diffed the guard logic to prove it byte-identical. Result: clean `/security-review`, clean Copilot review (0 comments), zero behaviour delta, all 13 test files green.
- **Mirrored CI's *environment*, not just its commands** — used `SQLX_OFFLINE=true` to dodge the live-but-dead `DATABASE_URL` trap the worktree `.env` set up.

### What could be better
- **A shell alias produced a false green I briefly trusted.** `sed` is aliased to `sd` in this shell; my first "CSRF byte-identical" check silently `diff`'d two empty files (both `sed` invocations had errored) and printed a confirmation anyway. I caught it and re-verified with `git diff`/`awk` — but for a moment a *security* verification rested on a command that hadn't actually run. The exit status was green; the output was bogus.
- **I wrote a verification whose failure mode was ambiguous.** The background `cargo test` "failed exit code 1" alarm was just my own `grep -c "FAILED"` returning 1 on *zero* matches — i.e. success looked like failure. A check shouldn't conflate "nothing wrong" with a non-zero exit.
- **Wasted compute re-running the workspace test suite ~3×** (a grep version, a count version, a foreground re-run that timed out) chasing "a cleaner summary," when I should have run it once, captured to a file, and parsed that.
- **A careless `Write` created a stray `routes.rs.tmp`** (immediately removed). Small, but avoidable.

### What I should change
1. **When a Bash command underpins a correctness/security claim, confirm it actually emitted the intended output — never read the exit code alone.** Shell aliases (`sed`→`sd` here) and grep-count semantics can both manufacture false greens. Prefer `git diff` / `rg` / `awk` over `sed` in this repo's shell.
2. **Run expensive gates once, capture to a file, parse the file.** Don't re-invoke `cargo test --workspace` for a prettier summary.
3. **Design verification one-liners so the exit status is unambiguous** — avoid `grep -c` as a pass/fail gate (zero matches → exit 1 reads as failure).

### Path forward
- **The decision wall is the headline for the next unit.** The cheap settled tickets are done; the next tier each needs one Engineer fork resolved. Highest-leverage move: a `/design-decision` — **event taxonomy** (unblocks ZMVP-27, feeds ZMVP-41) or **handle infra** (PLC key custody + resolution strategy; unblocks ZMVP-44 → then ZMVP-48/45/46). Recorded in the ledger's `backlog_decision_wall`.
- **One residual from this PR:** the scoped CSRF layer is fail-open for a *future* cookie sub-router that forgets to mount under `cookie_surface` (vs the old global fail-safe). Mitigated by the documented rule in `app()`/`routes/mod.rs` + the expectation that a new cookie router brings its own `csrf.rs` coverage. Worth a glance when the next cookie router lands.
- **Now that the router is split**, the in-flight `feature/openapi-infra` branch (code-first OpenAPI) will land against the new `src/routes/` layout — flag at its merge.
