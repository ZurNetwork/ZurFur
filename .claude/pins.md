# Project pins

Standing facts about how this repo is worked, re-stated to every subagent
at start and to the main session after compact/resume (scripts/hooks/pins.sh).
One line each; `[src: …]` names where the fact is recorded. Limit: 40 pins,
8,000 chars. A machine-specific line lives in the untracked, user-level
`~/.claude/pins.local.md` instead — see CLAUDE.md.

- Every domain decision — an entity's shape, a name, an invariant, a boundary, an API contract, a schema choice, a trade-off with two defensible answers — is the Engineer's; Claude lays out options with a recommendation and stops at the fork. [src: CLAUDE.md › Roles & decision authority]
- Domain-heavy tickets are the Engineer's to implement; Claude's lane is mechanical work and the execution of settled decisions. [src: CLAUDE.md › Roles & decision authority]
- Design truth is the corpus at `~/code/zurfur-design`, indexed by `docs/design-index.md`; a design fact is read from its page before it is asserted. Confluence and memory paraphrases are not sources. [src: CLAUDE.md › Project]
- A page under `superseded/` governs nothing; its index line names the successor. [src: docs/design-index.md]
- `main` is never pushed to directly; work lands as `[ZMVP-N][slice#]` slice PRs into a feature branch cut at `main`'s tip, and every commit on that branch is gate-green. [src: CLAUDE.md › Branch Strategy]
- A command counts as passed on its exit code and its inspected output together; a piped summary is not a gate. [src: retros 2026-07 (h0rnz), 2026-09-17]
- Tests live in sibling files (`foo.rs` + `foo/tests.rs`), never inline; there is no `mod.rs`. [src: Engineer ruling 2026-09-14]
- Every derive is path-qualified and never imported; error enums derive `thiserror::Error`; `Deref` is banned on domain newtypes. [src: 37519361 ruling 9 · 64290818]
- A doc comment says what the item does in 1–4 lines and carries no DD number, page id, Jira key or PR number; code→design pointers live in the nearest `NODE.json` under `refs`. [src: CLAUDE.md › The node tree]
- New Rust follows the semantic style rulebook: newtypes for domain primitives, multi-line constructions named into a `let` (tests too), `ok_or_else` / `let-else` / `map_err` over match plumbing. [src: 37519361]
- Enum variant lists come from `strum::VariantArray` (`Self::VARIANTS`), never a hand-written `const ALL`. [src: Engineer ruling 2026-09-14]
- Postgres table names are singular (`commission`, not `commissions`). [src: Engineer ruling, ZMVP-65]
- Migrations are created with `just migrate-add NAME`; a hand-typed version collides across branches. [src: CLAUDE.md › Configuration & database]
- Nothing spans both stores in one transaction: a Postgres write plus a PDS record is an outbox-style retryable dual write. [src: CLAUDE.md › Architecture]
- Actors are addressed by DID on wire, domain and store (`UserId(Did)`); there is no surrogate UUID for an actor. [src: 57081857]
- Work is sliced by module (User, Character, Account…), not by layer. [src: Engineer directive 2026-08-23]
- A security or protocol fact is cited from a checked source or marked unverified; a "settled" claim in a handoff is verified against its design page before it is relied on. [src: CLAUDE.md › Memory & references]
