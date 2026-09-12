---
path: contract/zurfur/api/v1
charted: 2026-09-12
fs:
  - name: session.proto
    role: GetMe (GET /me), Signin (POST /signin), Logout (POST /logout); SessionService
    node: false
  - name: account.proto
    role: ListAccounts, CreateAccount, ChangeHandle (PATCH /accounts/{id}/handle), DeleteAccount, AccountMembership (now carries an alias field); AccountService
    node: false
  - name: commission.proto
    role: ListCommissions, CreateCommission (owner POV), Maturity; google.protobuf.Timestamp; CommissionService
    node: false
  - name: problem.proto
    role: the RFC 9457 Problem shape declared ONCE for every surface
    node: false
---
**Is:** The v1 corpus — package `zurfur.api.v1`, one `.proto` per surface (session, account, commission, error), welded to `/api/v1`.

**Conventions:** every file opens with a plain-language orientation header (no cross-repo pointers — see Refs below for what each header used to cite). `service`/`rpc` blocks only declare routes for the descriptor test; only messages are generated. Vocabularies are strings with known values documented inline (R8). Request messages exist even when empty ("the session is the argument"). Timestamps are RFC 3339, Z-normalized. Success bodies are bare/flat resources, never wrapped; Problem `detail` is REQUIRED and never empty. Field comments carry the semantic contract — normative, not decoration.

**Entry points:** `session.proto` (smallest surface, shows the whole pattern) · `problem.proto`.

**Refs:**
- DD 40992770 — The API Contract, decision 3 (session.proto, account.proto, commission.proto: service blocks declare routes only, messages-only codegen).
- DD 23592962 — API Response Shape & Error Model (commission.proto: CreateCommissionResponse is a bare, unwrapped resource; problem.proto: the Problem shape itself).
- DD 29982722 — Maturity Vocabulary (commission.proto: `Maturity` message adopts the atproto self-label rating axis).
- DD 32112642 — The Linked Channel (commission.proto: `linked_channel` field).
- DD 27852802 — Account Handle Change Flow (account.proto: `ChangeHandleRequest`/`ChangeHandleResponse`, Owner-only rename).
- DD 39944194 — Frontend Stack (session.proto: the frontend's hand-written Layer, not generated decoding, owns Signin/Logout redirect-following and cookie harvesting).
- ZMVP-152 (account.proto: `DeleteAccountResponse.outcome` — the delete AC that needs the outcome distinguished).
- ZMVP-75 (commission.proto: `ListCommissions` stays owner-POV only; the non-participant projection is a separate, later surface).
- Engineer ruling 2026-07-24 (account.proto: `ListAccounts` returns every role the caller holds, not owned-only).
- Engineer ruling 2026-07-25, one ruling covering several call sites: account.proto (`DeleteAccountResponse` carries the outcome instead of a bodiless `204`), commission.proto (`CreateCommissionResponse`/`CreateCommission` carry the created resource so the caller can navigate to it), problem.proto (`Problem.detail` is required, resolving a prior three-way drift across problem.rs/plugin-v1.yaml/problem.ts), session.proto (R1: lowerCamelCase wire keys, `json_name` rejected as a one-way door).
