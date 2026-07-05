# 🔎 Understanding ZMVP-103 — Integration tests boot a throwaway PDS

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-103 · **Generated:** 2026-07-04 15:37 · **Snapshot:** `.understand/20260704-153753_ZMVP-103_throwaway-pds-harness.md`
> **Epic:** ZMVP-101 "The Twenty One" · **Wave 1, test-rig lane** · **Blocks:** ZMVP-105 · **uow:** 28ca4f

## 📊 Since last snapshot

Prior: `.understand/20260704-144932_ZMVP-103_throwaway-pds-harness.md` (2026-07-04 14:49).

- **Status:** To Do → **In Progress** (transitioned at /start; worktree `~/code/zurfur-zmvp-103-throwaway-pds-harness`, branch `feature/zmvp-103-throwaway-pds-harness`).
- **All three §8 ⚠️ forks are now DECIDED** (Engineer interview, recorded in `.understand/parallel-set.json` decisions{}, updated 15:45):
  1. Placement → **Option B: new shared crate `backend/crates/test-support/`**, scoped to the PDS fixture seam ONLY (no Postgres-pattern retrofit). Auto-joins the `backend/crates/*` workspace glob — no root `members` edit.
  2. Image pin → canonical `ZURFUR_PDS_IMAGE` lives in `.env.example`/compose (**102 owns, lands first**); 103's Rust default duplicates the same tag literal **with a `#[test]` asserting equality** against `.env.example`. 103 must NOT touch `.env.example`/`docker-compose.yml`.
  3. Fixture seam → returns **{ PDS endpoint (host + mapped port), acting DID/handle, EXTENSIBLE acting-credential }** — an enum/opaque wrapper, NOT a bare app-password; keeps ZMVP-105's auth fork (Jacquard OAuth vs local creds) open.
- **Owner changes:** the three P0 pieces previously 🧑 Engineer / 👥 Group (boot config, hermeticity, seam) are now **🤖 Claude-built** — the Engineer disposed the judgment via interview; only faithful execution remains.
- **`recommended_model` change:** ticket build model Opus 4.8 → **Fable 5 (explicit Engineer opt-in — hardest Wave-1 build)**. The prior "not a Fable candidate (security-nature)" note is superseded by the Engineer's framing: this is **local dev/test infrastructure** (throwaway container, fixture test accounts), not product auth or the real data boundary. Mandatory **Opus /security-review before PR** is retained and is explicitly OUT of this build's lane (stop at green + /critique + /document).
- **Done movement:** none in code — no piece has moved past ⬜ (main HEAD unchanged at `43d6112`; 102's worktree exists but has no diff yet).
- **Net movement:** *decision-complete, zero build progress; everything is now buildable in one lane (this one).*

## 🧭 1. Context (cold-start)

The Twenty One (ZMVP-101) makes the **atproto record write path** real against a PDS we own — local, wipeable — before anything touches the public network. Two Wave-1 rig lanes share one container image with different lifecycles:

- **ZMVP-102** — dev-loop lane: PDS joins `docker-compose.yml`, driven by `just` (building now, Sonnet, in its own worktree).
- **ZMVP-103** (this) — test-rig lane: the **testcontainers** harness grows a PDS sibling to Postgres — boot a fresh empty PDS per test, act against it, destroy it.

Ground truth (re-verified 15:30 in this worktree, HEAD `43d6112`):
- Still **no shared test-support crate**; every integration test inlines `Postgres::default().start().await` + a private `fresh_store()`/`fresh_pool()` (template: `backend/crates/adapter-atproto/tests/auth_store.rs:22-37`). Container handle returned and held; `Drop` tears down.
- `testcontainers-modules = "0.15"` (dev-dep in `adapter-pg`, `adapter-atproto`, `api`; declared per-crate, NOT in `[workspace.dependencies]`) → locks **`testcontainers 0.27.3`** (home of `GenericImage`, `ImageExt`, `runners::AsyncRunner`), re-exported as `testcontainers_modules::testcontainers`. No PDS module exists in testcontainers-modules — drive a **`GenericImage`**.
- Workspace glob `members = ["backend/crates/*"]` (root `Cargo.toml:4`) — a new crate directory auto-joins.
- `.env.example` has **no `ZURFUR_PDS_*` keys yet** (102 lands them); it has the PLC placeholder `ZURFUR_PLC_DIRECTORY_ENDPOINT=http://localhost:2582` (commented) + `ZURFUR_PLC_DIRECTORY_SUBMIT=false`. Compose has only `db` + `proxy` — **no PLC container exists anywhere yet**.
- CI: `cargo test --workspace` on `ubuntu-latest`, ambient Docker socket, no `services:` block — mechanism proven by Postgres tests; **no YAML change**.
- `reqwest` already in-tree (0.12 in adapter-atproto, 0.13 in api; both in `Cargo.lock`).

## 🗺️ 2. Domain

Unchanged from prior snapshot (public/private data boundary; PDS = atproto host; fixture account = pre-provisioned identity downstream tests act as; no cross-store transactions). One sharpened point:

- **Hermeticity invariant (now settled, uow decision):** dev loop + test rig make **ZERO requests to public atproto / canonical plc.directory** — stated once, both lanes enforce the same guarantee. For this lane that means the throwaway PDS's identity/PLC surface must resolve entirely locally.
- **Open protocol mechanic to verify empirically (not a design fork):** whether the reference PDS's `createAccount` requires a *reachable* PLC directory endpoint to mint `did:plc`, or tolerates an unreachable one. Determines whether the harness needs a loopback stub PLC. **Verify by experiment against the container, not from memory.** Whatever the answer, the fixture flow stays hermetic (dead loopback address or in-test stub — never a public host).

## 🎯 3. Goal & scope

**Goal:** an integration test can **boot a fresh empty PDS, act as a provisioned fixture account, and destroy it** — per-test isolation, zero public-network reach, `cargo test --workspace` green in CI.

**In scope** (unchanged): `GenericImage` boot helper (readiness-wait, mapped endpoint, teardown-on-drop); fixture-account provisioner returning the 105 seam; hermetic PDS config; isolation + hermeticity + boot→act→destroy tests; container-reuse escape hatch *designed for* (not switched on); docs note.

**Out of scope** (unchanged): `docker-compose.yml`/`just`/`.env.example` (102 owns); real record CRUD through the port (105); lexicons (104); blob specifics beyond provisioning. **Also out (new):** `/security-review` and `/prepare-pr` — this build stops at green + /critique + /document; Opus reviews before the PR after `/close-gaps --post`.

## 📦 4. Deliverables

- [ ] **`backend/crates/test-support/`** — new crate (decided), scope = PDS fixture seam only.
- [ ] **PDS boot helper**: `GenericImage`-based boot → readiness-wait → endpoint (host + mapped port) → teardown-on-drop, modeled on the Postgres template.
- [ ] **Fixture-account provisioner** returning `{ endpoint, DID/handle, extensible acting-credential }` (the ZMVP-105 seam; credential = `#[non_exhaustive]` enum, not a bare secret string).
- [ ] **Hermetic PDS env**: local/no PLC, no public resolution; loopback stub PLC only if the experiment proves `createAccount` needs one.
- [ ] **Image-tag single-source discipline**: Rust default = same literal as 102's `ZURFUR_PDS_IMAGE`; `#[test]` asserts equality against `.env.example` (arms when 102's key lands — see ⚠️ in §8).
- [ ] **Isolation test** (two PDSes, no shared state), **hermeticity test**, **boot→act→destroy skeleton** — green locally and in CI.
- [ ] Docs: how to write a PDS-backed integration test + the container-reuse escape hatch.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| PDS `GenericImage` boot helper (image, ports, readiness) | 5 — first PDS-in-container here; env/readiness verified by experiment | P0 | 🤖 Claude | Fable 5 (Engineer opt-in) | ⬜ |
| Network hermeticity (local/no PLC, sealed outbound) | 5 — soundness; verify empirically, cite container behavior | P0 | 🤖 Claude | Fable 5 | ⬜ |
| Fixture-account provisioner → extensible-credential seam | 4 — shape DECIDED; execution + forward-compat test | P0 | 🤖 Claude | Fable 5 | ⬜ |
| `test-support` crate placement + wiring | 1 — DECIDED (Option B) | P1 | 🤖 Claude | Fable 5 | ⬜ |
| Image-tag single-source (mirror 102's literal + drift `#[test]`) | 2 — 102 in flight; test arms at merge | P1 | 🤖 Claude | Fable 5 | ⬜ |
| Isolation + hermeticity + boot→act→destroy tests | 3 | P0 | 🤖 Claude | Fable 5 | ⬜ |
| CI stays green (no YAML change; PDS pull/boot time watched) | 2 | P0 | 🤖 Claude | Fable 5 | 🟡 mechanism proven by Postgres tests |

**Build model (ticket): Fable 5** — Engineer opt-in per the Fable gate (`feedback_model_assignment_policy`), superseding the prior Opus recommendation; all forks decided, long-horizon fully-specified build, local test infrastructure (non-security lane). **Opus /security-review still mandatory before the PR — outside this build.**

## ✅ 6. Test checklist (TDD)

**Headline:** *boot a throwaway PDS, act as a fixture account, destroy it — two instances provably isolated, nothing reaching the public network — CI green.*

- **Integration** — boot a fresh empty PDS, get a working endpoint (health OK), and the container is **gone after drop** (endpoint stops answering) → AC1.
- **Integration** — the provisioner **creates a fixture account** on the throwaway PDS and a caller can **act as it** (authenticated XRPC call succeeds against that PDS with the returned credential) → AC2.
- **Integration** — **two independently-booted PDSes share no state** (the same handle provisions successfully on both; a session/identity on A is unknown to B) → AC3.
- **Integration** — the fixture flow **completes with no public-network dependency** (PLC surface resolves against a dead-or-stub loopback endpoint we control; positive assertion that identity publication landed locally, never at plc.directory) → AC4.
- **Meta/CI** — `cargo test --workspace` green with the harness present (ambient Docker socket) → AC5.
- **Seam (forward-looking, ZMVP-105)** — the provisioner returns `{ endpoint, DID/handle, acting-credential }` sufficient to construct an authenticated atproto client, and the credential type is **extensible** (`#[non_exhaustive]` — consumers must survive new variants) → 105 contract.
- **Drift** — `#[test]`: the Rust default PDS image literal equals `.env.example`'s `ZURFUR_PDS_IMAGE` (arms when 102's key lands).

## 🧠 7. Logic & shape

```
backend/crates/test-support/           (new; auto-joins workspace glob)
  src/lib.rs
    DEFAULT_PDS_IMAGE: &str            = same literal as 102's ZURFUR_PDS_IMAGE
                                         (env override ZURFUR_PDS_IMAGE honored at runtime)
    ThrowawayPds::boot() ─▶ GenericImage(pds@pin)
        env: hermetic (PDS_DID_PLC_URL → loopback-only; no appview/crawlers/report svc)
        wait: poll /xrpc/_health on the mapped port (bounded)
        ─▶ ThrowawayPds { endpoint: http://127.0.0.1:{mapped}, _container guard }
    pds.provision_account(...) ─▶ com.atproto.server.createAccount (invite-free, .test handle)
        ─▶ FixtureAccount { endpoint, did, handle, credential: ActingCredential (#[non_exhaustive]) }
    drop(pds) ─▶ container removed; state gone
  tests/ … boot→act→destroy · two-instance isolation · hermeticity · seam · image-drift
```

Escape hatch (designed-for, off): a per-test-binary `OnceCell`-style shared instance — same lever the Postgres harness leaves available; flip only if CI boot time hurts.

## 🚀 8. Next steps

1. **Experiment first (in this worktree): pull the pinned reference-PDS image; determine minimal boot env + whether `createAccount` needs a reachable PLC.** Everything protocol-shaped gets cited from observed container behavior, not memory.
2. Scaffold `backend/crates/test-support/` + red tests from §6.
3. Implement boot helper → provisioner → hermetic env to green.
4. `/critique` + `/document`, then **STOP** (no /security-review, no /prepare-pr from this lane).
5. ⚠️ **Image-literal seam (not a fork — a timing note):** 102 owns the canonical `ZURFUR_PDS_IMAGE` and hasn't landed it yet (its worktree has no diff at 15:30). 103 ships its Rust default + a drift `#[test]` that **asserts equality when the key exists in `.env.example` and self-reports as unarmed when absent**; `/close-gaps --post` must confirm the two literals match once 102's diff exists. If 102 lands a *different* literal before this branch merges, update `DEFAULT_PDS_IMAGE` to match — mechanical.

**Collision watch (unchanged):** 103 must not touch `.env.example` / `docker-compose.yml` / `Justfile` / `dev.toml` / `worktree-init.sh` (102 owns); root `Cargo.toml` `[workspace.dependencies]` edits allowed but avoided if per-crate deps suffice (also dodges merge conflicts); no CI YAML change.
