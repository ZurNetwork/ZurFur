---
path: contract/zurfur/api/v1
charted: 2026-08-21
fs:
  - name: session.proto
    role: GetMe (GET /me), Signin (POST /signin), Logout (POST /logout); SessionService
    node: false
  - name: account.proto
    role: ListAccounts, CreateAccount, ChangeHandle (PATCH /accounts/{id}/handle), DeleteAccount; AccountService
    node: false
  - name: commission.proto
    role: ListCommissions, CreateCommission (owner POV); google.protobuf.Timestamp; CommissionService
    node: false
  - name: problem.proto
    role: the RFC 9457 Problem shape declared ONCE for every surface
    node: false
---
**Is:** The v1 corpus — package `zurfur.api.v1`, one `.proto` per surface (session, account, commission, error), welded to `/api/v1`.

**Conventions:** every file opens with a header comment citing the DD and rulings. Vocabularies are strings with known values documented inline (R8). Request messages exist even when empty ("the session is the argument"). Timestamps are RFC 3339, Z-normalized. Success bodies are bare/flat resources; Problem `detail` is REQUIRED and never empty. Field comments carry the semantic contract — normative, not decoration.

**Entry points:** `session.proto` (smallest surface, shows the whole pattern) · `problem.proto`.
