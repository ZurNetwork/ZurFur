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
  - name: workflow.rs + workflow/{create,delete}.rs + workflow/column/**
    role: the account's boards — create/delete a board, add/rename/remove/reposition its columns, and place or remove a commission card in a column
    node: false
---
**Is:** Account use cases — founding, renaming, deleting an Account and the membership lifecycle (invitations, roles, leaving, ownership transfer, listing), one Command/Query + Output + `run` file per action; the module root `../account.rs` holds `AccountError`, `AccountPorts`, `AccountResult`.

**Entry points:** `../account.rs`.

**Refs:** DESIGN "Account" (1966081) · "Roles" (2162692) · DESIGN "Workflow" (9895957) · DD 23003138 · DD 27852802 · DD 24182820 (invitation validity & issuer departure) · DD 29130754 (view grants).

## Notes
- `require_live_account` (in `../account.rs`) is the shared liveness gate: existence is settled BEFORE standing, because `role_of` reads the membership table with no tombstone predicate. An unknown id and a soft-deleted account get the same answer. (DD 23003138)
- Ordering rule, uniform across `invitation/revoke`, `role/grant` and `workflow/column/commission/set_in_column`: the actor's own standing is settled before the target is looked up or provisioned. `provision` is a WRITE keyed to a caller-named DID, and the refusals below it are distinguishable on the wire — probing first leaks membership and actor class.
- Two rails put a commission card on a board: PULL (the commission is publicly visible) or PUSH (the actor's OWN standing or OWN view grant). View keys are per-User, so membership of the owning account lifts nothing. Failure is the commission not-found, never `IncorrectRole`. (DD 29130754)
- `account/facts/exist.rs` is a stub that always answers false; it feeds `delete`'s soft-vs-hard branch, and `delete` carries the open `FIXME` for it.
