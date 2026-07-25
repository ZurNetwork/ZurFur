# Overnight report — 2026-07-25

**One ticket landed. One parked on a decision that is yours. Three are downstream of the parked one.**

That is a worse yield than "five PRs", and the reason is a real finding rather than a stall — but
you should judge that for yourself below.

---

## Landed

**[PR #156](https://github.com/ZurNetwork/ZurFur/pull/156) — ZMVP-158, typed response bodies**
→ into `feature/zmvp-25-api-contract`. Copilot requested. CI green at time of writing.

`AccountBody`, `AccountMembershipBody`, `CommissionBody`/`MaturityBody` replace `json!` literals on
the endpoints the contract will describe. **58 binaries, 622 passed, 0 failed. No test edited** —
which is the point: the existing integration tests read JSON keys directly (`owned_row["role"] ==
"owner"`), so their passing *is* the proof the wire didn't move.

**Scope narrowed, deliberately.** The ticket says every response body in `crates/api`; this does the
9 endpoints the contract covers, because you scoped the corpus to those. ~26 `json!` sites remain in
membership, invitations, transfer and the commission tree.

**Hazard found while refactoring:** `PATCH /accounts/{id}/handle` renders the NEW handle while the
loaded `account` still holds the OLD one. A shared `AccountBody::of(&account)` would have echoed the
handle the caller just left. Built explicitly with a comment; worth knowing the shared constructor
invites it.

---

## Parked — and this needs you

**ZMVP-159** stops on a wire-format decision. Full analysis:
`.understand/20260725-zmvp-159-wire-format-fork.md`.

You approved `json_name` overrides to preserve snake_case, on your principle that *the contract
documents the wire that exists*. **Researching ProtoJSON before implementing showed that mechanism is
a one-way door**: `buf`'s `FIELD_SAME_JSON_NAME` treats a changed `json_name` as breaking, so every
field name locks at v1 permanently. The principle is right; the mechanism I proposed to serve it is
the worst of the three available.

The real fork:

| | Wire | Cost |
|---|---|---|
| **(a) accept lowerCamelCase** | `display_name` → `displayName` | one deliberate pre-v1 rename; every client default-configured |
| **(b) `useProtoFieldName` globally** | unchanged forever | every consumer must run a non-default serializer, permanently |
| ~~(c) per-field `json_name`~~ | unchanged | **rejected** — locks all names forever |

Two more that change bytes, both needing your call:
- **int64 → decimal string.** `"seq": "42"`, not `42`. IEEE-754 rationale.
- **`null` never emitted.** But `actor_id: null` is a *documented* signal meaning "system entry"
  (`changelog.rs:19`). Under ProtoJSON the key is absent — a semantic change, not a formatting one.

I did not pick. Wire policy for a public API is the category you reserve, and I had already been
wrong once about the mechanism.

---

## Settled by research, no decision needed

- **Timestamps are already conformant.** chrono's `SecondsFormat::AutoSi` emits exactly 0/3/6/9
  fractional digits, Z-normalized — byte-identical to ProtoJSON. **Option C costs a validating
  newtype, nothing more**, because chrono *accepts* more than `Timestamp` allows (lowercase `t`/`z`,
  leap second `:60`, years outside 0001–9999). Fix at the boundary, not the encoder.
- **ZMVP-28's content is drafted and fully cited** — the enumerated breaking-change list, the
  RFC 9745/8594 encoding split, the notice window, the escape hatch, the tolerant-reader duty. Not
  yet written to the ticket; it should be reviewed alongside the fork above, since the two interact.
- **Serializer settings are contract text.** `useProtoFieldName`, `enumAsInteger`,
  `alwaysEmitImplicit` each change bytes *without changing the proto*, so `buf breaking` can never
  see them. They must be written down and pinned by a test.

---

## A defect in my own ZMVP-157 work

`GET /accounts` and `GET /commissions` return **bare top-level arrays**. Zalando rule #110 is *"MUST
always return JSON objects as top-level data structures"* — specifically so pagination can be added
later without breaking. I wrote and shipped that shape yesterday. Wrapping it (`{ "accounts": [...] }`)
is cheap now and breaking after `/api/v1` mints. `GET /commissions/{id}/changelog` has the same shape
and predates me.

---

## Where sources disagreed (worth your eye, from the research)

- **Adding a response field:** GitHub/Zalando/Microsoft say compatible; **Azure says breaking**. The
  disagreement is about where the obligation sits — server prohibition vs client tolerant-reader
  duty. Taking the permissive side is only legitimate if the tolerant-reader duty is binding and
  tested, which is exactly what `ignoreUnknownFields: true` + its pinning test buys.
- **URL versioning itself:** Zalando #115 says MUST NOT; Google AIP-185 mandates it. We follow
  Google, as you ruled.

---

## Next, in order

1. Your call on the wire-format fork (a) vs (b), plus int64 and null.
2. ZMVP-28's drafted content into the ticket.
3. ZMVP-159 → 160 → 161 → 162, then the epic-level gates and the epic→main PR.
