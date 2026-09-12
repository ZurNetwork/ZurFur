---
path: backend/crates/domain/src/elements
charted: 2026-09-06
fs:
  - name: account.rs
    role: Account, AccountId, membership, ListingScope
    node: false
  - name: account_keys.rs
    role: custody key material, zeroized on drop
    node: false
  - name: actor_identity.rs
    role: actor super-table row: kind, state, nullable DID, handle cache
    node: false
  - name: user.rs
    role: User — a recognized visitor
    node: false
  - name: user_account.rs
    role: membership tuple (user × account × role)
    node: false
  - name: role.rs
    role: Role — data-less Owner/Admin/Manager/Member enum + can_grant rule; RoleAlias newtype + UnknownRole/InvalidRoleAlias errors; Display impls
    node: false
  - name: invitation.rs
    role: pending account-membership offer
    node: false
  - name: id.rs
    role: IdError — shared parse-failure type every UUID-backed id newtype's FromStr returns
    node: false
  - name: did.rs
    role: DID newtype
    node: false
  - name: handle.rs
    role: validated, normalized atproto handle — the one claim-validation gate
    node: false
  - name: plc_operation.rs
    role: PLC operation record shape
    node: false
  - name: profile.rs
    role: PDS-owned public profile
    node: false
  - name: public_record.rs
    role: AtUri, BlobRef, PublicRecord, RecordRef
    node: false
  - name: maturity.rs
    role: atproto self-label axis + orthogonal Graphic flag
    node: false
  - name: markdown.rs
    role: markdown value object
    node: false
  - name: commission/
    role: the aggregate: mod.rs (birth shape, visibility), element (flat composition), fact, changelog, file, markup, positioning, seat, seat_invitation, slot
    node: false
  - name: achievement.rs / blob.rs / character.rs / workflow.rs
    role: documented stubs ahead of their epics
    node: false
---
**Is:** The domain's nouns: identity (user, account, actor identity, did, handle, keys), the commission aggregate, and value objects for maturity/markdown/profile/public records.

**Conventions:** each file owns its id type, value objects, and pure invariant logic; id newtypes may derive serde (`Did`, `UserId`, `AccountId` — the CLI identity file needs them) but loaded entities never do (projection is a separate concern). Closed-vocabulary enums (`GrantLevel`, `MaturityRating`) implement std `Display`/`FromStr`, never bespoke `as_str`/`parse` pairs. A UUID-backed id's `FromStr` returns `id::IdError`; `Display` impls pair with it.

**Entry points:** `commission/mod.rs`.

**Refs:** DESIGN "Commission" (3276807) · "The Changelog" (30408741) · "Commission Ownership Separation" (29130754).
