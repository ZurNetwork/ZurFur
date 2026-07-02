# 🔎 Understanding ZMVP-46 — Account-handle change flow (post-onboarding)

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-46 · **Generated:** 2026-07-01 15:54 · **Snapshot:** `.understand/20260701-155406_ZMVP-46_account-handle-change.md`
>
> ⚠️ **This ticket carries an UNSETTLED design fork.** The DD only decided the *initial* handle choice; the *change* flow is undecided. This briefing FRAMES the open questions for `/design-decision` — it does not resolve them. Every fork below is the Engineer's call.

```
  onboarding (ZMVP-30 / POST /accounts)          THIS TICKET (ZMVP-46)
  ┌────────────────────────────────┐            ┌────────────────────────────────┐
  │ pick handle ONCE, at creation  │            │ CHANGE the handle, later        │
  │ · Handle::try_new validate      │  ───►      │ · re-validate (reuse gate)      │
  │ · mint did:plc, genesis aKA     │            │ · UPDATE accounts.handle        │
  │ · INSERT accounts.handle        │            │ · re-point alsoKnownAs (PLC     │
  │                                 │            │   UPDATE op — DOES NOT EXIST)   │
  └────────────────────────────────┘            │ · old handle: free? quarantine? │
        DECIDED (DD 24870914)                    └────────────────────────────────┘
                                                    UNDECIDED — needs /design-decision
```

## 🧭 1. Context (cold-start)

Every Zurfur **Account** is a sovereign atproto identity: a Zurfur-operated `did:plc` plus a human-readable **handle** that resolves to that DID. Two independent halves make a handle "work," and they must agree (**bidirectional verification**, per the atproto handle spec):

- **handle → DID** — for `*.zurfur.app` handles Zurfur serves this itself via `GET /.well-known/atproto-did`, reading `accounts.handle` from Postgres (DD 26607618). For a **brought (BYO) domain** the *user's* own DNS/well-known answers.
- **DID → handle** — the account's DID document lists `at://<handle>` in its **`alsoKnownAs`** field, written into the `did:plc` operation log.

Today the handle is fixed **once**, at account creation (`POST /accounts`): validate → mint DID with the handle baked into the **genesis** operation's `alsoKnownAs` → `INSERT` the row. **There is no way to change it afterward.** ZMVP-46 is to specify and build that change flow with the *same* validation guarantees as the initial claim, and to make resolution follow the change on both halves.

The catch: changing a handle is not one write. It mutates the **private** store (`accounts.handle`) *and* the **public** DID document (`alsoKnownAs`), which live on opposite sides of the data boundary — so by the no-cross-store-transactions rule it must be two separate steps, and the public step needs a **`did:plc` UPDATE operation that does not yet exist in the codebase** (only genesis + tombstone are implemented).

## 🗺️ 2. Domain

- **Account** (glossary `1966081`) — owns its handle; handle is **independent of the User's** handle and does **not** auto-follow a User handle-swap (DD 24870914 open item; page 21594113). So this change is an explicit, Account-scoped action, never a side effect.
- **Handle** (`domain/src/elements/handle.rs`) — the `Handle(String)` newtype and its single validation gate `Handle::try_new`: atproto normalization, reserved-TLD/reserved-label rejection (ZMVP-45 ✅), punycode `xn--` rejection (ZMVP-48 ✅). A change **re-runs this same gate** — that half is settled and reusable.
- **Roles** (glossary `2162692`, Owner/Admin/Manager/Member) — the ticket's "Done when" says *an Owner* can change the handle. **Who exactly** may change it (Owner-only? Owner+Admin?) is an unsettled authorization decision.
- **did:plc identity custody** (DD 26804226) — a handle change re-points `alsoKnownAs` via a **signed PLC update op** chaining on the prior op's CID, signed with the account's operational rotation key. Byte-exact DAG-CBOR signing; security-critical.
- **alsoKnownAs binding = outbox dual-write** (DD 26804226 decision 9; DD 24150017) — the public-boundary write is a separate retryable step, never inside the private Unit of Work. ZMVP-50 owns the *initial* writer; it is **To Do**.
- **Handle resolution** (DD 26607618) — `*.zurfur.app` resolves from Postgres, so the private `accounts.handle` update *is* what flips handle→DID; the DID-doc update flips DID→handle. Ordering across the two determines the transient-inconsistency window.
- **Deletion/tombstone precedent** (DD 23003138) — `accounts.handle` uniqueness is **global** (a tombstoned account still reserves its handle; freed only on hard delete). A *change* vacates a handle differently from deletion — what happens to the freed handle is an open question with impersonation stakes.

## 🎯 3. Goal & scope

**Goal:** let an authorized member change an existing Account's handle after onboarding, re-validated to the same guarantees as the initial claim, with *both* resolution halves (Postgres handle→DID and DID-doc `alsoKnownAs`) updated to agree — **without** a cross-store transaction.

**In scope (once the fork is settled):**
- Re-validation/normalization of the new handle (reuse `Handle::try_new`) + availability check.
- Private-store handle mutation (new `AccountWrites` method + `UPDATE accounts SET handle`).
- A `did:plc` **UPDATE** operation type re-pointing `alsoKnownAs`, run as a retryable outbox step.
- An authenticated HTTP endpoint to request the change.
- Disposition of the **old** handle and its resolution.

**Explicitly OUT (or pending a decision to pull in):**
- Deciding the policy itself — that is `/design-decision`, a prerequisite, not this build.
- **User** handle changes (this is Account-scoped only).
- Character handles.
- User-recovery-key enrollment / sovereignty (ZMVP-52) — orthogonal.
- The initial-issuance `alsoKnownAs` writer (ZMVP-50) — a **dependency**, not this ticket, though ZMVP-46 extends it from write→update.

**Scope honesty:** the ticket has **no acceptance criteria** — only a one-line "Done when" and an explicit ⚠️ design-open flag. Sections 5/6 are therefore provisional and cannot be finalized until `/design-decision` settles the forks in §8.

## 📦 4. Deliverables

Artifacts that must exist when this is done (contingent on the settled design):

- [ ] **A DD page** ("Account Handle Change Flow") recording the settled fork — authorization, cadence, old-handle disposition, BYO re-binding, `alsoKnownAs` replace-vs-append, cross-store ordering — plus a `/design-sync` of the Account / The Account Handle pages.
- [ ] **PLC UPDATE operation** — an `UpdateOperation` in `adapter-atproto/src/plc.rs` (new `op_type` in `plc_operation.rs`), chaining on `PlcOperationLog::latest_cid`, signed with the operational key; a `DidMinter::update_handle` (or `HandleBinder`) port method in `domain/src/ports.rs` (today only `mint` + `tombstone`).
- [ ] **Outbox/retry step** for the `alsoKnownAs` update — retryable + idempotent, outside the private UoW (extends/depends on ZMVP-50).
- [ ] **Private-store mutation** — `AccountWrites::update_handle` (+ `Account` domain method) and `PgAccountWrites` `UPDATE accounts SET handle`, mapping the `accounts_handle_key` violation → `HandleTaken` (409).
- [ ] **HTTP endpoint** — e.g. `PATCH /accounts/{id}/handle`, cookie/BFF-authenticated, authorized to the decided role, body re-validated via `Handle::try_new`, availability pre-check.
- [ ] **Old-handle disposition** — code enforcing the decided policy (immediate free vs quarantine/cooldown) and ensuring the old handle stops resolving.
- [ ] **BYO-domain re-binding** path (if in v1 scope) — verify the new brought handle resolves + bidirectional before commit.
- [ ] **Tests** — unit (validation/authz), integration (private update + resolver flip), e2e/outbox (DID-doc `alsoKnownAs` reflects the new handle; retryable).
- [ ] **`/security-review`** — touches identity/DID binding + the private↔public boundary.

## 🧩 5. Work breakdown

> Provisional — the whole table is gated on the §8 design fork. Owners skew 🧑/👥 because this is a domain- and identity-critical ticket; mechanical slices become 🤖 Claude **only after** the policy is decided.

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Settle the design fork** (`/design-decision` → DD + `/design-sync`) | 5 — domain judgment, impersonation/security stakes, no single right answer | P0 | 🧑 Engineer | ⬜ — undecided; ticket flagged ⚠️ |
| **PLC `UpdateOperation` + `DidMinter::update_handle`** (byte-exact DAG-CBOR signing, chain on `latest_cid`) | 8 — crypto exactness, identity blast-radius, security-critical | P1 | 👥 Group | ⬜ — only genesis+tombstone exist (`plc.rs`, `ports.rs` L406) |
| **`alsoKnownAs` update outbox/retry step** (idempotent, outside UoW) | 6 — dual-write correctness, retry semantics; overlaps ZMVP-50 (To Do) | P1 | 👥 Group | ⬜ — outbox path referenced only in comments (`plc_operation_log.rs`, ZMVP-50) |
| **Old-handle disposition** (free vs quarantine; stop resolving; impersonation guard) | 4 — domain + security policy | P1 | 🧑 Engineer | ⬜ — no policy; global uniqueness holds handle today (`add_account_handle.sql`) |
| **BYO-domain re-binding verification** (verify new brought handle resolves + bidirectional pre-commit) | 5 — external DNS dependency, drift, verification handshake | P2 | 🧑 Engineer | ⬜ — resolver serves only `*.zurfur.app` (`wellknown.rs` `handle_from_host`) |
| **HTTP endpoint + authorization** (who may change; `PATCH /accounts/{id}/handle`) | 3 — authz is a domain call; wiring is mechanical | P1 | 🧑 Engineer | ⬜ — no update/PATCH route on `accounts_router()` |
| **Private-store `update_handle`** (`AccountWrites` + `UPDATE accounts SET handle`, `HandleTaken`→409) | 2 — mirrors existing UoW writes | P2 | 🤖 Claude *(post-decision)* | ⬜ — only `create` writes handle (`adapter-pg/src/account.rs` L281) |
| **Cross-store ordering / transient-window semantics** (which half flips first; fail-open vs fail-closed) | 4 — correctness of the visible inconsistency window | P2 | 🧑 Engineer | ⬜ |
| **Cadence / rate-limit enforcement** (cooldown between changes, if any) | 2 — policy then mechanical | P3 | 🧑 Engineer *(policy)* / 🤖 Claude | ⬜ |
| **Tests** (unit/integration/e2e/outbox) | 2 — reuses testcontainers harness | P1 | 🤖 Claude | ⬜ |

**Dependency chain:** ZMVP-49 (real minter) *In Progress* → ZMVP-50 (`alsoKnownAs` **initial** writer) *To Do* → ZMVP-46 extends that to an **update**. ZMVP-46 cannot land its public half before a PLC update-op capability exists. ZMVP-44 (resolution + `accounts.handle`) *In Review*; ZMVP-45/48 (reserved-label/punycode) ✅.

## ✅ 6. Test checklist (TDD)

> Maps to the *implied* Done-when (no formal ACs). Finalize after the fork is decided.

- **Unit** — _asserts that_ a changed-to handle runs the full `Handle::try_new` gate (punycode/reserved/normalization all re-enforced on change) → Done-when "same validation guarantees as the initial claim".
- **Unit** — _asserts that_ only the decided role (e.g. Owner) is authorized; other roles are rejected → Done-when "an Owner can change".
- **Unit** — _asserts that_ changing to a handle already held (incl. a tombstoned account's reserved handle) is rejected with `HandleTaken`/409 → uniqueness backstop.
- **Integration** — _asserts that_ after the private update, `find_did_by_handle(new)` returns the DID and `find_did_by_handle(old)` behaves per the decided old-handle policy → resolution updates accordingly.
- **Integration** — _asserts that_ the handle update is a UoW write and the `alsoKnownAs` update is **not** in that UoW (no cross-store txn) → boundary invariant.
- **E2E / outbox** — _asserts that_ after the outbox step runs, the DID document's `alsoKnownAs` equals `[at://<new-handle>]` and bidirectional verification passes; the step is retryable and idempotent → Done-when "resolution updates accordingly".
- **E2E (BYO, if in scope)** — _asserts that_ a change to a brought domain only commits once the new handle resolves back to the DID (no broken handle committed).

## 🧠 7. Logic & shape

Ordering is the crux — two stores, no shared transaction. The change is a private write followed by a retryable public update; the transient window between them is where handle→DID and DID→handle disagree, so the direction and the fail-open/closed choice are a **decision**, not a default.

```
Client ──PATCH /accounts/{id}/handle {new}──► API
                                              │ 1. authorize (decided role)
                                              │ 2. Handle::try_new(new)         ← reuse gate
                                              │ 3. availability pre-check
        ┌─────────────────────────────────────┘
        ▼  PRIVATE (UnitOfWork — one store)
   UPDATE accounts SET handle=new WHERE id=…      → flips handle→DID resolution NOW
   (unique violation → HandleTaken/409)
   old-handle disposition: free | quarantine?  ← DECISION
        │
        ▼  PUBLIC (outbox — separate retryable step, NOT in the UoW)
   PLC UpdateOperation: prev = latest_cid,
       alsoKnownAs = [at://new]   (replace? or append old as alias?) ← DECISION
   sign(operational key) → submit to plc.directory   → flips DID→handle
        │
        ▼
   bidirectional verification holds again
```

Open ordering question: if the private write lands first, the well-known resolver serves `new → DID` immediately while the DID doc still says `old` — external verifiers briefly see a mismatch. Reverse the order and the DID doc points at a handle Postgres doesn't yet serve. Either way there is a window; the Engineer decides which failure mode is acceptable and whether the endpoint reports success optimistically or only after the outbox confirms.

## 🚀 8. Next steps

1. ⚠️ **BLOCKING — run `/design-decision` first.** No code until the fork is settled; the ticket itself says "settle the open questions … before building." The open questions:

   - **In v1 at all, or deferred?** Confirm whether the change flow ships in v1 or is documented-and-deferred. (Ticket: "Confirm scope — v1 vs deferred.")
   - **Who may change an Account's handle?** Owner-only, or Owner + Admin? (Done-when says "an Owner"; Roles hierarchy leaves it open.)
   - **How often — cadence / cooldown?** Unlimited, or a rate-limit/cooldown between changes? (Handle churn has resolution-cache and impersonation cost.)
   - **What happens to the OLD handle?** Freed immediately for anyone to claim, or **quarantined/reserved for a cooldown** to stop a squatter grabbing a just-vacated identity and impersonating? (Note: today's global uniqueness even holds a *tombstoned* account's handle — a change is a different case.)
   - **`alsoKnownAs`: replace or append?** Does the DID doc drop `at://old` entirely (only `new` verifies) or keep the old as an alias (both resolve)? Affects whether the old handle still validates.
   - **BYO-domain re-binding in v1 scope?** Which transitions are allowed (`*.zurfur.app`↔BYO, BYO↔BYO), and what's the verification handshake — do we verify the new brought handle resolves + bidirectional *before* committing, to avoid persisting a broken handle?
   - **Cross-store ordering / window semantics.** Which half flips first, and is the change reported as done optimistically or only after the outbox confirms? (Fail-open vs fail-closed during the inconsistency window.)
   - **Dependency sequencing.** ZMVP-46 needs a `did:plc` **UPDATE** op that doesn't exist (only genesis+tombstone) and the ZMVP-50 outbox (To Do). Decide: does ZMVP-46 build the update-op capability itself, or block on ZMVP-50 first?

2. Offer to capture the outcome as a **DD page** + `/design-sync` the *Account* and *The Account Handle* pages (the latter's "handle vs user handle-swap" open row resolves here).
3. Only then plan the build: sequence behind ZMVP-49/50; land the PLC update-op + outbox as a 👥 Group/🧑 Engineer piece; hand Claude the mechanical private-store `update_handle` + tests once the policy is fixed.
4. Route through `/security-review` before any PR (identity binding + private↔public boundary).

---

*Read-only briefing. No branch, worktree, or Jira transition was created. `/design-decision` is a hard prerequisite to `/implement`.*
