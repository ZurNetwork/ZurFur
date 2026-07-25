# Port type audit — raw material (2026-07-25)

Every generated query row and return type, extracted mechanically from
`adapter-pg/src/queries.rs` + `adapter-atproto/src/queries.rs` (both `@generated`).
Postgres types inferred from the Rust mapping (i64←bigint, i32←int4, String←text,
DateTime←timestamptz, Uuid←uuid, bool←bool, Vec<u8>←bytea; Option←NULLABLE).

Two judgment columns are yours, Engineer:
- **ceiling** — the smallest type the domain can defend for the major's lifetime
  (int32 / int64-as-string / n/a). Ruling: widening later is BREAKING.
- **absence** — for Option fields: `not-set` (plain absence, key omitted) or
  `MEANING` (absence carries positive semantics → needs an explicit discriminant,
  never presence-encoding).

## ⚑ The audit surface: 5 integer fields

| adapter | module | row | field | rust | pg | **ceiling (yours)** |
| --- | --- | --- | --- | --- | --- | --- |
| pg | changelog | EntriesRow | seq | `i64` | bigint |  |
| pg | commission | CurrentPlacementRow | seq | `i64` | bigint |  |
| pg | commission | LoadTreeRow | position | `i32` | int4 |  |
| pg | commission | PlacementLogRow | seq | `i64` | bigint |  |
| pg | file | GetRow | byte_size | `i64` | bigint |  |

## ⚑ The audit surface: 33 nullable fields

| adapter | module | row | field | rust | **absence (yours)** |
| --- | --- | --- | --- | --- | --- |
| pg | account | FindRow | did | `Option<String>` |  |
| pg | account | FindRow | deleted_at | `Option<chrono::DateTime<chrono::Utc>>` |  |
| pg | account | ListForUserRow | did | `Option<String>` |  |
| pg | account | ListForUserRow | deleted_at | `Option<chrono::DateTime<chrono::Utc>>` |  |
| pg | actor_identity | ActorIdentityRow | did | `Option<String>` |  |
| pg | actor_identity | ActorIdentityRow | handle | `Option<String>` |  |
| pg | changelog | EntriesRow | actor_id | `Option<uuid::Uuid>` |  |
| pg | changelog | EntriesRow | note | `Option<String>` |  |
| pg | commission | CommissionRow | deadline | `Option<chrono::DateTime<chrono::Utc>>` |  |
| pg | commission | CommissionRow | maturity | `Option<String>` |  |
| pg | commission | CommissionRow | graphic | `Option<bool>` |  |
| pg | commission | CommissionRow | direction_status | `Option<String>` |  |
| pg | commission | CommissionRow | deadline_status | `Option<String>` |  |
| pg | commission | CommissionRow | linked_channel | `Option<String>` |  |
| pg | commission | CommissionRow | archived_at | `Option<chrono::DateTime<chrono::Utc>>` |  |
| pg | commission | FindRow | deadline | `Option<chrono::DateTime<chrono::Utc>>` |  |
| pg | commission | FindRow | maturity | `Option<String>` |  |
| pg | commission | FindRow | graphic | `Option<bool>` |  |
| pg | commission | FindRow | direction_status | `Option<String>` |  |
| pg | commission | FindRow | deadline_status | `Option<String>` |  |
| pg | commission | FindRow | linked_channel | `Option<String>` |  |
| pg | commission | FindRow | archived_at | `Option<chrono::DateTime<chrono::Utc>>` |  |
| pg | commission | LapsedDeadlinesRow | deadline_status | `Option<String>` |  |
| pg | commission | LoadTreeRow | parent | `Option<uuid::Uuid>` |  |
| pg | commission | LoadTreeRow | mode | `Option<String>` |  |
| pg | commission | RequireSurfaceParentRow | mode | `Option<String>` |  |
| pg | commission | SeatsRow | prompt | `Option<String>` |  |
| pg | commission | SeatsRow | link | `Option<String>` |  |
| pg | commission | SeatsRow | occupant | `Option<uuid::Uuid>` |  |
| pg | plc | LatestOpRow | prev | `Option<String>` |  |
| pg | profile | GetRow | display_name | `Option<String>` |  |
| pg | profile | GetRow | avatar_url | `Option<String>` |  |
| pg | user | FindRow | did | `Option<String>` |  |

## Functions returning bare integers (60) — internal, but listed for completeness

| adapter | module | fn | returns |
| --- | --- | --- | --- |
| pg | account | accept_invitation_flip | `u64` |
| pg | account | change_handle_audit | `u64` |
| pg | account | change_handle_repoint | `u64` |
| pg | account | count_handle_changes_since | `i64` |
| pg | account | create_account | `u64` |
| pg | account | create_invitation | `u64` |
| pg | account | create_owner_membership | `u64` |
| pg | account | departure_delete_membership | `u64` |
| pg | account | departure_rehome_children | `u64` |
| pg | account | departure_revoke_invitations | `u64` |
| pg | account | grant_role | `u64` |
| pg | account | hard_delete_account | `u64` |
| pg | account | hard_delete_invitations | `u64` |
| pg | account | hard_delete_memberships | `u64` |
| pg | account | revoke_invitation | `u64` |
| pg | account | soft_delete | `u64` |
| pg | account | transfer_demote_owner | `u64` |
| pg | account | transfer_promote_heir | `u64` |
| pg | actor_identity | cache_handle | `u64` |
| pg | actor_identity | create | `u64` |
| pg | changelog | append | `u64` |
| pg | commission | add_component | `u64` |
| pg | commission | add_file | `u64` |
| pg | commission | add_participant | `u64` |
| pg | commission | add_surface | `u64` |
| pg | commission | create_commission | `u64` |
| pg | commission | create_root_surface | `u64` |
| pg | commission | create_seat_invitation | `u64` |
| pg | commission | declare_seat_node | `u64` |
| pg | commission | declare_seat_satellite | `u64` |
| pg | commission | declare_slot_node | `u64` |
| pg | commission | declare_slot_satellite | `u64` |
| pg | commission | delete | `u64` |
| pg | commission | grant_view | `u64` |
| pg | commission | place_append | `i64` |
| pg | commission | place_repoint_current | `u64` |
| pg | commission | remove_node_delete | `u64` |
| pg | commission | remove_node_renumber | `u64` |
| pg | commission | revoke_seat_invitation | `u64` |
| pg | commission | revoke_view | `u64` |
| pg | commission | set_archived | `u64` |
| pg | commission | set_deadline | `u64` |
| pg | commission | set_deadline_status | `u64` |
| pg | commission | set_direction_status | `u64` |
| pg | commission | set_linked_channel | `u64` |
| pg | commission | set_maturity | `u64` |
| pg | file | delete | `u64` |
| pg | file | put | `u64` |
| pg | health | is_reachable | `i32` |
| pg | key_store | put | `u64` |
| pg | plc | append | `u64` |
| pg | profile | put | `u64` |
| pg | session | create | `u64` |
| pg | session | delete | `u64` |
| pg | session | delete_expired | `u64` |
| pg | session | save | `u64` |
| atproto | auth_store | delete_auth_req_info | `u64` |
| atproto | auth_store | delete_session | `u64` |
| atproto | auth_store | save_auth_req_info | `u64` |
| atproto | auth_store | upsert_session | `u64` |

## Full inventory — 22 row structs

**pg · account · `AcceptInvitationSeatRow`**
- `account_id: uuid::Uuid`
- `user_id: uuid::Uuid`
- `role: String`

**pg · account · `AccountInvitationsRow`**
- `id: uuid::Uuid`
- `account_id: uuid::Uuid`
- `invited_user: uuid::Uuid`
- `role: String`
- `inviter: uuid::Uuid`
- `state: String`
- `created_at: chrono::DateTime<chrono::Utc>`
- `updated_at: chrono::DateTime<chrono::Utc>`

**pg · account · `FindRow`**
- `id: uuid::Uuid`
- `did: Option<String>`
- `handle: String`
- `name: String`
- `created_at: chrono::DateTime<chrono::Utc>`
- `updated_at: chrono::DateTime<chrono::Utc>`
- `deleted_at: Option<chrono::DateTime<chrono::Utc>>`

**pg · account · `ListForUserRow`**
- `id: uuid::Uuid`
- `did: Option<String>`
- `handle: String`
- `name: String`
- `created_at: chrono::DateTime<chrono::Utc>`
- `updated_at: chrono::DateTime<chrono::Utc>`
- `deleted_at: Option<chrono::DateTime<chrono::Utc>>`
- `role: String`

**pg · actor_identity · `ActorIdentityRow`**
- `id: uuid::Uuid`
- `kind: String`
- `did: Option<String>`
- `state: String`
- `handle: Option<String>`
- `first_seen: chrono::DateTime<chrono::Utc>`

**pg · changelog · `EntriesRow`**
- `seq: i64`
- `kind: String`
- `actor_id: Option<uuid::Uuid>`
- `payload: serde_json::Value`
- `note: Option<String>`
- `created_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `CommissionFileRow`**
- `id: uuid::Uuid`
- `commission_id: uuid::Uuid`
- `uploaded_by: uuid::Uuid`
- `created_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `CommissionInvitationRow`**
- `id: uuid::Uuid`
- `commission_id: uuid::Uuid`
- `seat_id: uuid::Uuid`
- `invited_user: uuid::Uuid`
- `inviter: uuid::Uuid`
- `state: String`
- `created_at: chrono::DateTime<chrono::Utc>`
- `updated_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `CommissionRow`**
- `id: uuid::Uuid`
- `title: String`
- `owner_id: uuid::Uuid`
- `lifecycle: String`
- `visibility: String`
- `deadline: Option<chrono::DateTime<chrono::Utc>>`
- `maturity: Option<String>`
- `graphic: Option<bool>`
- `direction_status: Option<String>`
- `deadline_status: Option<String>`
- `linked_channel: Option<String>`
- `archived_at: Option<chrono::DateTime<chrono::Utc>>`
- `created_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `CurrentPlacementRow`**
- `seq: i64`
- `account_id: uuid::Uuid`
- `placed_by: uuid::Uuid`
- `placed_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `FindRow`**
- `title: String`
- `owner_id: uuid::Uuid`
- `lifecycle: String`
- `visibility: String`
- `deadline: Option<chrono::DateTime<chrono::Utc>>`
- `maturity: Option<String>`
- `graphic: Option<bool>`
- `direction_status: Option<String>`
- `deadline_status: Option<String>`
- `linked_channel: Option<String>`
- `archived_at: Option<chrono::DateTime<chrono::Utc>>`
- `created_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `LapsedDeadlinesRow`**
- `id: uuid::Uuid`
- `deadline: chrono::DateTime<chrono::Utc>`
- `deadline_status: Option<String>`

**pg · commission · `LoadTreeRow`**
- `id: uuid::Uuid`
- `parent: Option<uuid::Uuid>`
- `type_tag: String`
- `mode: Option<String>`
- `position: i32`
- `created_by: uuid::Uuid`
- `created_at: chrono::DateTime<chrono::Utc>`
- `payload: serde_json::Value`

**pg · commission · `PlacementLogRow`**
- `seq: i64`
- `account_id: uuid::Uuid`
- `placed_by: uuid::Uuid`
- `placed_at: chrono::DateTime<chrono::Utc>`

**pg · commission · `RequireSurfaceParentRow`**
- `type_tag: String`
- `mode: Option<String>`

**pg · commission · `SeatsRow`**
- `id: uuid::Uuid`
- `kind: String`
- `prompt: Option<String>`
- `link: Option<String>`
- `occupant: Option<uuid::Uuid>`

**pg · file · `GetRow`**
- `filename: String`
- `content_type: String`
- `byte_size: i64`
- `bytes: Vec<u8>`

**pg · plc · `LatestOpRow`**
- `cid: String`
- `op_type: String`
- `prev: Option<String>`
- `operation: serde_json::Value`

**pg · profile · `GetRow`**
- `did: String`
- `handle: String`
- `display_name: Option<String>`
- `avatar_url: Option<String>`

**pg · user · `FindByDidRow`**
- `id: uuid::Uuid`
- `created_at: chrono::DateTime<chrono::Utc>`

**pg · user · `FindRow`**
- `id: uuid::Uuid`
- `did: Option<String>`
- `created_at: chrono::DateTime<chrono::Utc>`

**pg · user · `ProvisionRow`**
- `id: uuid::Uuid`
- `created_at: chrono::DateTime<chrono::Utc>`

## Full inventory — 97 query functions

| adapter | module | fn | returns |
| --- | --- | --- | --- |
| pg | account | accept_invitation_flip | `u64` |
| pg | account | accept_invitation_seat | `Option<AcceptInvitationSeatRow>` |
| pg | account | change_handle_audit | `u64` |
| pg | account | change_handle_repoint | `u64` |
| pg | account | count_handle_changes_since | `i64` |
| pg | account | create_account | `u64` |
| pg | account | create_invitation | `u64` |
| pg | account | create_owner_membership | `u64` |
| pg | account | departure_delete_membership | `u64` |
| pg | account | departure_membership | `Option<Option<uuid::Uuid>>` |
| pg | account | departure_rehome_children | `u64` |
| pg | account | departure_revoke_invitations | `u64` |
| pg | account | find | `Option<FindRow>` |
| pg | account | find_did_by_handle | `Option<String>` |
| pg | account | find_invitation | `Option<AccountInvitationsRow>` |
| pg | account | find_pending_invitation | `Option<AccountInvitationsRow>` |
| pg | account | grant_role | `u64` |
| pg | account | handle_reserved_for_other | `bool` |
| pg | account | hard_delete_account | `u64` |
| pg | account | hard_delete_invitations | `u64` |
| pg | account | hard_delete_memberships | `u64` |
| pg | account | list_for_user | `Vec<ListForUserRow>` |
| pg | account | revoke_invitation | `u64` |
| pg | account | role_of | `Option<String>` |
| pg | account | soft_delete | `u64` |
| pg | account | transfer_demote_owner | `u64` |
| pg | account | transfer_promote_heir | `u64` |
| pg | actor_identity | cache_handle | `u64` |
| pg | actor_identity | create | `u64` |
| pg | actor_identity | find | `Option<ActorIdentityRow>` |
| pg | actor_identity | find_by_did | `Option<ActorIdentityRow>` |
| pg | actor_identity | intern | `ActorIdentityRow` |
| pg | changelog | append | `u64` |
| pg | changelog | entries | `Vec<EntriesRow>` |
| pg | commission | add_component | `u64` |
| pg | commission | add_file | `u64` |
| pg | commission | add_participant | `u64` |
| pg | commission | add_surface | `u64` |
| pg | commission | create_commission | `u64` |
| pg | commission | create_root_surface | `u64` |
| pg | commission | create_seat_invitation | `u64` |
| pg | commission | current_placement | `Option<CurrentPlacementRow>` |
| pg | commission | declare_seat_node | `u64` |
| pg | commission | declare_seat_satellite | `u64` |
| pg | commission | declare_slot_node | `u64` |
| pg | commission | declare_slot_satellite | `u64` |
| pg | commission | delete | `u64` |
| pg | commission | find | `Option<FindRow>` |
| pg | commission | find_file | `Option<CommissionFileRow>` |
| pg | commission | find_pending_seat_invitation | `Option<CommissionInvitationRow>` |
| pg | commission | grant_view | `u64` |
| pg | commission | is_participant | `bool` |
| pg | commission | lapsed_deadlines | `Vec<LapsedDeadlinesRow>` |
| pg | commission | list_owned_by | `Vec<CommissionRow>` |
| pg | commission | load_tree | `Vec<LoadTreeRow>` |
| pg | commission | place_append | `i64` |
| pg | commission | place_repoint_current | `u64` |
| pg | commission | placement_log | `Vec<PlacementLogRow>` |
| pg | commission | remove_node_delete | `u64` |
| pg | commission | remove_node_gate | `Option<Option<uuid::Uuid>>` |
| pg | commission | remove_node_renumber | `u64` |
| pg | commission | require_surface_parent | `Option<RequireSurfaceParentRow>` |
| pg | commission | revoke_seat_invitation | `u64` |
| pg | commission | revoke_view | `u64` |
| pg | commission | seats | `Vec<SeatsRow>` |
| pg | commission | set_archived | `u64` |
| pg | commission | set_deadline | `u64` |
| pg | commission | set_deadline_status | `u64` |
| pg | commission | set_direction_status | `u64` |
| pg | commission | set_linked_channel | `u64` |
| pg | commission | set_maturity | `u64` |
| pg | commission | view_grant | `Option<String>` |
| pg | file | delete | `u64` |
| pg | file | get | `Option<GetRow>` |
| pg | file | put | `u64` |
| pg | health | is_reachable | `i32` |
| pg | key_store | get | `Option<Vec<u8>>` |
| pg | key_store | put | `u64` |
| pg | plc | append | `u64` |
| pg | plc | latest_cid | `Option<String>` |
| pg | plc | latest_op | `Option<LatestOpRow>` |
| pg | profile | get | `Option<GetRow>` |
| pg | profile | put | `u64` |
| pg | session | create | `u64` |
| pg | session | delete | `u64` |
| pg | session | delete_expired | `u64` |
| pg | session | load | `Option<Vec<u8>>` |
| pg | session | save | `u64` |
| pg | user | find | `Option<FindRow>` |
| pg | user | find_by_did | `Option<FindByDidRow>` |
| pg | user | provision | `ProvisionRow` |
| atproto | auth_store | delete_auth_req_info | `u64` |
| atproto | auth_store | delete_session | `u64` |
| atproto | auth_store | get_auth_req_info | `Option<Vec<u8>>` |
| atproto | auth_store | get_session | `Option<Vec<u8>>` |
| atproto | auth_store | save_auth_req_info | `u64` |
| atproto | auth_store | upsert_session | `u64` |