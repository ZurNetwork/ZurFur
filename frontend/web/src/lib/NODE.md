---
path: frontend/web/src/lib
charted: 2026-08-21
fs:
  - name: server/
    role: the Effect half — runtime, API port, programs, form schemas
    node: true
  - name: api/
    role: component-facing plain-data contracts: Problem, Session, Account, HttpStatus, API_PREFIX, FetchFunction
    node: false
  - name: types/
    role: branded primitives (brand.ts — the one blessed `as` site) + Effect-free handle grammar (handle-format.ts) shared by both sides
    node: false
  - name: components/
    role: ProblemNote (the one way a problem renders) + SessionHeader
    node: false
  - name: testing/
    role: spec-only harnesses: fetch stub, problem-body builder, redirect assert, superform prop stub
    node: false
  - name: assets/
    role: favicon.svg
    node: false
  - name: index.ts
    role: stock placeholder, not a barrel
    node: false
---

**Is:** The shared library, cut by the seam: `api`/`types`/`components`/`testing` are Effect-free and browser-safe; `server/` is the Effect half.

**Conventions:** `$lib/api/*` is the vocabulary that crosses the seam — plain interfaces + `as const` maps. A rule stated once lives in the shared module both sides import (`handle-format.ts` backs both the Schema field and the brand validator). New shared UI goes in `components/`.
