# Zurfur — Agent Instructions

Zurfur is an AT Protocol-native art commission platform in Rust (edition 2024).
Architecture is ports & adapters:

- `backend/crates/domain` — pure domain types and ports (traits); no IO
- `backend/crates/adapter-pg` — PostgreSQL adapter (private data boundary; sqlx; migrations live here)
- `backend/crates/adapter-atproto` — AT Protocol adapter (public data boundary)
- `backend/crates/adapter-mem` — in-process fakes for both boundaries
- `backend/crates/api` — composition root: axum HTTP, config, tracing

Dependency rule: adapters depend on domain, never the reverse; only `api` knows which adapter is live.

## Commands

- `cargo build` / `cargo test --workspace` (integration tests spin up their own testcontainers Postgres; a container runtime must be available)
- `cargo fmt --all` and `cargo clippy --workspace --all-targets` must be clean before claiming a task done
- `just dev` / `just up` / `just down` / `just db-shell` for the dev stack
- `just migrate-add <name>` is the ONLY way to create a migration — never hand-write migration filenames
- After changing any sqlx `query!` macro: run `cargo sqlx prepare` and include the `.sqlx/` changes

## Conventions

- Doc comments are `///` directly on the item; plain `//` inside bodies
- New Postgres tables use singular names (`commission`, not `commissions`)
- All private-store writes go through a `UnitOfWork` from `Database::begin()` — never a bare pool; it won't compile otherwise
- API errors are RFC 9457 problem+json (`urn:zurfur:error:*`); success responses are bare, no `{data, error}` envelope

## Your lane

You handle mechanical, locally-verifiable work: doc comments, test scaffolding, commit-message drafts, renames, formatting, explaining failures.

- Do NOT make design or domain decisions (entity modeling, naming, schemas, API shapes, invariants). If a task requires one, stop and report it as an open question instead of choosing.
- Never push to `main`. Never commit unless explicitly asked.
- Design truth lives in Confluence (space DESIGN); work is tracked in Jira (project ZMVP). You have READ-ONLY access via the `atl` CLI (see the `atlassian` skill): match the topic in `docs/confluence-design-index.md`, then `atl page <id>`; tickets via `atl issue ZMVP-N`. Never guess a design decision — fetch the page, or say you couldn't resolve it.
