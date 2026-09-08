---
path: backend/crates/application/src/account
charted: 2026-09-06
fs:
  - name: create.rs
    role: found an Account — handle uniqueness + quarantine checks, mints a DID, seats the founder as Owner
    node: false
  - name: delete.rs
    role: Owner-gated soft-delete (has facts) or hard-delete (none); retries the did:plc tombstone on hard delete
    node: false
  - name: change_handle.rs
    role: rename under the rate-limit + quarantine rules (DD 27852802)
    node: false
  - name: leave.rs
    role: a member leaves; blocked for the Owner
    node: false
  - name: list.rs
    role: the caller's own memberships (self-view scope)
    node: false
  - name: transfer_ownership.rs
    role: reassign the Owner role to another member
    node: false
  - name: facts.rs + facts/exist.rs
    role: has-facts probe feeding delete's soft-vs-hard branch; stub, always false
    node: false
  - name: invitation.rs + invitation/{issue,accept,decline,revoke}.rs
    role: the pending account-membership invitation lifecycle
    node: false
  - name: role.rs + role/{grant,revoke}.rs
    role: grant or revoke a member's Role
    node: false
---
**Is:** Account use cases — founding, renaming, deleting an Account and the membership lifecycle (invitations, roles, leaving, ownership transfer, listing), one Command/Query + Output + `run` file per action; the module root `../account.rs` holds `AccountError`, `AccountPorts`, `AccountResult`.

**Entry points:** `../account.rs`.

**Refs:** DESIGN "Account" (1966081) · "Roles" (2162692) · DD 23003138 · DD 27852802 · DD 24182820 (invitation validity & issuer departure).
