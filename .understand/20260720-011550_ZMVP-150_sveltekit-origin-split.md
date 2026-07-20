# 🔎 Understanding ZMVP-150 — The SvelteKit app boots in the dev loop behind the origin split

> **Status:** In Progress · **Source:** https://zurnetwork.atlassian.net/browse/ZMVP-150 · **Generated:** 2026-07-20 · **Snapshot:** `.understand/<ts>_ZMVP-150_sveltekit-origin-split.md` (saved after approval) · **Worktree:** `~/code/zurfur-zmvp-150-sveltekit-dev-loop` (branch `feature/zmvp-150-sveltekit-dev-loop`)

## 🧭 1. Context (cold-start)

Every epic so far shipped backend behavior; the only UI is The Who's bare sign-in surface. Epic **ZMVP-149 "The Other Side"** pulls the frontend toolchain at last — SvelteKit as the first real client of the API, a workbench, not a design pass. ZMVP-150 is "Roots for the other side": scaffold, serving topology, CI. **Nothing domain-shaped.**

Epic rulings (2026-07-19, recorded on ZMVP-149): one application, one origin; lives in the existing repo, one compose, one CI; same-origin so **no CORS/BFF**; Caddy strips `/api`, backend routes untouched; SvelteKit owns the sign-in form, axum keeps the OAuth callback; host-only `zurfur.sid` cookie (a `Domain=.zurfur.app` cookie would leak sessions into the handle namespace).

Unit rulings (ledger `45163`): **fresh scaffold in-repo** (`sv create`, current versions); the `~/code/zurfur-web` spike stays untouched as a reference crib, mined at ZMVP-151; **ZMVP-151 must not start until the Engineer merges 150**.

## 🗺️ 2. Domain

None — deliberately. This is serving topology + toolchain. The domain-adjacent edges it must not disturb:

- **Auth surfaces & CSRF** (DD 24543244): cookie surface guarded by `require_first_party_origin`; same-origin design, zero CORS in the codebase. The Caddy single origin is what keeps that true.
- **OAuth callback**: redirect URI is `{ZURFUR_PUBLIC_URL}/signin-callback` (`api/src/main.rs:56`), registered as the loopback client's sole redirect target. Read, not guessed, per the ticket note.
- **Handle resolution** (DD 26607618): `GET /.well-known/atproto-did` is Host-routed — must reach axum directly, never SvelteKit.

## 🎯 3. Goal & scope

Stand up the SvelteKit (TypeScript) app in-repo behind a Caddy origin split so that `just dev` boots postgres + axum + Caddy + both frontends, an in-app `fetch('/api/...')` works identically in browser and SSR, and CI gates the frontend.

**In scope:** scaffold, Caddyfile + compose service, `handleFetch` SSR rewrite + cookie forwarding, dev-loop/worktree wiring, CI job, config docs.
**Out:** any real screens (`/login` etc. — ZMVP-151+), design/components, mining the `zurfur-web` crib, `/plugin/v1` consumers, prod deployment topology, touching backend routes.

## 📦 4. Deliverables

- [ ] `frontend/web/` — fresh `sv create` SvelteKit + TypeScript app (Svelte 5, current versions), **adapter-node**, eslint + prettier + vitest, **yarn** (repo convention)
- [ ] `frontend/web/src/hooks.server.ts` — `handleFetch`: rewrite `/api/*` → internal axum origin with prefix stripped + forward the `zurfur.sid` cookie; one fetch code path both sides
- [ ] A crude proof page (root `+page.server.ts` load hits `/api/health`) demonstrating the one-code-path fetch
- [ ] `caddy/Caddyfile` + compose `caddy` service (default profile, replaces the nginx `proxy` profile; `nginx/` deleted) — `/api/*` → axum stripped; `/signin-callback`, `/.well-known/*`, `/plugin/v1/*` carve-outs → axum; else → SvelteKit dev server (ws/HMR passes through)
- [ ] Port re-topology: Caddy owns the public origin (dev `:8080` = `ZURFUR_PUBLIC_URL`, unchanged → OAuth redirect URI unchanged); axum moves to `127.0.0.1:8081` in `dev.toml`/`.env.example`
- [ ] Justfile: `dev-web` recipe, `dev` gains the third leg + Caddy, `setup`/`clean` extended, `gate` mirrors the new CI commands
- [ ] `scripts/worktree-init.sh`: allocate a web-dev port (`ZURFUR_WEB_PORT`), point `ZURFUR_PUBLIC_URL` at the **proxy** port (it currently points at axum — wrong under the split)
- [ ] `.github/workflows/ci.yml`: `web` job (svelte-check, lint, vitest, build) in the same flat job family (pattern: `24f2afe`)
- [ ] `.env.example` documents every new key; stays the single config reference

## 🧩 5. Work breakdown

| Piece | Difficulty (0–10) | Priority | Owner | Model | Done |
|---|---|---|---|---|---|
| SvelteKit scaffold at `frontend/web` | 2 — boilerplate breadth | P0 | 🤖 Claude | Sonnet — settled tooling, non-security | ⬜ no `frontend/web` exists |
| Caddy origin split + port re-topology | 3 — blast radius (every dev loop); ruling-settled boilerplate → stays Claude | P0 | 🤖 Claude | Opus — the carve-outs are the auth-surface boundary | ⬜ only nginx HMR passthrough exists (`nginx/nginx.conf`) |
| `handleFetch` rewrite + cookie forwarding + proof page | 3 — security-nature (session cookie) | P1 | 🤖 Claude | Opus — session-token handling | ⬜ |
| Dev-loop + worktree wiring | 2 | P1 | 🤖 Claude | Sonnet | ⬜ `just dev` has two legs today |
| CI `web` job + `gate` mirror | 1 | P1 | 🤖 Claude | Haiku — pattern-following (`24f2afe`) | ⬜ |
| Config docs (`.env.example`, `dev.toml`) | 1 | P2 | 🤖 Claude | Haiku | ⬜ |
| `/security-review` before PR | — (process) | P1 | 🤖 Claude | Opus — mandated for session/auth-touching changes | ⬜ |

**Ticket build model: Opus 4.8** — the two security-nature pieces pull the build up; Sonnet/Haiku take the mechanical pieces. No Fable gate (not a long-horizon build).

## ✅ 6. Test checklist (TDD)

- **Unit (vitest)** — _asserts that `handleFetch` rewrites `/api/x` to `{internal axum origin}/x` (prefix stripped)_ → AC3
- **Unit (vitest)** — _asserts that the rewrite forwards the incoming `zurfur.sid` cookie header, and forwards nothing for a cookie-less request_ → AC3
- **Unit (vitest)** — _asserts that non-`/api` fetches pass through untouched_ → AC3
- **Integration (scripted smoke via the Caddy origin, run in the dev loop)** — _asserts that `GET /api/health` → 200 from axum; `GET /` → SvelteKit HTML; `GET /.well-known/atproto-did` and `GET /signin-callback` reach axum (axum-shaped errors, not SvelteKit 404s)_ → AC2
- **CI** — _asserts that check/lint/test/build run as a `web` job on the same triggers; a red gate blocks merge via existing branch protection_ → AC4
- **Docs** — _asserts that every introduced key appears in `.env.example`_ → AC5
- AC1 (`just dev` boots everything) — verified live in the worktree (`just run`-style manual proof; ports 32786–32790 + web).

## 🧠 7. Logic & shape

```
                       one origin (dev: 127.0.0.1:8080 = Caddy = ZURFUR_PUBLIC_URL)
                    ┌──────────────────────────────────────────────┐
  browser ──────────►  Caddy                                       │
                    │   /api/*            → strip /api → axum :8081│
                    │   /signin-callback  → axum :8081  (carve-out)│
                    │   /.well-known/*    → axum :8081  (carve-out)│
                    │   /plugin/v1/*      → axum :8081  (fwd-compat)│
                    │   everything else   → vite dev :ZURFUR_WEB_PORT (ws/HMR ok)
                    └──────────────────────────────────────────────┘
  SSR (SvelteKit server) ── handleFetch: '/api/x' → http://127.0.0.1:8081/x
                            + forward zurfur.sid   (same '/api/…' code in app code)
```

Key coupling: `ZURFUR_PUBLIC_URL` must stay the **browser-visible** origin (Caddy), because it builds the OAuth redirect URI and feeds the CSRF Origin guard. Keeping Caddy on 8080 means the registered redirect URI doesn't change; axum vacates to 8081.

## 🚀 8. Next steps

1. Approve this plan → save the `.understand/` snapshot → `/implement` in the worktree (TDD order: vitest `handleFetch` tests first).
2. Slice-PR flow per the two-tier convention: integration branch `feature/zmvp-150-sveltekit-dev-loop` exists at main's tip; slices PR into it.
3. `/security-review` before the feature→main PR (session-cookie forwarding, origin carve-outs, `ZURFUR_PUBLIC_URL` coupling).

**Decisions folded into this plan (flagged; approval = ratification, edit if wrong):**
- ⚠️ **nginx retired**: delete `nginx/` and replace the compose `proxy` profile with a default-on `caddy` service — the ruling makes nginx dead weight; keeping both would leave two proxies claiming 8080.
- ⚠️ **Port re-topology**: Caddy takes dev `:8080` (public origin unchanged, OAuth redirect URI untouched); axum's `dev.toml`/`.env.example` bind moves to `127.0.0.1:8081`; `worktree-init` repoints `ZURFUR_PUBLIC_URL` at the proxy port and allocates a `ZURFUR_WEB_PORT`.
- `frontend/auth` (React) + `just dev-auth` stay untouched this ticket — still reachable directly on `:5173`; its retirement is ZMVP-151+ business.
- **adapter-node** over adapter-auto (self-hosted target; ticket delegates this to implementation), **yarn**, location **`frontend/web`** beside `frontend/auth`.
- `/plugin/v1/*` carve-out ships even though no route serves it yet (forward-compatible, per ticket wording).
