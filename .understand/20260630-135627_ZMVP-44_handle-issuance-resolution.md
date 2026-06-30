# 🔎 Understanding ZMVP-44 — Zurfur handle issuance & resolution for `*.zurfur.app` (DNS/well-known + DID alsoKnownAs)

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-44 · **Generated:** 2026-06-30 13:56 · **Snapshot:** `.understand/20260630-135627_ZMVP-44_handle-issuance-resolution.md`
> **Parent epic:** ZMVP-13 *The Citizen (Accounts)* (In Progress) · **Priority:** Medium · **Relates:** ZMVP-30 (onboarding UX), ZMVP-45 (reserved labels), ZMVP-46 (handle-change flow), ZMVP-48 (punycode)

```
   handle (alice.zurfur.app)  ──resolve──▶  did:plc:…        ① Zurfur serves DNS/well-known
            ▲                                   │
            └────────── alsoKnownAs ────────────┘             ② Zurfur writes the back-link in the DID doc
                  (at://alice.zurfur.app)
              BOTH directions must agree, or the handle is not trusted
```

## 🧭 1. Context (cold-start)

This ticket is the **infrastructure half** carved out of DD *The Account Handle* (`24870914`), split from ZMVP-30's onboarding UX. An atproto **handle** is a domain name that resolves to a **DID** (the real, stable identity). Zurfur issues account handles under a domain it operates, `*.zurfur.app` (e.g. `alice.zurfur.app`), exactly as Bluesky issues `*.bsky.social`.

For a handle to be *trusted*, atproto requires **bidirectional verification** (verified against the live spec, atproto.com/specs/handle + /specs/did):
1. **handle → DID**: either a DNS TXT record at `_atproto.<handle>` with value `did=did:plc:…`, **or** an HTTPS `GET https://<handle>/.well-known/atproto-did` returning the **bare DID** as `text/plain`. (When both exist and disagree, **DNS wins**.)
2. **DID → handle**: the DID document's **`alsoKnownAs`** array must contain `at://<handle>` (exact format: `at://` + handle hostname, nothing else).

A handle is valid **only if both agree** — otherwise anyone could alias a third party's account.

**The load-bearing reality of this ticket:** writing direction ②'s `alsoKnownAs` requires signing a **did:plc operation** with a **rotation key** Zurfur controls. Today Zurfur mints only *synthetic, unregistered* `did:plc` values (a floor stub). So this ticket sits on top of a prerequisite — a **real PLC minter + key custody** — that **does not exist and is not yet designed** (the DID:PLC DD `4358151` explicitly leaves "PDS topology, key custody, recovery" as a *separate infra DD, status: in review*).

## 🗺️ 2. Domain

- **Account** (DESIGN/Account, `1966081`): the platform-custodied sovereign entity. Carries its own Zurfur-minted `did:plc`. **Has no handle field today** (`backend/crates/domain/src/elements/account.rs` — only `did`, `name`, timestamps).
- **Handle** (DD `24870914`): user-chosen at `POST /accounts`, never auto-derived. Two sources — Zurfur subdomain `<label>.zurfur.app` **or** bring-your-own-domain — **one mechanism**; the only difference is *who controls the DNS record*. The `did:plc` is **always Zurfur-operated**.
- **did:plc** (DD `4358151`, DECIDED): the sole DID method platform-wide. Identifier = `did:plc:` + first 24 chars of base32(sha256(signed genesis op)). Controlled by **rotation keys**; the `plc.directory` (Bluesky-operated) holds a transparent audit log. "Operating a did:plc for a user" = Zurfur builds/signs the genesis op and later updates, holding a rotation key.
- **Data boundary** (adapter-atproto = public/PDS side): serving handle resolution and writing `alsoKnownAs` is **public-boundary** work. Per the crate's invariants, the PLC write is a **dual write** — a separate retryable (outbox-style) step, never in the private unit of work.
- **Punycode** (DD `26050561`, ZMVP-48): reject `xn--` labels in both namespaces — handle *validation*, owned by ZMVP-48, consumed here.

## 🎯 3. Goal & scope

**Goal:** stand up the infrastructure that makes a Zurfur-issued `*.zurfur.app` account handle a *real, bidirectionally-verifiable* atproto handle — Zurfur serves the handle→DID lookup, and Zurfur writes/maintains the DID→handle `alsoKnownAs` back-link.

**In scope (per the ticket):**
- Serve `_atproto.<label>.zurfur.app` DNS TXT lookups **or** HTTPS `/.well-known/atproto-did`, resolving to the account's `did:plc`.
- Write/maintain `alsoKnownAs` on each account's DID document so verification is bidirectional.

**Out of scope (explicitly, → other tickets):**
- Onboarding UX / handle-choice step → **ZMVP-30**.
- Reserved-label list (`api`, `admin`, `www`…) → **ZMVP-45**.
- Punycode/confusable rejection → **ZMVP-48**.
- Handle-change flow (post-onboarding) → **ZMVP-46**.
- Handle *normalization/validation* rules themselves (spec charset, segment rules) — defined in the DD, implemented alongside ZMVP-30/48; this ticket *consumes* a validated handle.

**Scope ambiguity to flag:** the ticket presumes a real, registered `did:plc` you can write `alsoKnownAs` to. That **real PLC minter + key custody is a prerequisite this ticket does not own and is not yet decided** (see §8 ⚠️). As written against the current floor stub, ZMVP-44 can only deliver a *resolution endpoint over a synthetic DID that nothing actually resolves* — i.e. not the real bidirectional goal.

## 📦 4. Deliverables

- [ ] A **`Handle`** value on the `Account` (domain field + `accounts.handle` column/migration) — the persisted handle that resolution reads. *(Today: absent.)*
- [ ] **Handle → DID serving:** either an axum `GET /.well-known/atproto-did` route in `api` (Host-header → account → DID), **or** a DNS-zone integration for `_atproto.*.zurfur.app` TXT records — **strategy is an open fork (§8)**.
- [ ] **DID → handle write:** an `alsoKnownAs` writer in `adapter-atproto` that signs a PLC update setting `at://<handle>` — a domain **port** (e.g. `HandleBinder`/`AlsoKnownAsWriter`) + adapter impl, run as a **separate retryable dual-write step** (outbox-style).
- [ ] **Config** in `api`'s figment `Config`: the base handle domain (e.g. `handle_domain = "zurfur.app"`), PLC directory endpoint, and signing-key custody config. *(Today: only `env/http_addr/public_url/database_url/log_level`.)*
- [ ] **Real `did:plc` minter** (or its dependency landed first): keypair gen + signed genesis op + directory submission + key custody — currently `StubDidMinter` (synthetic). **Prerequisite, likely its own ticket/DD.**
- [ ] Tests: resolution endpoint returns the bare DID; bidirectional check (`alsoKnownAs` contains `at://<handle>`); `handle.invalid` tolerance.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Done |
|---|---|---|---|---|
| **Resolution strategy fork** — DNS TXT vs `/.well-known` for `*.zurfur.app` (+ wildcard-DNS vs per-host TLS) | 4 — *uncertainty/domain* | P0 (unblocks the rest) | 🧑 Engineer | ⬜ open fork; nothing in code |
| **Rotation-key custody & recovery model** — who holds the PLC rotation key; does the user co-hold one; run vs Bluesky `plc.directory` | 8 — *security blast-radius* | P0 | 👥 Group | ⬜ DD `4358151` lists this as a *separate infra DD, "in review"* — undecided |
| **Real `did:plc` minter** (keypair/genesis/sign/submit/custody) | 8 — *crypto + key custody* | P1 | 👥 Group | ⬜ `did_minter.rs` is a synthetic stub; "dress when The Who closes" |
| **`Account.handle` field + `accounts.handle` migration + repo plumbing** | 3 — *boilerplate, but collides* | P1 | 🤖 Claude (with Engineer on the column shape) | ⬜ `account.rs`/`accounts` table have **no handle**; collides w/ ZMVP-48/47 |
| **Handle→DID serving (well-known route)** — *if* well-known chosen | 3 — *mechanical once decided* | P2 | 🤖 Claude | ⬜ no `/.well-known/atproto-did` route in `api/src/lib.rs` |
| **DNS TXT issuance** — *if* DNS chosen (zone writes / wildcard) | 6 — *external infra + deploy* | P2 | 👥 Group | ⬜ no DNS integration anywhere |
| **`alsoKnownAs` writer port + adapter** (signed PLC update, outbox dual-write) | 6 — *depends on real PLC + keys* | P1 | 👥 Group → 🧑 Engineer | ⬜ only consumer-side `also_known_as` *read* exists (`profile.rs`) |
| **Config: base domain + PLC + signing-key** in figment `Config` | 2 — *mechanical* | P2 | 🤖 Claude | ⬜ absent from `dev.toml`/`Config` |
| **Tests** (resolution, bidirectional, `handle.invalid`) | 2 — *mechanical* | P2 | 🤖 Claude | ⬜ no handle-resolution test scaffolding |

> **Owner reality:** the *bulk by weight* is 🧑 Engineer / 👥 Group — the two highest-difficulty pieces (key custody, real PLC operation) are undecided domain/security forks, and the alsoKnownAs writer depends on them. Claude's genuinely-ownable slice (handle field, well-known route, config, tests) is real but **cannot deliver the ticket's actual value** until the forks resolve.

## ✅ 6. Test checklist (TDD)

- **Unit** — _asserts that_ `Handle` round-trips/normalizes per the spec charset and the `Account` carries it → ticket "resolving to the Account's did:plc"
- **Unit** — _asserts that_ the `alsoKnownAs` writer emits exactly `at://<handle>` (no extra URI parts) → ticket "alsoKnownAs handle binding"
- **Integration (well-known path)** — _asserts that_ `GET /.well-known/atproto-did` with Host `alice.zurfur.app` returns the bare `did:plc:…` as `text/plain`, 2xx → ticket "HTTPS /.well-known/atproto-did"
- **Integration (DNS path)** — _asserts that_ a TXT record `_atproto.alice.zurfur.app` = `did=did:plc:…` is provisioned for an account → ticket "DNS _atproto TXT"
- **Integration (bidirectional)** — _asserts that_ after issuance, handle→DID and DID-doc→handle agree (resolver accepts the handle) → ticket "verification is bidirectional"
- **Integration (dual-write)** — _asserts that_ the PLC `alsoKnownAs` write is a separate retryable step, not in the private UoW (no cross-store transaction) → adapter-atproto invariant
- **Edge** — _asserts that_ an account with no valid handle resolves to the `handle.invalid` sentinel without error → spec gotcha

## 🧠 7. Logic & shape

```
POST /accounts (ZMVP-30/47)            ── chosen handle ──▶  validate (ZMVP-48) + reserved (ZMVP-45)
        │                                                          │
        ▼                                                          ▼
  mint REAL did:plc  ◀── PREREQUISITE (today: synthetic stub)   persist Account{ did, handle }  [private UoW]
        │
        ▼  (separate retryable outbox step — NOT one unit of work)
  ┌─────────────────────────── ZMVP-44 ───────────────────────────┐
  │  ① publish handle→DID:  DNS TXT _atproto.<h>=did=…   OR   serve /.well-known/atproto-did
  │  ② write  DID→handle:  PLC update, alsoKnownAs = [ at://<h> ]   (sign w/ rotation key)
  └────────────────────────────────────────────────────────────────┘
        │
        ▼
  resolver: handle→DID == DID-doc→handle ?  →  trusted ✅   else  handle.invalid
```

The two writes are independent surfaces (a DNS/HTTP write and a PLC directory write); both must land for the handle to verify. ② is impossible without a rotation key Zurfur holds — which is the undecided custody fork.

## 🚀 8. Next steps

1. ⚠️ **BLOCKING — resolve the resolution-strategy fork (Engineer):** DNS TXT vs `/.well-known` for `*.zurfur.app` (and wildcard-DNS vs per-host TLS). Verified trade-off: DNS TXT is the conventional, lower-overhead choice for a provider that owns the parent zone (it's how Bluesky does `*.bsky.social`); well-known fits user-owned BYO domains. **Offer a DD** (or extend `24870914`).
2. ⚠️ **BLOCKING — rotation-key custody & recovery model (Engineer/Group):** who holds the PLC rotation key, whether the user co-holds one (portability off Zurfur), and whether to run against Bluesky's `plc.directory`. This is the *separate infra DD* that DD `4358151` lists as **"in review" — not decided.** Touches DID/handle correlation + credential custody → **will need `/security-review`.** This likely needs its own DD/ticket and **gates** the `alsoKnownAs` half.
3. ⚠️ **Prerequisite ordering:** the real `did:plc` minter must land before `alsoKnownAs` can be written to anything real. Confirm whether ZMVP-44 absorbs the real minter or depends on a separate ticket.
4. **Decide where the handle persists (Engineer):** add `Account.handle` + `accounts.handle` column. **Coordinate with ZMVP-48** (introduces the `Handle` type + punycode validation) and **ZMVP-30/47** (the `POST /accounts` write) — all three touch the same Account/handle surface + migration.
5. Once 1–4 are decided, Claude can take the mechanical slice: well-known route (if chosen), config fields, handle field plumbing, tests.

**Open questions / decisions needed:**
- ⚠️ DNS vs well-known (and DNS deployment ownership)?
- ⚠️ Rotation-key custody/recovery; user co-hold; PLC directory choice?
- ⚠️ Does ZMVP-44 include the real did:plc minter, or depend on a prerequisite ticket?
- Where does `Account.handle` live, and who lands the migration (ZMVP-44 vs ZMVP-48 vs ZMVP-30)?
- Cache lifetime for resolution results (impl choice; spec says don't blindly inherit DNS TTL).
