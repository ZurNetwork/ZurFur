---
path: frontend/web/src/routes/(session)
charted: 2026-09-12
fs:
  - name: accounts/
    role: listing + create action; [id]/ detail with typed-handle delete confirmation
    node: false
  - name: logout/
    role: an action, not a page — a stray GET goes home
    node: false
  - name: +layout.server.ts
    role: the session gate: redirect to /login when anonymous
    node: false
  - name: layout.server.spec.ts
    role: spec for the gate — anonymous redirects, signed-in passes through
    node: false
---

**Is:** The signed-in area — a group layout that bounces anonymous visitors to `/login`, containing the account screens and the logout action.

**Conventions:** the gate is UX ONLY and covers page LOADS — form-action POSTs run before layout loads, so the backend's own 401 is the real guard for actions. New signed-in routes JOIN this group rather than re-implementing the check. Detail pages derive from the caller's own listing — there is no fetch-by-id rpc in v1; destructive actions require typed confirmation (WCAG 3.3.4).

**Entry points:** `+layout.server.ts` (the gate) · `accounts/+page.server.ts`.

**Refs:** internal ruling "F1" — no `GetAccount` rpc in v1, account detail derives from the caller's own account listing (cited in `accounts/[id]/+page.server.ts`); not yet a recorded DD, ask the Engineer if this should become one.
