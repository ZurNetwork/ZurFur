# Zurfur — Agent Instructions

Zurfur is an AT Protocol-native art commission platform in Rust (edition 2024).
Architecture is ports & adapters:

- `backend/crates/shared` — build-time constants every layer may read, the domain crates included
- `backend/crates/kernel` — the base domain crate: the clock, the string builder, `Did`, text, maturity, `BlobId`, `Markdown`
- `backend/crates/identity` — User, Account, Character, actor identities, handles, roles, invitations, profiles, public records, and their ports
- `backend/crates/workflow` — Commission and Workflow boards, and their ports
- `backend/crates/gallery`, `backend/crates/moderation` — stub crates until their first types
- `backend/crates/domain` — the façade: re-exports the domain crates that hold types as `domain::…` and owns the cross-domain unit of work, its only code of its own; no IO
- `backend/crates/atta` — the Den's engine
- `backend/crates/application` — use cases, called by every driver
- `backend/crates/adapter-pg` — PostgreSQL adapter (private data boundary; sqlx; migrations live here)
- `backend/crates/adapter-atproto` — AT Protocol adapter (public data boundary)
- `backend/crates/adapter-mem` — in-process fakes for both boundaries
- `backend/crates/composition` — composition root: config and the port bag; the only crate that knows which adapter is live
- `backend/crates/api`, `backend/crates/cli` — the HTTP and terminal drivers

Dependency rule: adapters depend on domain crates, never the reverse; consumers name only `domain::…`; domain crates point one way (kernel ← identity ← workflow) and never depend on the façade, an adapter or a driver; only `composition` knows which adapter is live.

## Commands

- `cargo build` / `cargo test --workspace` (integration tests spin up their own testcontainers Postgres; a container runtime must be available)
- `cargo fmt --all` and `cargo clippy --workspace --all-targets` must be clean before claiming a task done
- `just dev` / `just up` / `just down` / `just db-shell` for the dev stack
- `just migrate-add <name>` is the ONLY way to create a migration — never hand-write migration filenames

## Conventions

- Doc comments are `///` directly on the item; plain `//` inside bodies
- New Postgres tables use singular names (`commission`, not `commissions`)
- All private-store writes go through a `UnitOfWork` from `Database::begin()` — never a bare pool; it won't compile otherwise
- API errors are RFC 9457 problem+json (`urn:zurfur:error:*`); success responses are bare, no `{data, error}` envelope

## Your lane

You handle mechanical, locally-verifiable work: doc comments, test scaffolding, commit-message drafts, renames, formatting, explaining failures.

- Do NOT make design or domain decisions (entity modeling, naming, schemas, API shapes, invariants). If a task requires one, stop and report it as an open question instead of choosing.
- Never push to `main`. Never commit unless explicitly asked.
- Design truth lives in the design corpus — the sibling repo `ZurNetwork/zurfur-design`, checked out at `~/code/zurfur-design`, one Markdown page per file under `entities/ decisions/ project/ data-layer/ superseded/`. Match the topic in `docs/design-index.md`, then read `<id>-<slug>.md` in the corpus, or `rg` it. Tickets stopped on 2026-09-19; Confluence is a frozen archive, not a source. Never guess a design decision: read the page, or say you couldn't resolve it.
