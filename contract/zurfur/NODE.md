---
path: contract/zurfur
charted: 2026-09-12
fs:
  - name: api/
    role: pass-through; holds only v1/, which carries the corpus and its own node
    node: false
---
**Is:** Pass-through namespace directory mirroring the proto package `zurfur.api.v1`; the content is two levels down at `api/v1/`.

**Entry points:** `api/v1/` (charted).

**Refs:** none — the contract's pointers live one level up in `contract/NODE.md`.
