# ZMVP-106 — Capstone: a post (with blob) survives wipe-and-replay

`/understand` briefing · designer lane (Opus) · 2026-07-05 · READ-ONLY research
Epic **ZMVP-101** "The Twenty One" — this ticket **closes the epic**. Jira: In Progress, Medium, blocked-by ZMVP-105 (now merged → main `342a7a1`).
Worktree exists: `/home/zuri/code/zurfur-zmvp-106-wipe-replay-capstone` (branch `feature/zmvp-106-wipe-replay-capstone`, == main).

---

## 1. Cold-start context

The epic proved the atproto **write path** against a **local, wipeable PDS** in four waves, all now on main:

| Wave | Ticket | What landed (verified in code, this session) |
|---|---|---|
| 1 | ZMVP-102 | Dev-loop reference PDS: `docker-compose` (digest-pinned `ZURFUR_PDS_IMAGE`), `just pds-reset` (`down -v plc pds` → `up`), `just pds-smoke`, `scripts/pds-provision.sh`, hermetic `internal:true` net. |
| 1 | ZMVP-103 | `backend/crates/test-support/`: `ThrowawayPds` testcontainers fixture (tmpfs `/pds`, per-instance in-proc `StubPlc` via `host.docker.internal`, teardown-on-drop), `FixtureAccount{endpoint,did,handle,credential:ActingCredential::PdsSession{access_jwt,refresh_jwt}}`, `DEFAULT_PDS_IMAGE` drift-guarded == `.env.example`, hermeticity tripwire. |
| 1 | ZMVP-104 | `lexicons/app.zurfur.feed.post.json` (**FINAL** — required `createdAt`+`labels`; optional `text`/`embed`→`app.zurfur.embed.media`/`reply`/`credits`) + `feed.defs#credit` + vendored `com.atproto.repo.strongRef`. npm 12/12. |
| 2 | ZMVP-105 | domain `PublicRecords` port + protocol-free value types (`AtUri`/`StrongRef`/`BlobRef`/`RecordRef`/`FeedPost`/`PublicRecord`) in `domain/src/ports.rs` + `domain/src/elements/public_record.rs`; `AtprotoPublicRecords` (jacquard xrpc + raw-reqwest `uploadBlob` workaround) in `adapter-atproto/src/public_records.rs`; `MemPublicRecords`; **shared contract suite** `test-support/src/contract.rs` run vs both (atproto 4/4 on real `ThrowawayPds`, mem 3/3). Opus /security-review CLEAN. |

**The single most important finding of this research:** ZMVP-105's contract suite (`test-support/src/contract.rs`) **already proves AC1 and most of AC2 for a single run.** `blob_upload_is_content_addressed_and_referenced` + the adapter's `uploaded_blob_downloads_byte_identical` (`adapter-atproto/tests/public_records.rs:44`) already: upload TINY_PNG → embed it → `create_record` → `get_record` **field-identical** → raw `com.atproto.sync.getBlob` **byte-identical**, all against a real `ThrowawayPds`, with `created_at` **pinned** by `fixed_created_at() = 2026-07-05T12:00:00.000Z` (`contract.rs:34`).

So **106's genuine, non-existent-yet delta is exactly two things**: (a) run the identical flow **across a wipe** (a *second, provably-clean* PDS) and assert cross-run **determinism**, and (b) a **mechanical AC4** guard that the stored record carries no field outside the final lexicon. Everything else is composition of shipped, already-security-reviewed helpers. The ticket's self-description — "deliberately thin… composes ZMVP-102/103/104/105 rather than adding machinery" — is accurate.

---

## 2. Domain

- **The boundary.** `app.zurfur.feed.post` is **Class A** (atproto-native, born in the poster's own repo, PDS-canonical) — the boundary's "first one-way door" (Gallery Posts DD `29949954` §9; Boundary Contract `29622283`). The capstone exercises the *write half* of the private↔public boundary. Nothing private crosses: descriptive tags, `medium`, any `commissionRef`, and the Product itself are **Index-side / Class B**, never record fields (verified: the lexicon and `FeedPost` carry none of these).
- **Content-addressing is the domain's determinism substrate.** Multi-Creator duplication is resolved by **blob-CID dedupe** — "byte-identical blobs across PDSes render as one gallery item" (DD `29949954` §2). The capstone is, in effect, the executable proof of that guarantee: same bytes → same CID on a *fresh* PDS.
- **Maturity self-labels ride the record** (protocol norm: safety metadata travels with the blob). Vocabulary = atproto self-labels wholesale: Safe / Suggestive / Nudity / Adult + orthogonal Graphic; **"the label is derived from the rating — never chosen separately… required per work, server-side, blocking at publish"** (Maturity DD `29982722` §4). Safe = **empty** `values[]`. This last fact is load-bearing for fork F1 (below).
- **Publish-late doctrine.** A lexicon "publishes when its feature ships, never before" (Lexicon Registry `29818896` §3). The capstone **must NOT** publish the schema to `_lexicon.zurfur.app`; it writes *conforming records* to a throwaway repo. This is coherent: the reference PDS **does not validate custom `app.zurfur.*` lexicons** (105 finding, re-confirmed) — records are accepted as opaque, so conformance is Zurfur's to assert (→ AC4/F6), not the PDS's.

**This is a domain-adjacent, boundary-touching ticket, but a test, not a feature.** Under the epic-scoped delegation the conductor disposes forks F1–F6; the two that touch domain semantics (F1 publish-rules, F2 error-type) are flagged **Engineer-veto**.

---

## 3. Real goal & scope

**Goal:** make the epic's exit criterion *executable and green in CI* — "the full write path proven deterministic against a rig that holds no hidden state." Concretely: a valid image-bearing `app.zurfur.feed.post`, written through the **domain port** (not raw XRPC), read back **field- and byte-identical**, and shown to produce the **identical content-addressed result on a second, provably-clean PDS** — with a mechanical guard that the record shape is the final ZMVP-104 lexicon.

**In scope (the whole ticket):**
1. One new integration test that boots a `ThrowawayPds`, runs the flow, **drops it**, boots a *fresh* one, runs the **identical** flow, and asserts cross-run identity + determinism (AC1, AC2).
2. A mechanical **no-draft-fields** assertion against `lexicons/app.zurfur.feed.post.json` (AC4).
3. It runs under `cargo test --workspace`, unignored (AC3).
4. Opus /security-review (boundary — see §5 last row).

**Explicitly OUT of scope (scope guards — record these):**
- **No compose/publish API feature.** There is none in the repo (`grep` of `api/src` finds no publish/compose/feed handler; no `validate_for_publish`/`PublishViolation` exists). Do not build one.
- **No new record fields** (one-way door; `feed_post_field_set_UNIFIED` is frozen).
- **No lexicon registry publish** (publish-late).
- **No commit-CID / `rev` / AtUri equality assertions** (see F5 — those are instance-specific by design).
- **No overloading `PublicRecordsError::InvalidRecord`** with app-side publish violations (see F2).
- Do **not** exercise the ZMVP-102 `just pds-reset` dev-loop wipe from CI (see F4).

### 3.5 Decision forks — evidence + one recommendation each

The conductor holds epic-scoped domain-decision delegation and disposes these. F1/F2 are flagged **Engineer-veto** (they reinterpret 105's scope note and touch maturity-label semantics).

**F1 — Publish-rules ownership (Engineer-veto).**
*Evidence:* 105 deferred four app-side rules to "106": (1) ≥1 of text/embed, (2) conditional maturity label, (3) image ≤10 MB sub-cap, (4) non-blank alt. These live today only as **doc-comments** in `domain/src/elements/public_record.rs` (lines 251, 306, 329) that say *"enforced at the compose/publish layer, not here (ZMVP-106)."* **There is no compose/publish layer and no consumer** — `api/src` has no such handler; nothing in the epic (105 = faithful CRUD; 106 = round-trip test; 107 = burner smoke) consumes them. Critically, rule (2) is **un-buildable in 106**: Maturity DD `29982722` §4 says the label "is derived from the **rating** — never chosen separately," and a *rating* comes from commission/Product/compose context that does not exist here; a `FeedPost` value type just has a `labels` bag. Building a validator now = speculative machinery with only its own test as consumer, against the repo idiom (memory `feedback_traits_dependency_inversion`) and the ticket's "no machinery" mandate.
*Options:* (a) a minimal typed domain validator (`validate_for_publish(&FeedPost) -> Result<(), PublishViolation>`) exercised by the capstone; (b) re-defer all four to the (future) compose/publish feature, done **properly** — not silently.
**Recommendation: (b), executed cleanly.** Keep 106 a pure composition capstone (its ACs need no validator), and *discharge the "deferred to 106" debt* by: (i) correcting the three stale `(ZMVP-106)` doc-comments to point at the compose/publish feature (a successor ticket, not "106"); (ii) recording the publish-rules owner as an explicit open item (successor ticket or note on the future feature). If the Engineer instead wants the seam now, scope it to the **three self-contained rules (1/3/4) only** as a pure domain fn, and still leave (2) to compose — never invent rating context here.

**F2 — `InvalidRecord` producer (Engineer-veto, downstream of F1).**
*Evidence:* `PublicRecordsError::InvalidRecord` exists (`ports.rs:484`) and its doc explicitly says the enum **classifies the XRPC outcome, not app-side validation** ("The variants classify the XRPC outcome… deliberately not the auth transport"). It *does* have producers already — the `jac_did`/`jac_nsid`/`jac_rkey` structural-construction failures in the adapter map to it — so it is **not** producerless; it simply has no *publish-rule* producer.
**Recommendation:** regardless of F1, **do not** route app-side publish violations into `PublicRecordsError::InvalidRecord`. If a validator is ever built (F1-a), its failures are a **distinct type above the port** (`PublishViolation`), because the port's error enum is a transport/XRPC-outcome classifier by contract. For 106 as recommended (F1-b): **no new producer** — respect 105's faithful-CRUD port.

**F3 — Capstone test home.**
*Evidence:* the capstone composes the **real adapter** (`adapter-atproto`) + the **fixture/helpers** (`test-support`: `ThrowawayPds`, `TINY_PNG`, `fixed_created_at`, the `getBlob` byte-download pattern) + a repo-root lexicon file. That is exactly the surface of the *existing* real-PDS tests in `adapter-atproto/tests/public_records.rs`. `cargo test --workspace` already runs that dir in CI with a container socket (103/105 proven).
**Recommendation:** a **new file `adapter-atproto/tests/wipe_replay.rs`.** Not `test-support` (that's the harness, not a specific capstone), not a new crate (violates "thin"), not `api/tests` (no HTTP layer is exercised — there is no publish endpoint). Picked up by CI automatically → satisfies AC3 with **no CI YAML change.**

**F4 — "Wipe" mechanics.**
*Evidence:* `ThrowawayPds::boot()` already **is** a wipe primitive: each call creates a *fresh container* with **tmpfs `/pds`** (RAM-backed, gone on drop) and a **fresh per-instance `StubPlc`** — "per-instance state is what makes two booted PDSes provably share nothing" (`pds.rs:52`). `test-support/tests/throwaway_pds.rs::two_instances_never_observe_each_others_state` already demonstrates two isolated instances. The ZMVP-102 `just pds-reset` (`down -v plc pds` → `up`) is a *docker-compose dev-loop* affordance — interactive, not CI.
**Recommendation:** **fresh `ThrowawayPds` per run** — `boot()` → run flow → `drop()` → `boot()` → run identical flow, all inside one `#[tokio::test]`. This is *stronger* than a reset (a brand-new container + PLC shares literally nothing, vs a reset that could leave image/volume residue) and satisfies AC2's "no manual intervention between runs" by construction (it's one test body). Do **not** invoke `just pds-reset` from the test.

**F5 — Determinism assertion (verified this session).**
*Evidence (verified, not from memory — atproto Repository spec via web):* DAG-CBOR (DRISL) is the **canonical deterministic encoding for both signing and CID generation.** A **record CID** is the content hash of the *individual DAG-CBOR-encoded record value*; a **commit CID** is the hash of the whole commit object (root MST CID + monotonic `rev` TID + **signature**). Therefore, across two fresh PDS instances:
  - **Blob CID** — identical for identical bytes (content-address). Safe **hard** assert. (Note: the *real PDS* blob CID scheme differs from `MemPublicRecords`' raw-SHA-256 CIDv1 — irrelevant here; the capstone is atproto-only, so both runs use the real scheme.)
  - **Record CID** (`RecordRef.cid`, which the adapter takes from `createRecord`'s top-level `cid` = the **record** CID, `public_records.rs:114`) — identical **iff** the record bytes are byte-identical, which requires **pinning `created_at`** (the wire encodes `to_rfc3339_opts(Millis, true)`, `public_records.rs:511`). Verified-sound to assert.
  - **`rev` / commit CID** — *change every commit* (new per-instance signing key + monotonic rev). **Never assert.** (The port doesn't even expose them — good.)
  - **AtUri / rkey** — the repo mints a **fresh TID** each `createRecord`, so the rkey **differs** across runs. **Never assert equality** (optionally assert they *differ*, to document the boundary).
**Recommendation — layered assertion:**
  - *Primary (the ticket's literal AC1/AC2):* read-back `FeedPost` **field-identical** to written, on both runs; `getBlob` download **byte-identical** to `TINY_PNG`, on both runs.
  - *Strengthening (the "deterministic" the Context line asks for):* `blob_cid_run1 == blob_cid_run2` **and** `record_cid_run1 == record_cid_run2`.
  The record-CID equality is not a hopeful claim — it **encodes the determinism hypothesis as the pass condition.** If it ever fails, that is precisely the ticket's own signal ("if something still feels unproven… run ZMVP-107"), not a flaky test. Add a comment distinguishing record-CID from commit-CID/rev so no future reader "fixes" it by asserting the wrong hash.

**F6 — AC4 "no draft fields" — what is mechanically assertable.**
*Evidence:* the ledger notes "no mechanical draft-marker exists." But the **final lexicon JSON is on disk** at `lexicons/app.zurfur.feed.post.json`, reachable from the crate via `env!("CARGO_MANIFEST_DIR")/../../../lexicons/…` (confirmed this session — same depth `test-support` already uses for `.env.example`). Its `defs.main.record.properties` keys are exactly `{text, embed, reply, credits, labels, createdAt}`, required `{createdAt, labels}`.
**Recommendation — mechanical, not review-based:** in the flow, after `create_record`, do a **raw `com.atproto.repo.getRecord`** (like `throwaway_pds.rs` already does) and assert the stored `value` object's **top-level keys, minus `$type`, are a subset of** the lexicon's declared `properties` (and that required `createdAt`/`labels` are present). This exercises the **actual wire shape the PDS stored** (so it doesn't need the crate-private `WireFeedPost`) and turns "no draft fields" from a PR claim into a green test that a stray/draft field would break. Loading the lexicon from disk also transitively pins AC4 to the *final* file (a lexicon edit that drifted from the record would fail it).

---

## 4. Concrete deliverables

1. **`adapter-atproto/tests/wipe_replay.rs`** — the capstone `#[tokio::test]`:
   - a `run_capstone_flow(pds) -> FlowWitness` helper (provision `capstone.test` → `upload_blob(TINY_PNG)` → build a valid image-bearing `FeedPost` with pinned `created_at`, non-blank `alt`, `SelfLabels::safe()` → `create_record` **through the port** → `get_record` field-identical → raw `getBlob` byte-identical → raw `getRecord` for the AC4 field-subset check → return `{feed_post, blob_bytes, record_cid, blob_cid, uri}`);
   - the test body: `boot → flow → drop → boot → flow`, then cross-run assertions per F5.
2. **AC4 field-subset assertion** (inside that flow or a sibling `#[test]`/helper) loading `lexicons/app.zurfur.feed.post.json`.
3. **(F1-b) doc-comment correction** in `domain/src/elements/public_record.rs` (3 sites) redirecting the `(ZMVP-106)` promise to the compose/publish successor — **conditional on the Engineer's F1 call.**
4. **(possibly) `pub` on `test_support::contract::fixed_created_at`** (or a shared `const FIXED_CREATED_AT`) so the capstone pins the same timestamp instead of duplicating the literal.
5. Green local gate mirroring CI; Opus /security-review; PR to main.

*No production `src` change is required for the recommended path* (F1-b touches only doc-comments). This is a test-and-docs ticket.

---

## 5. Weighted work-breakdown

Difficulty 0–3. Priority: the ticket is Jira-Medium but is the **epic-closer**, so ship-priority is High. Owner: **Claude** unless flagged. Model per repo policy (memory `feedback_model_assignment_policy`): Sonnet builds mechanical; Opus for judgment/boundary/security; Haiku for trivial glue.

| # | Piece | Diff | Prio | Owner | Model + effort | Done — with evidence |
|---|---|---|---|---|---|---|
| W1 | Capstone `wipe_replay.rs`: boot→flow→drop→boot→flow + F5 layered assertions (AC1/AC2) | 2 | High | Claude | **Opus**, med — the record-vs-commit CID assertion is a protocol-correctness call where confident-and-wrong is expensive; small surface. (Sonnet-scaffold + Opus-review acceptable.) | Test compiles; passes locally twice-booting a real PDS (podman); read-back field-identical + `getBlob` byte-identical on both runs; `blob_cid`/`record_cid` equal across runs; `rev`/uri never asserted. |
| W2 | AC4 no-draft-fields subset check vs `lexicons/app.zurfur.feed.post.json` (AC4) | 1 | High | Claude | **Sonnet**, low (Opus if folded into W1) | Loads the final lexicon; raw `getRecord` stored `value` keys − `$type` ⊆ declared `properties`; required `createdAt`+`labels` present; a deliberately-injected stray field fails it. |
| W3 | Expose pinned timestamp / helper reuse from `test-support` | 0 | Med | Claude | **Haiku/Sonnet**, trivial | Capstone reuses one `fixed_created_at`/`FIXED_CREATED_AT`; no duplicated RFC-3339 literal; `cargo test -p test-support` green. |
| W4 | **F1 disposition** — *re-defer path:* correct 3 stale `(ZMVP-106)` doc-comments + record publish-rules owner. *validator path:* build pure `validate_for_publish` (rules 1/3/4). | 0 *(re-defer)* / 2 *(validator)* | Med | **FLAG Engineer-veto** | **Haiku** (doc edit) / **Opus** (validator — maturity semantics, boundary) | No dangling `(ZMVP-106)` promise; publish-rules ownership recorded in Jira/Confluence. Gated behind the Engineer's F1 call — do not execute before it. |
| W5 | **Opus /security-review** (boundary trigger — see below) | 1 | High | Claude | **Opus**, max | CLEAN, **cited not from memory**: nothing beyond the ZMVP-104 public field set crosses (W2 partly proves it); token stays quarantined (105 property, unchanged); credits-DIDs are the *decided* opt-out correlation surface, no regression. Run before PR. |
| W6 | Ship gates: fmt · clippy `-D warnings` · `cargo test --workspace` (incl. 2-boot capstone) · /critique · /document · /prepare-pr | 1 | High | Claude | **Haiku/Sonnet** glue + **Opus** /critique | Local gate mirrors CI green; PR opens targeting main; first commit `[ZMVP-106][uow:28ca4f]`; `.understand/` excluded. |

**Ordering weight (diff×prio):** W1 > W2 > W5 > W6 > W3, with **W4 gated on the Engineer's F1 decision** (do it first if it blocks doc-truth, but it does not block W1).

### Security-review determination (required call-out)
**YES — /security-review applies.** The change **touches the private↔public data boundary** (it is the executable proof of the public write path) and exercises **session/token handling** (the fixture `PdsSession` Bearer) and a **DID correlation surface** (`credits[]` DIDs). It adds **no new** boundary-crossing logic — it composes 105's already-CLEAN-reviewed adapter — so the review is **light**, but the Definition of Done ("Security-reviewed when it applies") and the repo posture (Opus does **all** security-review; memory `feedback_security_viability_over_speed`) make it mandatory. Focus: (1) the AC4 subset check genuinely bounds what crosses to the ZMVP-104 field set; (2) the token remains quarantined inside `adapter-atproto` (no leak via the new test); (3) no private fact (`commissionRef`/`medium`/tags/`snapshot`) appears in the written record. **If the Engineer chooses F1-a (build a validator), the review becomes heavier** — maturity self-labels are safety metadata on the boundary and rule (2) is boundary-semantic.

---

## 6. Layered TDD test checklist — every AC → a named test

Write these red-first against the recommended F-dispositions.

- **AC1** — *clean PDS: provision, write `app.zurfur.feed.post` with an image blob through the domain port, read back field- and byte-identical.*
  - `post_with_blob_round_trips_field_and_byte_identical` (the flow helper, asserted on run 1): `get_record` returns a `FeedPost` **==** the written value (field-identical, incl. embed cid/mime/size/alt/aspect); raw `com.atproto.sync.getBlob` bytes **==** `TINY_PNG` (byte-identical). *(Extends the pattern already green in `adapter-atproto/tests/public_records.rs::uploaded_blob_downloads_byte_identical` + `contract.rs::blob_upload_is_content_addressed_and_referenced`.)*
- **AC2** — *wipe + rerun identical flow green, no manual intervention.*
  - `post_with_blob_survives_wipe_and_replay` (the top-level capstone): boot pds1 → flow → **drop** → boot pds2 (fresh container + PLC, tmpfs — provably clean) → identical flow; assert **cross-run** field-identity (`witness1.feed_post == witness2.feed_post`), byte-identity (`witness1.blob_bytes == witness2.blob_bytes == TINY_PNG`), `witness1.blob_cid == witness2.blob_cid`, `witness1.record_cid == witness2.record_cid`. "No manual intervention" = the whole thing is one `#[tokio::test]` body. Optionally `assert_ne!(witness1.uri, witness2.uri)` to document the fresh-TID rkey boundary.
- **AC3** — *green in CI.*
  - No dedicated unit test: the capstone is a plain `#[tokio::test]` (no `#[ignore]`, no feature gate), so `cargo test --workspace` runs it on the container-enabled runner (proven by 103/105). **Evidence = the PR's CI `test` job green.** Guard against accidental exclusion: do not gate it behind an env flag.
- **AC4** — *lexicon is the final ZMVP-104 version, no draft fields.*
  - `stored_record_has_no_field_outside_the_final_lexicon`: load `lexicons/app.zurfur.feed.post.json`; extract `defs.main.record.properties` keys; raw `getRecord` the written record; assert stored `value` top-level keys **minus `$type`** ⊆ those properties, and required `{createdAt, labels}` present. (A stray/draft field, or a lexicon that lost a field the record still writes, fails it.)

Guard/negative behaviors (`Unreachable`, `Rejected`, delete→NotFound) are **already** covered by 105 and are **not** re-done here — 106 is the happy-path determinism capstone.

---

## 7. Pseudocode / diagram

```
#[tokio::test]  // adapter-atproto/tests/wipe_replay.rs
async fn post_with_blob_survives_wipe_and_replay() {
    let w1 = { let pds = ThrowawayPds::boot().await?; run_capstone_flow(&pds).await }; // pds dropped here → wiped
    let w2 = { let pds = ThrowawayPds::boot().await?; run_capstone_flow(&pds).await }; // fresh, provably-clean PDS

    // AC1/AC2 — literal criteria
    assert_eq!(w1.feed_post, w2.feed_post);          // field-identical read-back, both runs
    assert_eq!(w1.blob_bytes, TINY_PNG);             // byte-identical (run 1)
    assert_eq!(w2.blob_bytes, TINY_PNG);             // byte-identical (run 2, post-wipe)
    // determinism (strengthening) — F5, verified sound
    assert_eq!(w1.blob_cid,   w2.blob_cid);          // content-address stable across a wipe
    assert_eq!(w1.record_cid, w2.record_cid);        // DAG-CBOR record CID stable (createdAt pinned)
    // DO NOT assert w1.uri == w2.uri (fresh TID rkey) or any rev/commit CID (per-instance signature)
    assert_ne!(w1.uri, w2.uri);                      // documents the boundary
}

async fn run_capstone_flow(pds: &ThrowawayPds) -> FlowWitness {
    let acct  = pds.provision_account("capstone.test").await?;      // 103 seam
    let store = AtprotoPublicRecords::new(&acct.endpoint, jwt(&acct))?; // 105 adapter, Bearer
    let did   = Did::new(acct.did);

    let blob = store.upload_blob(TINY_PNG.to_vec(), "image/png").await?;   // AC1 blob
    let post = FeedPost {                                                   // valid, ZMVP-104-shaped
        text: None,
        embed: Some(Embed { blob: blob.clone(), alt: "capstone probe".into(), aspect_ratio: Some(1x1) }),
        reply: None, credits: vec![],
        labels: SelfLabels::safe(),          // empty = Safe (rule (2) not evaluated here — F1)
        created_at: FIXED_CREATED_AT,         // pinned → deterministic record CID (F5)
    };
    let rref = store.create_record(&did, &PublicRecord::FeedPost(post.clone())).await?; // THROUGH THE PORT

    let read = expect_feed_post(store.get_record(&rref.uri).await?);        // AC1 field-identical
    let dl   = raw_get_blob(pds, &did, &blob.cid).await;                    // AC1 byte-identical
    assert_no_draft_fields(raw_get_record(pds, &rref.uri).await);          // AC4 (F6): value keys ⊆ lexicon

    FlowWitness { feed_post: read, blob_bytes: dl, record_cid: rref.cid, blob_cid: blob.cid, uri: rref.uri }
}
```

```
   run 1                     WIPE                      run 2
 ┌─────────┐   drop(pds) → container+tmpfs+PLC gone  ┌─────────┐
 │ PDS #1  │  ───────────────────────────────────▶  │ PDS #2  │   (shares NOTHING)
 └────┬────┘                                         └────┬────┘
  same bytes / same pinned record                    same bytes / same pinned record
      │                                                   │
   blob_cid₁ ═══════════ assert_eq ════════════════ blob_cid₂     (content-address)
 record_cid₁ ═══════════ assert_eq ════════════════ record_cid₂   (DAG-CBOR determinism)
      uri₁   ─────────── assert_ne (fresh TID) ────────  uri₂
   rev/commit ── NOT compared (per-instance signature) ──
```

---

## 8. Ordered next steps

1. **Resolve F1 first** (Engineer-veto): confirm re-defer (recommended) vs build-validator. This decides whether W4 is a 3-line doc fix or a domain build, and whether W1's post construction stays pure. Everything else can proceed in parallel once F1 is set. *(Also offer the Engineer: capture the publish-rules re-home as a successor ticket / note — `/design-decision` is overkill; a Jira note suffices.)*
2. **`/start`** already effectively done (worktree + branch exist); transition stays In Progress. Drive skills against the worktree path per memory `feedback_skills_run_in_cwd_not_worktree` (security-review/prepare-pr act on the primary checkout — point them at the worktree).
3. **Red-first (W3→W2→W1):** expose the pinned timestamp (W3); write `stored_record_has_no_field_outside_the_final_lexicon` (W2); write the flow helper + `post_with_blob_survives_wipe_and_replay` (W1). Confirm they fail for the right reason before green.
4. **Green** locally: `cargo test -p adapter-atproto` (needs podman/docker socket — 2 PDS boots + the 60s health headroom each; expect a slower run). Verify the **emitted output**, not just exit status (memory `feedback_verify_command_output_not_exit_status`).
5. **W4** per the F1 decision (doc-truth: no dangling `(ZMVP-106)`).
6. **/critique** then **/document** the changed test/doc signatures.
7. **W5 — Opus /security-review** (boundary): CLEAN + cited before the PR (memory `feedback_close_review_loop_before_pr` — apply findings *before* opening).
8. **/prepare-pr** → PR to main, first commit `[ZMVP-106][uow:28ca4f]`, request Copilot review (complexity gate: boundary/security surface → yes). Triage via `/address-comments` post-PR.
9. **After merge:** integrate (sync main, `/cleanup` the worktree), then the epic's tail — **decide ZMVP-107 run-or-skip-and-close** using 106's outcome as the evidence (if 106 is green *and* confidence is high, skip 107 and close ZMVP-101; if green-but-something-feels-unproven, run 107 rung-2 first). This is the delegated call the ledger's `next_action` names.

---

### Appendix — key source coordinates (verified this session)
- Port + error enum: `backend/crates/domain/src/ports.rs:457` (`PublicRecordsError`), `:541` (`PublicRecords` trait).
- Value types: `backend/crates/domain/src/elements/public_record.rs` (`FeedPost:332`, `RecordRef:198`, `BlobRef:213`; publish-rule deferral comments at 251/306/329).
- Real adapter: `backend/crates/adapter-atproto/src/public_records.rs` (`create_record:90` returns `output.cid` = record CID at `:114`; `created_at` wire encode `:511`; raw `uploadBlob` `:180`).
- Harness: `backend/crates/test-support/src/pds.rs` (`ThrowawayPds::boot:51`, tmpfs+per-instance PLC), `src/fixture.rs` (`FixtureAccount`/`ActingCredential`), `src/lib.rs:70` (`DEFAULT_PDS_IMAGE`, drift guard `:240`), `src/contract.rs:24` (`TINY_PNG`), `:34` (`fixed_created_at`).
- Existing real-PDS tests to mirror: `backend/crates/adapter-atproto/tests/public_records.rs` (`uploaded_blob_downloads_byte_identical:44`); raw-XRPC getRecord pattern: `backend/crates/test-support/tests/throwaway_pds.rs:74`.
- Lexicon (AC4 source): `lexicons/app.zurfur.feed.post.json` — reach from crate via `env!("CARGO_MANIFEST_DIR")/../../../lexicons/…`.
- CI: `.github/workflows/ci.yml` — `test` job = `cargo test --workspace` (container socket ambient; no YAML change needed).
- DDs: Gallery Posts `29949954` (§9 field list, §2 CID dedupe), Maturity `29982722` (§4 rating→label), Lexicon Registry `29818896` (§3 publish-late), Boundary Contract `29622283`.
- F5 protocol fact (verified via atproto Repository spec): record CID = DAG-CBOR content hash of the record value (deterministic); commit CID/`rev` include signature + monotonic TID (per-instance). Source: https://atproto.com/specs/repository
