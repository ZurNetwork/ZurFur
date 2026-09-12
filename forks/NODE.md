---
path: forks
charted: 2026-09-12
fs:
  - name: 001-a6-tombstone-seam/
    role: one boardroom SCRUTINIZE run's working files (brief, seat transcripts, sealed ranking, verdict)
    node: false
  - name: .counter
    role: next fork number for the boardroom skill to mint
    node: false
---
**Is:** The `/boardroom` skill's working directory — one numbered subfolder per multi-model deliberation run (each holds the brief, per-phase seat transcripts, the sealed Engineer ranking, and the judge's synthesis).

**Conventions:** Numbered `NNN-slug` folders, minted from `.counter`. `sealed.md` holds the Engineer's ranking given BEFORE the seats run and must never leak into seat-facing prompts (`run.py` asserts this). Deliberation only — nothing here is a decision; a decided fork still routes through `/design-decision`.

**Entry points:** `forks/<NNN-slug>/brief.md` (the proposal put to the board); `forks/<NNN-slug>/run.py` (the runner).

**Refs:** DD 23003138 — Account Deletion, Tombstoning & Handle Reuse · DD 57081857 — Actor Addressing, DID everywhere · DD 55836674 — The Application Layer (all three grounded fork 001's brief). Citations inside a brief or transcript stay put: these are debate artifacts, not doc comments, and stripping them would break the record of what was actually scrutinized.
