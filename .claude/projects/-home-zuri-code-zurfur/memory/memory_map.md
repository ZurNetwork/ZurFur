---
name: repo memory map
description: Where knowledge lives in the zurfur repo — knowledge-homes, their CLAUDE.md / memory status; the placement plan for /optimize-memory
type: reference
---

Compact map of the repo's **knowledge-homes** (rebuilt by `/optimize-memory`, last pass 2026-06-30). Facts live in **memory**; a folder `CLAUDE.md` just ensures the right memories/DDs get checked there. Confluence DESIGN is the source of truth; `docs/confluence-design-index.md` is the page pointer index.

- **root** — `CLAUDE.md` ✓ (project, roles, DoD, architecture, branch strategy, **Memory & references** convention).
- **backend/crates/domain** — pure domain entities/ports. No CLAUDE.md (root architecture covers it). Per-entity design = DESIGN glossary pages.
- **backend/crates/adapter-pg** — private data boundary. `CLAUDE.md` ✓ → DDs `24150017` (UoW/transactions), `10354698` (Data Boundaries), `10125341` (Blobs/PDS).
- **backend/crates/adapter-atproto** — public data boundary. `CLAUDE.md` ✓ → DDs `4358151` (DID:PLC), `10354698`, `9207856` (Platform Authority), `10125341`.
- **backend/crates/adapter-mem** — both boundaries faked in-process. No CLAUDE.md (root covers).
- **backend/crates/api** — composition root; HTTP/config/tracing. No CLAUDE.md yet; governing DDs `23592962` (RFC 9457 error model), `24543244` (Auth Surfaces/CSRF) — add a checkpoint here if api-local questions recur.
- **frontend/auth** — Leptos OAuth frontend. No memory yet.
- **docs/** — `confluence-design-index.md` (DESIGN pointer index, snapshot 2026-06-30, 53 pages — verified current).
- **scripts/** — `worktree-init.sh` (per-worktree DB/port isolation), `uow-status.sh` (SessionStart hook surfacing in-flight unit of work).

Memory layer: repo-local memories = feedback (tests, no-generic-names, readability, assign-before-Ok), vision, this map; global = `project-zurfur`. Slug is `-home-zuri-code-zurfur` (fixed from a stale `-Documents-zurfur` on 2026-06-30).
