# The Zurfur API versioning & deprecation contract

**Status: DRAFT for Engineer review — research-complete, every claim cited (ZMVP-28).**
Becomes binding when ZMVP-28 closes. **Hard ordering constraint (DD
[40992770](https://zurnetwork.atlassian.net/wiki/spaces/DESIGN/pages/40992770)): this
document must be DECIDED before the contract's first version is cut** — ZMVP-159 must
not tag `v1` while this is a draft.

Research method, per the Engineer's standing directive (2026-07-25): every factual
claim below was web-fetched from a primary source and carries its URL; nothing is
asserted from memory; items that could not be verified are labelled UNVERIFIED in §8.

## Rulings already made (Engineer, 2026-07-25 · R9/R10 2026-07-27)

| # | Ruling | Consequence here |
| --- | --- | --- |
| R1 | **Field naming: canonical lowerCamelCase** (§7.1 option a). | The `/api/v1` mint renames six snake_case keys; the pre-`/api/v1` surface is treated as pre-GA (§5 exempts it). serde `rename_all = "camelCase"` bridges until generated types land. `useProtoFieldName` and per-field `json_name` are both rejected. |
| R2 | **Integers: minimum defensible range.** Declare the smallest type whose ceiling the domain can defend for the major's lifetime — widening is breaking (§1 item 8), so the bet must hold forever, not merely today. int32 rides as an exact JSON number; a genuine 64-bit field takes the canonical string (§7.2). Boundary narrowing from storage is checked (`TryFrom`), never `as`. Signedness pending one verification (uint vs vendor guidance). |
| R3 | **Sequential storage keys never cross the wire.** The changelog's global bigserial is a volume oracle; wire ordering is dense per-projection renumbering (0..n, computed AFTER the visibility filter). Viewer-relative — never a shared reference or durable bookmark; a future resume point is an opaque cursor. |
| R4 | **Absence: canonical, and absence may only mean "not set"** (§7.4). Keys omitted, `T \| undefined` client-side (converges with DD 39944194's no-Option ruling). Absence that would carry positive meaning MUST be an explicit discriminant (oneof / kind field) — "no actor" and "actor withheld" must never be the same bytes. |
| R5 | **Upload cap: 50 MiB (52,428,800 bytes), Bluesky PDS blob-cap parity** — MVP value, "increase eventually." Bounds §7.2 for `byte_size`: int32 is defensible by construction while the cap stays ≤ 2 GiB; crossing that line flips the field to the canonical int64 string and is a planned break, never an accident. Sources: https://github.com/bluesky-social/atproto/discussions/1740 · https://docs.bsky.app/docs/tutorials/video |
| R6 | **Ids are opaque** (resolves §8 Q2, taking the Microsoft Graph side of the AIP/Graph contradiction). Every id is a stable, unique, opaque string of at most 64 characters. Clients must not parse ids, derive meaning from their structure, or order by them; ordering, timestamps, and any other derived fact are provided as explicit fields or response order. Changes to id length or format within the bound are **not breaking**. Binds consumers only — the backend keeps full internal knowledge of its own formats (`Uuid` parsing, `ORDER BY id`, the embedded v7 timestamp). |
| R7 | **Listing responses are wrapped objects at the `/api/v1` mint** (resolves §8 Q4; ZAL #110). `GET /accounts` → `{ "accounts": [...] }`, `GET /commissions` → `{ "commissions": [...] }` — so pagination can later land additively (`next_cursor` sibling) instead of costing a major. Rationale is pagination-extensibility, not protobuf: `HttpRule.response_body` could have preserved the bare array. The pre-`/api/v1` bare-array shape is pre-GA and exempt (§5). The changelog's bare array is wrapped whenever that endpoint is next touched. |
| R8 | **All v1 vocabularies are `string` fields, never proto enums** (extends the `role` precedent to `lifecycle`, `visibility`, `direction_status`, `deadline_status`, `maturity.rating`; defers §8 Q1 until the first true enum enters the corpus). Rationale, the Engineer's: **the backend enforces vocabularies — the contract just says "this is this."** Enforcement lives in the domain (newtypes, `try_from`, the state machines); a proto enum would move domain law into the schema layer. Consequences: (a) the contract documents each field's known values as an **extensible vocabulary** — clients MUST tolerate unknown strings with a defined fallback (the ZAL #112 pattern, pinned by a §6-style test); (b) vocabulary changes are invisible to `buf breaking` and are therefore a **review obligation** under §1's semantics items — changing a value's meaning is breaking-by-review; (c) wire values stay lowercase, unchanged. |

| R9 | **The `buf skip breaking` label is banned** (resolves §8 Q5; ruled 2026-07-27). The `contract` CI job is a **required status check** on `main` — a PR label cannot skip a required check, so the bypass is structurally unreachable rather than policed. A genuine §5 emergency must edit the workflow in the PR itself, which is loud, reviewed, and diffed. No signed-justification lane exists. |
| R10 | **Notice window: 12 months, bound to per-major telemetry** (resolves §8 Q7; ruled 2026-07-27). ≥ 12 months from `Deprecation` to `Sunset` for any GA path-major (Google Cloud ToS floor); pre-GA surfaces get zero. The commitment is honest only with eyes: Zalando #188 **per-major request-count telemetry is committed alongside it** — an implementation obligation (nothing is instrumented today) that must land before the first GA deprecation is announced. A major retires when the numbers show migration, never calendar-blind. 12 is a floor, not a ceiling. |

**One Engineer question open: Q9** (payload-blob wire representation — opened by the
2026-07-27 Q8 sweep, deferred by the Engineer to the ratification read; blocks the
changelog message's corpus entry, not ratification). Q2/Q4/Q5/Q7 are resolved above
(R6, R7, R9, R10); Q3 by R1–R4; Q6 verified; **Q8 fully swept 2026-07-27** (all six
items resolved — R10 gained a second precedent, §3 gained the CORS clause, §7.9 is
new). **Spikes owed before v1 tags:** Q1 only (protobuf-es unknown-enum behaviour —
deferred by R8 until the first true enum enters the corpus).

---

## 1. The enumerated breaking-change list

Binding for `zurfur.api.v1`. Any change on this list mints `v2`; anything not on it is additive and ships in place. Sources: **GH** = GitHub REST breaking changes (https://docs.github.com/en/rest/about-the-rest-api/breaking-changes) · **AIP** = Google AIP-180 (https://google.aip.dev/180) · **ZAL** = Zalando rule # (https://raw.githubusercontent.com/zalando/restful-api-guidelines/main/chapters/compatibility.adoc) · **AZ** = Azure breaking-changes guidelines (https://github.com/Azure/azure-rest-api-specs/blob/main/documentation/Breaking%20changes%20guidelines.md) · **MSG** = Microsoft Graph (https://learn.microsoft.com/en-us/graph/versioning-and-support) · **BUF** = rule name at https://buf.build/docs/breaking/rules/ · **PB** = https://protobuf.dev/programming-guides/proto3/

**Removal / renaming**
1. **Removing an endpoint (RPC / HTTP binding).** GH ("Removing an entire operation"); AIP: "Existing components … **must not** be removed from existing APIs in the same major version"; AZ #5; MSG. BUF: `RPC_SAME_REQUEST_TYPE`/`RPC_SAME_RESPONSE_TYPE` catch signature swaps, **not** deletion of the method itself — see §2.
2. **Removing a request field.** GH; AIP; AZ; MSG. BUF `FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED` + `FIELD_NO_DELETE_UNLESS_NAME_RESERVED`.
3. **Removing a response field.** GH; ZAL #107 "Don't remove fields"; AZ #1/#2; MSG ("Removal, rename, or change to the type of a declared property").
4. **Renaming any field, at a fixed number.** AIP: "Renaming a component is semantically equivalent to 'remove and add'." On a JSON wire the *name is the identity* — BUF `FIELD_SAME_NAME`: "This affects generated source code, but also affects JSON compatibility because JSON uses field names."
5. **Renaming an enum value.** BUF `ENUM_VALUE_SAME_NAME`: "Doing so results in potential JSON incompatibilities and broken source code." ProtoJSON emits the enum *name* by default (https://protobuf.dev/programming-guides/json/).
6. **Changing the proto package** (`zurfur.api.v1` → anything). BUF `FILE_SAME_PACKAGE`. This is the rule that mechanically enforces the path-major binding.
7. **Changing an explicit `json_name`.** BUF `FIELD_SAME_JSON_NAME`: "which breaks JSON compatibility."

**Type & shape**
8. **Changing a field's type**, even wire-compatibly. AIP: fields "**must not** have their type changed, **even if the new type is wire-compatible**." BUF `FIELD_WIRE_JSON_COMPATIBLE_TYPE` is *laxer* than AIP (it permits int32↔uint32, int64↔uint64, enum-subset widening) — **we adopt AIP's stricter rule; the delta is a review obligation, not a CI one.**
9. **Changing cardinality** (singular↔repeated, repeated↔map). BUF `FIELD_WIRE_JSON_COMPATIBLE_CARDINALITY`. Note WIRE would permit `repeated`↔`map`; WIRE_JSON does not (https://buf.build/docs/breaking/rules/).
10. **Moving a field into or out of a `oneof`.** AIP; BUF `FIELD_SAME_ONEOF`.
11. **Returning a non-object at the top level** where an object was returned, or shipping a new endpoint that returns a bare array. ZAL #110 "MUST always return JSON objects as top-level data structures" — so pagination can be added later "without breaking backwards compatibility." ⚠️ `GET /commissions/{id}/changelog` currently returns a bare array (`backend/crates/api/src/routes/commissions/changelog.rs:36`) — see §8 Q4.

**Requiredness**
12. **Adding a new required request field.** GH; AIP: "**must not** add a new required field to an existing request message"; AZ #11; MSG; ZAL #107.
13. **Making an optional request field required.** GH; AZ #8; ZAL #107.
14. **Making a required response field optional.** AZ Guidelines: "a breaking change to remove a required field or make an optional field required or vice versa"; ZAL #107 (output-only). BUF `MESSAGE_SAME_REQUIRED_FIELDS` only sees proto2 `required` — for us this is 100% a review obligation (§2).

**Validation & constraints**
15. **Adding or tightening a validation rule on an existing field.** GH ("Adding a new validation rule to an existing parameter"); ZAL #107 "**Never** change the validation logic to be more restrictive."
16. **Tightening an id / resource-name format.** AIP (formats becoming "more stringent" break existing requests); AZ #12.
17. **Widening a string field's max length.** AIP: "APIs **should** avoid increasing the upper bound for the size or limit … of `string` fields." (SHOULD, not MUST — we take it as a review flag, not a hard bar.)

**Semantics & behaviour** — none of these are visible to `buf`.
18. **Changing the meaning of an existing field.** ZAL #107 "**Never** change the semantic of fields"; AIP.
19. **Changing a default value, or how a default-valued field serializes.** AIP: "Changing the default value is considered breaking and **must not** be done" / "APIs **must not** change the way a field with a default value is serialized."
20. **Changing the format or algorithm that constructs an existing field's value.** AIP.
21. **Any other user-visible behaviour change.** AZ #6; AZ: "anything that would violate the Principle of Least Astonishment."
22. **Changing the RFC 9457 problem *envelope*** (`type`/`title`/`status`/`detail`/`instance`, or the `urn:zurfur:error:*` scheme). AZ #7 "Error contract modifications". **Adding a new `urn:zurfur:error:*` member is additive** — MSG lists "Changes to error codes" as backward compatible. This split is ours to state and is stated deliberately (DD 23592962).
23. **Changing authentication or authorization requirements on an existing endpoint.** GH only — **no other source in the corpus enumerates it.** Adopted anyway: it is the one most likely to bite an OAuth/PDS-backed surface.

### Where the sources disagreed

- **Adding a response field.** GH "additive"; ZAL #107 "compatible"; MSG "compatible"; **AZ #10 BREAKING** — "Adding fields breaks GET-PUT pipelines when clients ignore unknown properties." The disagreement is about *where the obligation sits*: server prohibition (Azure) vs client tolerant-reader duty (ZAL #108, which requires clients "not eliminate them from payload if needed for subsequent `PUT` requests"). **We take the Zalando/GitHub side — which is only legitimate because §6 makes the tolerant-reader duty binding and tested.** Without §6, Azure is right and we cannot add response fields.
- **Adding an enum value.** GH additive; MSG compatible; AIP permitted but "should document that new values are possible"; **ZAL #107 BREAKING** ("`enum` ranges **cannot** be extended — clients may not be prepared to handle it"), escapable only via #112's `[Extensible enum]` marker; **AZ #4 BREAKING** ("clients fail deserializing unknown enum values"). proto3 enums are open by construction, which structurally puts us on the GH/AIP side — **but only if the generated TS client does not narrow them to a closed union.** Open question, §8 Q1.
- **Removing an enum value.** GH/AIP/BUF breaking; **ZAL #107 says compatible** for output-only enums ("`enum` ranges can be reduced"), and reducible for input enums "only if the server continues to accept and handle old range values." Zalando is the outlier; we follow GH/AIP/BUF (item 3 above applies).
- **Id format / opacity.** **AIP:** "A resource **must not** change its name" — and this "affects major versions as well," i.e. not fixable by minting v2. **MSG explicitly says the opposite:** "Changes to the length or format of opaque strings, such as resource IDs" are backward compatible. Direct contradiction; resolution depends on whether we publish ids as opaque. §8 Q2.
- **URL versioning itself.** For the record, and not to re-litigate: **ZAL #115 is "MUST not use URL versioning"**, mandating media-type versioning (`application/x.<type>+json;version=<n>`, #114). **AIP-185 mandates the opposite** — major version "included as the first part of the URI path" (https://google.aip.dev/185). We follow Google. If this contract cites Zalando for the tolerant reader (it does), it must also say plainly that we consciously do **not** adopt #114/#115. Related and non-conflicting: ZAL #116 semver governs the *spec document*; AIP-185 "Google APIs **must not** expose minor or patch version numbers" governs the *wire*. Both hold: semver the proto corpus release, expose major only.

---

## 2. What buf enforces vs what we must build

Level: `WIRE_JSON`. Buf's own words: "Detects changes that break wire (binary) or JSON encoding. Because JSON is common across many transports, **this is the recommended minimum level**" (https://buf.build/docs/breaking/rules/). Nesting: "Passing a stricter category implies passing every looser one: schemas that pass `FILE` also pass `PACKAGE`, `WIRE_JSON`, and `WIRE`" (https://buf.build/docs/breaking/) — so FILE ⊃ PACKAGE ⊃ WIRE_JSON ⊃ WIRE, and `FILE` is the default when `breaking.use` is omitted. **We must set `use: [WIRE_JSON]` explicitly; leaving it unset gives us FILE, which is stricter and would block harmless file/package reorganisation.**

The WIRE_JSON-over-WIRE delta is exactly the JSON-name layer: WIRE alone would let a PR rename `display_name` → `displayName` at field 3 and pass CI.

| `buf breaking --against` at WIRE_JSON enforces | We must build |
|---|---|
| **Field identity by name.** `FIELD_SAME_NAME`, `FIELD_SAME_JSON_NAME`, `FIELD_NO_DELETE_UNLESS_NAME_RESERVED`, `FIELD_NO_DELETE_UNLESS_NUMBER_RESERVED` | **Item 15–17: validation rules.** Business constraints (handle regex, length caps, punycode ban) live in Rust, not in the proto. Review gate. |
| **Enum identity by name.** `ENUM_VALUE_SAME_NAME`, `ENUM_VALUE_NO_DELETE_UNLESS_NAME_RESERVED`, `ENUM_VALUE_NO_DELETE_UNLESS_NUMBER_RESERVED`, `ENUM_SAME_JSON_FORMAT`, `ENUM_SAME_TYPE` | **Item 23: auth/authz changes.** Invisible to the schema entirely. |
| **Type & cardinality.** `FIELD_WIRE_JSON_COMPATIBLE_TYPE`, `FIELD_WIRE_JSON_COMPATIBLE_CARDINALITY`, `FIELD_SAME_ONEOF`, `FIELD_SAME_DEFAULT` | **Items 18–21: semantics, defaults, value-construction format, behaviour.** No schema diff exists for "this field now means something else." |
| **Package pinning.** `FILE_SAME_PACKAGE` — this is what mechanically welds `package zurfur.api.v1` to `/api/v1/…` | **Items 12–14: requiredness.** `MESSAGE_SAME_REQUIRED_FIELDS` only sees proto2 `required`; our requiredness is application validation. Fully manual. |
| **RPC signatures.** `RPC_SAME_REQUEST_TYPE`, `RPC_SAME_RESPONSE_TYPE`, `RPC_SAME_CLIENT_STREAMING`, `RPC_SAME_SERVER_STREAMING`, `RPC_SAME_IDEMPOTENCY_LEVEL` | **Item 22: error contract** — unless we make the RFC 9457 problem shape *itself a proto message*, in which case buf covers the envelope for free. **Recommended.** |
| **Reservation ledger.** `RESERVED_ENUM_NO_DELETE`, `RESERVED_MESSAGE_NO_DELETE`, `MESSAGE_SAME_JSON_FORMAT` | **The serializer option set** — `useProtoFieldName`, `enumAsInteger`, `alwaysEmitImplicit` change emitted bytes with an unchanged schema. buf compares schemas and will never see it. Must be frozen in the contract text and pinned by a golden test (§7). |
| — | **Everything runtime**: `Deprecation`/`Sunset` header emission, the notice window, 410 after sunset, the route lifecycle. buf has none of this (§5 of the buf lane: `buf source edit deprecate` only stamps `option deprecated = true`, with no dates or schedule — https://buf.build/docs/reference/cli/buf/source/edit/deprecate/). |

**Does `reserved` cover names? Yes — and that settles the retired-name register question.**

protobuf.dev proto3 guide (https://protobuf.dev/programming-guides/proto3/), verbatim: "Reusing an old field name later is generally safe, **except when using TextProto or JSON encodings where the field name is serialized.** To avoid this risk, you can add the deleted field name to the `reserved` list." And on deletion: "you **must** reserve the deleted field number … **You should also reserve the field name to allow JSON and TextFormat encodings of your message to continue to parse.**" Names and numbers go in *separate* statements — "you can't mix field names and field numbers in the same `reserved` statement."

Consequence, with a sharp edge:
- **Compile time: the `.proto` file IS the retired-name register.** "The protocol buffer compiler will complain if any future users try to use these identifiers." Combined with `FIELD_NO_DELETE_UNLESS_NAME_RESERVED`, buf forces the reservation to be written and protoc forbids re-minting the name with new semantics. **We do not build a parallel register.** The policy rule is one line: *every field or enum-value deletion reserves both the number and the name.*
- **Runtime: it does nothing.** "Runtime JSON parsing is not affected by reserved names." A client still sending a retired key gets it treated as an unknown field, subject to our unknown-field policy — not a targeted error. If we want "you are sending a field we retired in v1.4, here is why," that is ours to build.

**The bypass.** `bufbuild/buf-action@v1` supports a `buf skip breaking` PR label that skips the check ("Runs on pull requests, skipped when the `buf skip breaking` label is present" — https://github.com/bufbuild/buf-action). That is a one-click bypass of this entire contract. → **RESOLVED 2026-07-27: banned — see R9 in the header.** The `contract` job is a required status check on `main`; the label is inert against it.

**Two gates, not one.** The action's default `breaking_against` is "Base of the PR (or the commit before a push)" — that catches intra-PR regressions but is *not* the release contract. We need both: (a) against PR base, (b) against the last released tag, `buf breaking --against '.git#tag=api/v1.4.0,subdir=proto'` (git input fragment syntax: https://buf.build/docs/reference/inputs/; `subdir=` is used in every git example in https://buf.build/docs/breaking/quickstart/).

**Lane contradiction, surfaced not smoothed:** the vendor lane transcribed a 20-rule WIRE_JSON set omitting `FIELD_SAME_NAME`, `ENUM_SAME_TYPE`, `FIELD_SAME_DEFAULT`, and `ENUM_VALUE_NO_DELETE_UNLESS_NAME_RESERVED`; the buf lane transcribed 23 rules including them. The 23-rule list is used above because that lane quoted per-rule descriptions for the disputed entries. **RESOLVED 2026-07-25 — verified against `buf config ls-breaking-rules --version v2` on buf 1.72.0.** The WIRE_JSON category holds **22 rules**. The disputed entries: `FIELD_SAME_NAME` ✅ **in** WIRE_JSON ("Checks that fields have the same names in a given message") — field renames are CI-enforced, item 4 stays in the left column; `FIELD_SAME_DEFAULT` ✅ in; `ENUM_VALUE_NO_DELETE_UNLESS_NAME_RESERVED` ✅ in; `ENUM_SAME_TYPE` ❌ **not** in WIRE_JSON (FILE/PACKAGE only) — the 23-rule lane was right on three, wrong on one. Full transcript in the repo at `contract/` CI setup; the buf version is pinned in `.github/workflows/ci.yml`.

---

## 3. The deprecation triple

Three signals, emitted together, on every response from a deprecated surface.

**(a) `Deprecation` — RFC 9745** (https://www.rfc-editor.org/rfc/rfc9745.txt), *Standards Track*, March 2025, no errata as of 2026-07-25 (https://errata.rfc-editor.org/search/?rfc_number=9745).

> "Deprecation is an Item Structured Header Field; its value MUST be a Date as per Section 3.3.7 of [RFC9651]." (§2.1)

Encoding: `@` + integer Unix seconds. `Deprecation: @1688169599`. The normative reference is **RFC 9651** (https://www.rfc-editor.org/rfc/rfc9651.txt), *not* RFC 8941 — 9651 obsoletes 8941 and is what added the Date type: "their serialization … is similar to that of Integers, distinguished from them with a leading `@`." May be a past or future date (§2.1). Presence changes nothing about behaviour (§5: "The act of deprecation does not change any behavior of the resource").

**(b) `Sunset` — RFC 8594** (https://www.rfc-editor.org/rfc/rfc8594.txt), *Informational*, May 2019, no errata as of 2026-07-25.

> "The Sunset value is an HTTP-date timestamp … and SHOULD be a timestamp in the future. Sunset = HTTP-date. For example: `Sunset: Sat, 31 Dec 2018 23:59:59 GMT`" (§3)

⚠️ RFC 8594 cites *RFC 7231 §7.1.1.1*, which is **obsoleted**. Cite **RFC 9110 §5.6.7** (https://www.rfc-editor.org/rfc/rfc9110.txt): "When a sender generates a field that contains one or more timestamps defined as HTTP-date, the sender **MUST** generate those timestamps in the IMF-fixdate format," with `GMT = %s"GMT"` — a **case-sensitive literal `GMT`**.

**(c) `Link; rel="deprecation"` — RFC 9745 §6.2.**

> Relation Name: `deprecation`. Description: "Refers to documentation (intended for human consumption) about the deprecation of the link's context."

`Link: <https://docs.zurfur.app/api/deprecation>; rel="deprecation"; type="text/html"`. §3.1 explicitly permits this link **without** a `Deprecation` header — publish the policy before anything is deprecated. `rel="sunset"` is a *separate* registration, by **RFC 8594 §7.2** ("Identifies a resource that provides information about the context's retirement policy"), **not** by RFC 9745. RFC 9745 registers only `deprecation`.

For "here is the successor major," the relation is **`successor-version`, registered by RFC 5829** (https://www.rfc-editor.org/rfc/rfc5829.txt, Informational, April 2010): "When included on a versioned resource, this link points to a resource containing the successor version in the version history." RFC 5829 also registers `latest-version`, `predecessor-version`, `version-history`, `working-copy`, `working-copy-of`.

### The encoding trap — this is the known implementation defect

**The two headers use different date formats.** RFC 9745 §4 says so itself:

> "The timestamp given in the Sunset HTTP header field **MUST NOT** be earlier than the one given in the Deprecation header field. … Please note that for historical reasons the Sunset HTTP header field uses a different data format for date.
> `Deprecation: @1688169599`
> `Sunset: Sun, 30 Jun 2024 23:59:59 UTC`"

🔴 **Do not copy that example. It is wrong.** RFC 9110 §5.6.7 fixes the zone token as `GMT = %s"GMT"`; an IMF-fixdate cannot end in `UTC`. RFC 8594's own examples correctly use `GMT`. No erratum has been filed against RFC 9745 (checked 2026-07-25). **Zurfur emits `GMT`.** If a reviewer cites the RFC's example against us, this paragraph is the answer.

**Implementation rule:** one helper takes a single `DateTimeUtc` pair and formats each side, so the encodings cannot drift and the ordering invariant cannot be violated per-handler. The `MUST NOT be earlier` binds the *server*; the RFC's only remedy for violation is a SHOULD on the client developer to "consult the resource developer" — **nothing in the protocol enforces it, so it must be a unit-test assertion** (§4).

### Scope — this bites path-major versioning directly

RFC 9745 §2.2: "The Deprecation header field applies to the resource identified with the response it occurred within … Resources are free to define such an increased scope, and usually this scope will be documented by the resource … it is important to take into account that such increased scoping is invisible for consumers who are unaware of the increased scoping rules."

We want *"a `Deprecation` header on any `/api/v1/*` response means the whole `v1` major is deprecated."* That increased scope is legal, **but only if this document says so in as many words.** Drafted clause: *"Zurfur scopes `Deprecation` and `Sunset` to the entire path-major. A `Deprecation` header on any `/api/{major}/*` response announces the deprecation of that major in full, not of the responding resource alone. Per-endpoint deprecation within a live major is signalled only through `option deprecated = true` in the corpus and the changelog — never through these headers."*

### CORS — the triple must be exposed to be seen (RULED 2026-07-27: §3 clause + §4 assertion)

Neither `Deprecation` nor `Sunset` is a CORS-safelisted response header — the Fetch spec's safelist is `Cache-Control`, `Content-Language`, `Content-Length`, `Content-Type`, `Expires`, `Last-Modified`, `Pragma`, and neither name appears anywhere in the spec (https://fetch.spec.whatwg.org/). A cross-origin browser client therefore **cannot read the deprecation triple at all** unless the server lists the headers in `Access-Control-Expose-Headers` (MDN: "Only the CORS-safelisted response headers are exposed by default."). This is not hypothetical: every `api.github.com` response carries `access-control-expose-headers: …, Deprecation, Sunset, Warning` (verified on the wire 2026-07-27) — GitHub, this document's own §4 precedent, closes exactly this gap.

**Clause:** any response that carries the deprecation triple MUST also carry `Access-Control-Expose-Headers: Deprecation, Sunset, Link` (emitted by the same middleware that stamps the triple — one derived path, per §4 assertion 4). The first-party BFF is same-origin and unaffected; the public bearer-token surface (`/plugin/v1`) is the textbook cross-origin consumer and the reason this clause exists. Enforced by §4 assertion 7.

### What clients are obliged to do: nothing, per the RFCs

Neither RFC prescribes automated client behaviour. RFC 9745 §7: "The Deprecation header field **should be treated as a hint** … Developers of client applications consuming the resource SHOULD always check the referred resource's documentation." RFC 8594 §3: "Clients SHOULD treat Sunset timestamps as hints … The Sunset header field does not expose any information about which of those behaviors can be expected." RFC 8594 §8: "What an appropriate reaction is … is outside the scope of this specification."

**Therefore: anything Zurfur wants the SvelteKit client to *do* — log, banner, fail CI — is Zurfur-invented policy layered on top and must be written as such, never attributed to the RFCs.**

Two further RFC 8594 notes worth carrying: §4 — "the Sunset header field and HTTP caching should be seen as complementary and not as overlapping in scope"; §8 — sunset information can leak facts ("a resource representing a registration can leak information about the expiration date"), which touches the private↔public boundary if we ever sunset a per-resource surface rather than a major.

---

## 4. The notice window

**RULED 2026-07-27 (R10): 12 months minimum from the `Deprecation` timestamp to the `Sunset` timestamp, for any GA path-major, bound to Zalando #188 per-major usage telemetry. Pre-GA surfaces get zero.**

Precedents, all verified:

| Vendor | Committed | Source |
|---|---|---|
| **Google Cloud ToS §1.4(e)** | **≥ 12 months** | "Google will notify Customer at least 12 months before: (i) discontinuing any Service … or (ii) significantly modifying a Customer-facing Google API in a backwards-incompatible manner." (https://cloud.google.com/terms) |
| **GitHub REST** | **≥ 24 months** | "When a new REST API version is released, the previous API version will be supported for at least 24 more months following the release of the new API version." Mechanism: `Deprecation` header, then `Sunset` (RFC 8594), then `410 Gone`. (https://docs.github.com/en/rest/about-the-rest-api/api-versions) |
| **Microsoft Graph (API)** | **≥ 24 months** | "Microsoft declares a version as deprecated at least 24 months in advance of retiring it." (https://learn.microsoft.com/en-us/graph/versioning-and-support) |
| **Microsoft Graph (SDKs)** | 12 months, security fixes only | same page |
| **Microsoft Modern Lifecycle Policy** | **≥ 12 months** | "Microsoft will provide a minimum of 12 months' notification prior to ending support if no successor product or service is offered—excluding free services or preview releases." Plus 30 days for required-action changes; up to three years for select Azure services. (https://learn.microsoft.com/en-us/lifecycle/policies/modern) |
| **Preview surfaces** | **zero — with one narrowing** | Graph: beta breaking changes "without notice"; Azure: "Between preview versions, a service can break any of the newly added preview features"; Google Cloud ToS: policy "does not apply to … pre-general availability Services, offerings, or functionality." **Corrected 2026-07-27:** Google's *beta* channel is not zero — AIP-185 recommends 180 days for beta turndown and AIP-181 "a good rule of thumb is 90 days"; only *alpha* "may be removed without notice." Zurfur's pre-GA surface is alpha-shaped, so §5's zero stands. |
| **Zalando** | **no number, by design** | #186 requires the owner to define "a reasonable time span" and **all external partners to consent before using the API**; #185 requires client consent on the sunset date; #189 gives the header format; #188 obliges the owner to **monitor usage** of the sunsetting API "to observe migration progress and avoid uncontrolled breaking effects." (https://raw.githubusercontent.com/zalando/restful-api-guidelines/main/chapters/deprecation.adoc) |
| **Google AIP / Azure** | **no GA number, verified** | AIP-180 states no number; AIP-185/181's numbers are pre-GA-only (180/90 days); for stable components AIP explicitly declines one. Azure's REST breaking-changes guidelines state no number. `aka.ms/AzBreakingChangesPolicy` is **not a public document** — it 301s to an internal Microsoft SharePoint file returning 401; never cite it as a source. (Q8 sweep 2026-07-27) |

**Why 12 and not 24.** 24 months is what GitHub and Graph commit to *because they cannot enumerate their consumers.* Zurfur can — the first-party SvelteKit app plus a known plugin set. 12 is a real, published, enterprise-grade floor (Google Cloud ToS; independently corroborated by Microsoft's Modern Lifecycle Policy — Q8 sweep 2026-07-27) that we can actually honour, and it is a floor, not a ceiling: nothing stops us serving a major longer. Committing to 24 pre-launch is a promise we would be tempted to break, which is worse than the shorter number. **Adopt Zalando #188 alongside it — per-major request-count telemetry — because a shorter window is only defensible if we can see who is still on the old major.** Engineer's call; the defensible band is 12–24, and pre-GA is zero in every published precedent.

### Enforcement — a CI flag, not a promise

A promise in prose decays. This is enforced by a checked-in manifest plus tests.

`contract/lifecycle.toml` (tracked, one entry per path-major):
```toml
[major.v1]
status       = "live"          # live | deprecated | sunset
deprecated_at = "..."          # RFC 3339, absent while live
sunset_at     = "..."          # RFC 3339, absent while live
successor     = "/api/v2"      # → Link rel="successor-version"
```

CI assertions, each a test that fails the build:
1. **`sunset_at >= deprecated_at`** — RFC 9745 §4's MUST NOT, which the protocol itself does not enforce.
2. **`sunset_at - deprecated_at >= WINDOW`** where `WINDOW = 12 months`, for any major whose status was ever `live` at GA. This is the notice window as a compiler error.
3. **Routes follow the manifest, not a human.** A major with `status = "sunset"` and `sunset_at` in the past MUST have no routes mounted; a major with `status != "sunset"` MUST have its routes mounted. Removal is driven by the file, so "we forgot to keep v1 alive" and "we forgot to turn v1 off" are both build failures.
4. **Header emission is derived, not hand-written.** A middleware reads the manifest and stamps `Deprecation` / `Sunset` / `Link` on every response from a deprecated major. A test asserts the exact bytes: `@`-prefixed integer for `Deprecation`, IMF-fixdate ending in the literal `GMT` for `Sunset`.
5. **`buf breaking --against` the last released tag**, in addition to the action's default PR-base comparison.
6. **`410 Gone` after sunset** — GitHub's published behaviour for a retired version; a test pins the status code.
7. **The triple is CORS-visible.** Every response carrying `Deprecation`/`Sunset` also carries `Access-Control-Expose-Headers: Deprecation, Sunset, Link` (§3's CORS clause); a test asserts the exposure header's exact value alongside assertion 4's byte checks.

---

## 5. The escape hatch

Precedent, verbatim, GitHub (https://docs.github.com/en/rest/about-the-rest-api/api-versions):

> "Critical security vulnerabilities, data exposure risks, or severe reliability issues may require changes outside the normal release schedule. GitHub may release an unscheduled API version, backport fixes to supported versions, or **in rare cases, introduce a breaking change to an existing version to protect users and platform integrity**." … "GitHub will communicate such changes through release notes, changelogs, and direct communication explaining what changed and why. Where feasible, advance notice will be provided. **Immediate action may be taken without advance notice when required.**"

Corroborating: Google Cloud ToS §1.4(e) — "Nothing in this Section 1.4(e) limits Google's ability to make changes required to comply with applicable law, address a material security risk, or avoid a substantial economic or material technical burden" (https://cloud.google.com/terms). Microsoft Graph — "We make exceptions to this policy for service security or health reliability issues" (https://learn.microsoft.com/en-us/graph/versioning-and-support); note Graph's carve-out is against the *window*, not against the versioning promise. Zalando publishes **no** escape hatch at all: #185 requires that "all clients have given their consent on a sunset date," unconditionally.

The shape worth stealing from GitHub is three-part: enumerated **by cause** (not by convenience); **tiered in remedy**; and carrying a **communication duty that survives even when the notice duty is waived**. That third part is what stops it being a blank cheque.

**Drafted wording:**

> **Emergency changes.** A security vulnerability, a data-exposure risk (including any leak across the private↔public boundary or any DID/handle correlation defect), or a severe reliability defect may require a change outside this contract. In that case Zurfur will, **in this order of preference**: (1) ship an unscheduled additive fix within the live major; (2) release a new path-major early and backport the fix to the live one; (3) **only where (1) and (2) cannot close the exposure, introduce a breaking change to a live major.**
>
> An emergency change **never** waives the communication duty. Zurfur will publish, at the moment the change ships, a changelog entry naming what changed, which majors and endpoints are affected, and why. The notice window in §4 may be shortened or eliminated **only** by an emergency change; it may not be shortened for convenience, schedule, or cost.
>
> **Pre-GA surfaces are exempt from this contract in full.** Any path-major, endpoint, or field marked pre-GA may change or be withdrawn without notice. Pre-GA surfaces are not covered by the notice window and are not supported in production integrations.
>
> **This clause is invoked, not assumed.** Each invocation is recorded in the changelog with its cause (security / data exposure / reliability) and the reason tiers (1) and (2) were insufficient.

---

## 6. Tolerant-reader obligation

**Canonical source: Zalando #108 `MUST prepare clients to accept compatible API extensions`** (https://raw.githubusercontent.com/zalando/restful-api-guidelines/main/chapters/compatibility.adoc) — the only source in the corpus that names the pattern. It cites Postel and Fowler's *TolerantReader*. The four literal client obligations:

1. "Be tolerant with unknown fields in the payload … i.e. ignore new fields but **do not eliminate them from payload if needed for subsequent `PUT` requests**."
2. "Be prepared that extensible enum return parameters may deliver new values; either be agnostic or provide default behavior for unknown values, and do not eliminate them when passed to subsequent `PUT` requests."
3. "Be prepared to handle HTTP status codes not explicitly specified in endpoint definitions."
4. "Follow the redirect when the server returns HTTP status code `301` (Moved Permanently)."

Supporting: **#111** "API clients consuming data **must not** assume that objects are closed for extension"; **#110** top-level objects only; **#112** the `[Extensible enum]` marker. Microsoft Graph states a weaker equivalent: "your client applications should be prepared to receive properties and derived types not previously defined by the Microsoft Graph API service" (https://learn.microsoft.com/en-us/graph/versioning-and-support).

**The asymmetry — do not read #108 as licence for a permissive server.** ZAL #109 `SHOULD design APIs conservatively` says the opposite for the server side: *"Unknown input fields in payload or URL should not be ignored; servers should provide error feedback to clients via an HTTP 400 response code."* Zalando flags this explicitly as "a deviation from Postel's Law," justified to keep client bugs from going undetected. Our codebase already does this in one place — `#[serde(rename_all = "snake_case", deny_unknown_fields)]` at `backend/crates/domain/src/elements/commission/markup.rs:70`.

**The hard dependency.** ProtoJSON's own spec says additive changes are only safe with this caveat: "Unlike the binary wire format, ProtoJSON implementations generally do not propagate unknown fields. This means that adding to schemas is generally [safe only if] the relevant clients set an Ignore Unknown Fields flag" (https://protobuf.dev/programming-guides/json/), and the spec's default is the opposite: "The protobuf JSON parser **should reject unknown fields by default**." **protobuf-es ships that default:**

```ts
// packages/protobuf/src/from-json.ts
function makeReadContext(options?: Partial<JsonReadOptions>): JsonReadContext {
  return { ignoreUnknownFields: false, recursionLimit: 100, ...options, depth: 0 };
}
```
(https://github.com/bufbuild/protobuf-es/blob/main/packages/protobuf/src/from-json.ts — the docs page https://protobufes.com/guides/serialization/ does not state the default; the source does.)

**So "additive-only within a major" is void unless the SvelteKit client parses with `ignoreUnknownFields: true`.** That is not a nice-to-have; it is the precondition for item 1's disposition of the Azure conflict in §1.

**Drafted obligation:**

> **Client obligation (binding on the Zurfur first-party frontend and on every published plugin client):** decode all `/api/{major}/*` responses with unknown fields ignored, treat enum values outside the known set as unknown-but-valid with a defined fallback, tolerate status codes not enumerated in the corpus, and preserve unrecognised fields when echoing a payload back in a subsequent write. **Server obligation:** reject unknown fields in request payloads and query strings with `400` and an RFC 9457 problem — the tolerant-reader duty is a response-side client duty only and is explicitly not extended to the server.

**Tests that pin it (both required — one alone is not enough):**
- **Client side:** a test that decodes a canned v1 response body **with an extra unknown key and an unknown enum string** using *the shipped client's actual construction path* — not a hand-rolled `fromJson` call — and asserts success plus the defined enum fallback. This fails if anyone drops `ignoreUnknownFields: true` from the client factory.
- **Server side:** a test that POSTs a body with one unknown field and asserts `400` with the RFC 9457 problem shape (§1 item 22), pinning ZAL #109.
- **Round-trip:** GET → mutate one field → PUT, asserting a server-known-but-client-unknown field survives, pinning ZAL #108's second half. Without this, Azure #10 stands and §1 item "adding a response field" flips to breaking.

---

## 7. Wire-format facts that constrain implementation

Everything here comes from **ProtoJSON Format** (https://protobuf.dev/programming-guides/json/) unless noted. **Four of these change the currently-shipped bytes if adopted naively.**

**7.1 Field naming — 🔴 CHANGES THE CURRENT WIRE.** "Message field names are mapped to **lowerCamelCase** to be used as JSON object keys. If the `json_name` field option is specified, the specified value will be used as the key instead." Parsers are **required** to accept both: "Protobuf JSON parsers are **required** to accept both the converted lowerCamelCase name and the proto field name."

The API crate ships snake_case today — `ChangelogEntryBody { seq, kind, actor_id, payload, note, created_at }` at `backend/crates/api/src/routes/commissions/changelog.rs:21-29`, no `rename_all`, so serde emits `actor_id` / `created_at`. Only `adapter-atproto` uses `rename_all = "camelCase"` (`public_records.rs:423,446`), and `domain/.../markup.rs:70` explicitly pins `snake_case`. **Adopting the corpus naively renames every response key across the whole surface.** Three options, all Engineer's call: (a) accept the rename as part of minting `/api/v1` and treat the current unversioned surface as pre-GA (§5 exempts it); (b) set the serializer to `useProtoFieldName` and stay snake_case forever — legal ("An implementation may provide an option to use proto field name as the JSON name"), but it makes every generated client non-default and is a decision that cannot be reversed later without a major; (c) set `json_name` per field — **worst option**, because `json_name` is a one-way door that `FIELD_SAME_JSON_NAME` then locks forever. Note `\0` is not allowed inside a `json_name` value.

**7.2 int64 family — 🔴 CHANGES THE CURRENT WIRE.** "`int64, fixed64, uint64` | **string** | `"1", "-10"` | JSON value will be a **decimal string**. Either numbers or strings are accepted." The spec's rationale section ("Strings for int64") exists to avoid IEEE-754 loss past 2⁵³. **The acceptance is asymmetric — the bare-number form is lossy by design** (completed 2026-07-27, Q8 sweep): "When parsing a bare number when expecting an int64, the implementation should coerce that value to double-precision even if the corresponding language's built-in JSON parser supports parsing of JSON numbers as bigints" — only the quoted string is safe on input, not merely canonical on output. `ChangelogEntryBody.seq: i64` is emitted as a JSON *number* today; ProtoJSON emits `"seq": "42"`. Any client doing `entry.seq > x` breaks. This must be decided before `/api/v1` ships, not discovered by a frontend.

**7.3 Timestamps — ✅ ALREADY CONFORMANT, and this is the good news.** Spec: "Uses RFC 3339 … Generated output will always be **Z-normalized with 0, 3, 6 or 9 fractional digits**. Offsets other than `Z` are also accepted." `timestamp.proto` (https://github.com/protocolbuffers/protobuf/blob/main/src/google/protobuf/timestamp.proto): "the timezone is required," range `0001-01-01T00:00:00Z`–`9999-12-31T23:59:59.999999999Z`, leap seconds smeared.

`DateTimeUtc = chrono::DateTime<Utc>` (`backend/crates/domain/src/datetime.rs:16`) serializes byte-identically: chrono's serde impl calls `write_rfc3339(f, naive, offset, SecondsFormat::AutoSi, true)`, and `AutoSi` emits exactly 0/3/6/9 fractional digits (https://github.com/chronotope/chrono/blob/main/src/format/formatting.rs, https://github.com/chronotope/chrono/blob/main/src/datetime/serde.rs). Three residual gaps, all *input-validation* gaps rather than encoding gaps: (i) years outside 0001–9999 get chrono's `{year:+05}` ISO-8601 extension, outside `Timestamp`'s range; (ii) chrono can represent leap-second `:60`, which protobuf cannot; (iii) chrono's parser accepts lowercase `t`/`z` and arbitrary fractional-digit counts, while ProtoJSON "takes the decision that the timestamp format is the stricter definition of 'RFC 3339 as a profile of ISO-8601-2019'" where "lowercase letters are not allowed." **Net: the backend is laxer than the reference, so bad input passes Rust and fails at a generated client. Fix with a validating newtype at the boundary, not by changing the encoder.**

**7.4 Optionality and `null` — 🔴 CHANGES THE CURRENT WIRE.** "if a field supports presence, serializers **must** emit the field value **if and only if** the corresponding hazzer would return true"; "If the field doesn't support field presence and has the default value … serializers **should omit it**"; "**Serializers should not emit `null` values.** Parsers accept `null` as a legal value for any field … The field should remain unset, as though it was not present in the input at all." And "null values are not allowed within repeated fields." Presence table at https://protobuf.dev/programming-guides/field_presence/ — in proto3, singular scalars have *implicit* presence unless marked `optional`; message fields and `oneof` members always have explicit presence.

Consequences for our serde structs:
- A plain `string detail = 1;` (implicit presence) **omits the key entirely when empty**. serde's `detail: String` emits `"detail": ""`. Fix: `#[serde(skip_serializing_if = "String::is_empty")]`.
- `actor_id: Option<Uuid>` currently emits `"actor_id": null` for `None` — documented as intended ("`null` = a system entry", `changelog.rs:19`). ProtoJSON says do not emit null. Fix: `optional` in the proto + `#[serde(skip_serializing_if = "Option::is_none")]`, and the *client* must read absence as "system entry." **This is a semantic change to a documented contract, not a formatting nit.**
- `Option<T>` over an *implicit-presence* field **cannot round-trip**: `None` and `Some("")` collapse. Any field where absent ≠ empty must be declared `optional` in the proto.
- Spec caveat: "As of v25.x, the C++, Java, and Python implementations are nonconformant, as this flag affects proto2 optional fields but not proto3 optional fields." Rust/TS unaffected as far as this lane checked.

**7.5 Enums.** "The name of the enum value as specified in proto is used. **Parsers accept both enum names and integer values.**" Aliasing hazard: "Serializers must print the **first listed name** in the enum definition for a given numeric value" — **reordering aliases changes emitted bytes with no schema-breaking diff.** Add "do not reorder enum aliases" to the review checklist.

**7.6 Unknown fields.** Covered in §6. The one-line version: ProtoJSON parsers reject unknown fields by default, protobuf-es ships that default (`ignoreUnknownFields: false`), and our additive-only guarantee is void without an explicit client override.

**7.7 The serializer option set must be contract text, not a local choice.** `json_name`, `useProtoFieldName`, `enumAsInteger`, and `alwaysEmitImplicit` each change emitted bytes **while leaving the proto unchanged — so `buf breaking` will never see it.** protobuf-es write defaults: `{ alwaysEmitImplicit: false, enumAsInteger: false, useProtoFieldName: false }` (https://github.com/bufbuild/protobuf-es/blob/main/packages/protobuf/src/to-json.ts).

> **Canonical Zurfur ProtoJSON settings (part of the contract; changing any of them is a breaking change under §1 item 21):** lowerCamelCase keys *(pending 7.1)*; enum values as **names**, not integers; implicit-presence defaults **omitted**; `null` **never emitted**; int64 family as **decimal strings**; timestamps Z-normalised RFC 3339 with 0/3/6/9 fractional digits.

Pinned by a **golden-bytes test**: a fixture message with every wire-relevant shape (empty string, absent optional, i64, timestamp, enum, repeated) serialised and byte-compared to a checked-in JSON file. That test is the only thing standing between us and a silent breaking change from a flipped serializer flag.

**7.8 Route metadata.** `google/api/annotations.proto` declares `extend google.protobuf.MethodOptions { HttpRule http = 72295728; }` (https://raw.githubusercontent.com/googleapis/googleapis/master/google/api/annotations.proto) and is not gRPC-locked — it is a descriptor annotation. Three versioning consequences: (i) `additional_bindings` is the built-in way to serve an old and a new path from one method — the natural carrier for a **route-level** deprecation window, distinct from field-level deprecation — *in the spec*; in our toolchain it is unavailable (Q8 sweep 2026-07-27: prost cannot decode custom options at all, and the sole Rust HttpRule-reading codegen omits exactly `additional_bindings` — see §8 Q8 item 5), so a route-level window in Rust is hand-built either way; (ii) "All other fields are passed via the URL query parameters" means **adding a request field silently adds an accepted query parameter** — additive and safe, but it means the URL surface is derived from the message rather than declared, which this contract should state; (iii) `{var}` ≡ `{var=*}` matches a **single path segment**, so any id containing `/` (AT-URIs do) needs an explicit `{var=**}` template. Whether a Rust/axum codegen path actually consumes `HttpRule` → **RESOLVED 2026-07-27: nothing mature does** (§8 Q8 item 5); the annotations remain documentation of the URL surface, consumed by nothing in the Rust build by construction — which vindicates DD 40992770's generate-messages-only ruling.

**7.9 `google.protobuf.Struct`/`Value` cannot carry a large integer — the payload-blob hazard (NEW 2026-07-27, Q8 sweep).** `Value.number_value` is declared `double` (https://protobuf.dev/reference/protobuf/google.protobuf/, struct.proto); there is no int64 arm, so §7.2's decimal-string rescue **does not reach inside a `Struct`** — an integer above 2⁵³ in a payload blob is not at risk, it is unrepresentable. The tiers then diverge on failure mode: workspace-pinned `pbjson-types` 0.8.0 **errors** on an out-of-range integer (`visit_i64`: "out of range integral type conversion attempted" — while its `visit_f64` accepts `1e30` lossily, a guard inconsistent by magnitude, not lossiness), whereas protobuf-es receives values **already through `JSON.parse`** and silently truncates (MDN: "numbers in JSON text will have already been converted to JavaScript numbers, and may lose precision in the process"). Same bytes: Rust 500s, TypeScript stores wrong data quietly. **Not live today** — the corpus contains no `Struct`/`Value`, and `payload: serde_json::Value` exists only in the hand-written changelog handler — but it **arms the day the changelog message enters the corpus**, at which point stored payloads that round-trip today can start failing to deserialize. Resolution is §8 Q9, deliberately open.

---

## 8. Open questions

**Q1 — Enum extension: additive or breaking?** GH/AIP say additive; ZAL #107 and AZ #4 say breaking, both on the grounds that generated clients fail to deserialise unknown values. proto3 enums are open on the wire, so the answer turns entirely on whether the generated TypeScript narrows them to a closed union. **Closed by:** generating a throwaway enum from a candidate `.proto`, adding a value on the server, and observing what protobuf-es does at the SvelteKit boundary. If it throws, ZAL #112's `[Extensible enum]` discipline plus a mandated client fallback becomes a §6 obligation; if it round-trips the integer, enum addition is additive and §1 stands.

**Q2 — Are Zurfur ids opaque?** AIP: "A resource **must not** change its name," and this "affects major versions as well" — i.e. frozen even across a major bump. MSG says the exact opposite: "Changes to the length or format of opaque strings, such as resource IDs" are backward compatible. **Direct contradiction; genuinely a domain fork, not a default.** If we publish "these are UUIDv7," AIP applies and the format is frozen forever. If we publish "opaque, do not parse," Graph applies. **Closed only by an Engineer ruling**, written into the contract in one sentence. → **RESOLVED 2026-07-25: opaque — see R6 in the header.**

**Q3 — The three ProtoJSON wire changes (§7.1, §7.2, §7.4).** Field-name casing, int64-as-string, and null-omission each change bytes the current handlers emit. **Closed by:** ruling that the current unversioned surface is pre-GA and therefore exempt (§5) — in which case all three land as part of minting `/api/v1` and cost nothing — or by pinning `useProtoFieldName` and diverging from every generated client's default. **Do not let this be decided implicitly by whoever writes the first handler.**

**Q4 — The bare-array response.** `GET /commissions/{id}/changelog` returns a top-level JSON array, which ZAL #110 forbids precisely because it forecloses adding pagination without a break — and ZMVP-100 is the pagination ticket. **Closed by:** deciding whether `/api/v1` wraps it in an object at mint time (free) or accepts that paginating it later costs a major (expensive). Recommend wrapping. → **RESOLVED 2026-07-25: wrap — see R7 in the header.**

**Q5 — The `buf skip breaking` label.** `bufbuild/buf-action@v1` documents a PR label that skips the breaking check outright. **Closed by:** an Engineer ruling — ban it via a required-status-check that cannot be label-skipped, or permit it only with a signed justification comment naming the §5 emergency cause. Left unruled, it is a one-click bypass of this entire document. → **RESOLVED 2026-07-27: banned via required status check — see R9 in the header.**

**Q6 — `buf` rule-list discrepancy (§2).** → **RESOLVED 2026-07-25**: verified on buf 1.72.0 — `FIELD_SAME_NAME` IS in WIRE_JSON (renames stay CI-enforced); `ENUM_SAME_TYPE` is not. See §2.

**Q7 — Notice window: 12 or 24 months?** Recommended 12 (Google Cloud ToS) over 24 (GitHub, Graph) because our consumers are enumerable; band is 12–24, pre-GA is zero in all precedents. **Closed by** an Engineer ruling plus a commitment to ZAL #188 per-major usage telemetry, which is what makes the shorter number honest. → **RESOLVED 2026-07-27: 12 months + telemetry — see R10 in the header.**

**Q8 — the UNVERIFIED list.** → **SWEPT 2026-07-27; every item resolved below. Two produced new material (see §7.9 and §3's CORS clause).**

- **Google AIP and Azure numeric deprecation windows.** → **RESOLVED.** AIP-180 states no number at all. AIP-185 and AIP-181 *do* state numbers, but **only for pre-GA channels** — "we recommend 180 days" for beta turndown (https://google.aip.dev/185) and "a good rule of thumb is 90 days" (https://google.aip.dev/181); alpha functionality "may be removed without notice." For **stable** components AIP explicitly declines a number. Azure's REST breaking-changes guidelines state no number (definition + review process only). `aka.ms/AzBreakingChangesPolicy` **is not a public document** — it 301s to an internal Microsoft SharePoint file (APEXProgram tenant) that returns 401; it must never be cited here as a source. Microsoft's actual published customer-facing number lives in the **Modern Lifecycle Policy** (https://learn.microsoft.com/en-us/lifecycle/policies/modern): "a minimum of 12 months' notification prior to ending support if no successor product or service is offered." **Consequence: R10's 12 months now rests on two independent enterprise precedents, not one.** §4's table corrected accordingly.
- **`bufbuild/buf-action@v1` with no `BUF_TOKEN`.** → **MOOT — the action is not in our pipeline.** `.github/workflows/ci.yml` uses `bufbuild/buf-setup-action@v1` (buf 1.72.0) and invokes `buf lint` / `buf generate` / `buf breaking` **directly**. The underlying question is **RESOLVED for the direct CLI: no token is required.** BSR auth gates push, private modules, and admin actions only (https://buf.build/docs/bsr/authentication/); `.git#branch=…,subdir=…` reads the **local** `.git` (https://buf.build/docs/reference/inputs/); `buf.gen.yaml` uses a `local:` plugin (runs on disk, no registry call); `contract/buf.yaml` declares no `deps:` and there is no `buf.lock`; every `buf-setup-action` token input is optional; CI runs all three commands green today with none. **Tripwire:** adding a `remote:` plugin or a `deps:` entry re-opens this — unauthenticated remote code-gen is rate-limited to 10 req/hour (https://buf.build/docs/bsr/rate-limits/). This also makes R9 doubly safe: `buf skip breaking` is a *buf-action* feature and we never load the action.
- **buf has no deprecation-window/sunset feature.** → **RESOLVED — verified across a real sweep**, not a sample: the full CLI command index (https://buf.build/docs/reference/cli/buf/), both deprecate subcommands flag-by-flag (`buf source edit deprecate` → `--prefix`, `--diff`, `--path`, `--exclude-path`; `buf registry module deprecate` → no date flag), the buf.yaml v2 / buf.gen.yaml v2 / buf.lock key lists, and a live GitHub issue search (`"deprecation window"` → 0 hits in `bufbuild/buf`, 0 org-wide). buf's only mechanism is **static, undated marking**; §4's `lifecycle.toml` + CI assertions duplicate nothing. *Limit: a broad negative, not machine-proof.*
- **ProtoJSON `google.protobuf.Struct`/`Value` numeric precision.** → **RESOLVED — and it opens a new fork; see §7.9 and Q9.** `Value.number_value` is `double`; there is no int64 arm; §7.2's decimal-string rescue does not reach inside a `Struct`. The tiers diverge: `pbjson-types` errors on out-of-range integers while protobuf-es (fed by `JSON.parse`) silently truncates. Not a live bug — the corpus uses no `Struct`/`Value` today — but it arms the moment the changelog message (`payload: serde_json::Value`, `changelog.rs:26`) enters the corpus. Full detail in §7.9; §7.2's incomplete quote also completed (the bare-number input form is lossy by design).
- **Rust/axum codegen consuming `google.api.HttpRule`.** → **RESOLVED — nothing mature exists; hand-written handlers are the state of the art.** tonic has carried the request unfulfilled since 2020 (hyperium/tonic#332, open). prost cannot decode custom options at all (tokio-rs/prost#674, open) — `google.api.http` is extension 72295728 on `MethodOptions`, reachable only by hand-parsing a `FileDescriptorSet`. connect-rust derives routes from `{package}.{Service}/{Method}`, never HttpRule. The one genuine hit, `tonic-rest-build` (v0.1.5, 7★, one maintainer, ~5 months stale), reads HttpRule but **omits `additional_bindings`**. Envoy's `grpc_json_transcoder` is the standard answer and runs as a sidecar. **Sharp consequence recorded in §7.8(i):** the route-level deprecation carrier nominated there is unavailable in our toolchain — hand-built either way. Vindicates DD 40992770's generate-messages-only ruling. *Coverage caveat: crates.io keyword scanning was JS-gated; a small/oddly-named crate could have been missed.*
- **Browser / proxy / CDN handling of `Deprecation` and `Sunset`.** → **RESOLVED in the direction that matters; sparse elsewhere, stated honestly.** Intermediaries: no evidence of interference — RFC 9110 §7.6.1's removal duty covers only `Connection`-listed options plus a fixed legacy list; both headers are end-to-end; Cloudflare/CloudFront/Fastly document header removal as opt-in configuration only (Akamai unchecked). Browsers: no data — MDN documents neither header (both reference pages 404), no compat table exists; tracker searches returned nothing (weak negative, not proof). **The one real implementation consequence — CORS:** neither header is CORS-safelisted, so a cross-origin client cannot read them without `Access-Control-Expose-Headers`; verified on the wire — every `api.github.com` response exposes `Deprecation, Sunset, Warning` explicitly. **Ruled 2026-07-27: §3 gains the CORS clause AND §4 gains assertion 7.** *(Could not capture a live `Sunset` value; the exposure declaration is the artifact.)*

**Q9 — How does an unstructured `payload` blob cross the wire?** *(NEW 2026-07-27, opened by the Q8 sweep; see §7.9.)* `google.protobuf.Struct` cannot carry an integer above 2⁵³, and the two tiers disagree on the failure mode — Rust rejects, TypeScript truncates. **Closed by** an Engineer ruling among: (a) declare payload as `Struct` and bind a contract rule that payload numbers fit ±2⁵³, enforced at write; (b) carry payload as an opaque JSON `string`/`bytes`, moving precision outside ProtoJSON entirely; (c) type the payload per changelog `kind` as real proto messages and retire the free-form blob. **Must be decided before the changelog message enters the corpus** — not after stored payloads start failing to deserialize. *Engineer 2026-07-27: deliberately deferred to the ratification read — it does not block ratifying §1–§7 as written, and it hard-blocks the changelog message's corpus entry.*