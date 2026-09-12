---
path: backend/crates/domain/src/elements
charted: 2026-09-12
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
  - name: achievement.rs / blob.rs / character.rs
    role: documented stubs ahead of their epics
    node: false
  - name: workflow.rs
    role: an account's boards — Workflow, Column, card placement, and the base-62 fractional Position key
    node: false
---
**Is:** The domain's nouns: identity (user, account, actor identity, did, handle, keys), the commission aggregate, and value objects for maturity/markdown/profile/public records.

**Conventions:** each file owns its id type, value objects, and pure invariant logic. The serde-deriving id newtypes are `Did`, `UserId` and `AccountId` — the CLI identity file needs them. A UUID-backed id's `FromStr` returns `id::IdError`; its `Display` impl pairs with that.

**Entry points:** `commission/mod.rs`.

**Refs:** DESIGN "Commission" (3276807, commission/mod.rs) · "The Changelog as the Only Record" (59310081, supersedes-in-part 30408741; commission/changelog.rs, commission/mod.rs Archived variant) · "Commission Ownership Separation" (29130754, amended 2026-09-04; commission/positioning.rs) · "Commission Composition" (45514754; commission/element.rs) · "Actor Addressing" (57081857; did.rs, account.rs AccountId, user.rs UserId, character.rs CharacterId) · DD 3014657 "Deletion of Commissions" (commission/fact.rs, commission/file.rs, commission/mod.rs archived_at) · DD 29982722 "Maturity Vocabulary" (maturity.rs, commission/mod.rs maturity field) · DD 34013187 "Identities — the Actor Super-Table" (actor_identity.rs) · DD 28311564 "Referenceable, Slot & Seat" (commission/seat.rs) · DD 26804226 "did:plc Identity Custody, Minting & Credible Exit" (account_keys.rs, plc_operation.rs) · DD 24870914 "The Account Handle" (handle.rs module doc) · DD 26050561 "Confusable Handles & the Punycode Policy" (handle.rs PunycodeLabel variant) · DD 4358151 "DID:PLC vs DID:Web" (did.rs) · DD 23003138 "Account Deletion, Tombstoning & Handle Reuse" (account.rs handle field) · DD 21594113 "User-Profiles, the Handle Swap & Content Maturity" (account.rs ListingScope) · DD 29949954 "Gallery Posts, the Product Snapshot & Index-Side Tagging" (public_record.rs module doc) · DD 30572573 "Comments — The Replyable Trait" (public_record.rs ReplyRef) · DD 24150017 "Transactions as a capability" (profile.rs cache-fill exception) · DESIGN "Account" (1966081, account.rs) · DESIGN "Achievement" (1933322, achievement.rs) · DESIGN "Workflow" (9895957, workflow.rs) · DESIGN "Character" (5668866, character.rs) · DESIGN "Blob" (9994275, blob.rs) · DESIGN "Roles" (2162692, role.rs, user_account.rs) · DESIGN "Slots" (5931025, commission/slot.rs) · ZMVP-31 (commission/mod.rs doctest: maturity is unrated at birth, only the widening gate requires it) · ZMVP-48/ZMVP-45 (handle.rs, plain `//` body comments left in place — punycode reject / reserved-label reject test sections).

## Notes

- `commission/` layout: `mod.rs` = the envelope (id, title, owner, lifecycle, visibility, deadline, maturity, the two status axes, linked channel, archive); `element.rs` = the flat composition (tabs/surfaces/elements, `SKELETON`, `effective_visibility`); `fact.rs` = the hard-delete gate contract; `changelog.rs`; `file.rs`; `markup.rs`; `positioning.rs` (view grants only); `seat.rs` / `seat_invitation.rs` / `slot.rs` (element satellites).
- Composition structure is CODE (`element::SKELETON`), modes are DATA. Effective visibility = `min(tab, surface, element)` under the commission's own `Visibility`. An absent `commission_surface_mode` row means `Total`. (DD 45514754)
- Surface names must be globally unique across tabs: `commission_surface_mode`'s PK is `(commission_id, surface)` with no tab column, so a duplicate name would make widening one tab's surface widen the other's. Pinned by a unit test in `element.rs`.
- `CommissionComposition`, `ElementRow` and `ElementPayload` implement no `serde::Serialize` **by design** — content leaves only through a viewer projection that applied `effective_visibility` server-side. A compile-time probe test in `element.rs` pins the absence.
- Slots and Seats share one identity with their carrying element (`ElementType::SLOT_TAG` / `SEAT_TAG`); the interpreted data lives in a satellite table keyed by the element id.
- Placement is account-side and lives in `workflow.rs` (a card on a board), never in `commission/positioning.rs`. View grants are issued per-**User**. (DD 29130754 as amended)
- `workflow.rs` orders columns and cards by `Position`, a base-62 fractional key compared **bytewise** — a store must order it bytewise too (`text COLLATE "C"`).
- Fact-bearing types must register their table in the pg adapter's `COMMISSION_FACT_TABLES` in the same change that introduces them, and mirror the check in the mem fake; non-fact commission-owned tables go in `COMMISSION_NON_FACT_TABLES`.
- Persisted enums (`LifecycleStep`, `Visibility`, `DirectionStatus`, `DeadlineStatus`, `MaturityRating`, `ChangelogEntryKind`, `VisibilityMode`, `GrantLevel`, `ActorKind`, `ActorState`, `InvitationState`, `Role`) own their storage tokens: renaming one is a migration, and an unknown token is always an error, never a silent default.
- `Late` is derived at read (`commission::derive_deadline_status`), never persisted; the `deadline_status` column only ever holds `Delayed`.
