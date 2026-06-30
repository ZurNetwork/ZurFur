# 🔎 Understanding ZMVP-34 — Owner deletes an Account

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-34 · **Generated:** 2026-06-28 23:24 · **Snapshot:** `.understand/20260628-232445_ZMVP-34_owner-deletes-account.md`

## 🧭 1. Context (cold-start)

An **Account** in Zurfur is a sovereign **atproto identity** — a `did:plc` + handle whose public face lives in its own PDS repo, paired with an app-private `accounts` row (UUIDv7) in PostgreSQL. Users join an account with one of four roles (`Owner` > `Admin` > `Manager` > `Member`), seated in a parent/child role-tree.

This ticket is the **account-level exit**: the Owner destroys the whole account, not a single membership. It is the destructive counterpart to the already-built per-member departures (`leave` ZMVP-21, `revoke_role` ZMVP-40) and the sibling of `transfer ownership` (ZMVP-33). It is also the escape hatch the Roles rule relies on: *"the Owner cannot leave while still Owner — they must transfer ownership or delete the account first."*

The shape is **not** plain CRUD — it earned a dedicated, already-**DECIDED** Design Decision (`Account Deletion, Tombstoning & Handle Reuse`, 2026-06-25) because deleting a DID-bearing entity touches **PLC tombstone semantics**, **handle disposition**, and the **public↔private data boundary**. The rule is **soft by default, hard only when empty**:
- **Soft-delete** = atproto **deactivation** (`active=false`): hides the account's own surface, leaves commissions/products individually visible, never cascades, recoverable, **never escalates** to hard.
- **Hard-delete** = atproto **deletion** + `did:plc` **tombstone** on atproto's native ~72h PLC recovery window + **handle freed** for reuse — but **only** when the account holds **no facts**.

## 🗺️ 2. Domain

Confluence DESIGN entities in play:
- **[Account](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/1966081)** — "Any account can be deleted, including the first… soft by default; hard only when it holds no facts." The glossary already encodes the soft/hard split.
- **[Account Deletion, Tombstoning & Handle Reuse (DD)](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/23003138)** — the single source of truth for this ticket. Settles: (1) facts = commissions, products, recurring billing either direction; once an account *ever* held facts it is **soft-delete-only permanently**; (2) soft = scoped deactivation, no cascade; (3) hard = deletion + PLC tombstone, empty-only; (4) handle freed immediately on hard-delete; (5) attribution is **DID-anchored** so a reused handle never collides; (6) two frontend dangling-ref states (downstream); (7) **Owner-only**.
- **[Roles](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/2162692)** — Owner-only authority; deletion is the Owner's path out (rule 7).
- **[Data Boundaries](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/10354698)** — private (PostgreSQL `accounts`) vs public (PDS/PLC). Deletion spans **both**, so it is a **dual write** (no cross-store transaction; outbox-style retryable step).

Why it matters *here*: the destructive op is the first to cross the data boundary in an irreversible (hard) direction, and the "facts" gate ties account lifecycle to entities (**commissions/products**) **that do not yet exist in the schema** — making the soft/hard branch partly hypothetical at today's state.

## 🎯 3. Goal & scope

**Goal:** give an Owner a single endpoint to delete their account that honors the DD — soft-deactivate by default, hard-delete (atproto deletion + PLC tombstone + handle free) only when the account holds no orphan-able facts, owner-only, and crossing the public/private boundary as a **dual write**, never one transaction.

**In scope:**
- Owner-only `DELETE /accounts/{id}` endpoint + handler (auth: 401/403/404).
- Private-store soft-delete: set `accounts.deleted_at`, tear down memberships, revoke pending invitations.
- The **facts emptiness check** that decides soft vs hard (gating on whatever facts exist today).
- The **"ever held facts → soft-only permanently"** rule (needs a persisted marker).
- atproto **deactivation** (soft) and atproto **deletion + PLC tombstone + handle unbind** (hard), run as a retryable outbox-style step.

**Out of scope:**
- Frontend rendering of dangling actor-refs (deactivated vs deleted) — explicitly downstream/frontend-era (DD decision 6).
- Subscriptions / plugin billing as facts — post-MVP; gate only on facts that exist at delete time.
- Account-ownership **transfer** (ZMVP-33, sibling ticket).
- Recovery/undelete UX (atproto's native 72h window is the mechanism; no custom Zurfur retention).

## 📦 4. Deliverables

- [ ] `DELETE /accounts/{id}` route in the api router (`api/src/lib.rs::app`).
- [ ] `delete_account` handler — owner-only check, 401/403/404/204(or 200).
- [ ] `AccountRepo::delete` (or `soft_delete` + `hard_delete`) port method(s) in `domain/src/ports.rs`.
- [ ] PG adapter impl: transactional private-store teardown (set `deleted_at`, delete `account_members`, revoke pending `account_invitations`).
- [ ] A **facts-emptiness** query/predicate + persistence of the "ever held facts" flag (schema change likely — migration).
- [ ] atproto boundary ops in `adapter-atproto`: `deactivate` (soft) and `delete + tombstone + unbind handle` (hard) — **decision-gated** (see §8).
- [ ] Outbox/retryable orchestration so the boundary-crossing step is not a cross-store transaction.
- [ ] New `Problem` constructor(s) if needed (e.g. only-owner-may-delete → reuse `forbidden()`).
- [ ] Tests: domain unit, adapter integration, api e2e (see §6).
- [ ] `/design-sync` of the Account glossary if any rule shifts at build time.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| Owner-only `DELETE /accounts/{id}` route + handler | 3 — boilerplate; mirrors `leave_account`/`revoke_role` | P1 | 🤖 Claude | ⬜ no delete route in `api/src/lib.rs::app` (260–282); only `members`/`members/me`/`invitations` exist |
| Private-store soft-delete (deleted_at + memberships + invitations teardown) | 3 — reuses `settle_member_departure` pattern; one tx | P1 | 🤖 Claude | 🟡 `deleted_at` column + `find` filter exist (`adapter-pg/account.rs` find 141, schema `20260621120000`); no delete write yet |
| `AccountRepo::delete` port + mem fake | 2 — trait method + in-proc fake | P1 | 🤖 Claude | ⬜ port (`domain/ports.rs` 92–187) has create/leave/revoke, no delete |
| **Facts-emptiness check + "ever held facts" marker** | 5 — domain fork: no fact entities in schema yet; needs a persisted flag + decision | P1 | 🧑 Engineer | ⬜ no commissions/products tables exist; emptiness is currently vacuously true → would always hard-delete |
| **atproto soft deactivation** (public boundary dual write) | 6 — public boundary, adapter support unverified, outbox step | P2 | 👥 Group | ⬜ `adapter-atproto` deactivation capability unconfirmed (not surveyed; likely absent) |
| **atproto hard deletion + `did:plc` tombstone + handle unbind** | 8 — irreversible, custodied-key PLC op, ~72h window, public boundary | P2 | 👥 Group | ⬜ no tombstone/handle-free path exists |
| **Dual-write / outbox orchestration** (no cross-store tx) | 6 — architectural; spans both boundaries retryably | P2 | 👥 Group | ⬜ no outbox primitive; current writes are single-store `pool.begin()` |

## ✅ 6. Test checklist (TDD)

- **Unit (domain)** — _asserts that_ a non-Owner member cannot delete (authority gate) → AC1
- **Unit (domain)** — _asserts that_ an account that has ever held facts resolves to **soft-only**, never hard → AC3
- **Unit (domain)** — _asserts that_ an account with zero facts resolves to **hard-delete** → AC4
- **Integration (adapter-pg)** — _asserts that_ soft-delete sets `deleted_at`, removes `account_members`, and revokes pending `account_invitations` in one transaction → AC2
- **Integration (adapter-pg)** — _asserts that_ a soft-deleted account is excluded from `find` yet its DID/row persists (recoverable) → AC2, AC3
- **Integration (adapter-atproto / mem)** — _asserts that_ soft → atproto `deactivated`, commissions/products untouched (no cascade) → AC2
- **Integration (adapter-atproto / mem)** — _asserts that_ hard → atproto `deleted`, DID tombstoned, **handle unbound/freed** → AC4, AC5
- **E2E (api)** — _asserts that_ `DELETE /accounts/{id}` returns 403 for Admin/Member, 404 for non-member/unknown, 204/200 for Owner → AC1
- **E2E (api)** — _asserts that_ deleting an Owner's sole-member empty account succeeds (the exit ZMVP-21 relies on) → AC1, AC4

## 🧠 7. Logic & shape

```
DELETE /accounts/{id}  (session → user)
        │
        ├─ load account (404 if missing/already deleted)
        ├─ role_of(user) == Owner ?  ── no ──▶ 403 forbidden
        │                              yes
        ▼
   has_facts(account)  ──► (commissions? products? recurring billing?)   [none exist in schema yet]
        │                         │
   ever_held_facts flag set? ◄────┘
        │
   ┌────┴───────────────┬───────────────────────────┐
   │ SOFT (default,     │ HARD (empty AND never      │
   │ or ever-held)      │ held facts)                │
   ▼                    ▼                            
 PRIVATE (one tx):    PRIVATE (one tx):              
   accounts.deleted_at   delete row / mark           
   delete members        delete members              
   revoke pending invs   revoke pending invs         
   ─────────────────     ─────────────────           
 BOUNDARY (outbox,    BOUNDARY (outbox, retryable):  
  retryable):           atproto deleteAccount        
   atproto deactivate   + did:plc tombstone (~72h)   
                        + unbind/free handle         
```

**Critical constraint:** the private teardown and the atproto op are **two stores** → must be a **dual write** (separate retryable step / outbox), **never** one unit of work (CLAUDE.md "No cross-store transactions"). The private soft-delete should commit first; the boundary op follows and is retried until durable.

## 🚀 8. Next steps

1. **⚠️ DECISION (Engineer):** With no fact-bearing entities (commissions/products) in the schema yet, today *every* account is "empty" → the DD's hard-delete path would fire on essentially all deletes, including ones that should later be soft-only. Options: (a) implement only the **soft path** now + scaffold hard as a failing test until ZMVP-18 (Commissions) lands a fact; (b) implement the full branch with a placeholder `has_facts()` that returns false + a persisted `ever_held_facts` flag set by future fact-creation; (c) defer the whole ticket until at least one fact entity exists. The ticket notes lean toward (a)/(b) ("gates on whatever exists at delete time").
2. **⚠️ DECISION (Engineer):** Where does the **"ever held facts → soft-only permanently"** marker live — a boolean column on `accounts`, or derived? It must outlive the facts themselves, so a persisted flag is implied (schema migration).
3. **⚠️ UNKNOWN:** Does `adapter-atproto` today support account **deactivate / deleteAccount** and a **PLC tombstone + handle unbind**, and does Zurfur custody the signing keys to do so? Not surveyed; assume **absent** → Group build. Verify before scoping the boundary pieces.
4. **⚠️ DECISION (Engineer):** Outbox primitive — none exists yet. Build a minimal retryable step here, or wait for a shared outbox? This intersects ZMVP-36 (Unit of Work).
5. Sequencing: this collides with several in-flight account-surface tickets — land after **ZMVP-21** (leave) and **ZMVP-40** (revoke invites) merge, ideally after **ZMVP-36** (UoW) and **ZMVP-39** (router split) settle the shared `account.rs`/router surface, and coordinate with **ZMVP-33** (transfer ownership, the sibling exit).
6. Claude can start the **soft-path private-store scaffold** (route + handler + port + PG/mem impl + tests) on the Engineer's word; hold the atproto + facts pieces for the Engineer/Group.
