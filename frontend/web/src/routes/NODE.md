---
path: frontend/web/src/routes
charted: 2026-08-21
fs:
  - name: (session)/
    role: the auth-gated group (accounts, logout) behind a redirect-to-login layout guard
    node: true
  - name: login/
    role: sign-in screen: handle form action → PDS redirect; callback-errors.ts maps axum's ?error= codes to copy
    node: false
  - name: +layout.server.ts
    role: one whoami per server render shared via layout data; unreachable backend degrades to signed-out, contract breakage still 500s
    node: false
  - name: +layout.svelte
    role: the shell — favicon + SessionHeader + children
    node: false
  - name: +page.svelte
    role: the public landing page
    node: false
---

**Is:** The route tree: a root layout that resolves whoami once per render, a public home and `/login`, and an auth-gated `(session)` group.

**Conventions:** loads and actions are the seam's consumers — `runApi(program)` then branch; never `Effect` in a route file. Forms are progressive-enhancement-first: `superValidate` + the effect adapter in the load, a form action for the POST, `problemMessage` for backend failures on the same form. Navigation targets go through `resolve()` from `$app/paths`.
