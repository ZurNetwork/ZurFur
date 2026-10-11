# Roadmap: architects

Lead: Termite · Stages covered: pre-alpha (after 0.1), alpha, beta, release

This file is the **architecture runway**: the structural changes that must land, and the slot each must land by, so the features can. It plans no feature team's work. Where a change is built in another team's code, that team is named under "Cross-team dependencies". Slots are named as the ASK says. **P-last** means the last pre-alpha slot, the alpha cut, whatever number the consolidator gives it. Alpha, beta and release items attach to the slot that ships the card named, and the consolidator numbers them.

## In one screen

- **Pre-alpha ends with the alpha's structure built.** The transitional `domain` crate is split by domain, and no driver opens a transaction any more (P2). Every write that leaves PostgreSQL (to the PLC directory or a PDS) goes through one durable outbox, run by a separate worker process. Every secret Zurfur keeps sits under one KMS-backed custody. A staging copy runs that only you can reach (all three by P3).
- **The alpha cut (P-last) builds production** (hosting, backups with a restore you have watched work, alerts, the production sign-in client, the deployment checklist) **and gets the doors ready to close.** The stored shapes the alpha will hold are ruled. Pre-alpha migrations collapse into one baseline. CI gates make the proto additive-only, released migrations unchangeable and lexicons additive-only.
- **Alpha:** from the first alpha tag the proto is additive-only, and every did:plc operation is public for good. Zurfur runs its own PDS for hosted repos, with its admin credential outside the app process. The storage-routing crate (your note 1) gets a checkpoint, and the groundwork for external-storage mounts (your note 2) is in place by the end.
- **Beta:** Kafka arrives as an adapter behind the outbox. The plugin API runs as a second, bearer-token zone with its own crates. The changelog hash chain covers entries written during the alpha. The API and the worker run as more than one instance.
- **Release:** v1 becomes generally available, with its 12-month notice window, per-version telemetry and the deprecation machinery. Disaster recovery is drilled. Signed webhooks and payments reuse the same egress guard and custody.

## Where the architecture stands after 0.1 (read 2026-10-10)

What the slots below start from, each checked in the code or the corpus:

- **One `domain` crate**, about 10,000 lines. It is already cut into façade modules with a fixed set of leaf file names, so a split is mechanical. Two facts shape any split:
  - `commission` and `workflow` import each other.
  - The unit-of-work trait hands out every domain's write port (`domain/src/ports/unit_of_work.rs`), so it has to sit above all the per-domain crates.
- **The application layer's terminus isn't reached.**
  - Five driver-opened transactions remain, all through `Runtime::transaction`: the commission routes `channel.rs` (2) and `elements.rs` (2), and `session.rs` (1, sign-in provisioning).
  - The clippy ban on opening a unit applies inside `application` only.
  - You ruled on 2026-10-08 that the CLI's user commands go over HTTP, which the CLI can't do while the generated contract types live inside `api`.
- **No outbox exists in the code.** Four corpus pages and CLAUDE.md assume one (the handle change, decision 7; did:plc custody, decision 9; Characters on ATProto; Domains and Applications). `application/src/account/change_handle.rs` sends the PLC update, then writes PostgreSQL, with no record and no retry in between.
- **No deployment exists.** There is no Dockerfile for the API or web, no CD workflow, no metrics or error reporting (logs only) and no backup.
  - The API refuses to boot as staging or production while custody is config-backed (`composition/src/config.rs`, `ensure_custody_hardened`).
  - Sign-in uses atproto's loopback client, which works only on this machine.
  - Commission file bytes sit in a PostgreSQL `bytea` table, keyed by a UUIDv7. The Den decision (2026093001, decision 18) wants them stored once, by content hash.
  - The connection pool is fixed at 5 (`adapter-pg/src/lib.rs`).
- **No team owns deployment.** `.agent-board.toml` gives the internal team CI (`.github/**`), but no team owns hosting, infrastructure or operations.

## Pre-alpha

**P1 (0.1, fixed).** No architecture work of its own. The architects' first review is 0.1's end-of-release review (`.understand/releases/0.1/architecture.md`). Its requests feed P2.

- **P2: clean seams before the alpha fan-out.**
  - **Lands by:** the start of P2, at the quiet seam right after 0.1 merges, before any P2 feature branch is cut. A crate split touches every import path, so it must run when nothing else is open. The open WIP branches (`feature/character-claims-usecases`, `feature/user-page-profile-route`) restack once.
  - **What lands:**
    1. **The domain crate split** (shape: question 1). The per-domain crates hold their own entities and ports. A thin `domain` façade keeps today's import paths and holds the unit-of-work trait. CLAUDE.md's trigger is "as those namespaces get built", and in P2 moderation and posts start building theirs. Teams' path globs in `.agent-board.toml` move with it.
    2. **The application layer's terminus, part 1** (The Application Layer, 55836674, "Migration terminus"):
       - the last five driver-opened transactions become use cases, and `Runtime::transaction` is deleted;
       - the clippy ban on opening or settling a unit extends to `api`, `cli` and `composition`, with test seeding allowed by name;
       - the owed guards: no `adapter_pg::Pg*` in routes or CLI commands outside the allow-list, a test that no error's display text leaks its cause, and `shared` stays a leaf.
    3. **Content-addressed file storage, as a shape behind the file port.** Bytes are keyed by their hash. A file node is separate from its bytes, and the bytes are freed when no node points at them (The Den, decision 18). Moderation's P2 scan gate holds bytes "by content hash, everywhere they appear", so this shape is its prerequisite. The bytes may stay in PostgreSQL until P-last moves them to an object store.
  - **Depends on:**
    - commission: the `channel` and `elements` use cases, and the file nodes;
    - identity: sign-in provisioning as a use case;
    - moderation: the pending area sits on the new shape;
    - internal: path globs and CI.
  - **Done when:**
    1. `ls backend/crates` shows the per-domain crates, and `cargo tree -e normal` shows that no domain crate depends on another, apart from the shared base crate (question 1);
    2. `rg -n '\.transaction\(' backend/crates/api/src backend/crates/cli/src backend/crates/composition/src` prints nothing, and a `db.begin()` planted in a route fails `cargo clippy`;
    3. the same image uploaded to two commissions is stored once (`just db-shell`, one bytes row). Deleting one commission leaves the other's copy downloadable.

- **P3: one road out of PostgreSQL, and a place to run it.**
  - **Lands by:** P3, before any slot that turns on the canonical PLC directory, writes to a PDS or deploys.
  - **What lands:**
    1. **The outbox.** A durable step log in PostgreSQL for every write that leaves it: to the PLC directory, to Zurfur's PDS, to a person's own PDS.
       - A step is recorded in the same unit of work as the private change it belongs to. Where a decision orders the public step first (the handle change: "DID document first"), the step is recorded first and the private change waits for it.
       - The worker runs each step with retries, an idempotency key and ordering per DID. A step ends in either "done" or a "failed" state that an operator can see.
       - The three existing cross-store writes move onto it: founding an Account's DID, the handle change, and the tombstone at deletion.
       - This is the "separate, retryable step" CLAUDE.md requires. Your note 1's routing crate, if it comes, fronts this mechanism and doesn't replace it.
    2. **A worker process.** A third driver beside `api` and `cli`, over the same composition root. It runs the outbox and the deadline sweep, which moves out of `api`. Later it runs the PLC log monitor, byte cleanup and the scan queue (question 4).
    3. **One KMS-backed custody** for every secret Zurfur keeps:
       - did:plc rotation keys and stored OAuth tokens today;
       - later the Den's Class B MAC key (The Den, decision 25), hosted-repo write credentials and the production OAuth client's key.
       - Today two copies exist: `adapter-atproto`'s secret vault mirrors `adapter-pg`'s key vault (`adapter-atproto/NODE.json`). One port, one KMS adapter, and a key id stored beside each wrapped secret, so the root can rotate. The curve is secp256k1, which rules out Vault Transit (did:plc custody, decision 5).
    4. **The contract's generated Rust types in their own leaf crate**, so `cli` can be an HTTP client of the API (your ruling of 2026-10-08) without depending on `api`. The drift test moves with them.
    5. **Staging, reachable only by you.** It needs item 3, because the API refuses to boot as staging with config-backed custody.
       - It also needs the production sign-in client: a confidential OAuth client whose metadata is served at an `https` address, authenticating with a private key (`private_key_jwt`) under item 3. The atproto OAuth spec, read 2026-10-10: the `client_id` "is a fully-qualified web URL"; confidential clients "must include `token_endpoint_auth_method` as `private_key_jwt`".
       - Staging stays network-restricted. Under your ruling on moderation's first deadline (PDD D-26), a build anyone else can reach triggers moderation's floor.
  - **Depends on:**
    - identity: the handle change, the delete and the founding moved onto the outbox, and the OAuth client;
    - internal or the deployment owner (question 5): the staging host and the CD pipeline.
  - **Security:** items 1, 3 and 5 touch the private↔public boundary, custody and session handling, so each passes `/security-review` before it merges.
  - **Done when:**
    1. with the local PLC directory stopped, a handle change shows as pending. Start it again, and the change completes with nothing from you. `just db-shell` shows the step's history;
    2. `ZURFUR_ENV=STG` boots against the KMS key and refuses to boot with the shipped dev key;
    3. `cargo tree -p cli` shows no dependency on `api`;
    4. you open staging from your own machine and sign in with Bluesky. From another network, the address doesn't answer.

- **P-last: the alpha cut. Production exists, and the doors are ready to close.**
  - **Lands by:** the last pre-alpha slot. Every item is a precondition of opening the alpha.
  - **What lands:**
    1. **Production.**
       - Hosting: question 6.
       - PostgreSQL with point-in-time recovery and encrypted backups. Once the PDS exists, the backup set grows to include its data directory and blob store.
       - A restore drill on staging.
       - The connection pool sized from config.
       - Observability: structured logs under the redaction rules, plus metrics and alerts to you for errors, latency, pool saturation, outbox age and failed steps, and the PLC monitor's alarms.
       - The deployment checklist: an egress IP no internal service trusts, IMDSv2 only, cookie redaction in proxy logs, no user content on any `zurfur.app` subdomain, and the sign-in callback sent straight to the API (the hardened-resolver design, "Deployment checklist").
       - Moderation's floor, and moderation's two database roles with "the API boots without migrating" (moderation's P3).
    2. **Canonical PLC readiness.**
       - The transparency-log monitor runs in the worker. The custody decision makes it "in v1": it is what makes the 72-hour recovery window usable.
       - Tombstone durability and mint ordering (two security-asap items) run on the outbox.
       - The KMS root key has deletion protection and a written recovery procedure. Losing it loses every custodial rotation key.
    3. **The stored shapes the alpha will hold, each reviewed as a door and ruled by you:**
       - the Den's node storage: how metadata and payload are laid out, the payload hash, what the Class B MAC binds (still open on The Den's page: level, name, a version), attributes as rows, and room for a node's availability, which your note 2 will need;
       - content-addressed files on an object-store backend;
       - the changelog's entry encoding, canonical and versioned (question 9);
       - the custody envelope's format;
       - sessions.
    4. **The migration baseline** (question 3). The pre-alpha migrations collapse into one baseline, and the alpha database is born from it. From then on:
       - a CI gate refuses any edit to a migration released in an alpha tag;
       - a migration linter flags table-locking or destructive statements;
       - changes go expand-then-contract, so each deploy also runs on the previous schema.
    5. **The contract freeze machinery** (question 2):
       - CI's breaking-change check compares against the last alpha tag as well as the PR's base. The versioning contract asks for both; only the PR-base check exists (the pre-alpha tag ruling, PDD D-19);
       - `contract/lifecycle.toml` records the first published client, and once it is set, CI refuses a declared baseline reset. This closes The API Contract's open row on how the reset's expiry is enforced;
       - a final audit lands as the last declared baseline reset. The known warts: `Account.id` in `account.proto`, the changelog's bare array, and the Den's provisional type names (TDD "Parked": settled before the alpha, or shipped as they are);
       - tolerant-reader tests on every client that exists at A1 (the versioning contract, §6).
    6. **Lexicon publication tooling:**
       - Zurfur's lexicon-authority DID, its `_lexicon.zurfur.app` record, and schema records on Zurfur's PDS (The Lexicon Registry, decision 1);
       - a tracked list of published NSIDs, and a CI gate that allows only additions against it;
       - Permission sets are lexicons too ("published using the same tooling", atproto's permission-sets guide, read 2026-10-10). So the first OAuth scope upgrade can't happen before this exists.
  - **Depends on:**
    - den: the node storage proposal;
    - commission: the changelog encoding;
    - identity: the invite gate at sign-in. The alpha is a closed group, and `main` has no sign-in allowlist (moderation QA, 2026-10-09);
    - moderation: its floor;
    - internal: the CI gates;
    - compliance: backup retention against deletion requests, and what telemetry may hold.
  - **Done when:**
    1. you watch a restore of last night's staging backup reach a given minute, then sign in to the restored copy;
    2. an alert reaches you when the PLC directory is stopped for longer than the outbox's patience;
    3. `just db-reset` on a fresh database runs exactly one baseline migration plus anything after it;
    4. a PR that edits a released migration, renames a proto field or removes a lexicon field fails CI;
    5. the deployment checklist is ticked on staging, by you.

**Exit, before the alpha opens:**
- P2, P3 and P-last are on `main`.
- The security-asap card is closed. Every item on it that touches custody, the boundary or sessions has passed `/security-review`.
- Each P-last door has your ruling, and the architects' door review of the alpha cut is written.

## Alpha

- **A1: a real domain and real data.**
  - **Structural:**
    - the first production deploy from the alpha tag;
    - the PLC directory switches to canonical submission. From here, every operation is in a public log for good;
    - v1 is additive-only;
    - the restore drill is re-run on production's backups;
    - the web is the first published client, and the CLI is too if it ships to anyone.
  - **Depends on:** everything in the pre-alpha exit; identity's invite gate.
  - **Done when:**
    1. founding an Account in production shows its DID's operation log on `plc.directory`, and nowhere else;
    2. `buf breaking` against the alpha tag runs in CI;
    3. the production restore drill's notes are in the release folder.

- **With the slot that first writes into a person's own repo** (friendship or claims): **writes Zurfur can't finish alone.**
  - A friendship is "two reciprocal, public records — one in each party's repo" (Friendship and Double-Sided Relationships, its summary), and only that person's OAuth session, with the scope granted at first use, can write it (Publish Consent & OAuth Scopes).
  - So the outbox gains a "waiting on the person" state. A step stays there until the person re-consents or their session is live again, never retried blind.
  - Refresh-token lifetime depends on the client. The atproto OAuth spec allows a confidential client 180-day refresh tokens and an "untrusted" public client 2 weeks, which is one more reason for P3's confidential client.
  - **A ruling that conflicts with the protocol, for identity and Vulpes (question 11).**
    - Publish Consent & OAuth Scopes (amended 2026-09-12) rules that Zurfur's first permission set, `app.zurfur.authPublish`, "grows to carry repo-write authority for `net.got-paws.acp.*`".
    - atproto's permission spec, read 2026-10-10, says: "Permission sets are limited to expressing permissions that reference resources under the same NSID namespace", and a set "can not address 'sibling groups' or 'parents'".
    - So an `app.zurfur.*` set can't grant Vulpes's `net.got-paws.acp.*` records. The claims slot needs Vulpes's own permission set, published under its own authority, or plain `repo:` scopes requested beside Zurfur's set.

- **With the slot that ships account-pds or character-presence: Zurfur runs a PDS.**
  - The reference PDS runs in production. Its admin credential lives in an operator-only process, "outside the app process, network-restricted, attach/recovery only" (Characters on ATProto, the write-credential ruling).
  - The PDS's own rotation key is in KMS. The reference PDS takes a KMS key id for it (`packages/pds/src/config/secrets.ts`, read 2026-10-10).
  - The PDS's data directory and blob store are in the backup set.
  - **Joining the relay mesh is its own deliberate switch.** It is the PDS's crawler list, empty in dev. From then on records propagate, and "the network never forgets" (The AT Protocol Boundary Contract, decision 7).
  - **Security finding for the pending Hosted Repos decision.**
    - The reference PDS has one server-wide PLC rotation key (same source).
    - The attach step adds "the PDS rotation key" to each hosted DID (Project MVP, "Account public presence") at rank 2 of `[owner, pds, zurfur]`, above Zurfur's per-DID key.
    - So one key spans every hosted repo, which the custody decision's first rule exists to forbid ("a shared key means one leak takes over every identity"). See question 10.
  - **Depends on:** identity (the Hosted Repos decision), moderation (its A1, take-down reaching the hosted PDS).

- **Checkpoint: the storage-routing crate (your note 1).**
  - **When:** at the end of the slot that ships the second kind of public record (friendship and claims, or what an Account's repo holds).
  - **Built only if:** three or more use cases hand-write the same "private change plus outbox step" sequence, or one use case coordinates writes to two repos (claims writes both halves).
  - **Its rules:** see question 7.

- **By the end of the alpha: the groundwork for external-storage mounts (your note 2).** The feature's slot is question 8. Each tension in your note needs something built first:
  - "content hashes are what let us verify the bytes": the payload hash and its check, live (The Den, decision 25; not exercised in 0.1);
  - "an honest state in the tree": an availability field reserved in the node metadata when its storage shape is ruled at P-last. Atta's metadata is "rigid and closed", so adding it later is an Atta change and a migration;
  - SSRF: the guarded client generalized from identity lookups to any address a user supplies, plus the network-layer egress guard the resolver design lists as missing;
  - third-party OAuth tokens: custody under P3's envelope;
  - "a Private level can't be enforced": a design question for its slot, not settled here.

**Exit, before the beta opens:**
- Every alpha card has a home.
- The storage-routing checkpoint is decided.
- The mount groundwork is built.
- The architects' reviews of each alpha release are closed or carried, each with a slot.

## Beta

- **With the kafka slot: the broker as an adapter.**
  - The outbox's port gains an event side. Kafka sits behind it, with policy kept separate from mechanism (Project MVP: "the swap is an adapter change").
  - A running broker is new operations work: managed or self-hosted is your call when the slot comes.
  - Prerequisite: P3's outbox.
- **With the plugin-api slot: a second auth zone.**
  - The bearer `/plugin/v1` surface mounts outside the cookie CSRF layer. The design already places it there (Auth Surfaces, decision 1: "CSRF is a cookie-surface concern only").
  - The `plugin` and `plugin-api` crates, which the plugin team's charter already claims. App keys sit under P3's custody.
  - Its contract surface is additive within v1.
- **With the hash-chain slot:** the chain is computed over the canonical entry encoding fixed at P-last, so it can cover alpha-era entries (question 9).
- **With the seals slot:**
  - the Provider signing key sits under the same custody;
  - the labeler service is a new deployable;
  - its endpoint goes through the deployment checklist.
- **With the external-mounts slot, if it lands in beta (question 8):**
  - storage providers come in as plugins under the plugin trust ladder;
  - every fetch goes through the generalized egress guard.
- **More than one instance** (lands by the slot that opens the bigger group):
  - the API runs as two or more stateless instances, sessions staying in PostgreSQL;
  - the worker keeps its leader lock;
  - the pool and the Den's parallel reads are re-sized from the alpha's metrics.

**Exit:** the API and the worker survive losing one instance. Kafka, if adopted, carries no domain change.

## Release

- **R1: general availability.**
  - v1 becomes generally available (question 2), so the 12-month notice window binds.
  - The per-version telemetry the versioning contract requires before any deprecation is live.
  - The deprecation machinery is built: the headers, `410` after sunset, and routes following `contract/lifecycle.toml` (the versioning contract's seven CI assertions).
  - Disaster recovery is drilled from a cold start.
  - Data export: a CAR file for hosted repos and an export of the person's private data.
  - The recovery key moves into an HSM-held key (did:plc custody, decision 4's hardening follow-up).
  - Atta is split into its own library once a second consumer exists. It is built to allow that (The Den, objective 5).
- **With the webhook-signing slot:** signed webhooks reuse the egress guard and the custody.
- **With the notifications slot:** external channels ride the outbox, or Kafka. Notifications' own decision names the outbox as what external channels bring in.
- **With the subscriptions and marketplace slots:** payments are a bounded context of their own. Inbound processor webhooks are verified at the edge, and card data never touches Zurfur. That last one is a compliance dependency, not decided here.

**Exit:** the architects' review of R1 finds no structural must-fix.

## One-way doors, and the slot that closes each

| Door | Kind | Closes at | What makes it safe to close |
| --- | --- | --- | --- |
| The shape of every table the alpha database holds | Stored data | A1 | P-last's baseline; the released-migration gate; expand-then-contract; backups with a watched restore |
| The Den's node storage (metadata layout, payload, hash, MAC, attributes) | Stored data | A1 (ruled at P-last) | Your ruling on den's proposal; an availability field reserved for mounts |
| How file bytes are keyed (by content hash) | Stored data | A1 (shape from P2) | P2's port shape, then the object-store backend at P-last |
| The changelog's entry encoding | Stored data | A1 | Canonical and versioned, so the beta's hash chain can cover alpha entries (question 9) |
| The custody envelope and the KMS root | Stored data + security | A1 (built P3) | A key id beside each wrapped secret; deletion protection; a recovery procedure |
| The first canonical did:plc operations (key lists, per-DID keys, handle-less Characters) | Published, permanent | A1 | Mint ordering and tombstone durability on the outbox; the PLC log monitor running |
| v1 contract: no more in-place breaking changes | Published contract | First alpha tag (already ruled: PDD D-19; the alpha is the first published client) | The tag-based gate, the published-client fact in `lifecycle.toml`, the final audit reset |
| v1's 12-month notice window | Published contract | R1, if you rule question 2 as proposed | Per-version telemetry and the deprecation machinery |
| Each published lexicon, permission sets included | Published schema | The slot that first publishes it | The additive-only gate and its published list; your review of the text, as with den.proto |
| Joining the relay mesh | Publication | The slot whose hosted repo first holds public records | A deliberate switch; take-down reaching the hosted PDS (moderation A1) first |
| Hosted repos' rotation-key list | Published, permanent | The account-pds slot | Question 10, before the first attach |
| The Den's type names | Published contract | P-last (they ship in den.proto's `type` field) | The type catalog, or shipping them as they are (your ruling on provisional type names, question P10) |

## Cross-team dependencies (what you need, from whom, by which slot)

| What | From | By |
| --- | --- | --- |
| The `channel` and `elements` routes moved into use cases | commission | P2 |
| Sign-in provisioning as a use case | identity | P2 |
| File nodes on content-addressed bytes; the scan gate's pending area on the same shape | commission, moderation | P2 |
| Team path globs following the crate split; the new CI gates (released migrations, lexicons, the tag-based proto check, the migration linter) | internal | P2 (globs), P-last (gates) |
| The handle change, the Account delete and DID founding on the outbox; the production OAuth client | identity | P3 |
| A staging host and a CD pipeline | whoever owns deployment (question 5) | P3 |
| The Den's node storage proposal, for your ruling | den | P-last |
| The changelog's canonical entry encoding | commission | P-last |
| The invite gate at sign-in | identity | P-last |
| The floor (scan gate, report path), the two database roles, booting without migrating | moderation | P-last (moderation plans P2–P4) |
| Backup retention against deletion requests; what logs and metrics may hold; a legal entity, if the scanning vendor needs one | compliance | P-last |
| The Hosted Repos decision, answering question 10 | identity, with the Designer's security review | Before the account-pds slot |
| Vulpes's own permission set, or plain `repo:` scopes, for claim records (question 11) | vulpes, identity | The claims slot |
| A plugin-provided storage port, if mounts are plugins | plugin | The mounts slot (question 8) |
| The moderation console's own origin, reviewed against the one-origin topology (Domains and Applications) and the `__Host-` cookie rule | moderation, with architects | Moderation's P3 |

## Not on the board (proposals, each with why)

- **The outbox and the worker process (P3).** Four decisions and CLAUDE.md assume an outbox, and no code has one. Without it, a failed write after a public step leaves the PLC log and PostgreSQL disagreeing, with nothing to retry it. At A1 that disagreement is public.
- **One KMS-backed custody for every secret (P3).** It is the security-asap card's key-custody item, widened to the four other secrets that need the same treatment. Today two copies of the envelope code exist.
- **The contract's Rust types in a leaf crate (P3).** Your CLI-over-HTTP ruling needs it. The in-process CLI holds database admin power, so it should never reach anyone but the operator.
- **Staging, Engineer-only (P3).** It avoids a big-bang deploy at P-last. Network restriction keeps it inside your ruling on moderation's first deadline (PDD D-26).
- **The migration baseline and the released-migration gate (P-last).** The alpha database starts empty anyway (question 3), so collapsing about 42 pre-alpha migrations is free once. The gate stops the one mistake sqlx turns into a failed boot: editing an applied migration.
- **The lexicon gate and its published list (P-last).** "Additive-only" is a rule today, not a check. `buf` checks the proto, and nothing checks lexicons.
- **The changelog's canonical encoding (P-last).** It makes the beta's hash-chain card possible over alpha data.
- **Deployment ownership (question 5).** "deployment" is an alpha card with no team.
- **The storage-routing crate and external-storage mounts.** These are your notes 1 and 2, placed above with their checkpoints and prerequisites, not designed.
- **The architects' door review at the alpha cut.** The end-of-release review, with one extra pass over the door table above.

## Questions for the Engineer (each pre-answered)

1. **The domain crate split: what shape, and when?** *Proposed:*
   - At the start of P2, at the quiet seam.
   - A small base crate for what every domain shares: DIDs, handles, text, time, actor ids.
   - Then `identity` (users, Accounts, characters, roles, invitations, keys, profiles) and `workflow` (commission and Workflow together, because they import each other; the corpus already names a `workflow` crate).
   - Then `gallery` (posts, blobs, public records), `moderation` (new, not in the corpus's list of four), and `plugin` (created with its first code, by the beta).
   - A thin `domain` façade re-exports them and holds the unit-of-work trait, so no consumer changes.
   - *Why now:* every alpha card adds to `domain`, team ownership maps onto crates, and a later split moves twice the code. It isn't a one-way door, so if you'd rather wait, the latest sensible slot is before the plugin API (beta).
2. **What does "the proto freezes at the alpha" mean?** *Proposed:*
   - From the first alpha tag, v1 takes additive changes only, and a breaking change means v2. This is already ruled (PDD D-19, the alpha as the first published client).
   - The alpha and beta stay "pre-GA" for the notice window, because their clients are first-party. v1 becomes generally available at R1, when the 12-month window and per-version telemetry bind.
   - The tag-based check compares against the latest stage tag (`alpha-v…`), with no separate contract tags. This amends the versioning contract's example tag (`api/v1.4.0`).
3. **Does the alpha start from an empty database, born from one baseline migration?** *Proposed: yes.*
   - Pre-alpha DIDs are minted on the local PLC directory, so they can't carry into the alpha (`.env.example`, `ZURFUR_PLC_DIRECTORY_SUBMIT=false`).
   - Your local data stays on your machine and isn't deleted, so this is not the exception in your migrations ruling (TDD D-36).
   - The squash is also when the older plural table names could be made singular, if you want that. Proposed: leave them.
4. **Does background work run in its own worker process?** *Proposed: yes, a third driver over the composition root.*
   - The privileged hosted-PDS steps (attach, recovery) run in an operator-only process, because the ruling keeps the PDS admin credential "outside the app process".
   - Running tasks inside `api` with leader locks would also work for the alpha, but it would put that credential in the API.
5. **Who owns deployment?** *Proposed:*
   - The internal team, as a project with the alpha cut as its deadline. It already owns CI (`.github/**`).
   - Architects review the result. Leads don't own infrastructure.
6. **Hosting, and how much data you can afford to lose.** *Proposed:*
   - Managed PostgreSQL with point-in-time recovery.
   - One container host for the API, web, worker, Caddy and later the PDS.
   - An S3-compatible object store and KMS from the same provider (AWS KMS or GCP KMS; both support secp256k1, per did:plc custody's open row).
   - At most 15 minutes of data lost, and service back within 4 hours.
   - Vendor and cost are yours.
7. **If the storage-routing crate is built, what are its rules?** *Proposed:*
   - It is the outbox's front door, never a second mechanism.
   - It picks a write's destination from the type alone, by The AT Protocol Boundary Contract's class test, with an exhaustive `match` like Atta's host and no runtime registry.
   - It never makes a write atomic across stores.
   - It sits below `application`.
   - Built only at the checkpoint's trigger.
8. **External-storage mounts: alpha or beta?** *Proposed: beta, after the plugin API,* if storage providers are plugins (your "composable, plug-in storage"). Alpha only for first-party adapters, and only once the alpha's groundwork above is built.
9. **Is the changelog's entry encoding fixed before the alpha, so the beta's hash chain can cover alpha-era entries?** *Proposed: yes.* It uses the same canonical encoding the Den's payload hash uses (The Den, decision 25: DAG-CBOR), with a version per entry. The chain itself stays a beta decision.
10. **The hosted PDS's shared rotation key.** *Proposed:* the Hosted Repos decision either leaves the PDS key out of each hosted DID's rotation list, or ranks it below Zurfur's per-DID key, and it gets a security review first.
    - *The case:* the reference PDS has one server-wide rotation key (its `secrets.ts`, read 2026-10-10). In `[owner, pds, zurfur]` that key ranks above every per-DID key Zurfur holds, across every hosted repo. For an Owner who hasn't enrolled a key, it is the senior key.
    - *Against:* you ruled the layout on 2026-09-12, partly so an independent PDS later restores two independent failures.
    - This is your fork, raised here because it closes at the first attach.
11. **How does a person's consent reach Vulpes's claim records?** *Proposed:* Vulpes publishes its own permission set under its own namespace authority, and the claims slot requests it beside `app.zurfur.authPublish`. Plain `repo:net.got-paws.acp.*` scopes are the fallback, but they show the person a rawer consent screen. Either way, the 2026-09-12 amendment to Publish Consent & OAuth Scopes is corrected by a design-sync, because the protocol doesn't allow it as ruled (see the alpha's claims bullet). Owed before the claims slot.

## Drift found while reading (for one design-sync)

- The Application Layer (decision 10) and Domains and Applications cite "The API Contract, decision 11" for moving the contract into a leaf crate, but that decision is "generated code is committed". The leaf-crate move is recorded nowhere as a decision.
- The handle change decision (7) and did:plc custody (decision 9) describe an outbox. The code has none (`application/src/account/change_handle.rs`, lines 74–82).
- did:plc custody's decision 8 puts the PLC log monitor "in v1". Nothing in the code watches the log.
- `backend/crates/domain/README.md` still names `api` as the composition root.
- Publish Consent & OAuth Scopes (amended 2026-09-12) puts `net.got-paws.acp.*` into an `app.zurfur.*` permission set, which atproto's permission spec doesn't allow (question 11).
