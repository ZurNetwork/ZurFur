---
path: backend/crates/application/src/commission
charted: 2026-09-06
fs:
  - name: create.rs
    role: create a Commission for the caller (+ creation changelog entry, skeleton tabs)
    node: false
  - name: delete.rs
    role: fact-free hard delete
    node: false
  - name: archive.rs + unarchive.rs
    role: toggle the archived state
    node: false
  - name: list.rs
    role: list Commissions (owner point of view)
    node: false
  - name: place.rs
    role: owner places a Commission under one of their Accounts
    node: false
  - name: markup.rs
    role: record a review-loop Markup on a file entry; authorize first, numeric bounds validated here
    node: false
  - name: maturity.rs + maturity/set.rs
    role: replace the maturity rating + graphic flag
    node: false
  - name: notes.rs + notes/attach.rs
    role: attach a note
    node: false
  - name: seats.rs + seats/declare.rs
    role: declare a vacant typed Seat
    node: false
  - name: slots.rs + slots/declare.rs
    role: declare an all-or-nothing Slot batch
    node: false
  - name: deadline.rs + deadline/{set,clear}.rs + deadline/status/{set,clear}.rs
    role: set/clear the deadline; set/clear the manual Delayed flag
    node: false
  - name: status.rs + status/direction/{set,clear}.rs
    role: set/clear the direction-status override
    node: false
  - name: invitations.rs + invitations/{issue,revoke}.rs
    role: issue/revoke a seat invitation
    node: false
  - name: view.rs + view/{grant,revoke}.rs
    role: grant/revoke a per-User view grant (DD 29130754 as amended)
    node: false
  - name: files.rs + files/{upload,download}.rs
    role: streaming upload (auth before read, capped, blob written pre-tx) and gated download
    node: false
  - name: changelog.rs + changelog/read.rs
    role: cursor-paginated read of the changelog (DD 59310081)
    node: false
---
**Is:** Commission use cases — lifecycle (create/delete/archive/unarchive/place) and per-facet action trees (deadline, status, files, invitations, view, seats, slots, notes, maturity, markup, changelog), one Command/Query + Output + `run` file per action; the module root `../commission.rs` holds `CommissionError`, `CommissionPorts`, `CommissionResult` and the system-actor `sweep_deadlines`.

**Entry points:** `../commission.rs`.

**Refs:** DESIGN "Commission" (3276807) · DD 45514754 (composition) · DD 30408741 + 59310081 (changelog) · DD 29130754 (view grants) · ZMVP-88 (file entries streaming seam) · ZMVP-86 (deadline sweep).
