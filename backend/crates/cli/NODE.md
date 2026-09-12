---
path: backend/crates/cli
charted: 2026-08-29
fs:
  - name: Cargo.toml
    role: bin `zurfur`; deps on composition, application, domain, adapter-pg, clap; dev-deps assert_cmd/adapter-mem/test-support
    node: false
  - name: src/main.rs
    role: entry point — parse Cli, init tracing to stderr, run with ctrl-c racing to Interrupted exit class
    node: false
  - name: src/lib.rs
    role: Cli/dispatch, ExitClass, CliError, ties commands together; documents the stdout/stderr/exit-code conventions
    node: false
  - name: src/confirm.rs
    role: terminal confirmation gate for irreversible ops (Session enum, y/yes prompt on stderr, refuses non-terminal callers)
    node: false
  - name: src/identity.rs
    role: identity file — load/save, versioned JSON, atomic write 0600, database fingerprint
    node: false
  - name: src/output.rs
    role: stdout data channel — Format::Pretty/Compact JSON writer
    node: false
  - name: src/principal.rs
    role: Principal::resolve — identity file -> UserStore::find_by_did -> the acting User
    node: false
  - name: src/problem.rs
    role: CliError + ExitClass + Problem JSON rendering on stderr
    node: false
  - name: src/commands/
    role: one module per domain namespace (account/health/migrate/session)
    node: true
  - name: tests/common/mod.rs
    role: shared process-level test harness — spawns the real zurfur binary with env/config for tests
    node: false
  - name: tests/account.rs
    role: in-process/process tests for account create/delete
    node: false
  - name: tests/account_process.rs
    role: process-level (spawned binary) tests for account commands
    node: false
  - name: tests/health.rs
    role: tests for `zurfur health`
    node: false
  - name: tests/migrate.rs
    role: tests for `zurfur migrate`
    node: false
  - name: tests/session.rs
    role: tests for whoami/logout/login
    node: false
  - name: tests/in_process.rs
    role: in-process harness variant (adapter-mem ports, no subprocess)
    node: false
  - name: tests/process.rs
    role: process-level harness variant (real binary via assert_cmd)
    node: false
---
**Is:** The `zurfur` terminal driving adapter (epic ZMVP-199): one-shot clap subcommands that call the same application-layer use cases as the HTTP API, in-process via `composition::Runtime` — no HTTP, no contract, no token.

**Conventions:** stdout carries exactly one JSON value + newline on success (pretty by default, compact under `--json`); nothing else goes to stdout. stderr carries tracing diagnostics and, on failure, one compact JSON Problem as its LAST line regardless of `--json` — scripts parse `stderr.lines().last()`. Exit codes are the four ExitClasses: 0 ok, 1 domain error, 2 usage (clap), 3 infrastructure. Problem `code`s reuse the API's vocabulary (`api/src/problem.rs`, DD 23592962) where the same refusal exists, plus CLI-only codes (config, identity_*, not_implemented, cancelled, no_session). Commands are thin: each is the CLI face of the same application-layer use case the HTTP driver calls — no domain logic duplicated here. Irreversible operations go through `crate::confirm`: a person at a terminal is asked (only typed y/yes proceeds); anything without a terminal is refused, not defaulted — `--yes` is the explicit scripted opt-in. No `.env` loading in `main` — walking parent dirs for `.env` would leak `DATABASE_URL`/root key from an unrelated directory (security review, ZMVP-203 F1). The identity file (`$ZURFUR_CLI_HOME/identity.json` or platform config dir) is the one record of which User the CLI acts as; versioned JSON, written 0600 atomically, fingerprinted to its database.

**Entry points:** `src/main.rs` (bin: zurfur) · `src/lib.rs` (Cli, run(), dispatch, ExitClass) · `src/commands/mod.rs`.

**Refs:** DD 55836674 — The Application Layer · DD 23592962 — API Response Shape & Error Model · ZMVP-199 epic, ZMVP-201/202/203/204/205/206.
