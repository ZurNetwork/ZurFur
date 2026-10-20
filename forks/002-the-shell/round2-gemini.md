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