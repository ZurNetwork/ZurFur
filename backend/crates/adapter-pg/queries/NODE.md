---
path: backend/crates/adapter-pg/queries
charted: 2026-08-21
fs:
  - name: account/
    role: 26 statements: create/find/list, memberships & roles, invitations, handle change + rate audit, soft & hard delete, departure re-homing, ownership transfer
    node: false
  - name: commission/
    role: 38 statements: create/find/delete, elements & tabs & surface modes, seats & invitations, slots, placement & view grants, statuses, maturity, archive, files, linked channel
    node: false
  - name: actor_identity/
    role: create, intern (race-safe upsert), find, find_by_did, cache_handle
    node: false
  - name: changelog/
    role: append + ordered read
    node: false
  - name: session/
    role: create, load, save, delete, delete_expired
    node: false
  - name: user/
    role: find, find_by_did, provision
    node: false
  - name: profile/
    role: cache get/put
    node: false
  - name: file/
    role: blob get/put/delete by opaque FileKey
    node: false
  - name: key_store/
    role: custody key get/put
    node: false
  - name: plc/
    role: append, latest_cid, latest_op
    node: false
  - name: health/
    role: is_reachable
    node: false
---
**Is:** The SQL corpus as data: one statement per file under a namespace directory, described against a live migrated schema to generate typed Rust accessors.

**Conventions:** directory name = generated module; file name = generated function. A leading `--` comment becomes the function's doc. Adding/editing a `.sql` file requires regenerating `src/queries.rs` or `codegen_current` fails.

**Refs:** sqlx-rust-codegen (pinned in root Cargo.toml) · memory `project_sqlx_rust_codegen`.
