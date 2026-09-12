---
path: frontend
charted: 2026-09-12
fs:
  - name: web/
    role: the SvelteKit + Svelte 5 + TypeScript app (the only frontend)
    node: true
---
**Is:** The client tier — exactly one SvelteKit app fronting the axum backend; this directory is a thin container.

**Conventions:** plain SvelteKit + TypeScript — the Leptos and ZesTTY/Dialect alternatives were both retired 2026-08-09; don't re-litigate the stack without new evidence. The React auth frontend is deleted.

**Entry points:** `web/` (there is nothing else here).

**Refs:** DD 39944194 — Frontend Stack, Server-Only Effect & the Runes Seam (governing) · DD 47939586 — The Rust Frontend, Leptos (reverted 2026-08-09) · DD 46596098 — The Dialect, ZesTTY (retired 2026-08-09) · ZMVP-148 (the deleted React auth frontend) · memory `project_frontend_stack_history`.
