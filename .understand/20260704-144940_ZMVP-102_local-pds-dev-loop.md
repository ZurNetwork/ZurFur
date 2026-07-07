# 🔎 Understanding ZMVP-102 — The local PDS boots in the dev loop

> **Status:** To Do · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-102 · **Generated:** 2026-07-04 14:49 · **Snapshot:** `.understand/20260704-144940_ZMVP-102_local-pds-dev-loop.md`
> **Epic:** ZMVP-101 "The Twenty One" (the atproto record write-path) · **Wave 1**, parallel with ZMVP-103 (test-rig) / ZMVP-104 (lexicons); wires ZMVP-105.

## 🧭 1. Context (cold-start)

Zurfur straddles two data boundaries (Confluence *Data Boundaries* `10354698`): the **private** side (`adapter-pg`, Postgres, app-owned rows) and the **public/decentralized** side (`adapter-atproto`, user-owned records on a **PDS**, addressed by AT-URI via `did:plc`). Everything through ZMVP-100 lived on the private side or did *identity* work (OAuth, DID minting, PLC writes, handle resolution). **The Twenty One** makes the *record* write-path real — and it insists on doing so against a PDS **we own and can wipe**, before a single byte touches the public atproto network.

ZMVP-102 is **Rung 1 of that ladder** and the epic's literal foundation: stand up a **real reference PDS** (Bluesky's `pds` container speaking the real protocol) in the **dev loop** next to the existing dev Postgres, so a developer can `just up` and have a PDS to write records against. It is pure local infrastructure — nothing public, nothing permanent, one command to wipe.

Today this is **greenfield**: `docker-compose.yml` has only `db` (postgres:16-alpine) + an optional `proxy` (nginx, `profiles: [proxy]`); the `Justfile` has `up`/`dev`/`db-reset`/`_wait-for-db` but no PDS anything; `.env.example` documents Postgres + the `did:plc` minter keys but no PDS keys. Verified: `grep -rniE '\bpds\b|reference-pds|dev-env'` across compose/just/env/config returns nothing.

**One strong precedent already in tree:** `.env.example` ships `ZURFUR_PLC_DIRECTORY_ENDPOINT=http://localhost:2582` (commented, a deliberately *local* placeholder — never canonical `plc.directory`) and `ZURFUR_PLC_DIRECTORY_SUBMIT=false`. The ZMVP-49 minter (`adapter-atproto/src/plc.rs`, `did_minter.rs`) already targets that local endpoint. The atproto **dev-env** pattern bundles a **local PLC** on exactly that port — so 102's "local PLC" is very likely the endpoint the minter is already pointed at. That is an integration point, not a coincidence, and it doubles as the safety guard: the same `SUBMIT=false` / non-canonical-endpoint discipline is what keeps the dev loop off the public network.

## 🗺️ 2. Domain

- **PDS (Personal Data Server)** — the atproto server that hosts a repo of signed, content-addressed records for a DID. The *public data boundary* (`adapter-atproto/CLAUDE.md`). In Zurfur v1 production is **identity-only, no PDS** (DD 26935298, memory `project_pds_identity_only_v1`) — but that's *production*; this ticket stands up a **dev** PDS purely to exercise the record write-path locally.
- **Local PLC** — a local `did:plc` directory. The reference PDS mints/creates accounts whose DIDs resolve against a PLC; the dev-env pattern runs one locally on `:2582`. Ties to `did:plc` custody (DD 26804226, memory `project_didplc_custody`) and the existing minter.
- **Data Boundaries** (`10354698`): `Decentralized` = referential + public, whole-record signed `putRecord`; `Connected` = public record references private blob bytes. This ticket doesn't write records (that's ZMVP-105) — it provides the **server** those records will land in.
- **No cross-store transactions** (`adapter-atproto/CLAUDE.md`) — irrelevant to *booting* a PDS, but it is the reason the write-path needed a real PDS to test against at all.
- **Blocking Gaps for v1** (`9994307`): the last open blocking gap is **atproto Lexicon field lists** — that closes in ZMVP-104, *not here*. 102 is the rig that lets 104/105 be proven.

## 🎯 3. Goal & scope

**Goal:** `just up` (and `just dev`) brings up a wipeable, network-isolated local PDS reachable from the app, on which a test account can be provisioned and signed into, with every new config key documented in `.env.example` per the `ZURFUR_` conventions (env wins) — and a single documented command wipes it back to clean.

**In scope**
- A `pds` service (and, if the standalone PDS fights account provisioning, a `plc` sibling — the dev-env fallback the ticket sanctions) in `docker-compose.yml`, on the default profile, worktree-isolatable.
- `Justfile`: fold the PDS into `up`/`dev`, add a readiness wait, add a one-command **wipe** (extend `db-reset` or a sibling `pds-reset`), and a documented **provision + sign-in** path for a test account.
- `.env.example` + `backend/config/dev.toml`: new `ZURFUR_PDS_*` keys, coherent defaults, worktree host-port allocation.
- The **network-isolation guarantee** — nothing in the dev loop reaches the public atproto network / canonical `plc.directory`.

**Out of scope (explicit)**
- The **testcontainers throwaway-PDS harness** → ZMVP-103 (same image, different lifecycle).
- **Lexicon schemas** → ZMVP-104. **`adapter-atproto` record CRUD / blobs** → ZMVP-105. **Jetstream/read-path** → ZMVP-100.
- Any **production** PDS hosting/topology → Deployment epic (ZMVP-22); v1 stays identity-only (DD 26935298).
- Deciding the ZMVP-105 write-auth fork (Jacquard OAuth vs PDS local credentials) — 102 only needs to *provision + sign in*; it should surface which credential path it exposes, but the write-path auth call is 105's.

## 📦 4. Deliverables

- [ ] `docker-compose.yml` — a `pds` service (pinned reference-PDS image + digest), env-driven host port (`ZURFUR_PDS_HOST_PORT`, default e.g. 3000) for worktree isolation, a named volume for its data, on the **default** profile so `just up` starts it. Optional `plc` service if the dev-env fallback is taken.
- [ ] `Justfile` — PDS folded into `up`/`dev`; a `_wait-for-pds` readiness gate (mirrors `_wait-for-db`); a one-command **wipe** (`pds-reset`, or `db-reset` extended to `down -v` the PDS volume too); a documented `pds-provision` (or similar) that creates + signs into the test account.
- [ ] `.env.example` — new keys documented per conventions (`ZURFUR_` prefix, env-wins, secrets-only-here): PDS endpoint URL, PDS admin/invite secret, test-account handle + password, and the worktree-isolation managed block extended with `ZURFUR_PDS_HOST_PORT`. Reconcile with the existing `ZURFUR_PLC_DIRECTORY_ENDPOINT` (:2582).
- [ ] `backend/config/dev.toml` — a loopback PDS-endpoint default coherent with `.env` (the config key `adapter-atproto`/`api` will read to point at the PDS).
- [ ] `scripts/worktree-init.sh` — allocate a fourth stable per-worktree host port for the PDS (today it pins db/http/proxy), so parallel worktrees don't collide on the PDS port.
- [ ] (likely) `scripts/pds-provision.sh` / `pds-wipe.sh` — the provisioning + wipe mechanics the `just` recipes call.
- [ ] Docs — a short note (repo README or `adapter-atproto/CLAUDE.md` pointer) on the dev PDS + the "nothing reaches the public network" guarantee.
- [ ] A verification path (scripted smoke) proving: boot → provision → sign-in → wipe → clean.

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| **Choose PDS topology** — standalone reference PDS vs dev-env (local PLC + PDS); image + pin; how it relates to the existing `:2582` PLC placeholder | 5 — *uncertainty/domain fork* | P0 | 🧑 Engineer | — (domain/infra fork — propose+interview, Engineer disposes) | ⬜ greenfield; `.env.example` PLC `:2582` precedent is the only anchor |
| **`pds` (+ maybe `plc`) compose service** — image, ports, volume, default profile, env-driven host port | 4 — effort once topology decided | P0 | 🧑 Engineer | — | ⬜ compose has only `db`+`proxy` |
| **`just` PDS lifecycle** — fold into `up`/`dev`, `_wait-for-pds`, one-command wipe | 3 — mechanical, mirrors `_wait-for-db`/`db-reset` | P1 | 🤖 Claude | **Opus** — shares the network-egress-safety surface; not clearly non-security | ⬜ no PDS recipe |
| **Provision + sign-in a test account** — the thorny bit the ticket itself flags ("if provisioning fights back, fall back to dev-env"); yields a session/token | 6 — *uncertainty is the driver* | P0 | 👥 Group | — | ⬜ none; jacquard 0.12 (`loopback` feat) is the client |
| **`.env.example` + `dev.toml` keys** — new `ZURFUR_PDS_*`, reconcile with PLC endpoint, conventions | 2 — mechanical, high-clarity | P1 | 🤖 Claude | **Opus** — PLC-endpoint / credential / egress config is the `ZURFUR_PLC_DIRECTORY_SUBMIT` safety class | ⬜ env has PLC keys, no PDS keys |
| **worktree-init PDS port** — 4th stable per-worktree port | 2 — extends existing awk block | P2 | 🤖 Claude | **Sonnet** — isolated, mechanical, no security surface | ⬜ script pins 3 ports |
| **Network-isolation guarantee** — prove nothing reaches public atproto / canonical `plc.directory` | 4 — verification + judgment | P0 | 🧑 Engineer | — (safety invariant — Engineer signs off) | ⬜ relies on `SUBMIT=false` + local endpoint discipline |

**Ticket build model = Opus.** Two positive drivers pull the whole build up past the safe default: (1) the **no-public-network-egress** invariant is the same safety class as the existing `ZURFUR_PLC_DIRECTORY_SUBMIT=false` / non-canonical-endpoint guard — a stray submit to canonical `plc.directory` is exactly the failure it defends against; (2) the piece handles **dev credentials + a PDS session/token**. Neither clears the "positive evidence of triviality *and* non-security" bar for a downgrade. The purely-isolated worktree-port edit is the one genuinely non-security piece → Sonnet.

## ✅ 6. Test checklist (TDD)

> **Headline:** this is a **dev-loop / infra** ticket, so "tests" are **scripted smoke + isolation assertions, not `cargo` unit tests** — the *automated* PDS test harness (testcontainers, `#[tokio::test]`) is **ZMVP-103's lane**, deliberately. Keep 102's proof at the shell/`just` level; don't duplicate 103's harness here.

- **Smoke (scripted)** — _asserts that_ `just up` brings the PDS up healthy alongside Postgres and the app can reach it → AC1.
- **Smoke (scripted)** — _asserts that_ the documented provision command creates a test account and a sign-in returns a valid session/token → AC2.
- **Idempotence (scripted)** — _asserts that_ the wipe command returns the PDS to a clean state and a re-provision then succeeds (wipe is repeatable, not one-shot) → AC3.
- **Isolation (assertion)** — _asserts that_ the dev loop makes **no** request to the public atproto network / canonical `plc.directory` (PLC endpoint is local `:2582`, `SUBMIT=false`, PDS not federated) → AC4.
- **Config coherence (lint-style)** — _asserts that_ every new key used by compose/`just`/config is documented in `.env.example` with `ZURFUR_` prefix and env-wins semantics, and the worktree block carries the new port → AC5.
- **Worktree isolation (scripted, optional)** — _asserts that_ two worktrees each get a distinct PDS host port and don't collide → supports AC1 under parallel work.

## 🧠 7. Logic & shape

```
        docker-compose (default profile)                 Justfile
   ┌───────────────────────────────────────┐     up:  db + pds  → _wait-for-db
   │  db   (postgres:16-alpine, :5432)      │          → _wait-for-pds
   │  pds  (reference PDS, :3000)  ◄── app  │     pds-provision: create acct + sign in
   │  plc? (local PLC, :2582)  ◄─ minter    │     pds-reset / db-reset -v: wipe → clean
   │  proxy(nginx, profile only)            │
   └───────────────────────────────────────┘     .env.example: ZURFUR_PDS_* + PLC(:2582)
        volumes: pg_data, pds_data                 worktree-init: +1 stable port
                    │
                    ▼   ⛔ no route to public atproto / canonical plc.directory (AC4)
```

**Key seam already in tree:** the minter (ZMVP-49) points at `ZURFUR_PLC_DIRECTORY_ENDPOINT` (local `:2582`, `SUBMIT=false`). If 102 takes the dev-env fallback, that local PLC *is* this endpoint — reconcile, don't duplicate. If 102 takes the standalone-PDS path, decide explicitly whether the PDS carries its own PLC or reuses `:2582`.

## 🚀 8. Next steps

1. **⚠️ DECISION (Engineer, P0): PDS topology.** Standalone reference `pds` container vs the atproto **dev-env** (local PLC + PDS) bundle. Recommendation: **start standalone `pds`** (smaller, one service) and fall back to dev-env only if account provisioning fights back — exactly the ticket's own sequencing. Claude will research the current reference-PDS provisioning story (admin invite-code flow, `com.atproto.server.createAccount`) and bring a pinned image + a proposed provision command; **Engineer picks.** This shapes the dev loop everyone in the epic builds on → offer to record as a short DD if it sets lasting topology.
2. **⚠️ DECISION (Engineer, P0): reconcile with the existing `:2582` PLC placeholder** — does the dev PDS use it, or bring its own? Keep the `SUBMIT=false` / non-canonical discipline intact (AC4).
3. **Coordinate with ZMVP-103 BEFORE either lands** (`/close-gaps --pre` territory): 102 (dev) and 103 (tests) must agree on **one** image pin, **one** config key name for the PDS endpoint, and ideally **one** fixture-account provisioning routine (dev calls it from `just`, tests call it from the testcontainers helper). Divergence here is the top seam risk.
4. Once topology is decided, Claude builds the mechanical pieces (compose service block, `just` recipes + wait, `.env.example`/`dev.toml` keys, worktree-init port); Group tackles provision/sign-in.
5. Note for **ZMVP-105**: it consumes 102's wire — the PDS endpoint config key + the fixture account's session/token. 105 owns the write-auth fork (Jacquard OAuth vs PDS local creds); 102 must expose *a* working sign-in but need not settle that fork.

**Open questions (record, don't guess):**
- ⚠️ Reference-PDS image + pin, and whether a local PLC is bundled (topology decision above).
- Exact `ZURFUR_PDS_*` key names (endpoint, admin secret, test handle/password) — name once, shared with 103/105.
- Whether wipe is `pds-reset` (own recipe) or folded into `db-reset` (one "reset everything" command). Recommendation: a combined reset + a PDS-only one.
