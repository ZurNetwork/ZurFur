---
path: lexicons
charted: 2026-08-21
fs:
  - name: app.zurfur.feed.post.json
    role: the unified content primitive record (key: tid): gallery publication, comment, shout — a reply is a post with `reply` set
    node: false
  - name: app.zurfur.feed.defs.json
    role: shared feed defs — credit {role, did} with OPEN knownValues
    node: false
  - name: app.zurfur.embed.media.json
    role: single visual-media embed: blob + required alt + optional aspectRatio; 100 MB maxSize, per-mime caps app-side
    node: false
  - name: app.zurfur.graph.collection.json
    role: public homogeneous Collection record — static hand-curated or dynamic Lens-derived
    node: false
  - name: app.zurfur.graph.defs.json
    role: graph defs — `lens` is an explicit NON-NORMATIVE placeholder
    node: false
  - name: lexicons.test.js
    role: the gate: loads every doc into one @atproto/lexicon Lexicons instance (clean .add() IS validity) + structural assertions
    node: false
  - name: package.json
    role: private CommonJS `zurfur-lexicons`; npm test → node --test; devDep @atproto/lexicon
    node: false
  - name: vendor/
    role: upstream atproto core lexicons vendored verbatim (label.defs, repo.strongRef) so refs resolve offline — a plain tracked dir, not a submodule
    node: false
---
**Is:** The source of truth for Zurfur's atproto lexicon JSON — the Class A, PDS-canonical record and def schemas under `app.zurfur.*`, plus a hermetic test validating the graph against the real atproto meta-schema.

**Conventions:** JSON-only — no Rust codegen from lexicons. Filename == NSID == the doc's `id`; `"lexicon": 1` everywhere. One-way door: fields only ADDED, never removed or tightened — a breaking change is a NEW NSID. Publish-late: a lexicon publishes only when its feature ships. Vocabularies are OPEN strings with `knownValues`. What the meta-schema can't express (either-of rules, per-mime caps, consent paths) is enforced app-side and documented inline. `description` fields carry the normative rationale with DESIGN citations; stubs are labelled PLACEHOLDER. `vendor/*.json` are never edited.

**Entry points:** `app.zurfur.feed.post.json` · `npm test` from this directory.

**Refs:** DD "The Lexicon Registry — Publish-Late, Additive-Only" (29818896) · "The AT Protocol Boundary Contract" (29622283) · "Gallery Posts" (29949954) · "Comments — The Replyable Trait" (30572573) · DESIGN "Lexicon" (10354710).
