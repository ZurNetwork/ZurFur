---
path: backend/crates/cli/src/commands
charted: 2026-08-29
fs:
  - name: mod.rs
    role: declares the account/health/migrate/session submodules
    node: false
  - name: account.rs
    role: account namespace — create/delete, thin CLI face over runtime.app().accounts().{create, delete}, delete gated by crate::confirm
    node: false
  - name: health.rs
    role: zurfur health — same probe as GET /health via adapter_pg::is_reachable plus latency
    node: false
  - name: migrate.rs
    role: zurfur migrate — applies embedded sqlx migrations, idempotent, exempt from schema-drift gate
    node: false
  - name: session.rs
    role: session namespace — whoami/logout read/clear the identity file; login stubbed not_implemented pending client-model ruling
    node: false
---
**Is:** One module per domain namespace exposing a `clap::Subcommand` enum plus `run(&Runtime, op) -> Result<Value, CliError>`; the root dispatch in `lib.rs` routes to it.

**Entry points:** `commands::account::run` · `commands::health::run` · `commands::migrate::run` · `commands::session`.
