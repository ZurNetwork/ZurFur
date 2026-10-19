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