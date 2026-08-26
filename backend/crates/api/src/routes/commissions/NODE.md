---
path: backend/crates/api/src/routes/commissions
charted: 2026-08-21
fs:
  - name: mod.rs
    role: router assembly, body limits, require_participant closed-door gate
    node: false
  - name: create.rs
    role: POST /commissions (+ creation changelog entry, skeleton tabs)
    node: false
  - name: list.rs
    role: GET /commissions (owner POV)
    node: false
  - name: delete.rs
    role: DELETE — fact-free hard delete
    node: false
  - name: archive.rs
    role: POST archive / unarchive
    node: false
  - name: changelog.rs
    role: GET /{id}/changelog
    node: false
  - name: notes.rs
    role: POST /{id}/notes
    node: false
  - name: channel.rs
    role: PUT/DELETE /{id}/channel linked-channel pointer
    node: false
  - name: maturity.rs
    role: PUT /{id}/maturity (replace-only)
    node: false
  - name: elements.rs
    role: POST/DELETE elements — the flat composition write surface
    node: false
  - name: slots.rs
    role: POST /{id}/slots — all-or-nothing Slot batch
    node: false
  - name: seats.rs
    role: POST /{id}/seats — declare a vacant typed Seat
    node: false
  - name: invitations.rs
    role: POST/DELETE seat invitations
    node: false
  - name: status.rs
    role: PUT/DELETE /{id}/status/direction
    node: false
  - name: deadline.rs
    role: PUT/DELETE deadline + manual Delayed flag
    node: false
  - name: files.rs
    role: commission file entries (multipart upload)
    node: false
  - name: markup.rs
    role: POST /{id}/files/{file_id}/markup
    node: false
  - name: positioning.rs
    role: account-side placement + view grants
    node: false
---
**Is:** The user-scoped commission API, one file per act, all behind a uniform closed-door 404 for non-participants.

**Conventions:** commissions are user-scoped — no Account required (DD 26247170) — and entirely Index-side (never touch atproto). Existence is participant-only knowledge: non-participants and absent ids get the same 404, never a 403. A new act adds a file here rather than growing a hotspot.

**Refs:** DESIGN "Commission" (3276807) · DD "Commission Composition" (45514754) · DD "The Changelog" (30408741).
