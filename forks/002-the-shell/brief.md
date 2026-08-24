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
