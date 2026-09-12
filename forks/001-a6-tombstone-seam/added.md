- **F. Transactional intent + idempotent PLC command (proposed by the GPT seat)** — in the deletion
  transaction, enqueue an immutable command keyed by DID and the expected operation identity (`prev`
  and/or the signed-operation CID); the PLC adapter reconciles the local operation log and directory
  state before declaring success, so replay after a partial success is safe. Effectively a hardened B,
  with D as a guard and adapter-owned PLC *reconciliation* (not adapter-owned workflow durability).
  Also names a missing consideration: the recovery/cancellation race — tombstone retry, user-authorized
  recovery inside the 72h window, and eventual custody-key purge need one coherent state machine
  (generation numbers or command cancellation), or a delayed worker can tombstone a deliberately
  recovered identity.
- **G. Public-first ordering / reversible saga (proposed by the Gemini seat)** — the use case runs the
  tombstone on the directory BEFORE opening the private-store transaction (mirroring
  `change_handle.rs`), relying on the native 72h did:plc recovery window and the retained custody keys
  to reverse the public tombstone if the private commit then fails. No outbox at all. Attacks the
  framing's assumption that the seam must sit after the private commit.
