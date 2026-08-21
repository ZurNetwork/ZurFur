---
path: backend/crates/adapter-mem
charted: 2026-08-21
fs:
  - name: Cargo.toml
    role: domain + async-trait + cid/sha2 (real CIDv1 addressing in the fake)
    node: false
  - name: src/
    role: lib.rs = shared MemBackend, MemDatabase/MemUnitOfWork, auth/profile/DID fakes; actor_identity, commission, file_store, public_records add their seams
    node: false
  - name: tests/
    role: commission.rs + public_records.rs running the shared conformance suite
    node: false
---
**Is:** In-process fakes of every port so core development and most tests need neither a database nor a PDS — reproducing each contract (including transactional rollback) without the operational reality.

**Conventions:** fidelity, not realism — reproduce the contract a handler depends on; skip TTLs and real keypairs; call out intentional divergence on the item. Read/write split mirrors adapter-pg exactly. `begin()` deep-copies base+staged, `commit()` diffs key-by-key, so drop = rollback. `std::sync::Mutex`, never held across `.await`.

**Entry points:** `src/lib.rs`.
