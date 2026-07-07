# 🔎 Understanding ZMVP-103 — Integration tests boot a throwaway PDS

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-103 · **Generated:** 2026-07-04 14:49 · **Snapshot:** `.understand/20260704-144932_ZMVP-103_throwaway-pds-harness.md`
> **Epic:** ZMVP-101 "The Twenty One" (the atproto record write-path) · **Wave 1, test-rig lane** · **Blocks:** ZMVP-105

## 🧭 1. Context (cold-start)

The Twenty One (ZMVP-101) makes the **atproto record write path** real against a PDS we own — a **local, wipeable** one — before anything touches the public network. It has two Wave-1 rig lanes running in parallel over the *same container image* but *different lifecycles*:

- **ZMVP-102** — the **dev-loop** lane: the reference PDS joins Postgres in `docker-compose.yml`, brought up by `just`, wiped by one command.
- **ZMVP-103** (this) — the **test-rig** lane: grow the existing **testcontainers** harness so an integration test boots a **fresh, empty PDS container**, runs against it, and **destroys it** — the same per-test isolation the DB already enjoys, extended to the atproto boundary.

Ground truth on today's harness (verified in `backend/`):
- There is **no shared test-support crate**. Every integration test **inlines** `Postgres::default().start().await` and copy-pastes a private `fresh_store()`/`fresh_pool()` helper — e.g. `adapter-atproto/tests/auth_store.rs:22-37`, `adapter-pg/tests/account.rs:29`, `api/tests/session.rs:16`. One **container per test** (not per binary); the container handle is returned and held so it lives for the test, then `Drop` tears it down.
- `api/tests/common/mod.rs` is the *only* shared test module and it holds **assertion helpers only** (`assert_problem`), not container boot.
- Dependency: `testcontainers-modules = { version = "0.15", features = ["postgres"] }` is a **dev-dep in three crates** (`adapter-pg`, `adapter-atproto`, `api`). The core `testcontainers` crate (home of `GenericImage`, `runners::AsyncRunner`, `ImageExt`) is re-exported as `testcontainers_modules::testcontainers`. There is **no Postgres-style module for a PDS** — the PDS must be driven as a **`GenericImage`** (custom image, ports, env, readiness wait).
- CI (`.github/workflows/ci.yml`) runs `cargo test --workspace` on `ubuntu-latest` with **no `services:` block** — testcontainers uses the runner's ambient Docker socket. This already works (the Postgres tests are green), so adding a PDS `GenericImage` rides the same mechanism; **no CI YAML change is required** unless image pull time or network policy forces one.

## 🗺️ 2. Domain

- **The data boundary.** `adapter-atproto` is the **public** side (user-owned records on a PDS, AT-URI via DID); `adapter-pg` is the **private** side. (DESIGN *Data Boundaries* `10354698`; crate `adapter-atproto/CLAUDE.md`.) This ticket builds the *test rig* that lets the public-boundary adapter be exercised against a real PDS without touching the real network.
- **PDS (Personal Data Server).** The atproto host of a user's repo. Bluesky ships a **reference PDS as a container**; the atproto **dev-env** pattern (local **PLC** directory + PDS) is the documented fallback when standalone-PDS account provisioning fights back (ZMVP-102 notes). A PDS normally resolves handles/DIDs and reaches a **PLC directory** — the network surface this ticket must **seal off**.
- **Fixture account.** A pre-provisioned identity on the throwaway PDS that downstream tests *act as* — created via `com.atproto.server.createAccount` (or the dev-env's account bootstrap). Its **session/token + PDS endpoint** is the seam ZMVP-105 consumes.
- **Blocking Gaps for v1** (`9994307`): the one open blocking gap is **atproto Lexicon field lists** (closed by ZMVP-104), *not* this rig — but this rig is the harness those lexicon-validated records get tested through. This ticket carries **no unresolved design gap of its own**; it's infrastructure.
- **Settled invariant that shapes the rig:** *no cross-store transactions* — the write path is a dual write / outbox step. Not directly exercised here, but the harness must let ZMVP-105 test the atproto write in isolation from pg.

## 🎯 3. Goal & scope

**Goal:** extend the testcontainers harness so an integration test can **boot a fresh, empty PDS, act as a provisioned fixture account, and destroy the PDS** — with hard per-test isolation and zero reach to the public atproto network — and keep `cargo test --workspace` green in CI.

**In scope**
- A reusable helper that boots a **throwaway PDS `GenericImage`**, waits for readiness, exposes its mapped endpoint, and tears it down on drop (mirroring the Postgres pattern).
- A shared **fixture-account provisioner** that creates one account on that PDS and returns the **credentials/session + PDS URL** downstream tests use.
- **Network hermeticity**: the PDS is configured so nothing in the test can reach the public PLC/network (local-or-no PLC, offline-friendly config).
- Proof of **cross-test isolation** (two PDS instances never observe each other's state) and a **skeleton integration test** demonstrating boot→act→destroy.
- The **container-reuse-per-test-binary** escape hatch designed for (not necessarily switched on) — same lever the Postgres harness leaves available.

**Out of scope**
- The **docker-compose dev loop** PDS (that's ZMVP-102) — this lane does **not** touch `docker-compose.yml` or `just`.
- **Real record CRUD** through the port (ZMVP-105) — this ticket ships the *rig*, not the writes; the skeleton test proves the rig, not the adapter.
- **Lexicon field lists** (ZMVP-104) and any **public-network** confidence check (epic candidate, Rung 2).
- Blob-store specifics beyond what account provisioning needs.

## 📦 4. Deliverables

- [ ] A **PDS test-harness helper** (fn or small struct) that boots the reference-PDS `GenericImage`, waits for health, returns `{ endpoint_url, container_guard }`, tears down on drop.
- [ ] A decision + implementation on **where the helper lives** — new shared **`test-support` crate** vs. an inlined per-crate helper matching today's pattern (see §7; this is a real fork — the Postgres harness is currently *inlined everywhere*, so introducing a shared crate is a new convention).
- [ ] A **fixture-account provisioner** helper: creates one account on the booted PDS, returns the **session/token + acting identity (handle/DID) + PDS endpoint** — **the ZMVP-105 seam**.
- [ ] **Hermetic PDS config** (env for the `GenericImage`): local/absent PLC, no outbound resolution to public directories.
- [ ] Dev-deps wired: `testcontainers-modules`'s core `testcontainers` (for `GenericImage`) available where the helper lives; the PDS **image tag pinned in exactly one place** (coordinated with ZMVP-102 — see §8).
- [ ] **Isolation test** — two booted PDSes; a write/record visible in one is absent in the other.
- [ ] **Hermeticity test/assertion** — the harness cannot reach the public network (e.g. provisioning succeeds with no external PLC; a public-resolution attempt is not required for the fixture path).
- [ ] **Boot→act→destroy skeleton** integration test (green, and green in CI).
- [ ] Docs: a short note on how to write a PDS-backed integration test + how to flip on container-reuse if boot time hurts.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| PDS `GenericImage` boot helper (image, ports, **readiness wait**) | 5 — uncertainty: first PDS-in-container here; readiness/env unknowns | P0 | 👥 Group | — | ⬜ no PDS boot exists; only `Postgres::default()` inline (`adapter-atproto/tests/auth_store.rs:23`) |
| **Network hermeticity** (local/no PLC, seal outbound) | 5 — soundness: a leak silently makes every downstream atproto test non-hermetic | P0 | 🧑 Engineer | — | ⬜ not started |
| **Fixture-account provisioner → session/token seam** (the ZMVP-105 contract) | 5 — judgment: shapes a cross-ticket seam + auth path (OAuth vs local creds) | P0 | 🧑 Engineer | — | ⬜ not started; ZMVP-105 auth path noted "impl call inside 105" |
| Harness **placement** (shared `test-support` crate vs inline) | 3 — convention-setting, low code | P1 | 🤖 Claude | Opus 4.8 — sets a repo-wide test convention on the atproto boundary | ⬜ today's pattern is inline-per-file |
| testcontainers plumbing + dev-dep wiring (follow Postgres template) | 2 | P1 | 🤖 Claude | Sonnet 5 — mechanical, strong existing template | ⬜ core `testcontainers`/`GenericImage` not yet used |
| **Image-tag single-source** (coordinate w/ ZMVP-102) | 2 — cross-ticket drift risk | P1 | 🤖 Claude | Sonnet 5 — mechanical once the source-of-truth is decided | ⬜ |
| Isolation + hermeticity + boot→act→destroy tests | 3 | P0 | 🤖 Claude | Opus 4.8 — asserts the boundary/isolation guarantees the rig exists to give | ⬜ |
| CI stays green (verify, no YAML change expected) | 2 | P0 | 🤖 Claude | Sonnet 5 — verification against existing `cargo test --workspace` | 🟡 mechanism proven by Postgres tests; PDS pull time unverified |

**Recommended build model (ticket): Opus 4.8.** Rationale: the ticket touches the **public atproto boundary** and provisions a **fixture session/token** (two items on the `/security-review` trigger surface — auth + private↔public boundary), and its hardest pieces carry **real protocol uncertainty** (PDS-in-container, hermetic PLC) plus a **cross-ticket seam** whose shape is judgment. Policy safe-default is Opus and there is no positive evidence of triviality on the load-bearing pieces, so the ticket build-model = the most-capable any piece needs = **Opus**. The purely-mechanical Claude slices (plumbing, image-tag, CI verify) are Sonnet-appropriate *individually*, but they don't lower the ticket ceiling. **Not a Fable candidate** (security-nature surface bars it, and it's not a fully-specified long-horizon build).

## ✅ 6. Test checklist (TDD)

**Headline:** *boot a throwaway PDS, act as a fixture account, destroy it — with two instances provably isolated and nothing reaching the public network — and CI stays green.*

- **Integration** — _asserts that_ a test **boots a fresh empty PDS, gets a working endpoint, and the container is gone after the test (drop tears it down)_ → AC1.
- **Integration** — _asserts that_ the shared helper **provisions a fixture account on the throwaway PDS** and a caller can act as it (a session/token is obtained against that PDS) → AC2.
- **Integration** — _asserts that_ **two independently-booted PDSes never observe each other's state** (a record/account created in PDS-A is absent from PDS-B) → AC3.
- **Integration / Unit** — _asserts that_ the fixture flow **completes with no public-network dependency** (provisioning works against local/no PLC; no call resolves against the public directory) → AC4.
- **Meta / CI** — _asserts that_ `cargo test --workspace` is **green with the PDS harness present** on `ubuntu-latest`'s ambient Docker socket → AC5.
- **Seam (forward-looking, for ZMVP-105)** — _asserts that_ the provisioner returns **{ PDS endpoint URL, acting DID/handle, session or app-password/token }** sufficient to construct an authenticated atproto client — the contract ZMVP-105's adapter tests bind to.

## 🧠 7. Logic & shape

**The fork this ticket must decide (placement of the harness):**

```
Today (Postgres):  every test file ── inlines ──> Postgres::default().start()
                   (copy-pasted fresh_store() in ~15 files)

Option A — keep inline:  add a copy-pasted `fresh_pds()` per atproto test file.
           + matches current repo convention   − duplicated, drifts, seam re-declared per file

Option B — shared test-support crate:  crates/test-support/  (dev-dep)
           pub fn boot_pds() -> (PdsHandle, Guard)
           pub async fn fixture_account(&PdsHandle) -> FixtureSession {endpoint, did, token}
           + one home for the ZMVP-105 seam + hermetic config   − new crate/convention (Engineer's call)
```

Recommendation to surface (Engineer decides): **Option B for the PDS helper specifically**, because the fixture-account seam is consumed cross-ticket (105) and the hermetic-PLC config wants a single owner — duplicating either across files is exactly where a hermeticity leak or a seam-drift hides. The existing Postgres inline pattern can stay as-is; this need not retrofit it.

**Boot→act→destroy shape:**

```
#[tokio::test] boots ─▶ GenericImage(pds:pinned-tag)
                         │  env: hermetic (local/no PLC, no public resolution)
                         │  wait: until /xrpc/_health (or TCP) ready
       fixture ◀─────────┘
       provisioner ─▶ com.atproto.server.createAccount  ─▶ session/token
       test body  ─▶ (ZMVP-105 will write/delete here)
       drop       ─▶ container torn down (state gone)
```

## 🚀 8. Next steps

1. **⚠️ Decide the harness placement (Option A inline vs B shared `test-support` crate).** Engineer's call — it sets a repo-wide test convention and houses the 105 seam. *Blocks* clean implementation.
2. **⚠️ Pin the exact reference-PDS image + tag, in ONE source of truth shared with ZMVP-102.** Both lanes use the *same image*; if 102 pins it in `docker-compose.yml` and 103 pins it in Rust, they drift. Decide the single home (env-var default + a workspace test constant referencing the same tag is the cleanest). **Run `/close-gaps --pre` on the {102,103,104} set before pipelines diverge.**
3. **⚠️ Settle the fixture-account auth shape (the ZMVP-105 seam).** ZMVP-105 explicitly defers "OAuth (Jacquard) vs the PDS's simpler local credentials" as an "impl call inside 105" — but 103's provisioner must return *something* usable. Decide the minimum contract now: return **{ endpoint, DID/handle, app-password **or** session token }** and let 105 pick how it authenticates. Engineer to confirm the shape.
4. **Verify hermeticity is real, not assumed.** Confirm how the reference PDS avoids the public PLC in dev/offline mode (local PLC container vs `PDS_DID_PLC_URL` pointing nowhere) — cite the reference-PDS config, don't assert from memory. This is the AC most likely to pass silently while still leaking.
5. **Confirm CI headroom.** The Docker-socket mechanism is proven (Postgres tests are green); the unknown is **PDS image pull + boot time** under `ubuntu-latest`. If it hurts, flip on **container-reuse-per-test-binary** (the noted escape hatch) rather than changing CI infra.
6. Then TDD the §6 checklist: isolation + hermeticity + boot→act→destroy skeleton, red→green.

**Collision watch with ZMVP-102** (parallel-safe if these are honored): shared **image tag** (pin once — item 2); **`.env.example`** (102 owns the dev-loop PDS keys; 103 only adds a test-image override if needed — coordinate); **`docker-compose.yml`** (102-only; 103 must **not** touch it — testcontainers is programmatic); no shared **test-support crate** exists yet (if 103 creates one, 102 won't touch it — low risk). The **account-provisioning recipe** (`createAccount` call shape) is shared *knowledge*, not a shared file — expect two implementations (102 in dev scripting, 103 in Rust).

**Owner recommendation:** **👥 Group ticket, Engineer-led on the seam + hermeticity.** The three P0 load-bearing pieces — PDS boot config, network hermeticity, and the fixture-session seam consumed by 105 — are judgment/protocol/soundness calls that default to the **Engineer's lane** (they shape a cross-ticket contract on the public boundary). Claude runs the **mechanical lane in parallel**: testcontainers plumbing, dev-dep wiring, image-tag single-source, and the isolation/hermeticity/skeleton tests — all against the contract the Engineer fixes. Build model when Claude drives a slice: **Opus** for the boundary/convention/test-assertion slices, **Sonnet** for pure plumbing.

**Open questions recorded:** (a) reference-PDS image + tag and its offline/no-PLC config (needs a cited source before building); (b) does the fixture flow need a local **PLC** container too, or can the PDS run PLC-less for account creation?; (c) exact session/token shape returned to 105.
