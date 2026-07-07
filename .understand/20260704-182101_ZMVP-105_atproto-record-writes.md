# ZMVP-105 — adapter-atproto writes and deletes records (blobs included)

- **Snapshot:** 20260704-182101 · grounded in code at `origin/main` `dd2e920`
- **Epic:** ZMVP-101 "The Twenty One" (the record write path against a local, wipeable PDS)
- **uow:** 28ca4f · **worktree:** `/home/zuri/code/zurfur-zmvp-105-atproto-record-writes` · **branch:** `feature/zmvp-105-atproto-record-writes`
- **Isolated ports:** pg 25093 · backend 25094 · proxy 25095 · **dev PDS 25096** · **local PLC 25097**
- **Status:** In Progress · **Type:** Task · **Priority:** Medium · **Blocks:** ZMVP-106 · **Blocked-by:** ZMVP-103 (done #94), ZMVP-104 (done #95)
- **recommended_model: Opus (Designer).** Security-nature (public↔private data boundary + auth fork + cross-persona DID correlation via `credits`) forces Opus by policy; the domain-modeling core is the **Engineer's lane to implement**. Mandatory `/security-review` (Opus) before the PR.

> **STOP GATE.** This briefing ends at the checkpoint. Two Engineer-owned domain forks (§8) must be decided before `/implement`: **(A)** the record-write **port must be defined** — it does not exist (the ticket's "behind the existing ports" is inaccurate); **(B)** the **dev-write auth fork** (Bearer/local-credentials vs Jacquard OAuth). Plus a scope fork (**C**, publish-rules) and value-type modeling (**D**).

---

## §1 — Cold-start context: where this sits

`adapter-atproto` is Zurfur's **public data boundary** — the only crate that speaks AT Protocol; the `jacquard` OAuth/atproto client (v0.12.0) is quarantined inside it so "nothing protocol-shaped leaks past this crate's surface" (crate docs; DESIGN/"Domains and Applications"). Today the crate does three things, all **identity/read**, none of them a repo record write:

- `AtprotoAuthenticator` (`src/lib.rs:65`) — real OAuth sign-in (PAR + PKCE + DPoP) via `jacquard_oauth::OAuthClient`, implements `Authenticator`.
- `AtprotoProfileSource` (`src/profile.rs:24`) — **unauthenticated** public read of `app.bsky.actor.profile` via `jacquard::client::BasicClient::get_record`.
- `RealDidMinter` (`src/did_minter.rs:36`) — `did:plc` genesis/tombstone/update, signed secp256k1, submitted to a **PLC directory** (not the PDS).

`AtprotoAuthStore` (`src/auth_store.rs`, ZMVP-12) is a Postgres-backed `jacquard_oauth::ClientAuthStore` — durable persistence for OAuth grants (in-flight PKCE/DPoP state + established DPoP-bound sessions). **Note on the ticket's "PgAuthStore remnant from The Who (ZMVP-7)":** there is no type named `PgAuthStore`; the only auth store is `AtprotoAuthStore`, attributed to **ZMVP-12** (not ZMVP-7). It is the OAuth-grant store and it is touched by this ticket **only if the OAuth fork arm is chosen** (§8-B). Under the Bearer arm, 105 does not touch it at all.

**Key structural fact:** DID minting writes to a PLC directory, and profile reads are unauthenticated — so **105 introduces the first *authenticated write to a PDS repo* in the codebase.** There is no existing pattern to copy for authenticated `com.atproto.repo.*` / `com.atproto.sync.uploadBlob` writes; the closest model is the unauthenticated read in `profile.rs`.

Deps now on `main` that 105 rides:
- **ZMVP-103 `test-support`** (`backend/crates/test-support/`): `ThrowawayPds::boot()` (real Bluesky reference PDS in a testcontainer, hermetic in-process stub PLC, teardown-on-drop) → `provision_account(handle) -> FixtureAccount { endpoint, did, handle, credential: ActingCredential }`. `ActingCredential` is a `#[non_exhaustive]` enum with today exactly `PdsSession { access_jwt, refresh_jwt }` (from `com.atproto.server.createAccount`). Its doc *explicitly* records 105's open auth fork and is non-exhaustive **so 105 need not pre-commit it**.
- **ZMVP-104 lexicons** (`lexicons/`): `app.zurfur.feed.post.json` (unified post/reply/shout; `required:[createdAt,labels]`; optional `text`/`embed`/`reply`/`credits`), `app.zurfur.embed.media.json` (blob + required `alt` + optional `aspectRatio`; `accept` mime allow-list; `maxSize` 100 MB), plus `feed.defs`, `graph.*`, and vendored `com.atproto.label.defs` / `com.atproto.repo.strongRef`.

---

## §2 — The real goal & scope

**Goal:** make the *write half* of the Class A/B boundary contract real — a domain caller writes/updates/deletes an atproto record (and uploads a blob) into the acting identity's repo on the **local, wipeable** PDS, through a domain port, with faithful round-trip fidelity and honest error surfacing; and prove `adapter-mem` satisfies the *same* port via a shared contract-test suite. This is the third "Covers" item of ZMVP-101 and the input to its exit criterion (a `feed.post` round-trip surviving wipe-and-replay, which is ZMVP-106).

**In scope (from the ACs):**
1. create / put / delete a record through the port → appears in the repo, reads back **field-identical**.
2. **blob upload** (2026-07-04 ruling): image-bearing record uploads its blob, record references it, blob reads back **byte-identical**.
3. delete removes the record.
4. PDS failure (unreachable / rejected write / invalid record) → **domain error, no panic, no silent success**.
5. `adapter-mem` satisfies the same port, proven by a **shared contract-test suite run against both adapters**.

**Out of scope / deferred:** Jetstream/read intake (ZMVP-100), the wipe-and-replay round-trip proof (ZMVP-106, which this blocks), real-network confidence check (ZMVP-107 candidate), Gallery UI, deployment PDS topology. **Contested scope (fork C):** the app-side *publish rules* the lexicons attribute to "ZMVP-105" — see §8-C.

---

## §3 — Concrete deliverables

1. **A new domain port** (does not exist — §8-A) in `domain/src/ports.rs` for public-boundary record writes: create / put / delete a record + upload a blob. Name/shape/signatures are the Engineer's call (DESIGN calls the role "PublicRecords").
2. **Domain value types** it speaks in (§8-D): whatever of {AT-URI, record CID / strong-ref, record key, collection NSID, the `feed.post` record shape, a blob handle} must be **domain-level and protocol-free**. Today the domain has only `Did` (opaque `String`, `elements/did.rs`) and `BlobId(cid::Cid)` (stub, `elements/blob.rs`); the atproto value types live only inside `adapter-atproto` via `jacquard`.
3. **A typed public-boundary error** (§8, error strategy): today every port returns `anyhow::Result`; the only typed domain error is `HandleTaken` (`ports.rs:280`, downcast to 409). AC4 wants failures distinguishable — decide whether a typed error (unreachable / rejected / invalid / too-large / not-found) is warranted or `anyhow` suffices.
4. **`adapter-atproto` implementation** of the port over `jacquard`'s `AgentSessionExt` (`create_record` / `put_record` / `delete_record` / `upload_blob`), authenticated per §8-B, quarantining every jacquard type behind the port.
5. **`adapter-mem` fake** (`Mem…`) of the same port — fidelity-not-realism, an in-memory repo of records + blobs.
6. **Shared contract-test suite** (AC5) run against BOTH `adapter-mem` and the real adapter (the latter driving `test-support::ThrowawayPds` + `FixtureAccount`). This suite does **not exist** and has **no precedent** in the repo (adapter-pg tests are per-file inline testcontainers, not a shared conformance suite) — its structure is a build choice.
7. **Lexicon-structural validation** of written records against `app.zurfur.feed.post` (per ACs "validated against the ZMVP-104 lexicons").
8. `/document` on changed signatures · `/design-sync` if a documented entity/flow changed · **mandatory `/security-review`** · `/critique` + `/close-gaps --post` before PR.

---

## §4 — Work-breakdown (difficulty 0–3 · owner · done-with-evidence)

| # | Piece | Diff | Owner | Done when (evidence) |
|---|-------|------|-------|----------------------|
| A | **Define the record-write port** (methods, generics, domain arg/return types) | 3 | **🧑 Engineer** (domain modeling) | trait compiles in `domain/ports.rs`; every method speaks domain types only |
| D | **Domain value types** (AT-URI / strong-ref CID / rkey / NSID / record shape / blob handle) — which move into `domain`, protocol-free | 3 | **🧑 Engineer** | types exist in `domain/elements`; no `jacquard` type in a domain signature |
| B | **Auth fork decision + wiring** (Bearer/CredentialSession vs OAuth) | 2 | **🧑 Engineer decides**, Claude wires | adapter obtains an `AgentSession` from `FixtureAccount.credential`; token never crosses the port |
| — | Error strategy (typed vs `anyhow`) | 2 | **🧑 Engineer** | AC4 failures map to the chosen error; test asserts each |
| C | **Publish-rules scope** (either-of / conditional maturity label / image ≤10MB / non-blank alt — in 105 or deferred?) | 2 | **🧑 Engineer** | ruling recorded; ACs vs lexicon reconciled (§8-C) |
| E | adapter-atproto CRUD over `AgentSessionExt` (create/put/delete/upload_blob) | 2 | 👤 Claude (Opus) | integration test: record + blob round-trip field/byte-identical on ThrowawayPds |
| F | Error mapping (PDS unreachable / rejected / invalid → domain error) | 2 | 👤 Claude (Opus) | tests force each failure; no panic, no `Ok` on failure |
| G | adapter-mem fake of the port | 1 | 👤 Claude | contract suite green against mem |
| H | Shared contract-test suite (generic over the port; run vs both adapters) | 2 | 👤 Claude | one suite, two adapters, both green |
| I | Lexicon-structural validation of `feed.post` writes | 1–2 | 👤 Claude | invalid record → error (AC4); valid round-trips |
| J | `/document`, `/critique`, `/close-gaps --post`, `/security-review` | 1 | 👤 Claude (Opus) | gates green; security-review clean |

Per `feedback_engineer_implements_domain_work`: A, D, B-decision, C, error-strategy are domain-knowledge-heavy → **Engineer's lane to build/decide**; E–J are the mechanical execution lane once the port shape is fixed (Claude/Opus, given security nature). Claude must not choose the port shape, value types, error model, or publish-scope to keep momentum.

---

## §5 — Ownership bands

- **🧑 Engineer (decide + likely implement):** A (port), D (value types), B-decision, error strategy, C (scope). These shape entities/invariants and the boundary contract — every one is a domain fork.
- **👤 Claude / Opus (mechanical, after the forks resolve):** E (CRUD mechanics), F (error mapping), G (mem fake), H (contract suite), I (structural validation), J (gates + security-review).
- Nothing is Claude's to *decide*; the entire "shape" surface is the Engineer's.

---

## §6 — Layered TDD test checklist (headline)

Contract-suite tests (generic `<T: PublicRecords>` — run against `adapter-mem` fake **and** `adapter-atproto` + `ThrowawayPds`):

1. **create → read-back field-identical** — write a `feed.post`, read it from the repo, assert every field equal (AC1).
2. **blob upload → referenced → byte-identical** — upload image bytes, embed the returned blob ref in a record, fetch the blob back, assert bytes equal (AC2).
3. **put/update** — overwrite an existing record at its key; read-back reflects the update.
4. **delete → gone** — delete via the port; a subsequent read is a clean not-found (AC3).
5. **PDS unreachable → domain error** (atproto only) — point at a dead endpoint; `Err`, no panic (AC4).
6. **rejected write → domain error** — e.g. write to a repo the credential can't act in / malformed → `Err` mapped, not `Ok` (AC4).
7. **invalid record → domain error** — a record that fails `feed.post` structural validation → `Err` before/at write (AC4 + deliverable 7).
8. **mem ≡ atproto** — the *same* suite passes on both adapters (AC5) — the suite existing and running twice IS the AC.

adapter-mem-only fidelity tests: delete-then-read is not-found; put is idempotent; blob CID is content-addressed (same bytes → same id).

*(If fork C lands "in scope", add: either-of text/embed rejected-when-both-absent; mature-without-label rejected; image >10MB rejected; blank alt rejected.)*

---

## §7 — Port sketch + auth flow (illustrative only — the Engineer owns the real shape)

```rust
// domain/src/ports.rs  — role name/shape ENGINEER-OWNED (DESIGN: "PublicRecords")
#[async_trait]
pub trait PublicRecords: Send + Sync {
    // acting identity = the repo owner's Did; how the adapter authenticates as it is internal.
    async fn put_record(&self, acting: &Did, record: /*domain record*/) -> anyhow::Result</*StrongRef*/>;
    async fn delete_record(&self, acting: &Did, at: /*AtUri or (collection,rkey)*/) -> anyhow::Result<()>;
    async fn upload_blob(&self, acting: &Did, bytes: &[u8], mime: &str) -> anyhow::Result</*BlobRef*/>;
}
```

`jacquard` write surface (verified in `jacquard-0.12.0/src/client.rs`): `AgentSessionExt` (`impl<T: AgentSession + IdentityResolver>`) provides `create_record` (:707), `put_record` (:1082), `delete_record` (:1037), `upload_blob` (:1151), `get_record` (:784). **These methods are identical regardless of which `AgentSession` backs them** — so the auth fork is fully isolated to *how the adapter gets its session*:

- **Bearer/local:** `jacquard::client::credential_session::CredentialSession` — `access_token()` returns `AuthorizationToken::Bearer(access_jwt)`; maps 1:1 onto `ActingCredential::PdsSession { access_jwt, refresh_jwt }`.
- **OAuth:** `jacquard_oauth::OAuthClient::callback(...)` yields a DPoP-bound `OAuthSession` (also an `AgentSession`), persisted via `AtprotoAuthStore`.

**Invariant (security, my lane):** the PDS credential/session lives *inside* the adapter and must never appear in a port signature or leak past the crate boundary (mirrors `project_auth_surfaces_plugin_trust_csrf` — plugins never touch the PDS credential). The port speaks `Did` + domain records only.

---

## §8 — Engineer-owned decisions (the checkpoint)

### 8-A — The record-write port does not exist → 105 must DEFINE it *(fork)*
The ticket says "behind the existing ports," but no record-write/`PublicRecords` port exists in `domain` (exhaustive search: the 13 ports are Database, UnitOfWork, UserWrites/Store, Authenticator, ProfileSource, ProfileCache, AccountStore/Writes, CommissionWrites, DidMinter, KeyStore, PlcOperationLog). Defining the port — its methods, whether it is generic over record types or focused on `feed.post`, its arg/return domain types — is domain modeling = **the Engineer's call, likely the Engineer's to implement** (`feedback_engineer_implements_domain_work`). Recommend offering a **DD** if the shape sets a lasting boundary contract (it does). DESIGN refs: Data Boundaries `10354698`, Lexicon Registry `29818896`, Gallery Posts DD `29949954`.

### 8-B — Dev-write auth fork: Bearer/local-credentials vs Jacquard OAuth *(fork — my framed recommendation below)*
**The question:** how do dev writes authenticate to the local throwaway PDS?

| | **Bearer / `CredentialSession`** (local credentials) | **Jacquard OAuth** (`OAuthClient` + DPoP) |
|---|---|---|
| Source of the session | `ActingCredential::PdsSession { access_jwt }` the 103 seam already vends (from `createAccount`) | full PAR→authorize→**consent**→callback handshake, headless, against the reference PDS |
| Integration cost *this ticket* | **low** — construct a `CredentialSession`, done | **high** — must drive the PDS login+consent page programmatically to obtain a code |
| Exercises the record/XRPC surface (105's actual subject) | **yes, fully** — `com.atproto.repo.*` + `uploadBlob`; the CBOR/CID bytes are identical | yes |
| Exercises the *production* write-auth (DPoP binding, token lifecycle) | **no** | yes |
| Touches `AtprotoAuthStore` (ZMVP-12) | no | yes (this is where it "surfaces") |
| Security delta on a throwaway localhost PDS | none — the seam doc says these tokens "protect nothing durable" | none (same) |

**Recommendation (propose; Engineer disposes): choose Bearer/`CredentialSession` for ZMVP-105**, keep the port auth-agnostic, and *explicitly own the coverage gap*:
1. It is exactly what the 103 seam vends (`ActingCredential::PdsSession`) — zero new plumbing, no headless consent scripting.
2. It exercises 105's *actual subject* — the record write/blob XRPC surface — at full fidelity. The bytes that the ACs check (record CBOR round-trip, blob CID) are identical whichever session authenticates the call (`AgentSessionExt` is transport-agnostic).
3. `ActingCredential` was made `#[non_exhaustive]` *precisely* to keep this fork open — picking Bearer now does **not** foreclose adding an `OAuth` variant later.
4. The OAuth path's *extra* surface (DPoP proof-of-possession, PAR, refresh) is **auth-transport**, already proven for sign-in (ZMVP-12), and its real exercise is the explicit job of the epic's Rung-2 / **ZMVP-107** real-network check.

**Honest cost of that recommendation (do not hide):** Bearer does **not** prove the *production* write path — production writes should ride the same DPoP-bound OAuth session as sign-in, and that write-with-OAuth path stays **unproven until ZMVP-107** (or a dedicated follow-up that wires `OAuthSession` into `put_record`). Mitigation: (a) keep the port auth-agnostic so swapping the session type is a one-line adapter change; (b) write error-mapping against the **XRPC error**, not the auth transport, so it transfers; (c) log the OAuth-write coverage gap as a tracked follow-up so "writes work under Bearer" is never mistaken for "production write auth works." If the Engineer weights *production-fidelity-now* over *effort*, OAuth is the alternative — at materially higher integration cost for this dev-only ticket.

### 8-C — Publish-rules scope: ACs vs lexicon attribution *(fork)*
The lexicons annotate four app-side rules as "enforced ... at publish (**ZMVP-105**)": (1) at-least-one-of `text`/`embed`; (2) a mature work must carry the correct maturity self-label; (3) image sub-cap ≤10 MB (vs the 100 MB `maxSize`); (4) non-blank `alt`. **None of these appear in the ticket's ACs.** Architecturally the atproto adapter is the wrong home for app policy (esp. rule 2, which needs Index/Product knowledge of whether the work *is* mature — the adapter can't know that), and 105 *blocks* ZMVP-106 (the compose/round-trip layer). **Recommend:** keep 105 to faithful CRUD + **structural** `feed.post` validation + the five ACs; **defer the conditional business rules to the compose layer** (ZMVP-106 / a Product service). But the lexicon says otherwise — surface the conflict; the Engineer rules, and if deferred, `/design-sync` the lexicon annotations to point at the right ticket.

### 8-D — Which value types are domain vs adapter-quarantined *(fork, couples to 8-A)*
The port's signatures force a call on AT-URI, record CID / strong-ref, record key, collection NSID, the record shape, and a blob handle: which become **protocol-free domain newtypes** (`elements/`) and which stay inside `adapter-atproto` (jacquard). The architecture rule ("nothing protocol-shaped leaks past the crate") pushes toward domain newtypes; the domain has only `Did` + stub `BlobId` today. Engineer's modeling call.

---

## §9 — Ordered next steps

1. **Engineer decides §8-A (port shape) and §8-D (value types)** — ideally the Engineer drafts the trait + domain types (domain-heavy). Offer a DD for the boundary contract.
2. **Engineer decides §8-B (auth fork)** — recommendation: Bearer/`CredentialSession` + own the ZMVP-107 gap.
3. **Engineer decides §8-C (publish-rules scope)** and the error strategy.
4. Then `/implement` the mechanical lane (E–J): red tests from §6 → adapter CRUD over `AgentSessionExt` → mem fake → shared contract suite → error mapping → structural validation.
5. `/document` → `/critique` → `/close-gaps --post` → **mandatory `/security-review` (Opus)** → `/prepare-pr` (run from this worktree; skills default to the primary checkout — drive with `git -C` per `feedback_skills_run_in_cwd_not_worktree`).

**Definition of Done reminder:** ACs→green tests, gates green, docs, design-sync if a documented entity changed, `/close-gaps --post` clean, **security-review passed** (this ticket qualifies — public↔private boundary + auth + DID correlation), and **no decision was Claude's**.
