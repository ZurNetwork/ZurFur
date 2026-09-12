---
path: backend/crates/domain/src
charted: 2026-09-12
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

**Refs:** DESIGN "Domains and Applications" (11763713, lib.rs: the dependency rule) · DD 24150017 "Transactions as a capability" (ports/mod.rs Database::begin/ProfileCache, ports/commission.rs; also lib.rs's profile.rs cache-fill exception) · DD 59310081 "The Changelog as the Only Record" (ports/mod.rs UnitOfWork::changelog, ports/changelog.rs x2) · DD 34013187 "Identities — the Actor Super-Table" (ports/mod.rs UnitOfWork::actor_identities + HasDid marker, ports/actor_identity.rs) · DD 26607618 "Handle Resolution for *.zurfur.app" (ports/mod.rs find_did_by_handle) · DD 27852802 "Account Handle Change Flow" (ports/mod.rs count_handle_changes_since/handle_reserved_for_other/change_handle/update_handle) · DD 23003138 "Account Deletion, Tombstoning & Handle Reuse" (ports/mod.rs HandleTaken/soft_delete/hard_delete/tombstone) · DD 24182820 "Invitation Validity & Issuer Departure" (ports/mod.rs revoke_role) · DD 26935298 "Zurfur Public Presence & PDS — Identity-Only for v1" (ports/mod.rs DidMinter) · DESIGN "Workflow" (9895957, ports/workflow.rs) · DD 29130754 "Commission Ownership Separation" (ports/commission.rs view_grant/grant_view/revoke_view) · DD 45514754 "Commission Composition" (ports/commission.rs load_composition/add_element) · DD 3014657 "Deletion of Commissions" (ports/commission.rs commission_has_facts/delete/set_archived) · DD 29982722 "Maturity Vocabulary" (ports/commission.rs set_maturity).

## Notes

- Adapters: `adapter-pg` implements the private-store ports, `adapter-atproto` the public ones (`PublicRecords`, the real `DidMinter`), `adapter-mem` fakes both.
- `UnitOfWork` vends exactly seven write ports: `accounts`, `commissions`, `changelog`, `users`, `actor_identities`, `workflows`, `columns`.
- Documented exceptions to the Unit of Work — pool-backed `&self` writes: `ProfileCache` (read-through cache fill), `FileStore` (blob bytes can't ride a Postgres transaction), and the api-side `session_store`/`auth_store`.
- Changelog entries are appended in the same unit as the domain write they record, and nothing is derived from them (59310081).
- `commission_element` carries a **composite** FK `(tab_id, commission_id) → commission_tab (id, commission_id)`, so an element citing another commission's tab is unwritable; `UnknownTab` is the friendly 404 half of that rule (45514754).
- View grants are per-User, never per-Account; membership confers no view (29130754, amended 2026-09-04).
- `DidOperations` is interface-only — no adapter implements it, and whether `tombstone`/`update_handle` migrate off `DidMinter` is an open Engineer ruling.
