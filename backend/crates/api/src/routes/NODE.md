---
path: backend/crates/api/src/routes
charted: 2026-08-21
fs:
  - name: mod.rs
    role: group index + require_first_party_origin CSRF middleware
    node: false
  - name: session.rs
    role: browser sign-in: POST /signin, GET /signin-callback, GET /me, POST /logout
    node: false
  - name: accounts.rs
    role: account/membership/invitation JSON API
    node: false
  - name: commissions/
    role: the commission JSON API, one file per act
    node: true
  - name: health.rs
    role: GET /health, outside the CSRF layer
    node: false
  - name: wellknown.rs
    role: /.well-known/atproto-did handle→DID resolution for *.zurfur.app
    node: false
---
**Is:** Route groups split along the DESIGN subdomain→namespace→router seam, each an independently mounted policy zone.

**Conventions:** each module exposes exactly one `*_router()`; adding a domain = a new module + one line in `app()`. This split is the intermediate step toward per-domain crates — extraction should be a move, not a redesign.

**Refs:** ZMVP-39 (router split) · DD "Handle Resolution for *.zurfur.app" (26607618).
