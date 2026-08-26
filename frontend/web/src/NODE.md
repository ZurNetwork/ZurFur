---
path: frontend/web/src
charted: 2026-08-21
fs:
  - name: lib/
    role: shared code, split above/below the runes seam
    node: true
  - name: routes/
    role: the SvelteKit route tree
    node: true
  - name: hooks.server.ts
    role: handleFetch — points same-origin /api/v1/* at the internal axum origin during SSR, forwards the session cookie
    node: false
  - name: app.d.ts
    role: ambient types; App.Superforms.Message = Problem declared once
    node: false
  - name: app.html
    role: the shell template
    node: false
---

**Is:** The app proper: a server-side `handleFetch` rewrite, a `$lib` split into an above-the-seam plain-data half and a below-the-seam Effect half, and the route tree.

**Conventions:** THE RUNES SEAM — Effect lives only under `lib/server/**`; `runApi` (`ManagedRuntime.runPromise`) is the one place programs become promises; loads/actions/components receive plain data and never see a fiber. Generated protobuf types never cross the seam — they decode server-side and map to hand-written plain interfaces in `lib/api/`. One-channel forms: values, field errors AND the backend Problem all ride the superform `message`. Absence is `T | undefined` — no `Option`, no `null`. Domain strings are branded (`AccountId`, `Did`, `Handle`).

**Entry points:** `lib/server/runtime.ts`.
