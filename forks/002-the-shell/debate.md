# Fork 002 — The Shell (ad-hoc review: opinion + gaps; not scrutinize/debate mode)

Seats: GPT gpt-5.6-sol (web_search), Gemini gemini-3.1-pro-preview (google_search). No judge — the Engineer reads both. Tokens: GPT 22.7k in / 3.9k out; Gemini 1.6k in / 1.8k out.

## Brief
# The proposal — epic "The Shell": a first-party CLI/REPL over the Zurfur backend

This is NOT a debate and NOT a ranking exercise. The Engineer landed this epic skeleton in an ideation session and wants an independent, blunt review: what do you think of it, and what is MISSING (gaps, wrong assumptions, hidden forks, things that will bite later). Attack the framing too, not just the tickets.

## The skeleton (verbatim)

**Goal:** the Engineer uses Zurfur daily from a terminal, long before the web UI is complete. The backend *is* the domain; the CLI is its thinnest possible driver — one command per rpc in `contract/zurfur/api/v1`, nothing else. Doubles as a manual smoke of the API.

**Exit criterion:** sign in, create an account, open a commission, add elements/seats/slots/notes — end-to-end from the `zurfur>` prompt, with zero `curl`.

**Non-goals:** a TUI (that's another frontend); headless auth (no app-passwords); any domain logic in the CLI (if it needs a rule, the rule is missing from the backend).

**Shape:** crate `backend/crates/cli` → binary `zurfur`. `clap` subcommands, also usable one-shot from bash; `reedline` REPL with history + completion; `inquire` prompts for multi-field inputs. Talks JSON/HTTP through the generated prost types in `api/src/generated` (needs those types in a crate the CLI can depend on without pulling axum — split, or accept `api` as a lib dep; Engineer's call). Config: `ZURFUR_CLI_URL`, token file under `$XDG_CONFIG_HOME/zurfur`.

Tickets (module-sliced):
1. `cli`: crate scaffold, `clap` + `reedline` shell, `health`, `version` — mechanical
2. `api`: CLI session — `/signin?client=cli` + loopback callback → bearer token — Engineer; token family (cookie-BFF vs bearer) ruled in-ticket; security-review mandatory
3. `cli login` — spawn browser (`webbrowser`), catch callback on `127.0.0.1:0`, store token; `logout`, `whoami` (`/me`) — blocked by 2
4. Accounts: `account create/show/handle/members/invite/accept/transfer/leave` — mirrors `/accounts/*`
5. Commissions: `commission create/show` — blocked by ZMVP-163 (unmerged)
6. Commission composition: `element add/list`, `grant`, `maturity`, `note`, `seat`, `slot` — mirrors `/commissions/{id}/*`
7. Output: table default, `--json` raw contract, RFC 9457 problems rendered — cross-cutting
8. CI: `cli` in the workspace gates; `just cli` recipe — glue

Open for the Engineer: (a) crate boundary for the generated types, (b) token family for ticket 2, (c) epic name.

Rejected during ideation: a TUI (ratatui) — "would just be another frontend; REPL is CLI". A standalone Design Decision page for the CLI — "no reason".

## Project context (design record, condensed — treat as ground truth)

- Zurfur: AT Protocol-native art-commission platform. Rust ports-and-adapters backend (`domain` / `adapter-pg` private store / `adapter-atproto` public store / `adapter-mem` fakes / `api` axum composition root). Frontend: SvelteKit, server-side only (BFF with session cookie), no client→PDS path in v1. Pre-alpha, solo developer ("the Engineer").
- The API contract is an independent Protobuf corpus at repo root (`contract/zurfur/api/v1/*.proto`), authoritative over both tiers; JSON/HTTP is the v1 transport (not gRPC); Rust types generated via prost+pbjson into `backend/crates/api/src/generated` (inside the axum binary crate); TS via protobuf-es. Path-major bound to the proto package; additive-only; `buf breaking` in CI. Errors: RFC 9457 problem+json.
- Current HTTP routes on main: `/health`, `/me`, `/signin`, `/signin-callback`, `/logout`, `/accounts`, `/accounts/{id}`, `/accounts/{id}/handle`, `/accounts/{id}/invitations/accept`, `/accounts/{id}/members/me`, `/accounts/{id}/transfer`, `/commissions/{id}/{elements,grants,maturity,notes,seats,slots}`, `/.well-known/atproto-did`. `POST /commissions` + `GET /commissions/{id}` exist only on an unmerged branch (ZMVP-163).
- Auth: atproto OAuth (DPoP) handled entirely in the backend (`adapter-atproto`); the result is an HttpOnly SameSite=Lax session cookie for the web origin. CSRF = SameSite + Origin allowlist on cookie routes. A second, bearer-token surface `/plugin/v1` (per-plugin `app_key` exchanged for short-lived scoped tokens) is DESIGNED but not built. No CLI/native-app token family exists.
- Plugins: Golems (non-human actors with their own did:plc) and Portals (sandboxed renderer UIs). Plugins never touch the PDS credential.
- Writes to the private store go through a compile-enforced Unit of Work. Semantic style rulebook: newtypes for domain primitives, clarity over brevity.
- Work directive: slice units of work vertically by module (User, Account, Commission…), not by layer.
- Existing workspace deps include `webbrowser`, `clap`? (unverified), `reqwest` (unverified).

## Doctrine floor (binding)

1. Anti-domination: exit and voice preserved by construction; no lock-in.
2. The non-toxic path outranks optimization — a mechanic that hurts anyone's sanity is disqualified, not discounted.
3. Zurfur never holds funds absent an explicit ruling.
4. Consensus is not evidence.

You may challenge the doctrine explicitly; never ignore it.

## What to deliver (under 1500 words)

1. **Verdict** — is this epic worth doing as framed? One paragraph, blunt.
2. **Gaps** — numbered. Each: what's missing or wrong, why it matters, what you'd do. Prioritize by how badly it bites. Cover at least: auth/token design for a native client, the generated-types crate boundary, REPL vs one-shot ergonomics, testing strategy for the CLI, contract drift, and anything in the ticket list that is mis-sequenced or mis-owned.
3. **Hidden forks** — decisions the skeleton silently made that the Engineer should rule on explicitly.
4. **What you'd cut** — anything in the skeleton that is ceremony.
Cite sources (URLs) for any empirical claim about tools, RFCs, or crates; mark unverified claims UNVERIFIED. Do not converge for politeness.

## Seat: GPT
## 1. Verdict

**Worth doing, but not as framed.** A terminal client is useful for dogfooding and API pressure-testing, but this skeleton trivializes three substantial products: a native-client authentication surface, a reusable HTTP client, and a scripting interface. “One command per RPC, nothing else” is not thinness; it is an excuse to leave command semantics, compatibility, token lifecycle, and automation behavior accidental. Build the one-shot CLI first around a dedicated contract/client crate, rule native auth separately, and add a REPL only after repeated use proves it adds value. Otherwise “The Shell” will become an auth project wrapped in a mediocre command interpreter.

## 2. Gaps

### 1. Native-client auth is the architecture, not ticket 2

**Wrong:** `/signin?client=cli` plus a loopback callback is presented as an endpoint variation. It creates a new public OAuth client and credential family. It cannot safely reuse either the cookie-BFF session or the future plugin token exchange.

**Why it matters:** RFC 8252 requires native apps to use an external browser, treats them as public clients that cannot keep a secret, requires PKCE, and specifies loopback-IP handling—including arbitrary ephemeral ports and binding only to loopback. State validation, callback timeout, single-use codes, exact path matching, and closing the listener are not implementation trivia. ([rfc-editor.org](https://www.rfc-editor.org/info/rfc8252/?utm_source=openai))

“Bearer token” is also not a complete ruling. You must decide:

- access-token format, audience and scope;
- lifetime and refresh-token behavior;
- rotation, reuse detection and revocation;
- logout meaning: local deletion, server revocation, or both;
- whether tokens are bearer or sender-constrained;
- account/device/session listing and remote revocation;
- behavior when the browser cannot launch;
- whether one CLI authorization covers all accounts.

DPoP reduces replay risk by binding a token to a client-held key, whereas a bearer token works for anyone possessing it. That does not automatically make DPoP the right answer, but it makes “bearer versus cookie” an inadequate fork. ([rfc-editor.org](https://www.rfc-editor.org/info/rfc9449/?utm_source=openai))

**Do:** make native authorization a prerequisite design ruling and vertical implementation slice, independent of the CLI UI. Specify an authorization-code flow with PKCE and a narrow CLI audience. Test it with a fake authorization agent. Do not store long-lived bearer credentials in a plain XDG file by default; use an OS credential store where available, with an explicit file fallback and permissions model.

Also reconsider “no headless auth.” It excludes SSH, containers, and remote development—the environments where terminal clients are disproportionately useful. Device authorization is explicitly designed for browser/input-constrained devices, though it need not be v1. ([ietf.org](https://www.ietf.org/ietf-ftp/rfc/rfc8628.txt.pdf?utm_source=openai)) At minimum, rule whether headless use is permanently rejected or merely deferred.

### 2. Split generated models out of `api`; do not “accept” the dependency

**Wrong:** allowing the CLI to depend on the axum composition-root crate reverses the architecture and creates dependency leakage.

**Why it matters:** generated contract types are not API implementation types. The web server, CLI client, tests, and potentially migration tools need the same neutral package. Pulling `api` into clients invites axum/server configuration into downstream builds and makes future extraction harder.

**Do:** create something like:

- `zurfur-api-model`: generated prost/pbjson messages and contract-owned error types;
- `zurfur-api-client`: typed JSON/HTTP transport, auth injection and problem decoding;
- `api`: axum composition root;
- `cli`: command and presentation layer.

Generation must have one owner and a CI “generated tree is clean” check. Do not map contract messages through domain types merely to satisfy layering.

The phrase `--json raw contract` is ambiguous. If “raw” means exact server JSON, print the received response bytes. Parsing and reserializing through Protobuf can lose unknown fields in JSON workflows. ([protobuf.dev](https://protobuf.dev/programming-guides/proto3/?utm_source=openai))

### 3. “One command per RPC” is the wrong abstraction

**Wrong:** RPC declarations do not necessarily specify HTTP verbs, paths, path parameters, query encoding, headers, idempotency or pagination. The context does not establish whether the protos contain HTTP annotations: **UNVERIFIED**.

**Why it matters:** without authoritative transport bindings, every command will hand-code route knowledge and drift independently. Endpoint nouns also do not produce usable command semantics: `grant`, `maturity`, `note`, `seat`, and `slot` do not say whether they create, update, list, remove, or replace.

**Do:** either make HTTP bindings part of the contract or maintain one explicit route/client implementation with tests against the server. Commands should correspond to user operations, not mechanically to RPC names. Thin means “no domain rules,” not “no client abstraction.”

### 4. One-shot and REPL are different interfaces

**Wrong:** treating a REPL as a wrapper around `clap` ignores quoting, multiline notes, prompts, cancellation, history, state, and output redirection.

**Why it matters:** scripts require deterministic behavior; interactive use rewards defaults and context. `inquire` prompts must never appear merely because a required flag is absent in a non-TTY process. History may capture note contents, handles, invitation codes or accidental pasted tokens. Reedline supports history and completion, but Zurfur must define what is safe to persist. ([docs.rs](https://docs.rs/reedline/latest/reedline/?utm_source=openai))

**Do:** implement one-shot commands first. Define:

- flags versus stdin versus prompts;
- stdout for data, stderr for diagnostics;
- stable exit-code classes;
- `--no-input`, `--no-color`, timeout and pagination behavior;
- multiline/file input for notes;
- shell quoting rules;
- interruption semantics;
- history location, permissions and redaction.

Then have the REPL invoke the same parser/dispatcher. If it has no state such as current account or commission, it offers little over the user’s existing shell.

### 5. There is no credible testing strategy

**Wrong:** “manual smoke” is not a test plan.

**Do:** require four layers:

1. Parser/unit tests for every command and invalid argument combination.
2. HTTP-client tests against a mock server: methods, paths, headers, JSON, problems, timeouts and malformed responses.
3. Process-level tests asserting stdout, stderr and exit status.
4. End-to-end tests using the real API composition with `adapter-mem`.

Auth tests must cover state mismatch, PKCE failure, callback timeout, occupied ports, duplicate callbacks, browser-launch failure, token expiry, refresh rotation and revocation. CI cannot depend on a real browser or identity provider.

### 6. `buf breaking` does not prevent client/server drift

Buf compares Protobuf schemas for source, wire and JSON compatibility; it does not prove that axum routes implement them or that the CLI calls the correct route. ([buf.build](https://buf.build/docs/breaking/?utm_source=openai))

**Do:** add generated-code freshness checks, a server-route/contract inventory test, and black-box tests exercising every supported client operation. Decide version-skew behavior: minimum server version, unknown fields, unavailable operations, and whether `/version` advertises contract capabilities rather than merely a build string.

### 7. The sequencing is backwards

Output, transport and auth behavior are cross-cutting foundations, not ticket 7 cleanup. Ticket 6 is a grab bag, not a vertical module slice. Commission commands should not target an unmerged branch.

**Do, in order:**

1. auth ruling;
2. neutral model crate and typed client;
3. output/error/exit-code conventions;
4. one-shot `health`, login and `whoami`;
5. account vertical slice;
6. merged commission vertical slice;
7. composition modules individually;
8. REPL;
9. distribution and smoke automation.

“Engineer owns token family in-ticket” is not ownership; it is deferred architecture.

### 8. Configuration and credential isolation are underspecified

Bind credentials to the canonical server origin and token audience. Never silently send a production token after `ZURFUR_CLI_URL` changes. Rule HTTP versus HTTPS, redirects, proxies, custom CAs, multiple profiles and local development. Token writes need restrictive permissions, atomic replacement and symlink defenses.

### 9. Doctrine implications were ignored

Anti-domination argues for stable one-shot commands and machine-readable output, not REPL dependence. The non-toxic path argues against surprise prompts, plaintext credentials, and forcing browser access in every terminal environment. “No headless auth” is defensible for v1 only as an explicit capability exclusion with a fallback plan—not as a casual non-goal.

## 3. Hidden forks

1. Desktop-only human client versus SSH/container/automation client.
2. Opaque bearer, JWT bearer or DPoP-bound CLI tokens.
3. Refresh tokens versus repeated browser login.
4. OS credential store versus XDG file fallback.
5. Contract-declared HTTP bindings versus handwritten route mapping.
6. Exact response bytes versus normalized Protobuf JSON for `--json`.
7. Stable scripting API versus pre-alpha best-effort commands.
8. Stateless REPL versus current-account/current-commission context.
9. Static completion versus authenticated live completion.
10. Single endpoint versus named profiles and multiple environments.
11. Manual CLI releases versus packaged binaries/update mechanism.
12. Whether RFC 9457 extensions are preserved and machine-addressable; clients should not parse human-readable `detail` text. ([rfc-editor.org](https://www.rfc-editor.org/rfc/rfc9457.html?utm_source=openai))

## 4. What I’d cut

- **Cut the REPL from the initial epic.** Add it after the one-shot CLI survives daily use.
- **Cut `inquire` initially.** Flags, stdin and editor/file input are enough to expose the real command model.
- **Cut broad endpoint mirroring.** Implement only the stated end-to-end path plus `whoami` and logout.
- **Cut a handwritten `version` command** if clap’s generated version output is sufficient.
- **Cut `just cli`** unless it performs meaningful environment setup beyond `cargo run -p`.
- **Do not cut the design decision.** Rejecting a generic “CLI ADR” is reasonable; rejecting a native-auth ruling is not.
## Seat: Gemini
### 1. Verdict

The epic is fundamentally mis-scoped for a pre-alpha solo project. As framed, it conflates three distinct tools: a scriptable API test harness (one-shot `clap`), an interactive user environment (REPL + `inquire`), and a production-grade native client (OAuth loopback + token management). Building a bespoke auth surface and an interactive wizard to "smoke test" an API is a toxic path that prioritizes tooling architecture over shipping the domain. Strip it down to a stateless, one-shot CLI that uses static dev-tokens. The REPL and prompts are ceremony. 

### 2. Gaps

**1. Auth implementation is a massive, blocking distraction (Tickets 2 & 3)**
*   **The Problem:** Zurfur currently issues a SameSite session cookie for a web BFF. The proposal introduces a "loopback callback -> bearer token" flow. To achieve this, the backend must be modified to mint a new type of bearer token, execute an OAuth 2.0 native app flow (RFC 8252), and redirect to a local `127.0.0.1:0` web server spun up by the CLI. This is a massive yak-shave that blocks tickets 4, 5, and 6.
*   **Why it bites:** You are building a production native-app auth flow just so you can test your local API. This violates the non-toxic path doctrine by destroying velocity. 
*   **What to do:** Skip Ticket 2 and 3 for now. Implement a `--dev-token <account_id>` flag in the CLI and a matching bypass in the backend's `adapter-mem` and dev-mode `api` configuration that directly authenticates requests based on an injected header (e.g., `X-Dev-Account-Id`). When you do build real CLI auth later, use the already-designed `/plugin/v1` app-key mechanism, not a loopback cookie-proxying Frankenstein.

**2. The generated-types crate boundary is completely undefined**
*   **The Problem:** The API contract types (`prost` + `pbjson`) currently live inside `backend/crates/api`. If `backend/crates/cli` depends on `api`, it pulls the entire `axum` dependency tree, increasing compile times and bloat for a native binary.
*   **Why it bites:** CLI binaries must be lean. Coupling the CLI to the server framework makes the workspace compile graph a bottleneck.
*   **What to do:** Extract a `crates/contract-types` crate. It should depend *only* on `prost`, `serde`, and `pbjson`. Both `api` and `cli` depend on it.

**3. Interactive Prompts (`inquire`) vs. One-shot Execution**
*   **The Problem:** You want this to be usable "one-shot from bash" (e.g., in CI or scripts) but also want `inquire` prompts for multi-field inputs.
*   **Why it bites:** `inquire` requires a TTY. If a command prompts for missing fields via `inquire`, it will hang or panic in CI/bash scripts unless you meticulously wire up `--non-interactive` flags and validate that all required arguments were passed via `clap`.
*   **What to do:** Drop `inquire`. A CLI driver for an API should be strictly declarative. If a command requires a complex multi-field payload (like creating a commission), accept a JSON file path or a string (`--payload @file.json`). 

**4. Table Output Fallacy (Ticket 7)**
*   **The Problem:** "Table default, `--json` raw contract". 
*   **Why it bites:** Protobuf payloads representing aggregates (e.g., a Commission with nested Elements, Seats, and Slots) do not map to flat tables. You will waste days writing custom flattening logic for `cli-table` or `comfy-table` to make nested domain objects readable in standard terminal widths.
*   **What to do:** Default to pretty-printed JSON or YAML. Drop tables entirely. If you want readable terminal output, use something like `bat` or standard syntax highlighting. Do not build custom rendering logic in the CLI—it is a thin driver, remember?

**5. Lack of Automated CLI Testing**
*   **The Problem:** The epic describes this as a "manual smoke of the API" but has zero testing strategy for the CLI itself.
*   **Why it bites:** If this is your primary driver for the backend, regressions in the CLI will grind your workflow to a halt.
*   **What to do:** Add `assert_cmd` (https://docs.rs/assert_cmd/latest/assert_cmd/) to write black-box integration tests for the CLI binary against a running instance of the backend using `adapter-mem`. 

**6. Mis-sequenced Dependency Graph**
*   **The Problem:** Tickets 4 (Accounts) and 6 (Commission composition) are theoretically unblocked by domain logic, but in this epic, they wait for auth.
*   **What to do:** Rearrange. Build Ticket 1, then immediately build Ticket 4 using dev-tokens (Gap 1). Prove the CLI can parse JSON, send a request, and print the RFC 9457 error or success. Only touch auth (Ticket 3) when the CLI is fully functional.

### 3. Hidden forks (decisions made silently)

*   **String Parsing in the REPL:** The proposal states `clap` subcommands *and* a `reedline` REPL. How does a line typed into `reedline` get into `clap`? You cannot just pass a raw string to `clap`; it expects an iterator of arguments. You will need to parse shell quotes correctly (e.g., `zurfur note add "This is a single note"`). You must explicitly decide to pull in `shlex` to handle this tokenization safely, otherwise you will end up writing a broken string splitter.
*   **REPL Statefulness:** Does the REPL hold state? E.g., do I run `zurfur> use commission 123` and then `zurfur> element add`? The proposal says "one command per rpc" implying statelessness (`zurfur commission element add 123`). If it's strictly stateless, a REPL offers very little value over just staying in Bash/Zsh which already has history, completion, and environment variables.
*   **REST Paths vs RPC Commands:** The API contract is Protobuf, but the transport is JSON/HTTP REST (`GET /commissions/{id}/slots`). The CLI shape relies on `clap` subcommands (`commission slot list <id>`). You are silently committing to maintaining a manual translation layer between CLI flags/args and REST path/query parameters in the CLI codebase. Protobuf doesn't magically generate `clap` definitions. 

### 4. What you'd cut (ceremony)

*   **Cut `inquire` (prompts):** As stated in Gap 3, this is a distraction that breaks scriptability. Force all inputs through standard `clap` arguments or JSON files.
*   **Cut `reedline` (the REPL):** A REPL is a stateful environment. If your CLI commands map 1:1 to stateless API calls, you are just rebuilding Bash inside your binary. Let your system shell handle history and autocomplete (via `clap_complete`). A REPL directly conflicts with the "thinnest possible driver" goal. Just build a one-shot CLI first. If you truly feel the pain of typing `zurfur` repeatedly, add the REPL in v2.
*   **Cut Tables from Ticket 7:** Pretty-printed JSON (`--output=json-pretty`) is perfectly readable for an engineer and guarantees zero data loss or rendering bugs.
*   **Cut Tickets 2 & 3 (for now):** Do not build a custom native-app OAuth loopback for a dev tool before you even have a web UI working. Use static dev-tokens injected into the backend memory adapter. Real auth for the CLI can wait until the `/plugin/v1` app-key architecture is prioritized.
# Round 2 — ticket skeletons (add/remove/re-cut). Tokens: GPT 21.0k in / 3.3k out; Gemini 2.2k in / 1.1k out.

## Brief

## Seat: GPT

## Seat: Gemini

## Outcome
Epic ZMVP-199; tickets ZMVP-200 composition, 201 cli scaffold (T5 merged in), 202 health (kept on Engineer ask; Gemini CUT dissent recorded), 203 identity+principal+whoami/logout (board-added), 204 login (split; client-model fork A/B/C for the Engineer — verified against atproto OAuth spec), 205 application-layer landing zone (board-added; Engineer-owned fork domain vs application crate).

# Round 2 — ticket skeletons (add/remove/re-cut). Tokens: GPT 21.0k in / 3.3k out; Gemini 2.2k in / 1.1k out.

## Brief
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

## Seat: GPT
## T1 — **SPLIT**

This mixes three concerns: configuration parsing, dependency construction, and test wiring. Worse, exporting the current `api::AppState` merely canonizes a bag of ports before the application/use-case boundary exists. Call it `Runtime` or `Dependencies`; Axum should build its own `AppState` around that. Split into **T1a `config` extraction** and **T1b `composition` live builder**. Keep memory wiring in `test-support` or behind a test-only feature, not as a production composition variant. **Add AC:** config precedence and validation snapshot-tested; secrets absent from `Debug`/errors; `api` constructs its router from `Runtime`; adapters have deterministic shutdown; migrations are explicitly run or explicitly not run; API e2e behavior unchanged. **Strike:** “same `AppState` in ≤5 lines”—line count is not an architectural acceptance criterion.

## T2 — **REWRITE**

The shell contract is underspecified and the harness description is contradictory: a black-box `assert_cmd` child process cannot be handed an in-memory `AppState`; it starts from environment/config. Separate fast in-process parser/dispatch tests using memory adapters from process-level tests using Postgres and spawned fakes. `assert_cmd` is suitable for binary, environment, timeout, stdout, stderr, and exit-code assertions. ([docs.rs](https://docs.rs/assert_cmd/latest/assert_cmd/?utm_source=openai)) **Add AC:** every successful invocation emits exactly one JSON value plus newline on stdout; failures emit no stdout and one versioned problem object on stderr; no ANSI when redirected; `--json` placement works before or after subcommands; exit mapping has exhaustive tests; malformed config is exit 3, malformed arguments exit 2; `--help`/`--version` retain Clap behavior; Ctrl-C is handled without panic; completion output snapshot-tested. **Strike:** “booting `AppState` over adapter-mem + throwaway Postgres.” Replace with two distinct harnesses. `clap_complete` does support generated Zsh completion, so that part is fine. ([docs.rs](https://docs.rs/crate/clap_complete/latest?utm_source=openai))

## T3 — **REWRITE**

Do not put a health probe in `composition`; composition constructs dependencies, it should not become a miscellaneous shared-services crate. Extract a tiny DB health operation beside the application boundary or implement it on the database port. Also decide whether health means TCP/query reachability or schema readiness. **Add AC:** bounded timeout; cancellation; JSON schema includes `status` and optionally latency but never connection details; missing/invalid `DATABASE_URL`, timeout, authentication failure, and query failure all map to exit 3 with distinct machine-readable codes; API and CLI contract tests exercise the same operation. **Strike:** vague “problem” and “reachable DB.”

## T4 — **SPLIT, with a blocking feasibility ticket**

As written, this assumes that changing only `redirect_uri` creates a valid CLI OAuth client. It does not. The atproto localhost exception requires `client_id` origin `http://localhost` with no port/path, produces a **public native client**, permits loopback IP redirect URIs whose ports are ignored, and is optional for authorization servers. The specification frames this as localhost development support. ([atproto.com](https://atproto.com/specs/oauth?utm_source=openai)) RFC 8252 supports loopback IP redirects and ephemeral ports for native apps, but the atproto profile imposes narrower client-metadata rules. ([datatracker.ietf.org](https://datatracker.ietf.org/doc/rfc8252/?utm_source=openai)) The official atproto CLI tutorial explicitly says its loopback client is not a production solution. ([atproto.com](https://atproto.com/guides/oauth-cli-tutorial?utm_source=openai))

Split into:

1. **T4a OAuth architecture spike, blocking:** prove whether this is local-dev-only or design a production client identity/metadata strategy; verify the Rust authenticator supports a public-client configuration rather than silently reusing confidential web-client credentials.
2. **T4b loopback callback transport:** exact path/method validation, state and issuer validation, request-size/header/time limits, single-use completion, Ctrl-C cleanup, and user-denial errors. Binding `127.0.0.1:0` means “port-in-use” is not a sensible normal-path AC; test bind failure instead.
3. **T4c shared login/provision use case:** extracted from the web callback and invoked by both drivers.
4. **T4d identity store + `whoami`/`logout`:** versioned record containing DID plus environment/database identity and OAuth client identity—not merely DID and timestamp. Use platform-standard config directories rather than specifying only XDG; `ProjectDirs` covers Linux, macOS, and Windows. ([docs.rs](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html?utm_source=openai)) Add locking, symlink defenses, corruption handling, atomic replacement, and concurrent-login tests. Atomic replacement is not universally guaranteed merely by choosing a tempfile API, so define supported filesystem semantics. ([docs.rs](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html?utm_source=openai))

**Strike:** fake-PDS “full loop” until its support for PAR, PKCE, DPoP, issuer/state validation, and the localhost-client exception is demonstrated. **Add:** browser-launch failure prints the URL to stderr and continues waiting, or provide an explicit `--no-open`; do not make browser launching a single point of failure.

## T5 — **MERGE into T2**

This is delivery plumbing, not a standalone product ticket. “CI matrix includes the crate” and “cargo deny unchanged” are weak AC. **Add AC:** `just cli -- health` forwards arguments exactly; one canonical recipe name; clean-checkout CI builds the binary, generates completions, runs parser tests and black-box tests, and verifies no workspace crate is accidentally omitted. **Strike:** either `just cli` or `just zurfur`; having both without distinct semantics is clutter.

## Missing from Claude’s slice

1. **CLI principal-resolution/context ticket.** Every Engineer command immediately needs “identity record → environment validation → User → authorization actor.” Without one shared `PrincipalResolver`, 41 commands will invent it independently.
2. **Shared command result/error contract.** Define domain/conflict/not-found/authorization/validation/infrastructure mappings and the versioned JSON problem envelope before command implementation.
3. **Authentication-use-case extraction.** T4 references “the same use case,” but none exists. This is a hard dependency, not incidental refactoring.
4. **OAuth client configuration/deployment ticket.** Separate web-client and CLI-client metadata, keys, scopes, redirect rules, and grant storage namespaces.
5. **Test-support capability ticket.** Specify what the fake PDS actually emulates and add conformance tests for the OAuth features relied upon.
6. **Lifecycle/cancellation contract.** Composition must close pools/listeners and abort pending OAuth cleanly.

## Mis-owned

- **Opus/security cannot both build T4 and provide the decisive security review.** That destroys reviewer independence. Claude should implement the driving adapter; Opus should threat-model and review before merge.
- **Engineer should own the shared application/authentication use cases and authorization semantics.** Claude should own CLI transport, identity persistence, browser/loopback mechanics, and presentation.
- **Claude should not freeze `AppState` or choose `domain` versus `application` indirectly through composition.** That boundary belongs with the Engineer’s use-case extraction decision.
## Seat: Gemini
Here is the unvarnished review of the round 2 ticket skeletons.

### Ticket Evaluations

**T1. composition: extract Config + AppState + live wiring out of `api`**
**KEEP.** Extracting the DI container (`AppState`) and configuration is mandatory for a second driving adapter. Mechanically decoupling the dependency graph from the HTTP layer is a straightforward necessity.
*AC Additions:* Add a strict dependency enforcement test (e.g., via `cargo deny` or `cargo tree` assertions) to ensure the new `composition` crate does not accidentally pull in `axum`, `tower-sessions`, or HTTP-specific dependencies. 

**T2. cli: crate scaffold + boilerplate**
**KEEP.** The scaffold is standard and necessary. Sending logs to `stderr` and data to `stdout` is the correct UNIX approach, ensuring pipelines don't ingest tracing noise.
*AC Additions:* Ensure the `--json` flag applies globally to domain (exit 1) and infra (exit 3) error outputs as well. The CLI must output structured JSON errors so `jq` pipelines aren't broken by unexpected plain-text panics.

**T3. cli: `health`**
**CUT.** A database reachability probe (`zurfur health`) is a web-server pattern foolishly pasted onto a local CLI binary. The CLI is a short-lived, transient process. If the database is unreachable, any actual command will immediately fail with an infra error (exit 3). Do not waste Claude's time building useless diagnostic bloat.

**T4. cli: `login` / `logout` / `whoami`**
**REWRITE.** The premise of this ticket is factually broken on three fronts. First, ATProto loopback development clients (`client_id="http://localhost"`) are explicitly *Public Clients*; they cannot declare a JWKS or use confidential auth. Reusing the web backend's confidential `AtprotoAuthenticator` verbatim without branching logic will fail at the PDS token exchange. Second, ATProto OAuth strongly mandates DPoP (Demonstrating Proof-of-Possession). The CLI cannot just store a "DID + created-at" identifier in the identity file; it *must* generate and persist a local DPoP private key alongside the OAuth access/refresh tokens. Third, RFC 8252 requires the local redirect URI to bind strictly to `127.0.0.1`, not `localhost`.
*AC Additions/Changes:* Refactor `AtprotoAuthenticator` to explicitly support a Public Client mode for the CLI. Require the identity file to securely persist the DPoP keypair and tokens with 0600 permissions. Add an AC for handling token refresh lifecycles locally.

**T5. ci/just: glue**
**KEEP.** Standard CI/CD wiring. No changes required.

---

### Missing from Claude's Slice

**M1. The Application Layer Scaffold (Orchestration Landing Zone)**
You cannot unleash the Engineer on 41 operation tickets if orchestration is still trapped inside 5.2k lines of `axum` handlers. If Claude doesn't build the structural landing zone first (e.g., an `application` crate defining a Use Case/Command trait that manages the UnitOfWork and outbox), the Engineer will either illegally duplicate HTTP logic inside the CLI or invent 41 competing architectural patterns in the first hour. Claude must extract exactly *one* command as a blueprint.

**M2. CLI Context / DPoP Session Extractor**
The web tier uses `tower-sessions`. The CLI needs an equivalent, dedicated middleware or loader to read `$XDG_CONFIG_HOME/zurfur/identity`, validate/refresh the ATProto token via DPoP, and construct the `User` identity *before* executing a use case. The Engineer's tickets will stall immediately without this execution context.

---

### Mis-owned & Open Forks

*   **T4 (`login` / `logout`) is mis-owned:** Claude should not touch this. Implementing a public ATProto OAuth client with DPoP state management, PKCE, and local port binding is highly security-sensitive and spec-heavy. Assign this to **Opus** entirely for implementation, not just for a post-facto security review.
*   **Open fork (a) — Domain vs Application crate:** Do not leave this up to the Engineer's discretion mid-sprint. The board decides now: create an `application` crate. `domain` must remain pure (elements and port traits only) and have zero knowledge of database outboxes or transaction orchestration.
*   **Open fork (b) — Logout revocation:** Yes, `zurfur logout` should hit the OAuth revocation endpoint. Because the CLI operates as a separate Public Client, revoking its token will safely destroy the local session without invalidating the web backend's confidential session.
*   **Open fork (c) — Multiple identities:** Cut for v1. One identity file; overwrite it on subsequent logins.
## Outcome
Epic ZMVP-199; tickets ZMVP-200 composition, 201 cli scaffold (T5 merged in), 202 health (kept on Engineer ask; Gemini CUT dissent recorded), 203 identity+principal+whoami/logout (board-added), 204 login (split; client-model fork A/B/C for the Engineer — verified against the atproto OAuth spec), 205 application-layer landing zone (board-added; Engineer-owned fork domain vs application crate).
