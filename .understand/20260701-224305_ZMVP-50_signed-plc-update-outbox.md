# ZMVP-50 — Signed `did:plc` UPDATE op + retryable outbox (reusable)

- **Snapshot:** 2026-07-01T22:43:05-06:00 · `/understand` (unit-of-work, single-ticket unit)
- **Jira:** [ZMVP-50](https://zurnetwork.atlassian.net/browse/ZMVP-50) · Task · Medium · To Do · unassigned
- **Blocks:** ZMVP-46 (Account handle change — `is blocked by` this). **Relates:** ZMVP-49 (minter, merged as #83), ZMVP-51 (transparency-log monitor).
- **recommended_model:** **Opus 4.8** — security-nature (secp256k1 signing, DID↔handle binding, private↔public boundary) forces Opus per the model-assignment policy; **mandatory `/security-review`** before PR.

---

## 1. Cold-start context

Every Account is a sovereign atproto identity: a Zurfur-operated `did:plc` + a human handle that resolves to it. Two halves must agree (bidirectional verification, atproto Handle spec): **handle→DID** (Postgres `accounts.handle` served at `/.well-known/atproto-did`, DD 26607618) and **DID→handle** (the DID document's `alsoKnownAs = ["at://<handle>"]`, written into the `did:plc` op log).

Today the handle is fixed **once**, at `POST /accounts`, baked into the genesis op's `alsoKnownAs`. There is **no** signed `did:plc` **UPDATE** op — only genesis (`plc_operation`) and tombstone (`plc_tombstone`) exist. ZMVP-46 (post-onboarding handle change) needs to re-point `alsoKnownAs`, which requires that UPDATE op. Rather than build it inside ZMVP-46, DD 27852802 decision 8 **generalized ZMVP-50** into the one reusable, security-critical signing path — built once, consumed by both initial-maintain and handle change. *"The update-op crypto is never duplicated (make unsoundness unreachable, not caught twice)."*

**Governing DDs:** [26804226](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/26804226) (custody/minting/credible-exit — the crypto recipe), [27852802](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/27852802) (handle-change flow — REPLACE semantics, DID-doc-first ordering), [24150017](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/24150017) (no cross-store transactions), [26935298](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/26935298) (identity-only v1, local directory until launch).

## 2. Domain

- **`did:plc` = a chain of signed ops.** Every non-genesis op references `prev` = CID of the DID's most recent op, and is signed with a **rotation key**. Byte-exact DAG-CBOR is load-bearing: the DID hash and every signature depend on the exact bytes.
- **Crypto recipe (DD 26804226 §7, VERIFIED in code):** DAG-CBOR-encode the op *without* `sig` → ECDSA-SHA256 → **low-S** → 64-byte r‖s → base64url no-pad. `atrium-crypto` 0.1.3 `keypair.rs:75-77` does `try_sign` (RFC 6979 **deterministic**) + `normalize_s()` (low-S) → signing is **fully deterministic**.
- **REPLACE semantics (DD 27852802 §5):** on change, `alsoKnownAs` becomes `["at://<new>"]` only — the old entry is dropped (a retained dead alias fails bidirectional verification; name-history is served by the native PLC audit log, not by hoarding entries).
- **Outbox / no cross-store txn (DD 24150017, DD 26804226 §9, DD 27852802 §7):** the PLC update is a **separate, retryable, idempotent** step — never folded into the private-store UoW. Ordering for a *change*: DID-doc first, Postgres second, success only after the outbox confirms — worst transient state is `handle.invalid`, never a mis-pointing handle. (ZMVP-50 owns the *public step + its contract*; ZMVP-46 owns the ordering across both stores.)
- **Credible-exit invariant (load-bearing):** none of this may ever gate a user *leaving* with their DID+handle. ZMVP-50 doesn't touch departure; just don't introduce a coupling that would.

## 3. Real goal & scope

**Goal:** a reusable, signed `did:plc` **UPDATE** capability that re-points `alsoKnownAs` to an arbitrary new handle (REPLACE), chained on the DID's latest op, signed by the operational rotation key, byte-exact — run as a **separate, retryable, idempotent** step, **not** in the private UoW. Consumed by ZMVP-46.

**In scope:**
- An `UpdateOperation` op-type in `plc.rs` (sibling to `GenesisOperation`/`TombstoneOperation`) that sets `alsoKnownAs=["at://<handle>"]` and reconstructs the rest of the DID-doc state (rotationKeys, verificationMethods, empty services) from stored keys, chained on `prev`.
- A domain verb to drive it (build → sign with operational key → log → submit), mirroring `DidMinter::tombstone`.
- The **retryable/idempotent** contract on that step (idempotency via content-address + `UNIQUE(cid)`), reusing `PlcOperationLog` for `prev` chaining and the durable record. Directory submission stays the **gated no-op** (`plc_directory_submit=false`) it is in v1.
- Mem fake + PG-backed tests + a byte-exact/determinism vector test.

**Out of scope (fenced):**
- The Owner-only PATCH endpoint, policy, rate-limit, quarantine, BYO bidirectional pre-commit verification, cross-store ordering across *both* stores → **ZMVP-46**.
- The transparency-log monitor / background drain worker → **ZMVP-51** (and see Fork A).
- Real directory submission / KMS → launch-gated (ZMVP-53).

## 4. Concrete deliverables

1. `adapter-atproto/src/plc.rs`: `UpdateOperation` + `SignedUpdate` (or reuse `SignedOperation` — same `plc_operation` type as genesis, differing only in `prev` + `alsoKnownAs`), with `signing_bytes()`, `into_signed()`, `cid()`, `to_json()`.
2. Domain port method (Fork B) + adapter impl in `RealDidMinter` mirroring `tombstone`: load keys → `op_log.latest_cid(did)` → build → `operational.sign` → base64url → build `PlcOperationRecord{op_type:"plc_operation", prev, ...}` → `directory.submit` → `op_log.append`, retryable/idempotent.
3. `adapter-mem`: mem-fake behavior for the new verb (extend `MemDidMinter`).
4. `plc.rs` unit tests: byte-exact update signing_bytes exclude `sig`, deterministic-sig round-trip (same inputs → same CID), REPLACE drops old aka.
5. Adapter/integration tests: update chains onto genesis/prior op (`prev` = latest_cid), signed by operational key, replay is idempotent (second identical call = no-op via `UNIQUE(cid)`), keys/log ordering safe under submission failure.
6. `/document` on changed signatures; `/security-review`; DD sync check (likely none needed — DDs already describe this).

## 5. Weighted work-breakdown

| # | Piece | Diff (0–5) | Prio | Owner | Done-with-evidence |
|---|---|---|---|---|---|
| 1 | `UpdateOperation` op-type in `plc.rs` (REPLACE aka, reconstruct doc state, `prev` chain) | 2 | High | Claude (Opus) | unit test: signing_bytes exclude `sig`; aka == `["at://<new>"]`; rest of doc == genesis-shape |
| 2 | Determinism + byte-exact vector test | 2 | High | Claude (Opus) | test: sign twice → identical 64 bytes → identical CID; low-S verified via `k256 normalize_s` |
| 3 | Domain verb + `RealDidMinter` impl (mirror `tombstone`) | 2 | High | Claude (Opus) | test: chains on `latest_cid`, signed by operational key, appends `plc_operation` record |
| 4 | Retryable + **idempotent** contract (content-address dedup) | 3 | High | Claude (Opus) | test: replay identical update → second call is a no-op (`UNIQUE(cid)`), no double-log |
| 5 | `MemDidMinter` fake for the new verb | 1 | Med | Claude (Haiku/Opus) | mem test parity with PG |
| 6 | **Fork A — outbox durability depth** | — | Blocking | 🧑 **Engineer decides** | see Interview |
| 7 | **Fork B — port shape (extend `DidMinter` vs new port)** | — | Blocking | 🧑 **Engineer decides** | see Interview |
| 8 | `/security-review` (mandatory — crypto + boundary) | 3 | High | Claude (Opus) | clean review before PR |

**Ownership call:** pieces 1–5 & 8 are a near-mechanical mirror of the existing, DD-settled tombstone path → **Claude on Opus, with mandatory `/security-review`**. Pieces 6 & 7 are genuine domain/architecture forks → **Engineer's call** (this briefing is the pause). *Offered:* if the Engineer would rather write the crypto themselves given its weight, that's their lane — I'll hand off with the failing tests + notes.

## 6. TDD test checklist (layered)

**Unit (in `plc.rs`, no DB):**
- [ ] `update_op_signing_bytes_exclude_sig` — DAG-CBOR of `UnsignedView` has no `sig` key.
- [ ] `update_replaces_also_known_as` — aka == `["at://<new>"]`, old entry absent.
- [ ] `update_preserves_rotation_keys_and_verification_methods` — rest of doc == the genesis-shape reconstructed from the same keys.
- [ ] `update_chains_on_prev` — `prev` field == the supplied prior CID.
- [ ] `signing_is_deterministic` — sign the same bytes twice → identical output (enforces the RFC-6979 finding; the idempotency design rests on it).
- [ ] byte-exact vector (if a known-good directory UPDATE op is available) — matches published CID.

**Adapter (`RealDidMinter`, with mem doubles):**
- [ ] `update_chains_onto_latest_logged_op` — uses `op_log.latest_cid(did)` as `prev`.
- [ ] `update_is_signed_by_the_operational_key` — verify via `atrium_crypto::verify` against `rotationKeys[1]`; assert low-S via `k256`.
- [ ] `update_appends_a_plc_operation_record` — `op_type == "plc_operation"`, correct `cid`/`prev`.
- [ ] `replaying_an_identical_update_is_idempotent` — second call no-ops (dup CID), no second log row.
- [ ] `update_survives_directory_submission_failure_retryably` — failure leaves state replayable, no orphan/partial (mirror `keys_persist_when_directory_submission_fails`).

**PG integration (testcontainers):**
- [ ] round-trip through `PgPlcOperationLog`: append update → `latest_cid` returns it; duplicate cid rejected (idempotency at the DB boundary).

## 7. Pseudocode (impl of the verb, mirroring `tombstone`)

```
async fn rebind_handle(did, new_handle):           # name/shape per Fork B
    keys = key_store.get(did)?.ok_or(NoCustody)
    prev = op_log.latest_cid(did)?.ok_or(NoPriorOp)     # must chain on something
    op   = UpdateOperation::identity_only(
              rotation_keys = [keys.cold_recovery.did(), keys.operational.did()],
              atproto_signing_did = keys.signing.did(),
              handle = new_handle,                        # REPLACE: aka = ["at://<new>"]
              prev = prev)
    sig    = base64url_nopad(keys.operational.sign(op.signing_bytes()?)?)
    signed = op.into_signed(sig)
    cid    = signed.cid()?
    directory.submit(did, &signed.to_json()?).await?      # gated no-op in v1
    op_log.append(PlcOperationRecord{ did, cid, op_type:"plc_operation", prev:Some(prev), operation_json })
    # idempotent: identical inputs → identical cid → UNIQUE(cid) dedups a replay
```

## 8. Ordered next steps

1. **STOP — resolve Fork A & Fork B with the Engineer** (interview below). Blocks `/implement`.
2. `/start ZMVP-50` (worktree) → branch `feature/zmvp-50-signed-plc-update-outbox`, Jira → In Progress.
3. `/implement`: §6 tests red → pieces 1–5 green (mirror tombstone), respecting the Fork decisions.
4. `/critique` → `/document` → **`/security-review` (mandatory)**.
5. `/prepare-pr` (Copilot review — complex/security) → merge → integrate. ZMVP-46 unblocks.

---

## Interview — genuine forks (Engineer disposes)

**Fork A — How much outbox durability now?** DDs mandate a *retryable, idempotent, separate* step. Today the codebase does inline `directory.submit().await?` + `warn!` (no persisted pending-state, no worker). Since v1 submission is a **gated no-op** (`plc_directory_submit=false`, local directory until launch, DD 26935298):
- **A1 (recommended):** build the op + verb + **idempotent, replayable** contract, log to `plc_operations` (durable record), submit (no-op). **No background drain worker** — nothing real to retry against until launch; retry is a launch-gated follow-up (natural ZMVP-51 sibling). Satisfies the contract; avoids speculative infra (YAGNI).
- **A2:** add a durable outbox table (status/attempts) + background worker w/ backoff now.
- *My argument for A1:* a retry-worker against a no-op directory is infrastructure for a code path that can't fire in v1; the *invariant* (idempotent, separate, not-in-UoW) is fully met by A1, and A2's worker belongs with the real-submission/monitor work. Your call.

**Fork B — Port shape.** Where does the update verb live?
- **B1 (recommended):** add `rebind_handle`/`update_also_known_as` to the existing **`DidMinter`** port (already `mint` + `tombstone` — all "operate this DID's op-log" verbs). Cohesive; one operational-key-signing surface; mirrors tombstone exactly.
- **B2:** new sibling port `HandleBinder`/`AlsoKnownAsWriter` (the ticket's parenthetical "e.g.").
- *My argument for B1:* one signer, one op-log — a second port fragments the signing surface with no distinct consumer (cf. traits-only-when-polymorphism-is-consumed). The ticket says "e.g.", not binding. Your call on the domain verb name too.

**Confirmed by verification (not open):** idempotency = content-address + `UNIQUE(cid)` (signing is deterministic — atrium-crypto `try_sign`+`normalize_s`); `prev` from our own `PlcOperationLog`, not fetched from the directory; submission gated-no-op in v1.
