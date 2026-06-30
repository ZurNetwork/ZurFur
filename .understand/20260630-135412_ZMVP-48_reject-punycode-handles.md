# 🔎 Understanding ZMVP-48 — Reject punycode (xn--) labels in Account handle validation

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-48 · **Generated:** 2026-06-30 13:54:12 MDT · **Snapshot:** `.understand/20260630-135412_ZMVP-48_reject-punycode-handles.md`
> **Parent epic:** ZMVP-13 *The Citizen (Accounts)* · **Priority:** Medium · **Owner of the decision:** already settled (DD 26050561)

## 🧭 1. Context (cold-start)

An **Account handle** is the public, human-typeable name an Account is reached by — in Zurfur it's one of two things (DD 24870914 "The Default Account's Handle"): a Zurfur-issued `<label>.zurfur.app`, or a **brought (BYO)** domain the user already controls. Both are verified through the **atproto handle mechanism** (DNS `_atproto` TXT or HTTPS `/.well-known/atproto-did`, plus a bidirectional `alsoKnownAs` on the DID document).

The atproto charset is ASCII `a–z 0–9 -`. **Punycode labels** (`xn--…`) are valid ASCII hostnames, so an internationalized look-alike of a real name — a homoglyph IDN — can be encoded entirely within the rules and verified as a handle. The atproto handle spec *acknowledges* this "presents security and impersonation challenges" and then **explicitly leaves the policy to the implementer**. The attack: register a verified IDN look-alike of a real creator/brand and wear it as your Account handle to impersonate them.

**The decision is already made** — DD **26050561** "Confusable Handles & the Punycode Policy" (DECIDED 2026-06-30): for v1, **reject any handle whose normalized form contains a label beginning with `xn--`**, *uniformly* across both namespaces, at claim time, with the correct RFC 9457 error. This kills the homoglyph-IDN vector **by construction** — no confusables database to maintain. The rule only ever *adds* a restriction, so it's fully forward-compatible with the sanctioned future upgrade (UTS #39 "allow-with-checks"). This ticket is the **mechanical implementation** of that DD; no domain decision remains.

> ⚠️ **The load-bearing finding:** the "shared handle normalization/validation path" the ticket says to add the rule to **does not exist in code yet.** There is no `Handle`/`AccountHandle` type, no handle validation, no `xn--`/IDN/label-splitting code anywhere in `backend/`, and `Account` has no handle field. That path is created by **ZMVP-44** (handle issuance & resolution). So this ticket is effectively greenfield and **coupled to ZMVP-44** — see §5, §8.

## 🗺️ 2. Domain

- **Account** (DESIGN/Account `1966081`) — the platform-custodied sovereign identity. Today modeled as `Account { id: AccountId, did: Did, name: AccountName, created_at, updated_at, deleted_at }` — note: **`name` (AccountName) is a display name, NOT a handle.** No handle is modeled yet.
- **Account handle** (DD 24870914 `24870914`) — user-chosen at account creation; `*.zurfur.app` or BYO domain; this is the thing being validated. The "Punycode / confusable" open item on that DD is *closed by* this ticket's DD.
- **Confusable Handles & Punycode Policy** (DD 26050561) — the governing spec. Decision 1: reject `xn--` outright (v1). Decision 2: uniform across both sources. Decision 3: pure-ASCII confusables (`rn`→`m`, `0`→`O`) **out of scope**. Decision 4: UTS #39 is the sanctioned upgrade path, **not** this ticket.
- **API response shape & errors** (DD 23592962; memory `project_api_response_error_model`) — rejection must surface as an **RFC 9457 problem+json** with a `urn:zurfur:error:*` type + own `code`. The existing seam is `Problem::invalid_request(...)` in `api/src/lib.rs`.
- **Validated newtype idiom** — the repo's established pattern for "validate on the way in" is `AccountName` (`account.rs:45–109`): a `String` newtype, a `try_new` constructor that returns a typed error enum implementing `Display`/`Error`, an `as_str()`, and `///` doctests. A `Handle` type should mirror this exactly. (`Did` is the deliberate counter-example: no validating constructor.)

Memory consulted: `project_punycode_handle_policy`, `project_default_account_handle`, `project_user_as_actor_accounts_on_demand`, `project_api_response_error_model`, `feedback_traits_dependency_inversion`.

## 🎯 3. Goal & scope

**Goal:** guarantee that no Account handle containing an `xn--` label can ever be claimed — enforced in **one shared place** so every claim source (onboarding ZMVP-30, resolution infra ZMVP-44, reserved-label ZMVP-45) inherits it, returning an RFC 9457 error.

**In scope:**
- The `xn--`-label rejection rule, applied to the **normalized** handle, **uniformly** for `*.zurfur.app` labels and BYO domains.
- A single shared validation/normalization seam (not per-source) that this rule lives in.
- The RFC 9457 problem mapping at claim time.
- Tests: an `xn--` BYO domain rejected, an `xn--` `*.zurfur.app` label rejected, a normal ASCII handle accepted.

**Out of scope (explicit):**
- Pure-ASCII confusables (`rn`/`m`, `0`/`O`, `1`/`l`) — separate concern (DD Decision 3).
- The UTS #39 allow-with-checks path / ICU `SpoofChecker` / skeleton index — documented future direction, **not** this ticket (DD Decision 4).
- **Building the whole handle subsystem** (DNS/well-known verification, `alsoKnownAs`, issuance/resolution) — that's **ZMVP-44**. This ticket adds *one rule* to that subsystem's validation entry point.
- Display/rendering of IDN handles.

**The scope tension to resolve (§8):** the shared validation path doesn't exist. Does ZMVP-48 (a) wait for / layer on top of ZMVP-44, or (b) bootstrap a minimal `Handle` newtype now (mirroring `AccountName`) carrying just this rule, for ZMVP-44 to extend? That's a **sequencing/structure call for the Engineer**, not a domain decision.

## 📦 4. Deliverables

- [ ] A shared handle **normalization + validation** entry point (a `Handle`/`AccountHandle` newtype with `try_new`, or a `validate`/`normalize` fn) that all claim sources funnel through — **the single enforcement site**.
- [ ] An `xn--`-per-label check inside it: split the normalized handle on `.`, reject if **any** label starts with `xn--` (ASCII-case-insensitive), uniform across both namespaces.
- [ ] A typed rejection variant (e.g. `HandleError::PunycodeLabel`) implementing `Display` + `std::error::Error`, mirroring `AccountNameError`.
- [ ] RFC 9457 mapping at the claim site so the rejection surfaces as problem+json with a `urn:zurfur:error:*` type + `code`.
- [ ] Tests: `xn--` BYO domain rejected · `xn--` `*.zurfur.app` label rejected · plain ASCII handle accepted · (recommended) mixed-case `XN--`/embedded-label cases.
- [ ] Doc comments (`///`) on the new type/fn/error per repo convention.
- [ ] Design sync: mark the "Punycode/confusable" open item on DD 24870914 as closed (offer `/design-sync`; the DD 26050561 already records the decision).

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Sequencing/structure call** — does ZMVP-48 bootstrap the `Handle` newtype or layer on ZMVP-44's? | 2 — *coordination, not technical* | P0 | 🧑 Engineer | ⬜ — no handle type exists (`account.rs` has none; ZMVP-44 owns it per `parallel-set.json:54-59`) |
| **`Handle` newtype + normalization seam** (if bootstrapped here) | 3 — mirrors `AccountName`; uncertainty is *shape*, not logic | P1 | 🤖 Claude | ⬜ — `AccountName` at `account.rs:45-109` is the exact template |
| **`xn--` label rejection rule** (the actual ticket) | 1 — split on `.`, prefix check | P1 | 🤖 Claude | ⬜ — no IDN/label code anywhere (grep clean) |
| **`HandleError::PunycodeLabel` + Display/Error** | 1 | P2 | 🤖 Claude | ⬜ — mirror `AccountNameError` (`account.rs:71-90`) |
| **RFC 9457 mapping at claim site** | 2 — wire to `Problem` | P2 | 🤖 Claude | ⬜ — seam exists: `Problem::invalid_request` (`api/src/lib.rs:714-715`) |
| **Tests (both namespaces + ASCII pass)** | 1 | P1 | 🤖 Claude | ⬜ — none today; `accounts.rs` is the integration home |

**Domain weight: ~1/5.** Every domain fork — reject vs. allow, uniform vs. exempt, ASCII scope, upgrade path — is already settled in DD 26050561. The only non-mechanical item is the *sequencing* call (P0 above), which is coordination, not domain judgment.

**Owner split:** the **bulk is 🤖 Claude** (difficulty 1–3, fully specified by the DD, mirrors an existing idiom). One 🧑 Engineer item, and it's a 2-minute *which-order* decision, not design.

## ✅ 6. Test checklist (TDD)

- **Unit** — *asserts that* `Handle::try_new("xn--80ak6aa92e.zurfur.app")` returns `Err(HandleError::PunycodeLabel)` → AC "`xn--` `*.zurfur.app` label rejected"
- **Unit** — *asserts that* a BYO domain with an `xn--` label (e.g. `xn--e1awd7f.example.com`) is rejected → AC "`xn--` BYO domain rejected"
- **Unit** — *asserts that* an `xn--` label **anywhere** in the handle (not just the first), and **mixed-case** `XN--`, are both rejected → AC "uniform `xn--` rejection" (robustness over the DD's "any label")
- **Unit** — *asserts that* a normal ASCII handle (`alice.zurfur.app`, `alice.example.com`) is **accepted** → AC "normal ASCII handle passes"
- **Unit** — *asserts that* `HandleError::PunycodeLabel` renders a clear `Display` message → doc/error-quality
- **Integration** — *asserts that* claiming a handle with an `xn--` label at the claim endpoint returns **HTTP 4xx problem+json** with the right `urn:zurfur:error:*` type/`code` → AC "rejected at claim time with the correct RFC 9457 error" (home: `api/tests/accounts.rs`)
- **Integration** — *asserts that* an ASCII handle claims successfully end-to-end → guards against over-rejection

> Note: the integration tests presuppose a claim endpoint that accepts a handle — which **ZMVP-44 introduces**. If ZMVP-48 lands first, integration coverage may be deferred to a failing test + note (handed off) until the claim surface exists; the unit tests stand alone.

## 🧠 7. Logic & shape

The rule is tiny; the only subtlety is *where* it lives and that it runs on the **normalized** form (lowercased, trailing dot stripped) so casing/encoding can't sneak past.

```
fn validate_no_punycode(normalized_handle: &str) -> Result<(), HandleError> {
    // normalized = lowercased, no trailing '.', already charset-checked upstream
    for label in normalized_handle.split('.') {
        if label.starts_with("xn--") {        // ASCII-lowercased already → plain prefix check
            return Err(HandleError::PunycodeLabel);
        }
    }
    Ok(())
}
```

Enforcement topology — the point of the DD is **one gate, not three** (per memory `feedback_make_unsoundness_unreachable`: one shared enforced path beats per-site checks that drift):

```
  onboarding (ZMVP-30) ─┐
  resolution (ZMVP-44) ─┼──► Handle::try_new / normalize  ──►  [ xn-- reject ]  ──► claim
  reserved-label(45) ───┘            (the ONE shared seam — created by ZMVP-44)
```

## 🚀 8. Next steps

1. ⚠️ **Engineer decision (blocking, sequencing):** ZMVP-48 vs ZMVP-44 ordering. Three faithful options:
   - **(a) Sequence after ZMVP-44** — cleanest; ZMVP-44 builds `Handle` + the normalization seam, ZMVP-48 adds one rule + tests into it. Lowest collision risk, but ZMVP-48 is *blocked* until ZMVP-44 merges.
   - **(b) Co-develop in one worktree** — do ZMVP-44 and ZMVP-48 (and likely ZMVP-45) together, since they all converge on the same new file. Avoids merge churn.
   - **(c) Bootstrap minimal `Handle` here** — ZMVP-48 introduces the newtype (mirroring `AccountName`) with only the `xn--` rule; ZMVP-44 extends it. Unblocks ZMVP-48 now but risks divergence at the merge.
   *Recommendation to put to the Engineer: (b) or (a) — the DD itself frames the rule as "part of normalization shared by 30/44/45," which argues against building it in isolation.*
2. Once ordered: write the failing unit tests from §6 (red), then the `Handle` seam (if owned here) + the `xn--` rule (green).
3. Wire the RFC 9457 mapping at the claim site; add the integration test (or hand off with a failing test if the claim endpoint isn't built yet).
4. `/document` the new signatures; offer `/design-sync` to close the "Punycode/confusable" open item on DD 24870914.
5. **Collision watch** (for `/close-gaps`): same files as ZMVP-44 (creates `Handle` + seam), ZMVP-30 (onboarding handle choice, touches `account.rs`/`signin`), ZMVP-47 (write-gate on `create_account` call-site). Coordinate the seam ownership before parallelizing.

**Open questions / unknowns:**
- ⚠️ Does normalization happen in `domain` (a `Handle` newtype) or in the ZMVP-44 resolution adapter? The DD says "normalization" generically — Engineer/ZMVP-44 decides the home. Default per repo idiom: a `domain` newtype like `AccountName`.
- Which exact `urn:zurfur:error:*` code/type for the punycode rejection? (follows DD 23592962; pick during implement.)
- ZMVP-46 is referenced by the DD but not linked on the ticket — confirm its relevance during `/close-gaps`.
