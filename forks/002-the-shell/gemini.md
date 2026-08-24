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