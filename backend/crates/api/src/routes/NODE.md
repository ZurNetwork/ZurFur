---
path: backend/crates/api/src/routes
charted: 2026-09-12
fs:
  - name: mod.rs
    role: group index + require_first_party_origin CSRF middleware
    node: false
  - name: session.rs
    role: browser sign-in: POST /signin, GET /signin-callback, GET /me, POST /logout
    node: false
  - name: accounts.rs
    role: account/membership/invitation JSON API; thin driver: CallingUser → application::account::* → wire projection (ZMVP-205)
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

**Conventions:** each module exposes exactly one `*_router()`; adding a domain = a new module + one line in `app()`. This split is the intermediate step toward per-domain crates — extraction should be a move, not a redesign. `accounts.rs` handlers call `application::account::*` use cases and project the result to the generated wire type; no use-case logic lives in the handler.

**Entry points:** `mod.rs` (the group index + the CSRF layer).

**Refs:** ZMVP-39 (router split) · DD "Handle Resolution for *.zurfur.app" (26607618) (also `wellknown.rs` module doc) · DD "The Application Layer" (55836674, ZMVP-205) · DD "Domains and Applications" (11763713) (`mod.rs` module doc — subdomain→namespace→router seam) · DD "Actor Addressing — DID as the Only Identifier, Everywhere" (57081857) (`accounts.rs` — Account addressed by DID, no surrogate id) · DD "Account Handle Change Flow" (27852802) (`accounts.rs::PATCH /accounts/{id}/handle`).

## Notes
- `PATCH /accounts/{id}/handle`: DID-doc-first ordering — the did:plc UPDATE-op lands before the private-store write (DD 27852802).
- `wellknown.rs`'s namespace-membership check and `application::account`'s claim checks share one parsed `HandleDomain`, so they can never disagree on the namespace.
