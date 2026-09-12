---
path: docs
charted: 2026-09-12
fs:
  - name: confluence-design-index.md
    role: local pointer index of every Confluence DESIGN page (titles + ids + fetch coordinates)
    node: false
---
**Is:** The local pointer index into Confluence DESIGN — titles, page IDs, and fetch coordinates only, never copied page content.

**Conventions:** this is the index every `Refs` section resolves against, so it is the one place page ids belong in bulk — but still ids and titles only, never page bodies. `/optimize-memory` and `/save-reference` maintain it; don't hand-append.

**Entry points:** `docs/confluence-design-index.md`.

**Refs:** none — this directory is what the other nodes' Refs point at.
