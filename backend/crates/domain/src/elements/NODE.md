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
  - name: achievement.rs / blob.rs / character.rs
    role: documented stubs ahead of their epics
    node: false
  - name: workflow.rs
    role: an account's boards — Workflow, Column, card placement, and the base-62 fractional Position key
    node: false
---
**Is:** The domain's nouns: identity (user, account, actor identity, did, handle, keys), the commission aggregate, and value objects for maturity/markdown/profile/public records.

**Conventions:** each file owns its id type, value objects, and pure invariant logic; id newtypes may derive serde (`Did`, `UserId`, `AccountId` — the CLI identity file needs them) but loaded entities never do (projection is a separate concern). Closed-vocabulary enums (`GrantLevel`, `MaturityRating`) implement std `Display`/`FromStr`, never bespoke `as_str`/`parse` pairs. A UUID-backed id's `FromStr` returns `id::IdError`; `Display` impls pair with it.

**Entry points:** `commission/mod.rs`.

**Refs:** DESIGN "Commission" (3276807) · "The Changelog as the Only Record" (59310081, supersedes-in-part 30408741) · "Commission Ownership Separation" (29130754, amended 2026-09-04) · "Commission Composition" (45514754) · "Actor Addressing" (57081857).

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
