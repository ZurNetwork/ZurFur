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