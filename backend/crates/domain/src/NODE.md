---
path: backend/crates/domain/src
charted: 2026-08-29
fs:
  - name: lib.rs
    role: crate root; the ports-and-adapters position and the eventual split
    node: false
  - name: elements.rs
    role: module index for elements/, live entities vs stubs
    node: false
  - name: elements/
    role: one entity/value-object cluster per file
    node: true
  - name: ports/
    role: trait seams: mod.rs = Database/UnitOfWork/User/Account/Profile/Did/Key/Plc ports; actor_identity, changelog, commission, file re-exported flat
    node: false
  - name: datetime.rs
    role: DateTimeUtc — the single injected clock type
    node: false
  - name: string_builder.rs
    role: StringBuilder: the one trimmed/capped-string gate every newtype validates through (ZMVP-113)
    node: false
---
**Is:** Four public modules: the nouns (elements), the seams (ports), the one clock type, and the shared string-validation builder.

**Conventions:** a new port area adds a file under `ports/` rather than growing `mod.rs`. Time is injected, never read from the ambient clock.

**Entry points:** `ports/mod.rs` (Database + UnitOfWork traits).

## Notes

- Adapters: `adapter-pg` implements the private-store ports, `adapter-atproto` the public ones (`PublicRecords`, the real `DidMinter`), `adapter-mem` fakes both. `api` is the only crate that knows which is live (11763713).
- Writes vs reads: every private-store *write* port is vended by `UnitOfWork` (`accounts`, `commissions`, `changelog`, `users`, `actor_identities`, `workflows`, `columns`); every `*Store` read port is pool-backed (24150017).
- Documented exceptions to the Unit of Work — pool-backed `&self` writes: `ProfileCache` (read-through cache fill), `FileStore` (blob bytes can't ride a Postgres transaction), and the api-side `session_store`/`auth_store`.
- Ordering rule: a public-boundary step (PDS record write, `DidMinter` mint/tombstone/`update_handle`, `FileStore::put`) never runs inside a private transaction — it is its own retryable step, before or after the unit (no cross-store dual write).
- Changelog entries are appended in the same unit as the domain write they record, and nothing is derived from them (59310081).
- `commission_element` carries a **composite** FK `(tab_id, commission_id) → commission_tab (id, commission_id)`, so an element citing another commission's tab is unwritable; `UnknownTab` is the friendly 404 half of that rule (45514754).
- View grants are per-User, never per-Account; membership confers no view (29130754, amended 2026-09-04).
- `DidOperations` is interface-only — no adapter implements it, and whether `tombstone`/`update_handle` migrate off `DidMinter` is an open Engineer ruling.
