# Round 2 — ticket skeletons for epic "The Shell" (board: add / remove / re-cut)

Context from round 1 (you both reviewed the first skeleton; the Engineer ruled since):
- The CLI is a SECOND DRIVING ADAPTER beside `api`: `backend/crates/cli`, binary `zurfur`, links `domain` + `adapter-pg` + `adapter-atproto` IN-PROCESS. No HTTP, no protobuf contract, no bearer token. Needs `DATABASE_URL` + the same figment config as `api`.
- One-shot `clap` commands only in v1. REPL = later sugar over the same parser. `inquire` cut. Tables cut (pretty JSON default).
- Headless auth deferred: OAuth to the user's PDS needs a browser; the CLI spawns one and catches the redirect on a loopback port.
- Terminology: ports = traits in `domain`; adapters = crates outside it (driven: pg/atproto/mem; driving: api, now cli).
- Ownership: the Engineer builds every domain operation command (41 of them: account ×12, commission ×26, …). Claude builds ONLY: the composition extraction, the cli scaffold/boilerplate, `health`, `whoami`, `logout`, `login`.

Facts about the code as it stands (verified today):
- `domain` = elements + ports only. There is NO use-case/application layer; orchestration (load → authorize → UnitOfWork → write → outbox) lives in ~5.2k lines of `api/src/routes/**` handlers. A second driver cannot reach it.
- `Config::load()` (figment: `backend/config/<ZURFUR_ENV>.toml` + `DATABASE_URL` + `ZURFUR_*` env) and `AppState` (bag of `Arc<dyn Port>`s: auth, users, profile_source, profile_cache, did_minter, accounts, commissions, changelog, files, database, pool) live in `api/src/lib.rs`; the wiring of live adapters (~60 lines) is in `api/src/main.rs`.
- Auth today: `domain::ports::Authenticator { start(handle) -> auth_url; complete(callback) -> Did }` implemented by `adapter_atproto::AtprotoAuthenticator::new(redirect_uri, pool, oauth_vault)` — the redirect URI is fixed at construction (`{public_url}/signin-callback`). OAuth grants persist in Postgres (`auth_store`, encrypted by a `SecretVault` derived from `ZURFUR_DID_KEY_ROOT_KEY`). The web session is a tower-sessions cookie backed by Postgres (`PgSessionStore`); `/me` resolves session → User (`UserStore::find`) → profile.
- atproto OAuth client metadata: the backend is a confidential web client with a published `client_id` URL. (Whether a loopback/localhost client_id is acceptable for the CLI, per the atproto OAuth spec's "localhost development client" rules, is UNVERIFIED — check it.)

## Proposed tickets (Claude's slice only; the Engineer's 41 operation tickets are NOT in scope for this round)

E. **Epic: The Shell — the CLI as a second driving adapter.** Exit: `zurfur login` → account → commission → elements/seats/slots/notes on the local dev stack with zero curl; no `api` handler orchestrates anything a use case doesn't.

T1. **composition: extract Config + AppState + live wiring out of `api`** into a crate both binaries link (working name `composition`). `api` keeps only axum: router, extractors, problem+json, session layer. Same env/config contract; `adapter-mem` wiring variant for tests. AC: `api` binary behavior unchanged (e2e suite green); a second binary can build the same `AppState` in ≤5 lines.

T2. **cli: crate scaffold + boilerplate.** `backend/crates/cli` → `zurfur`; `clap` derive tree with the module namespaces pre-declared (`account`, `commission`, `session`); global flags `--json` (compact) / default pretty JSON; stdout=data, stderr=diagnostics; stable exit-code classes (0 ok / 1 domain error / 2 usage / 3 infra); `clap_complete` zsh; tracing to stderr under `RUST_LOG`; `assert_cmd` black-box harness booting `AppState` over `adapter-mem` + a throwaway Postgres (testcontainers, as the rest of the workspace); `just cli -- …`; crate in fmt/clippy/test CI. AC: `zurfur --help`, `zurfur completions zsh`, harness green.

T3. **cli: `health`.** Reuses the same DB-reachability probe the HTTP `/health` uses (moved to the shared crate in T1). AC: exit 0 + JSON on reachable DB; exit 3 + problem on unreachable.

T4. **cli: `login` / `logout` / `whoami`.** `login`: bind `127.0.0.1:0`, build a per-run `AtprotoAuthenticator` with the loopback redirect URI, `start(handle)` → open URL via `webbrowser`, catch the single callback, `complete()` → DID, provision/recognize the User exactly as the web callback does (same use case, no fork), write an identity file (`$XDG_CONFIG_HOME/zurfur/identity` = DID + created-at; 0600; atomic write). `whoami`: read the identity file → `UserStore::find_by_did` → print User + profile. `logout`: delete the identity file (grant in `auth_store` untouched — ruling needed on whether logout revokes it). Security-nature → Opus builds, `/security-review` before the PR. AC: full loop against the local PDS rig (`test-support` fake PDS) in the harness; state mismatch, second callback, timeout, port-in-use, browser-launch failure each handled and tested.

T5. **ci/just: glue** — `just cli`, `just zurfur`, CI matrix includes the crate, `cargo deny` unchanged.

Open forks left for the Engineer (not for you to resolve; you may add to the list): (a) use cases extracted into `domain` vs a new `application` crate — sequenced under the Engineer's operation tickets, not here; (b) does `logout` revoke the PDS grant; (c) multiple identities (profiles) in v1 or one; (d) epic name.

## What to deliver (under 1200 words)
For each ticket: KEEP / CUT / SPLIT / MERGE / REWRITE, one paragraph why, and the concrete AC you'd add or strike. Then: tickets MISSING from Claude's slice (things the Engineer's operation tickets will trip over in their first hour if absent). Then: anything mis-owned (Claude vs Engineer vs Opus/security). Cite sources for empirical claims (atproto OAuth spec, RFC 8252, crates); mark UNVERIFIED otherwise. Be blunt; do not converge.
