---
path: frontend/web/src/lib/server
charted: 2026-09-12
fs:
  - name: api/
    role: the backend port + tagged error union + generated protobuf messages
    node: true
  - name: forms/
    role: one Effect Schema per form (login, create-account, delete-account) over a shared handle field; problem-message.ts bridges a Problem onto the superform message
    node: false
  - name: runtime.ts
    role: THE SEAM — process-wide ManagedRuntime; runApi is the only Effect→Promise crossing
    node: false
  - name: session.ts
    role: session programs (sessionOrAnonymous, optionalSession, signinOutcome, signoutOutcome)
    node: false
  - name: accounts.ts
    role: account programs (accountsOutcome, accountOutcome, create/deleteAccountOutcome)
    node: false
  - name: api-proxy.ts
    role: pure rewriteApiRequest used by hooks.server.ts (no $env, unit-tests standalone)
    node: false
  - name: "*.spec.ts"
    role: node-project specs beside each program module (accounts, session, api-proxy)
    node: false
---

**Is:** Everything below the seam: one ManagedRuntime, one `ZurfurApi` port, outcome-returning programs per domain, one Schema module per form.

**Conventions:** programs return an OUTCOME UNION (data or a renderable Problem), not a raw Effect — loads/actions `runApi(program)` and branch. Ports by role with a prod Layer and an in-memory test Layer (`zurfurApiTest`) — adapter-mem parity; specs never touch the network. Validation is server-side; browser `required` is a courtesy. Auth-time accepts what claim-time rejects: punycode handles pass `handleField` at sign-in, fail `claimHandleField` at account creation.

**Entry points:** `runtime.ts` (the seam itself) · `session.ts` · `accounts.ts`.

**Refs:** DD 26050561 — Confusable Handles & the Punycode Policy (`forms/`: why `claimHandleField` is stricter than `handleField`).
