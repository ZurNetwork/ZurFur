---
path: frontend
charted: 2026-08-21
fs:
  - name: web/
    role: the SvelteKit + Svelte 5 + TypeScript app (the only frontend)
    node: true
---
**Is:** The client tier — exactly one SvelteKit app fronting the axum backend; this directory is a thin container.

**Conventions:** plain SvelteKit + TypeScript — the Leptos (DD 47939586) and ZesTTY/Dialect (DD 46596098) alternatives were both retired 2026-08-09; DD 39944194 governs. The React auth frontend (ZMVP-148) is deleted.

**Refs:** DD "Frontend Stack — Server-Only Effect & the Runes Seam" (39944194) · memory `project_frontend_stack_history`.
