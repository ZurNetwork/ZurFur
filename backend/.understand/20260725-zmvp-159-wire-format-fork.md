# ZMVP-159 parked — the wire-format fork (2026-07-25)

**Status: BLOCKED on an Engineer decision.** ZMVP-158 is unaffected and proceeding.

## Why it parked

The approved plan was: `json_name` overrides on every field, preserving snake_case, per the
standing rule *"the contract documents the wire that exists; it doesn't impose protobuf idiom
on a shipped surface."*

Researching ProtoJSON (https://protobuf.dev/programming-guides/json/) before implementing showed
that mechanism is a **one-way door**: `buf`'s `FIELD_SAME_JSON_NAME` treats a changed `json_name`
as breaking, so every field name is locked at v1 forever. The research lane rated it the worst of
the three available options. It honours the principle and pays an unreversible price to do it.

## The fork

| | Wire | Cost |
|---|---|---|
| **(a) Accept lowerCamelCase** | `display_name` → `displayName` across the surface | One deliberate pre-v1 rename. Every generated client is default-configured. Frontend + tests updated in the same PR. |
| **(b) `useProtoFieldName` globally** | unchanged, snake_case forever | Legal per spec ("An implementation may provide an option to use proto field name as the JSON name"), but every consumer must configure a non-default serializer, and reversing it later needs a major. |
| ~~(c) per-field `json_name`~~ | unchanged | **Rejected** — locks all names permanently via `FIELD_SAME_JSON_NAME`. |

Given only one client exists and it deploys in lockstep, **(a) is cheapest today and gets more
expensive every week**; (b) is the true "wire wins" answer and costs a permanent配置 obligation on
every future consumer. Not mine to pick.

## Two more that change bytes

1. **int64 → decimal string.** ProtoJSON: *"JSON value will be a decimal string."* `ChangelogEntryBody.seq: i64`
   ships as a number today; it would become `"42"`. Rationale is IEEE-754 loss past 2^53.
2. **`null` is never emitted.** *"Serializers should not emit `null` values."* But `actor_id: null`
   is a **documented signal** meaning "a system entry" (`changelog.rs:19`). Under ProtoJSON the key
   is simply absent, so the client must read absence as "system". That is a semantic change to a
   documented contract, not a formatting nit.

## Settled by the research, no decision needed

- **Timestamps are already conformant.** chrono's serde uses `SecondsFormat::AutoSi` → exactly
  0/3/6/9 fractional digits, Z-normalized; identical to ProtoJSON. Option **C** costs only a
  validating newtype, because chrono *accepts* more than `Timestamp` allows on input (lowercase
  `t`/`z`, leap second `:60`, years outside 0001–9999). Fix at the boundary, not in the encoder.
- **Serializer settings are contract text.** `useProtoFieldName`, `enumAsInteger`, `alwaysEmitImplicit`
  each change bytes *without changing the proto*, so `buf breaking` can never see them. They must be
  written into the contract and pinned by a test.

## A defect in my own ZMVP-157 work

`GET /accounts` and `GET /commissions` return **bare top-level arrays**. Zalando rule #110 is
*"MUST always return JSON objects as top-level data structures"* — precisely so pagination can be
added later without breaking. I wrote those endpoints yesterday and shipped the shape. Wrapping
them (`{ "accounts": [...] }`) is cheap **now** and breaking **after** `/api/v1` mints.
`GET /commissions/{id}/changelog` (`changelog.rs:36`) has the same shape and predates me.
