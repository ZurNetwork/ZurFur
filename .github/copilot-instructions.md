# Copilot code review instructions — Zurfur

House rules for reviewing this repo (an AT Protocol-native art commission
platform in Rust; ports & adapters: `domain`, `adapter-pg`,
`adapter-atproto`, `adapter-mem`, `api`).

## Semantic style (binding for new Rust code)

Canonical source: Confluence DESIGN page 37519361, "Code Style — Semantic
Rulings (Rust)" (pointer: repo CLAUDE.md, "Code style" section). Formatting
is rustfmt's job — these are shape rules:

- **Newtypes** for domain-meaningful primitives (ids, names, handles, seats,
  kinds); a bare `u64`/`String` shouldn't cross an API surface. Generated
  code (`queries.rs`, see below) is exempt.
- Small one-line `return Err(...)` guards are fine (ruling 2) — do not flag them.
- **Multi-line constructions are named into a `let`** first, then used or
  returned — in tests too (name the expected value, then assert against it).
- **Combinators over `match`** for Option/Result plumbing: `ok_or_else` for
  Option→Result, `let-else` for early-out binding, `map_err` for error
  mapping. `match` stays for genuine multi-way logic.
- **Clarity beats brevity.** The longer form is correct when it's easier to
  read; don't flag verbosity that's in service of comprehension.
- **Std-trait-first.** A conversion/parsing/validation/construction surface
  should be `From`/`TryFrom`/`FromStr`/`Display`/`Default`/`IntoIterator`/`AsRef` before
  it's a custom trait or inherent constructor (`try_new`, etc.); a custom
  shape needs a one-line doc-comment justification, not just an absence of
  one.
- **Builder naming.** A chainable construction type is `XBuilder`; its
  terminal method is `build()`.

## Repo facts (avoid false positives)

- `backend/crates/*/src/queries.rs` is **generated** (`just gen-queries`) —
  don't style-review it; flag only if it looks stale vs. its `queries/*.sql`.
- New Postgres tables are **singular** (`commission`, not `commissions`) —
  don't flag singular table names.
- API errors are **RFC 9457** `problem+json` (a `urn:zurfur:error:*` type +
  a `code` field) — there is no `{data, error}` envelope; don't ask for one.
- Private-store writes only compile against a `UnitOfWork` (a transaction-
  bound handle from `Database::begin()`); a bare-pool write is a compile
  error by construction, not a missing-transaction bug to flag.
- Migrations are created with `just migrate-add <name>`, never a hand-typed
  filename/timestamp.
- `&OsStr == &str` **compiles** (std's `impl PartialEq<str> for OsStr`) —
  don't flag it as a type error. A past review round did, on PR #133, and
  was declined with evidence (green clippy + test CI on exactly that code).

## Tone

Prefer a concrete failure scenario ("this panics when X", "this rejects a
valid Y") over a style opinion rustfmt/clippy already enforce — if clippy
would catch it, it doesn't need a comment here.
