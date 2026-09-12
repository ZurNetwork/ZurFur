---
path: backend/crates/adapter-mem
charted: 2026-09-12
fs:
  - name: Cargo.toml
    role: domain + async-trait + cid/sha2 (real CIDv1 addressing in the fake) + tokio io-util for the streaming FileStore seam
    node: false
  - name: src/lib.rs
    role: shared MemBackend (incl. markups map) + MemDatabase/MemUnitOfWork; blob_count() inspect helper; MemUserStore/Writes, MemAccountStore/Writes, MemAuthenticator, MemProfileSource, MemProfileCache, MemDidMinter, MemKeyStore, MemPlcOperationLog
    node: false
  - name: src/actor_identity.rs
    role: MemActorIdentityStore/Writes + StoredActorIdentity
    node: false
  - name: src/commission.rs
    role: commission/slot/seat/tab/element/changelog/markup fakes — MemCommissionStore/Writes, MemChangelogStore/Writes + Stored* rows
    node: false
  - name: src/workflow.rs
    role: MemWorkflowStore/Writes + MemColumnStore/Writes — the account's boards; a card on a column IS a commission's placement
    node: false
  - name: src/file_store.rs
    role: MemFileStore (streaming FileStore fake over Vec<u8>+Cursor, StoredBlob)
    node: false
  - name: src/public_records.rs
    role: MemPublicRecords — content-addressed (CIDv1/sha2-256) fake of the PDS boundary
    node: false
  - name: tests/commission.rs
    role: commission port conformance run against the mem fakes
    node: false
  - name: tests/public_records.rs
    role: shared PublicRecords conformance suite (test-support::contract) run against MemPublicRecords
    node: false
---
**Is:** In-process fakes of every domain port (private + public store) so core development and most tests need neither a database nor a PDS, reproducing each contract — including transactional rollback — without the operational reality.

**Conventions:** fidelity, not realism — reproduce the contract a handler depends on (idempotent recognition, soft-delete invisibility, cache hits); skip TTLs and real keypairs; call out intentional divergence on the item. One shared `MemBackend` owns the maps, read stores read `&self`, and `MemDatabase::begin` takes two independent deep copies (base + staged); `MemUnitOfWork::commit` diffs key-by-key and merges only this unit's changes back — not a wholesale replace; drop without commit discards both (rollback). The read-through `MemProfileCache` is the one Unit-of-Work exemption: its best-effort fill writes straight to the shared store, neither staged, merged, nor rolled back. Locking: `std::sync::Mutex`, never held across `.await` — each method locks, does sync map work, drops the guard, then returns; poisoned lock is unrecoverable so `.lock().expect()` throughout. Call counters use `AtomicUsize`, no lock needed. The `blobs` map is a second exemption: shared by `Arc`, never staged/merged (mirrors the FileStore's non-transactional port); `markups` is NOT — it stages/merges like every other commission-owned map.

**Entry points:** `src/lib.rs`.

**Refs:** DD "Transactions as a capability — compile-enforced Unit of Work" (24150017, src/lib.rs + src/commission.rs: the stage/commit/rollback contract) · memory `project_transaction_unit_of_work` · DD "Commission Ownership Separation — View Grants & Account Placement" (29130754, src/lib.rs + src/commission.rs + src/workflow.rs: view grants are a pure key, revoke hard-deletes, placement lives only on a column/card, participants unaffected by placement/grants) · DD "The Changelog as the Only Record" (59310081, src/lib.rs + src/commission.rs: changelog is append-only and commits atomically with the write it records) · DD "Commission Composition — Surfaces as Extension Points" (45514754, src/lib.rs + src/commission.rs: elements addressed by (tab, surface) pair, no parent field) · DD "Identities — the Actor Super-Table" (34013187, src/lib.rs: actor identity rows are immortal) · DD "Actor Addressing — DID as the Only Identifier" (57081857, src/lib.rs: a UserId IS the DID the map is keyed by) · DD "Account Deletion, Tombstoning & Handle Reuse" (23003138, src/lib.rs: soft-delete tombstones still reserve the handle; handle uniqueness spans live+soft-deleted rows) · DD "Account Handle Change Flow" (27852802, src/lib.rs: the append-only handle-change audit log) · DD "User-Profiles, the Handle Swap & Content Maturity" (21594113, src/lib.rs: `listed_on_profile` on the membership row) · DD "Deletion of Commissions" (3014657, src/commission.rs: `commission_has_facts` stays `false` until a fact-minter registers a map here).

## Notes
- Unit-of-Work exemptions — maps shared by `Arc` at `stage()`, never staged or merged: `profiles` (read-through cache fill) and `blobs` (the `FileStore` write sits outside the unit; a rolled-back unit accepts an orphaned blob). Every other map is deep-copied twice (`base` + `staged`).
- Known divergences from pg: `merge` does not model a same-key write-write conflict (last-writer-wins, where pg serializes on a row lock); `commission_has_facts` is constant `false` until a fact-minter exists — whoever registers the first fact table in `adapter-pg/src/commission.rs` must add the matching map here (DD 3014657); `CommissionWrites::delete` cascades tabs/elements/surface modes/satellites but NOT participants, files or positioning.
- Element addressing: every element write goes through `require_address` → `require_tab`, in that order, so an address wrong in both ways refuses as `UnknownTab`, not `UnknownSurface` — a parity test pins the order. Lock order is always `tabs` before `elements`.
- Placement lives only in `workflow.rs`: a card on a column IS a commission's placement, and the commission side never learns of it (DD 29130754).
- Rows that are never removed anywhere: `actor_identities`, `participants`, changelog entries (except a parent's cascade), `markups`.
