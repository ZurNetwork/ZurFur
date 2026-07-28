
---

# RESOLVED — Engineer rulings, 2026-07-25 (morning session)

1. **Field naming: (a) — accept lowerCamelCase.** Canonical ProtoJSON; six field
   reads move (`displayName`, `avatarUrl`, `directionStatus`, `deadlineStatus`,
   `linkedChannel`, `createdAt`); serde bridges via `rename_all = "camelCase"`
   until generated types replace the hand-written ones; the frontend re-adheres
   once, in the same PR, caught by svelte-check. Verified before ruling: parsers
   MUST accept both forms (protobuf.dev/programming-guides/json/), so reading is
   safe either way — the choice governs emission only.
2. **Integers: minimum defensible range.** Declare the smallest protobuf type
   whose ceiling the domain can defend for the major's lifetime — widening is
   BREAKING inside a major, so the bet must be defensible forever, not merely
   today. int32 rides as an exact JSON number (always ≤ 2^53); a genuine 64-bit
   field takes the canonical string. Boundary narrowing from storage is checked
   (`TryFrom`), never `as`. Signedness (uint32 for never-negative) pending one
   verification against vendor guidance before it enters contract text.
   - Audit result: the entire query corpus holds FIVE integer fields; none are
     in v1 scope. `changelog.seq` ruled separately (below); `byte_size` is the
     one real future judgment; `position` already int4.
3. **Changelog seq: never crosses the wire.** The global bigserial is a volume
   oracle (any signed-in user can mint an entry and read the platform-wide
   counter). Wire ordering = dense per-projection renumbering, 0..n, computed
   AFTER the visibility filter (filter-then-number — number-then-filter leaks
   withheld entries as gaps). Viewer-relative, so never a shared reference or
   durable bookmark; a future resume-point is an opaque cursor. Lands whenever
   the changelog endpoint is next touched (out of v1 scope; no consumer today).
4. **Absence: canonical, and absence may only mean "not set".** Keys omitted,
   `T | undefined` frontend-side (converges with DD 39944194's no-Option
   ruling; `null` ceases to exist between database and components). Absence
   that would carry positive meaning (system-actor) MUST be an explicit
   discriminant (oneof / kind field) — presence-encoding is forbidden because
   "no actor" and "actor withheld" must never be the same bytes.

**ZMVP-159 is UNPARKED.** Chain resumes: 28 (post drafted content) → 159 → 160 → 161 → 162.
The Engineer is independently annotating `queries/*.sql` with optimization comments;
sweep those up when they surface — they are input, not a gate.
