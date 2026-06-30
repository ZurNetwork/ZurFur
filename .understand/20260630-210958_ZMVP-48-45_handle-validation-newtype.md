# 🔎 Understanding ZMVP-48 + ZMVP-45 — One `Handle` validation newtype (punycode reject + reserved-label list)

> **Status:** both To Do · **Sources:** https://zurnetwork.atlassian.net/browse/ZMVP-48 · https://zurnetwork.atlassian.net/browse/ZMVP-45 · **Generated:** 2026-06-30 21:09:58 UTC · **Snapshot:** `.understand/20260630-210958_ZMVP-48-45_handle-validation-newtype.md`
> **Parent epic:** ZMVP-13 *The Citizen (Accounts)* · **Priority:** Medium (both) · **Decision owner:** mechanism settled (DD 26050561, DD 24870914 §6); ONE open domain touch-point — the reserved-label *contents* (§8).
> **Bundling:** one unit of work, one PR/worktree, closing **both** ZMVP-48 and ZMVP-45. Engineer-approved. Sequencing fork DISPOSED → **bootstrap the `Handle` newtype STANDALONE NOW** (do not wait for ZMVP-44; ZMVP-44 will later *consume* this validated newtype).

## 🧭 1. Context (cold-start)

An **Account handle** is the public, human-typeable name an Account is reached by. Per DD **24870914 "The Account Handle"** (DECIDED 2026-06-28) it is **user-chosen at `POST /accounts`** (never auto-derived) and is one of two things: a Zurfur-issued `<label>.zurfur.app`, or a **brought (BYO)** domain the user already controls. Both verify through the **atproto handle mechanism** (DNS `_atproto` TXT or HTTPS `/.well-known/atproto-did`, plus a bidirectional `alsoKnownAs` on the DID document); the Account's `did:plc` is Zurfur-operated in both cases. DD 24870914 §6 pins handles to be **"normalized to the atproto handle spec plus a Zurfur reserved-label list."** That single sentence is the seam this unit builds.

Two of that DD's open follow-up items are the two tickets bundled here:
- **ZMVP-48** — the atproto charset is ASCII `a–z 0–9 -`, so **punycode labels** (`xn--…`) are valid hostnames that can encode an internationalized homoglyph look-alike of a real creator/brand and be *verified* as a handle. The atproto handle spec acknowledges this "presents security and impersonation challenges" and explicitly punts the policy to the implementer. DD **26050561 "Confusable Handles & the Punycode Policy"** (DECIDED 2026-06-30) settles it: **reject any handle whose normalized form contains a label beginning with `xn--`**, *uniformly* across both namespaces, at claim time, with the correct RFC 9457 error. Kills the homoglyph-IDN vector **by construction** — no confusables database. Forward-compatible with the sanctioned UTS #39 "allow-with-checks" upgrade (it only ever *adds* a restriction).
- **ZMVP-45** — a **reserved-label set** (`api, admin, www, support, help, mail, status, …`) that cannot be claimed as a `*.zurfur.app` label, checked at claim time, rejected with the right error. Source: the "Reserved-label list — settle at build time" open item on DD 24870914.

**Why one unit (one gate, not three).** Both tickets create/modify the *same new file* — a `Handle` domain newtype — and both are checks *inside the same validation pass*. Splitting them would mean two PRs racing on one greenfield file. So this unit ships the **`Handle` newtype + its full rule set (atproto normalization + `xn--` reject + reserved-label reject) + unit tests** as one coherent change (memory `feedback_make_unsoundness_unreachable`: one shared enforced path beats per-site checks that drift).

> ⚠️ **The load-bearing finding (verified, unchanged from the ZMVP-48 snapshot):** the "shared handle normalization/validation path" both tickets reference **does not exist in code yet.** Grep over `backend/crates` for `xn--|punycode|reserved.?label|normalize` is **clean** (zero hits). There is no `Handle` type (`elements/handle.rs` does not exist), no handle validation, and **`Account` has no handle field**. `POST /accounts` (`api/src/routes/accounts.rs:138-172`) validates only `name` via `AccountName::try_new` and **takes no handle**. So there is **no claim site that accepts a handle yet** — that surface is introduced by ZMVP-44/30. **Consequence:** this unit delivers the *pure-domain newtype + unit tests*; the integration / RFC-9457 claim-site wiring is **DEFERRED (handed off with a failing test + note)** until a handle-accepting claim surface exists. See §3, §8.

## 🗺️ 2. Domain

- **Account** (DESIGN/Account `1966081`) — platform-custodied sovereign identity. Today `Account { id, did, name: AccountName, created_at, updated_at, deleted_at }`. `name` (`AccountName`) is a **display name, NOT a handle** — no handle is modeled yet. This unit does **not** add a handle field to `Account` (that is ZMVP-44's, with its own migration).
- **The Account Handle** (DD 24870914) — the thing validated. **§6 is the spec this unit implements:**
  - lowercase-fold (handles are case-insensitive);
  - each **segment** 1–63 chars of ASCII `a–z 0–9 -`, **no leading/trailing hyphen**;
  - **at least two segments**;
  - the **top-level (rightmost) segment cannot start with a digit**;
  - **≤253 chars** overall;
  - reject **reserved TLDs**: `.alt .arpa .example .internal .invalid .local .localhost .onion .test`;
  - on top of the spec, a **Zurfur reserved-label list** for its own namespace (`api`, `admin`, `www`, …) — **ZMVP-45**.
  These are **spec-mechanical**, not a Zurfur domain fork (they transcribe the atproto handle spec + the DD).
- **Confusable Handles & Punycode Policy** (DD 26050561) — governs **ZMVP-48**. Decision 1: reject `xn--` outright (v1). Decision 2: uniform across both sources. Decision 3: pure-ASCII confusables (`rn`→`m`, `0`→`O`) **out of scope**. Decision 4: UTS #39 is the *upgrade path*, not this ticket.
- **API response shape & errors** (DD 23592962; memory `project_api_response_error_model`) — a rejection at a claim site must surface as **RFC 9457 problem+json** with a `urn:zurfur:error:*` type + own `code`. The seam is `Problem::invalid_request(detail)` / specific constructors in **`api/src/problem.rs`** (note: moved here since the ZMVP-48 snapshot cited `lib.rs`). **No claim site exists yet**, so this unit defines the *typed error variants* and **defers** the HTTP mapping with a note.
- **Validated-newtype idiom** — the repo's established "validate on the way in" pattern is **`AccountName`** (`account.rs:45-109`): a `String` newtype + a `try_new` constructor returning a typed error enum that `impl Display + std::error::Error`, an `as_str()`, and `///` doctests. `Handle` mirrors this **exactly**. (`Did` is the deliberate counter-example: no validating constructor.) Per memory `feedback_traits_dependency_inversion`, this stays a plain struct + free function — **no trait**, no polymorphism (nothing consumes one).

Memory consulted: `project_punycode_handle_policy`, `project_default_account_handle`, `project_user_as_actor_accounts_on_demand`, `project_api_response_error_model`, `feedback_make_unsoundness_unreachable`, `feedback_traits_dependency_inversion`.

## 🎯 3. Goal & scope

**Goal:** create the **one** validated `Handle` domain newtype that every future claim source (onboarding ZMVP-30, resolution ZMVP-44) will funnel through, enforcing — in a single pass — atproto normalization, the `xn--` reject (ZMVP-48), and the reserved-label reject (ZMVP-45). Bootstrapped **standalone now**; ZMVP-44 *consumes* it.

**In scope (this unit ships):**
- `backend/crates/domain/src/elements/handle.rs` — a `Handle(String)` newtype with `Handle::try_new(raw) -> Result<Self, HandleError>` and `as_str()`, mirroring `AccountName`.
- **Normalization** (atproto spec, DD 24870914 §6): trim, lowercase-fold, strip a single trailing dot.
- **Charset / segment / length rules** (spec-mechanical): ≥2 segments; each segment 1–63 `[a-z0-9-]`, no leading/trailing hyphen; rightmost segment not digit-leading; ≤253 chars overall; reserved-TLD reject.
- **`xn--` per-label reject** (ZMVP-48): split normalized handle on `.`, reject if **any** label starts with `xn--` (case already folded → plain prefix check), uniform across both namespaces.
- **Reserved-label reject** (ZMVP-45): one `RESERVED_LABELS` set, checked against the **leftmost label** of a `*.zurfur.app` handle at claim time.
- A typed `HandleError` enum (`impl Display + Error`) with a variant per failure class.
- **Unit tests + `///` doctests** for every rule (the §6 checklist).
- Registration: `pub mod handle;` in `domain/src/elements.rs` (+ a module-doc line in its `//!` list).

**Out of scope / explicitly deferred:**
- **Pure-ASCII confusables** (`rn`/`m`, `0`/`O`, `1`/`l`) — DD 26050561 Decision 3.
- **UTS #39 / ICU `SpoofChecker` / skeleton index** — DD 26050561 Decision 4 (future).
- **The handle subsystem itself** — DNS/well-known verification, `alsoKnownAs`, issuance/resolution, *availability* (uniqueness) checks, the `Account.handle` field, and its migration — all **ZMVP-44** (which consumes this newtype). *Note: ZMVP-45's "availability" wording is about uniqueness/collision; this unit owns only the* reserved-label *gate, which is validation, not a DB lookup.*
- **🤝 The RFC-9457 claim-site wiring (HANDED OFF).** No endpoint accepts a handle yet (`POST /accounts` has no handle field, and adding one is ZMVP-44's, not this unit's). So the `HandleError → Problem` mapping and the integration tests are deferred — represented as a **failing/ignored test + a note** pointing at ZMVP-44/30. The pure-domain newtype + unit tests stand entirely on their own and are this unit's Definition of Done.

**No `Account`/migration changes in this unit.** Verified: this is greenfield pure-domain; touching `Account` or adding a migration would step on ZMVP-44.

## 📦 4. Deliverables

- [ ] **`domain/src/elements/handle.rs`** — `Handle(String)` newtype, `try_new`, `as_str`, full `///` docs + doctests (mirror `AccountName`).
- [ ] **Normalization** inside `try_new`: trim → lowercase → strip trailing `.`.
- [ ] **Charset/segment/length validation** (atproto §6): ≥2 segments; each segment `1..=63` of `[a-z0-9-]`, no leading/trailing `-`; rightmost segment not digit-leading; total `≤253`.
- [ ] **Reserved-TLD reject** (`alt arpa example internal invalid local localhost onion test`).
- [ ] **`xn--`-per-label reject** (ZMVP-48) — any label, uniform across namespaces.
- [ ] **`RESERVED_LABELS` set + reserved-label reject** (ZMVP-45) on the leftmost label of `*.zurfur.app` handles — *contents flagged for Engineer (§8)*.
- [ ] **`HandleError`** enum `impl Display + std::error::Error`, one variant per failure class (mirror `AccountNameError`).
- [ ] **Unit tests** covering every §6 row + doctests.
- [ ] **`pub mod handle;`** in `domain/src/elements.rs` + a `//!` module-doc line.
- [ ] **🤝 Handoff note + failing/ignored test** for the RFC-9457 claim-site mapping (depends on ZMVP-44/30's handle-accepting surface).
- [ ] **Design sync (offer):** mark DD 24870914's "Reserved-label list" and "Punycode/confusable" open items as closed/implemented (DD 26050561 already records the punycode decision).

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done (evidence) |
|---|---|---|---|---|
| **`Handle` newtype skeleton + `try_new`/`as_str`/docs** | 3 — mirrors `AccountName`; uncertainty is *shape*, not logic | P1 | 🤖 Claude | ⬜ — template at `account.rs:45-109`; greenfield (no `handle.rs`) |
| **Normalization (trim/lowercase/strip trailing dot)** | 1 | P1 | 🤖 Claude | ⬜ — DD 24870914 §6 |
| **Charset/segment/length rules (atproto §6)** | 3 — most logic lives here (segment loop, edge cases) | P1 | 🤖 Claude | ⬜ — spec-mechanical, no fork |
| **Reserved-TLD reject** | 1 | P2 | 🤖 Claude | ⬜ — fixed list from §6 |
| **`xn--` per-label reject (ZMVP-48)** | 1 — split on `.`, prefix check | P1 | 🤖 Claude | ⬜ — DD 26050561; grep clean |
| **`RESERVED_LABELS` set + reject (ZMVP-45) — mechanism** | 1 | P1 | 🤖 Claude | ⬜ — a `const`/`static` set + membership check |
| **Reserved-label set — exact *contents*** | 2 — *domain call, not technical* | P0 | 🧑 Engineer | ⬜ — starter set proposed §8; Engineer approves/extends |
| **`HandleError` + Display/Error** | 1 | P2 | 🤖 Claude | ⬜ — mirror `AccountNameError` (`account.rs:71-90`) |
| **Unit tests + doctests (all rules)** | 2 | P1 | 🤖 Claude | ⬜ — none today |
| **🤝 RFC-9457 claim-site mapping + integration test** | 2 — wire to `Problem` | P3 | 🤝 Handed off | ⬜ — *blocked:* no handle-accepting endpoint until ZMVP-44/30 |

**Domain weight: ~1/10.** Every fork in the *rules* is settled (DD 26050561 + DD 24870914 §6 + atproto spec). The **only** non-mechanical item is the reserved-label **list contents** (P0, 🧑 Engineer) — the *mechanism* (a set checked in the newtype) is mechanical; the *list* is the Engineer's call (§8).

**Owner split:** bulk is **🤖 Claude** (difficulty 1–3, fully specified, mirrors `AccountName`). **One 🧑 Engineer** item — approve the reserved-label set (a list, not a design). **One 🤝 handoff** — the claim-site RFC-9457 wiring, blocked on a surface this unit deliberately doesn't build.

## ✅ 6. Test checklist (TDD)

All **unit** tests in `handle.rs` (`#[cfg(test)]`) unless marked. Write red first, then green.

**Normalization**
- *asserts that* `Handle::try_new("Alice.Zurfur.APP")` lowercases → `as_str() == "alice.zurfur.app"` → AC "lowercase-fold"
- *asserts that* a trailing dot is stripped: `Handle::try_new("alice.zurfur.app.")` → `"alice.zurfur.app"` → AC "normalized form"
- *asserts that* surrounding whitespace is trimmed (mirror `AccountName`)

**Charset / segment / length (atproto §6)**
- *asserts that* a single-segment handle (`"alice"`) is rejected (`HandleError::TooFewSegments`) → AC "≥2 segments"
- *asserts that* a segment >63 chars, and a total >253 chars, are each rejected (`SegmentTooLong` / `TooLong`)
- *asserts that* a leading/trailing hyphen in a segment is rejected (`HyphenEdge`)
- *asserts that* an out-of-charset byte (`_`, space, unicode `é`) is rejected (`InvalidChar`)
- *asserts that* a digit-leading rightmost segment (`"alice.123"`) is rejected (`TldLeadingDigit`)
- *asserts that* an empty segment (`"alice..app"`) is rejected

**Reserved TLD**
- *asserts that* `"foo.local"`, `"foo.test"`, `"foo.onion"` are each rejected (`ReservedTld`)

**Punycode (ZMVP-48)**
- *asserts that* `Handle::try_new("xn--80ak6aa92e.zurfur.app")` → `Err(HandleError::PunycodeLabel)` → AC "`xn--` `*.zurfur.app` label rejected"
- *asserts that* a BYO domain `"xn--e1awd7f.example.com"` → wait, `.example` is reserved; use `"xn--e1awd7f.com"` → `Err(PunycodeLabel)` → AC "`xn--` BYO domain rejected"
- *asserts that* `xn--` **anywhere** (not just leftmost) and **mixed-case** `XN--` are both rejected (normalize-then-check; robustness over the DD's "any label")

**Reserved label (ZMVP-45)**
- *asserts that* `"api.zurfur.app"`, `"admin.zurfur.app"`, `"www.zurfur.app"` are each rejected (`HandleError::ReservedLabel`) → AC "claim on a reserved label rejected"
- *asserts that* a reserved *word* as the leftmost label of a **BYO** domain (`"api.example.com"`) is **accepted** — the reserved list guards only the `*.zurfur.app` namespace *(confirm this boundary with the Engineer in §8 — it's a small domain call: does the reserved set apply only to `*.zurfur.app`, or to the leftmost label of any handle?)*

**Happy path (guard against over-rejection)**
- *asserts that* `"alice.zurfur.app"` and `"alice.example.com"` are **accepted** and round-trip via `as_str()`

**Error quality**
- *asserts that* each `HandleError` variant renders a clear `Display` message (+ doctest, mirroring `AccountNameError`)

**🤝 Deferred (handed off — write as `#[ignore]` + note, do NOT build the surface)**
- *Integration:* claiming an `xn--` / reserved-label handle at the claim endpoint returns HTTP 4xx problem+json with the right `urn:zurfur:error:*` type/`code`. **Blocked:** `POST /accounts` has no handle field until ZMVP-44/30. Leave a failing/ignored test naming ZMVP-44 as the unblocker (home: `api/tests/accounts.rs`).

## 🧠 7. Logic & shape

The newtype mirrors `AccountName` exactly; the substance is one normalization pass then a sequence of guards. **One gate, all rules** — every claim source inherits the whole set by constructing a `Handle`.

```rust
// domain/src/elements/handle.rs  — SKETCH (understanding only; not the implementation)

/// atproto handle limits (DD 24870914 §6 / atproto handle spec).
pub const HANDLE_MAX_LEN: usize = 253;
pub const LABEL_MAX_LEN: usize = 63;

/// Reserved TLDs the atproto spec forbids as handles.
const RESERVED_TLDS: &[&str] =
    &["alt", "arpa", "example", "internal", "invalid", "local", "localhost", "onion", "test"];

/// Labels Zurfur withholds from the `*.zurfur.app` namespace (ZMVP-45).
/// ⚠️ CONTENTS are an Engineer call — see briefing §8. Starter set:
const RESERVED_LABELS: &[&str] = &[/* api, admin, www, … — Engineer-approved */];

/// A validated, normalized atproto-style Account handle (DD 24870914 §6).
/// Mirrors [`AccountName`]: validate on the way in, expose `as_str()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandleError {
    Empty,
    TooLong(usize),              // > 253 overall
    TooFewSegments,              // < 2 segments
    EmptySegment,
    SegmentTooLong(usize),       // a label > 63
    InvalidChar(char),           // outside [a-z0-9-]
    HyphenEdge,                  // leading/trailing '-' in a label
    TldLeadingDigit,             // rightmost segment starts with a digit
    ReservedTld(String),         // .local/.test/…
    PunycodeLabel,               // any label starts with "xn--"   (ZMVP-48)
    ReservedLabel(String),       // leftmost label reserved in *.zurfur.app (ZMVP-45)
}
// impl Display + std::error::Error  — one clear message per variant (mirror AccountNameError)

impl Handle {
    pub fn try_new(raw: impl Into<String>) -> Result<Self, HandleError> {
        // 1. NORMALIZE: trim, lowercase, strip one trailing '.'
        // 2. length: non-empty, <= HANDLE_MAX_LEN
        // 3. split('.') → labels; require >= 2; none empty
        // 4. per label: 1..=63; every byte in a-z|0-9|'-'; no leading/trailing '-'
        //    → on any "xn--" prefix: Err(PunycodeLabel)            (ZMVP-48, uniform)
        // 5. rightmost label must not start with a digit
        // 6. reject reserved TLD (rightmost in RESERVED_TLDS)
        // 7. if handle ends with ".zurfur.app": leftmost label not in RESERVED_LABELS  (ZMVP-45)
        // Ok(Self(normalized))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

Enforcement topology — the point of both DDs is **one gate, not many** (`feedback_make_unsoundness_unreachable`):

```
  onboarding (ZMVP-30) ─┐
  resolution (ZMVP-44) ─┼──►  Handle::try_new  ──► [normalize · charset/segment · xn-- · reserved-TLD · reserved-label] ──► claim
       (future consumers)┘            (THIS UNIT — the one shared seam; ZMVP-44 consumes it)
```

**Why standalone-now is clean (the disposed fork):** ZMVP-44's own briefing states "this ticket consumes a validated handle," so validation is the prerequisite, not a dependent. Building the newtype first means ZMVP-44 imports `Handle` instead of inventing parsing inline — and there is **zero merge collision** because the file is greenfield and ZMVP-44 only *adds a consumer* (the `Account.handle` field + claim site), never edits these rules.

## 🚀 8. Next steps

1. **🧑 Engineer touch-point (one, non-blocking for Claude's lane — propose, don't decide):** approve / extend the **reserved-label set contents** (ZMVP-45) and confirm its **scope boundary**. The *mechanism* (a set checked in the newtype) is mechanical and Claude builds it; the *list* is the Engineer's domain call. **Proposed starter set** (infrastructure / role / well-known labels — withheld from `*.zurfur.app`):
   - *Infra / service:* `api`, `admin`, `www`, `app`, `cdn`, `assets`, `static`, `media`, `blob`, `status`, `health`, `metrics`
   - *Auth / identity:* `auth`, `login`, `logout`, `signin`, `signup`, `oauth`, `sso`, `account`, `accounts`, `did`, `plc`
   - *Comms / abuse:* `mail`, `smtp`, `support`, `help`, `contact`, `abuse`, `security`, `postmaster`, `webmaster`, `hostmaster`, `noc`
   - *Brand / staff:* `zurfur`, `official`, `root`, `system`, `staff`, `team`, `moderator`, `mod`
   - *Protocol / well-known:* `well-known`, `atproto`, `xrpc`, `ns`
   - **Open sub-question for the Engineer:** does the reserved-label gate apply **only to `*.zurfur.app`** (recommended — Zurfur only controls its own namespace; a BYO `api.example.com` is the user's to claim), or to the leftmost label of **any** handle? The §6 test row above assumes *only `*.zurfur.app`* — confirm.
2. **🤖 Build (Claude's lane, can start immediately):** write the failing unit tests from §6 (red) → `handle.rs` newtype with normalization + charset/segment + `xn--` + reserved-TLD + reserved-label (green). Register `pub mod handle;` in `elements.rs`. `/document` the new signatures.
3. **🤝 Hand off the claim-site wiring:** add an `#[ignore]`d integration test + a note in `accounts.rs`/`api/tests/accounts.rs` stating the `HandleError → Problem` (RFC 9457) mapping lands when **ZMVP-44/30** introduce a handle-accepting `POST /accounts` field. Do **not** add a handle field or migration in this unit.
4. **Design sync (offer):** close DD 24870914's "Reserved-label list" + "Punycode/confusable" open items via `/design-sync` (DD 26050561 already records the punycode call).
5. **Collision watch (`/close-gaps`):** ZMVP-44 (adds `Account.handle` + claim site + migration — *consumes* `Handle`, no rule edits), ZMVP-30 (onboarding handle choice), ZMVP-47 (write-gate at `create_account`). This unit owns `handle.rs` outright; downstream tickets import it.

---

## ✅ ENGINEER DISPOSITIONS — FINAL (2026-06-30, uow planning)

- **Sequencing:** **bootstrap `Handle` standalone NOW**; ZMVP-44 consumes it later. SETTLED.
- **Reserved-label scope:** gate the reserved-label rejection on the **leftmost label of `*.zurfur.app` handles ONLY** — NOT BYO domains (`api.example.com` is the owner's call). SETTLED.
- **Reserved-label contents:** the proposed starter set is **APPROVED** — infra (`api admin www app cdn assets static media blob status health metrics`), auth (`auth login logout signin signup oauth sso account accounts did plc`), comms/abuse (`mail smtp support help contact abuse security postmaster webmaster hostmaster noc`), brand/staff (`zurfur official root system staff team moderator mod`), protocol/well-known (`well-known atproto xrpc ns`). SETTLED.
- **DD 24870914 open items** (punycode + reserved-label) → offer `/design-sync` to close at close-out.

**Open questions / unknowns:**
- ~~Reserved-label set contents + scope~~ — DECIDED above (approved set, `*.zurfur.app` only).
- Which exact `urn:zurfur:error:*` `code`(s) for the punycode / reserved-label / malformed rejections (follows DD 23592962; picked when the claim site is wired in ZMVP-44, not here).
- ZMVP-46 (Account handle-change flow) is DD-referenced but separate — not in this unit; confirm relevance during `/close-gaps`.
