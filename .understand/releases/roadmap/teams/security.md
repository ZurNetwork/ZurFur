# Roadmap: security

Lead: Heron · Reviewers: Mongoose, Hedgehog · QA: Nightjar · Stages covered: all four

**What this file is.** The security runway: what must be true about security before each stage's audience may use Zurfur, which team owns each piece, and the slot it must land by. It plans no feature team's work. Where a team has already placed an item in its own file, this file cites that slot and checks it against the gate the item guards. The team's charter is pending (PR #252); the board accepted the `security` team at registration.

**Read 2026-10-10:** the ASK, the stages ruling, the Release Board export, the Engineer's notes, the corpus page "Project MVP" and the decision pages it cites for security, the 0.1 product design, tech design, plan and rulings, the other teams' roadmap files as they stood at 12:05 (identity, moderation, posts, den, plugin, web, commission, architects), Internal's CI/CD study, the code on `main` and on the sign-in stack, and the Jira tickets behind the security card. Every claim about practice cites a source fetched today (listed at the end); anything not checked is marked *unverified*.

## In one screen

- **Pre-alpha: the security card lands by the gate each item guards, not all at "before alpha".** Two items are due with the first writes (P2): the CSRF origin policy and the soft-delete gate. That is earlier than web's P9 for the CSRF policy. The scan gate and the gated blob store land with the first upload change (P2). The report path, BYO-domain checks and handle edges land before anyone but you can reach a build. Key custody (KMS), mint ordering, the race cleanup and tombstone durability land before the PLC directory switch.
- **Threat model:** I write version 1 in P2, as designed for the alpha. It gets a delta at every end-of-release review and a full refresh at each stage's last slot. It is also refreshed whenever a new trust boundary opens: the hosted PDS, KMS, staging, plugins, third-party storage, or another sign-in source.
- **Before alpha opens:** every secret listed, held in one store, with one key per purpose under KMS. Every CI action is pinned by commit SHA before the first secret enters CI. Advisories block when a PR changes dependencies, and a daily run checks `main`. An incident runbook is rehearsed once. Backups are encrypted, held off the app host, and a restore has been watched. Sessions use `__Host-` cookies and have an absolute lifetime. Stored OAuth tokens are sealed under their own key and revoked at sign-out.
- **Alpha:** the PLC directory switch gets its own review before it flips, because every operation is public forever. The security contact goes live on the real domain. This is my proposal; the ASK places it at release.
- **Beta:** plugins stay first-party and pull-only, with one credential per Golem and a "suspend a Golem" switch. No third-party plugin or pushed delivery runs until per-install keys and signing exist. External-storage mounts need five rulings from you before their slot.
- **Release:** a disclosure policy with safe harbor, and an external test against ASVS level 2 in the last beta slot, retested before 1.0. No bug bounty at 1.0.
- **Reviews:** per slice, as before; after each big merge; at the end of every slot (a must-fix blocks the tag); and a deep stage-gate review in each stage's last slot.
- **Gaps for the consolidator:** no team owns deployment, and many prerequisites hang on it. Identity puts KMS at P8 and architects at P3. The security rule is only that KMS must exist before staging, and the boot guard already enforces that.

## How to read "lands by"

Each team numbers its own slots, so each prerequisite here is tied to a **gate**: an event that is the same for every team. The owning team's own slot follows in parentheses when it has proposed one.

| Gate | What it is | Where the teams place it today |
|---|---|---|
| **The first writes** | The first slot in which the Den and commissions accept writes from the web | den P2, commission P2, web P2 |
| **The first upload change** | The first slice that changes an upload path (your ruling on moderation's first deadline) | moderation P2, posts P2, commission P3 |
| **The first secret in CI or the first staging deploy** | Whichever comes first | architects P3, identity P8, web P9 |
| **Anyone else can reach a build** | The first build that anyone but you can reach (the same ruling) | the last pre-alpha slot: web P9 has a second person sign in |
| **The directory switch** | Submission to plc.directory turned on | identity A1 |
| **Alpha opens** | The first alpha tag | A1 |
| **Third-party code, credentials or storage** | The first plugin Zurfur runs, or the first third-party token it holds | plugin B1, den B2 |
| **Release opens** | `release-v1.0.0` | R1 |

## Reviews in the version sequence

| Review | When it runs | Who | What it blocks |
|---|---|---|---|
| **Per slice** (Definition of Done) | Before each slice PR opens, when the slice touches authentication, the private/public boundary, DID or handle correlation, sessions or tokens, plugin trust or the blob store | Heron, Mongoose or Hedgehog, at xhigh | That PR |
| **Big merge** (charter) | Right after a feature branch merges into `main`, when the feature touched those areas. It is a whole-system look at what the merge changed and what it now touches | The team | That slot's tag, if a must-fix is found |
| **End of release** (charter) | Every slot, after its last merge and before its tag, alongside compliance and architects. Report: `.understand/releases/<version>/security.md`. Nightjar QAs it before you read it | The team | The tag, unless you accept the risk in writing |
| **Stage gate** | The end-of-release review of each stage's last slot, at max effort: the threat model refreshed, and this file's exit list checked line by line | The team | The next stage's opening |
| **Door review** | Before a security one-way door opens: the directory switch, the first hosted repo on the real domain, the first Golem DID, the first stored third-party token | Heron | The switch |

Placed in the sequence:

- **P1 (0.1, fixed):**
  - per-slice reviews;
  - the end-of-slice review (WP-22). It is 0.1's end-of-release review, and the first under this charter if PR #252 merges first.
  - Mantis reviews bug reports in one sitting.
- **P2:** threat model version 1, the hardening baseline and the secrets inventory; then the end-of-release review.
- **P3 to the second-to-last P:** an end-of-release review each, plus a big-merge review whenever a feature branch lands.
- **Last P:** the alpha-readiness stage gate.
- **A1:** the directory-switch door review, before the flip.
- **Each A:** an end-of-release review.
- **Last A:** the beta-readiness stage gate.
- **B (plugin slot):** the plugin-surface review.
- **B (mounts slot):** the third-party-token door review.
- **Last B:** the release-readiness stage gate, with the external test.
- **R1:** the release review and the external retest.
- **Every release after:** the same cadence.

**Load.** The 0.1 plan already flags that security reviews pile up (its risk R13). With two reviewers we review slices as they land, and big-merge reviews start from the per-slice findings rather than from zero.

## The threat model: written when, refreshed when

- **Written in P2, version 1.** It covers the system as designed for the alpha, not only as built in 0.1, because P2 opens writes, uploads, scanning and the outbox all at once.
  - **Method.** A data-flow diagram with its trust boundaries:
    - browser ↔ web (the cookie BFF) ↔ API ↔ PostgreSQL;
    - the API's outbound calls to people's PDSes, handle hosts and the PLC directory;
    - Zurfur's hosted PDS and its admin credential;
    - KMS, the blob store and the scanning vendor;
    - the moderator console;
    - the command line's identity file;
    - CI and the deploy pipeline.
  - **Threats.** STRIDE per element (Microsoft's six categories). LINDDUN covers privacy, because DID and handle correlation is its "Linking" and "Identifying". Both answer the Threat Modeling Manifesto's four questions: "What are we working on? What can go wrong? What are we going to do about it? Did we do a good enough job?"
  - **Where it lives:** question 9.
- **Refreshed:**
  - with a delta at every end-of-release review;
  - in full at each stage gate;
  - after any incident;
  - at once when a new trust boundary opens:
    - a new outside destination: the hosted PDS on the real domain, the scanning vendor, KMS, third-party storage;
    - a new kind of credential: the production OAuth client, a Golem's credential, third-party tokens, other sign-in sources;
    - a new kind of principal: moderator, Golem;
    - a new public surface: the user-content domain, public pages.
  - OWASP: threat modeling is "ideally performed early in the SDLC, such as during the design phase" and the model "should be maintained, updated and refined alongside the system."

## Pre-alpha

**Never relaxes, even with only you looking:** every slice that touches the security areas gets its review. Breaking changes are allowed in pre-alpha; security regressions are not.

- **P2: The runway is on paper, and the first two gates hold.**
  - Features: security-asap (the CSRF origin policy and the soft-delete gate, with their owners; the general-hardening baseline). Proposals: the threat model, the secrets inventory.
  - Security builds:
    - threat model version 1;
    - **the hardening baseline:** the list each slot is reviewed against (headers, deadlines, body limits, a database statement timeout, rate limits, log redaction);
    - **the secrets inventory**, with an owner and a rotation procedure per secret (see "Before alpha" below);
    - this file's exit list as a tracked checklist inside each release report.
  - Depends on: web and identity (or the tech lead) for the CSRF policy at the first writes; identity for the soft-delete gate in the slot that makes Account facts real; posts and moderation for the blob store and the scan gate.
  - Done when:
    1. you open the threat model and find each trust boundary above, with its threats, an owner for each, and the slot that closes it;
    2. a test proves that a POST with a foreign `Origin`, or with no `Origin` and `Sec-Fetch-Site: cross-site`, is refused on every cookie-surface write route, `just dev` included;
    3. accepting an invitation into a soft-deleted Account answers 404 and creates no membership;
    4. P2's end-of-release report exists, with no open must-fix.
- **P3 to the second-to-last P: each slot reviewed.** No security feature work of our own. Done when: each slot's report is filed, and no must-fix is open at its tag.
- **Last P: Alpha readiness** (the stage gate).
  - Features: security-asap (closing review). Proposals: the incident runbook and its tabletop, the security contact, the security event log.
  - Done when:
    1. every row of "The security card, item by item" below shows *built and reviewed*;
    2. every line of "Before alpha" below holds, each with a test, a file reference or a drill record beside it;
    3. you have walked the incident runbook's tabletop once, from the directory watcher's alarm to the recovery step;
    4. the stage-gate report lists no open must-fix.

### The security card, item by item (each due before alpha opens; most sooner)

The card's 13 items, from "Project MVP", with the meaning I read in their tickets. "Today" was read in the code on 2026-10-10.

| Item | What it means | Owner | Lands by | Today |
|---|---|---|---|---|
| **Key custody / KMS** (ZMVP-53) | Each DID's keys are wrapped by a root key held in a KMS or HSM, never in config. The config-held path stays dev-only | identity (custody parts 1 to 4: its P2, P3, P6, P8); architects want one custody for every secret by P3 | The KMS adapter: before **the first staging deploy**. The API already refuses to boot as staging or production with config-held custody. The directory watcher: before **the directory switch** | One environment variable holds the root key. The **same key** also seals stored OAuth sessions. The cold key is stored beside the operational key. `key_version` is never read |
| **General hardening** (ZMVP-63) | The four open items: zeroize the root key, a timeout on the PLC directory client, an API-wide request deadline with an explicit body limit, no derived equality on secret bytes. Plus the baseline above | identity (key vault, directory client); the tech lead (API-wide layers); web (headers and CSP, its P9); security (the baseline) | Key items: with custody part 1 (identity P2), since they share files. API-wide items, headers and CSP: before **anyone else can reach a build** | `cargo-deny` is done. The four items are open. No CSP, HSTS, frame-ancestors or Referrer-Policy anywhere |
| **CSAM scanning** (ZMVP-97) | No byte is servable until a scan says clear; a match is held and reported | moderation (P2 the gate with a fake scanner; P5 the live vendor) | The gate: **the first upload change**. A live vendor and a registered reporting route: before **anyone else can reach a build** | Commission uploads are served unscanned. Whether the vendor needs a legal entity is *unverified* (moderation's biggest risk) |
| **A report path** (ZMVP-98) | Anyone can report anything, and the report reaches you | moderation (P4) | **Anyone else can reach a build** | None |
| **The gated blob store** (ZMVP-99) | Bytes stored by content hash behind a port and read only through the Den's access check. Public bytes come from a **separate registrable domain** with no cookies (a `*.zurfur.app` subdomain is same-site, per Auth Surfaces) | posts (P2 store, P3 public serving), with commission (uploads) and moderation (pins); the tech lead confirms the split | **The first upload change**, the same slot as the scan gate | Bytes sit unencrypted in a PostgreSQL table |
| **Tombstone durability** (ZMVP-56) | A failed tombstone is retried from durable state. A retry re-submits the same operation. Custody keys are purged once the recovery window closes and the tombstone is confirmed | identity (P2), on architects' outbox | **The directory switch** | A failed tombstone is logged and dropped |
| **BYO-domain verification** (ZMVP-58) | A brought domain is proved both ways before anything is minted or stored, at founding and at rename | identity (P4) | **Anyone else can reach a build**: an Account named `google.com` is visible impersonation even before the switch. Then the switch | Founding accepts any well-formed domain and mints a DID naming it. The July audit rated this High |
| **The soft-delete gate** (ZMVP-60) | Every write against a soft-deleted Account is refused in one place, the store, so no new route can forget. Identity also reads the facts check into this item | identity (Accounts, invitations, membership); den (archived and soft-deleted nodes through `admit`); commission (its own facts) | **The same slice that makes Account facts real** (identity P2). That slice makes soft delete reachable, and with it the bypass | Latent: Account facts always answer "none", so every delete is hard |
| **Mint ordering** (ZMVP-61) | No public operation exists for a founding that didn't commit | identity (P2), on the outbox | **The directory switch**. The ticket's fork (claim-then-mint, or deferred submission) is yours | The DID is minted and submitted before the Account row is stored |
| **Handle-namespace edges** (ZMVP-62) | Exactly one label before `.zurfur.app`. The suffix comes from configuration, not a literal. Reserved labels apply | identity (P4) | **Anyone else can reach a build**, then the switch | The apex is now refused. `x.api.zurfur.app` still passes, because only the leftmost label is checked. The suffix is hard-coded |
| **The https-only PDS redirect** (ZMVP-173) | The browser is sent only to an `https` authorization address | identity (built), web (the one-line relay check) | The backend half closes in 0.1. The web's check: **the first writes** (web P2) | The authorization endpoint passes the https-on-443 policy before the PAR (sign-in stack, reviewed). The web still relays the address verbatim |
| **The CSRF origin policy** (ZMVP-147, plus the two older gaps on my list) | Every cookie-surface write proves it is first-party where the browser's headers are visible, `just dev` included. The API refuses a write that carries no `Origin`, unless `Sec-Fetch-Site` says same-origin | web (form actions, the web tier's check); identity or the tech lead (the API guard; identity offered its P8) | **The first writes** (P2), not web's P9 or identity's P8. Every write route born after it is then guarded by construction (question 1) | The API lets a write with no `Origin` through, and has no `Sec-Fetch-Site` check. In dev, SvelteKit's check is off. The API can't see the browser's origin on a form action |
| **Handle-race cleanup** (ZMVP-209) | A founding that loses its handle leaves no published DID and no orphaned custody | identity (P2) | **The directory switch** | The loser of a race leaves an orphaned DID, its keys and its logged operation |

### Before alpha: what must be true beyond the card

**1. Secrets and key custody.** Owner: whoever owns deployment (the secret store; unowned today), identity (custody), each owning team (its secret's rotation), security (the inventory and review). Lands by: **the first secret in CI or the first staging deploy**, and complete before **alpha opens**.
- **The inventory**, each secret with an owner, a rotation procedure and a revocation drill:
  - the custody root key;
  - the OAuth token vault's key;
  - the production OAuth client's signing key. A confidential client authenticates with `private_key_jwt` and publishes its public keys (atproto OAuth spec);
  - the Den's private-data MAC key (den P2);
  - two database roles (moderation P3);
  - the hosted PDS admin password, which reaches every hosted account;
    - the reference PDS's admin script sends it as basic auth to `deleteAccount`, `updateSubjectStatus` and `updateAccountPassword` for any DID. Its docs don't say that outright: inferred;
  - per-repo PDS credentials;
  - blob-store credentials;
  - the scanning vendor's key;
  - a mailer, if moderation gets one;
  - the moderator passkey's one-time enrollment code;
  - CI deploy credentials;
  - **the DNS provider token for the wildcard certificate.** Let's Encrypt issues wildcards only through DNS-01, and warns that "putting your full DNS API credentials on your web server significantly increases the impact". So: a narrowly scoped token, or validation from a separate host.
- **One custody mechanism, one key per purpose.** Today one environment key seals both the DID custody blobs and the stored OAuth sessions. OWASP's Key Management Cheat Sheet: "a single key should be used for only one purpose." Architects' one KMS-backed custody, with a key id beside each wrapped secret, fits this if each purpose gets its own key under it: custody, OAuth tokens, the MAC, and later third-party tokens.
- **KMS provider.** Envelope encryption needs only a symmetric wrapping key, so any provider can do part 4. secp256k1 support matters only for the later step of signing the recovery key inside an HSM:
  - AWS `ECC_SECG_P256K1` signs, and returns DER signatures that must be converted to low-S;
  - Google's `EC_SIGN_SECP256K1_SHA256` exists at the HSM level only, and returns low-S signatures.
- **KMS key deletion is irreversible.** AWS waits 7 to 30 days, then the wrapped data "becomes unrecoverable". Deletion must be denied to the app's and the deploy's roles, with an alarm on any scheduled deletion and a written recovery procedure (architects place this at the last P).
- **The hosted PDS's server-wide rotation key.** Architects found that the reference PDS has one PLC rotation key for the whole server, and the attach step puts it at rank 2 on every hosted DID. That breaks the custody decision's first rule against a shared key. It must be answered in the pending Hosted Repos decision before the account-pds slot (identity P6).
- **Hygiene:**
  - no secret in the repository;
  - none in logs: OWASP's Logging Cheat Sheet lists tokens, passwords and keys as data that "should usually not be recorded directly in the logs";
  - none in CI output (OWASP Secrets Management: "Make sure that pipeline output does not leak secrets");
  - staging and production refuse every dev value committed in `.env.example`, not only the custody key: the PDS admin password, the PDS JWT secret, the PDS rotation key, Postgres `admin:password`;
  - the PDS admin credential stays outside the API process (identity P6).

**2. The deploy pipeline.** Owner: internal (CI), plus whoever owns deployment (the target and its environments). Lands by: **before the first secret enters CI**, which is no later than the first staging deploy.
- **Pin every action by full commit SHA.** That covers all eight in `ci.yml`, GitHub's own (`checkout`, `setup-node`) included, not only the six third-party ones the study counts. GitHub: "Pinning an action to a full-length commit SHA is currently the only way to use an action as an immutable release."
- **Make it enforced.** GitHub's repository policy has been able to require SHA pinning since 2025-08-15: "any workflow that attempts to use an action that isn't pinned will fail."
- **Dependabot for `github-actions`** keeps the pins current. Dependabot raises no vulnerability *alerts* for SHA-pinned actions, so its version PRs are the signal.
- **Deploy credentials through OIDC,** with no stored cloud keys: the token "is only valid for a single job, and then automatically expires".
- **Environments.** A `staging` and a `production` environment with you as the required reviewer, so "a job cannot access environment secrets until one of the required reviewers approves it". Deploys come only from `main` and tags.
- **Keep today's good defaults:**
  - `pull_request`, never `pull_request_target`;
  - `permissions: contents: read` by default, widened per job.
- **The cache.** A deploy build never restores a cache that a PR could have written. Internal's "save the cache from `main` only" also closes the poisoned-cache residual.
- **Container images** pinned by digest; PostgreSQL and Caddy are tag-only today.
- **The deployment checklist,** carried from my sign-in review and owned by whoever owns deployment:
  - an egress IP that no internal service trusts;
  - instance metadata on IMDSv2 only. AWS says it "protects against the vast majority of SSRF vulnerabilities";
  - cookie redaction in the proxy's logs;
  - no user content on any `zurfur.app` subdomain;
  - the sign-in callback routed straight to the API, checked with `scripts/web-smoke.sh`;
  - moderation's floor;
  - the production PDS must not run with the dev compose file's `PDS_DEV_MODE=true`. What that flag relaxes is *unverified*.

**3. Dependencies and advisories.** Owner: internal. Lands by: question 3 any time; the rest before **alpha opens**.
- **Today:**
  - `cargo-deny` checks advisories, licences, bans and sources on every PR, but it isn't a required check on `main`;
  - `deny.toml` ignores three advisories;
  - there is no Dependabot or Renovate, and no advisory check on the web's `yarn.lock`.
- **Policy** (question 3):
  - Internal's change: advisories block a PR only when it changes `Cargo.lock`, and a daily run on `main` notifies you;
  - `deny` becomes a required check on `main` before alpha;
  - every ignored advisory carries a reason and a review date, read at each end-of-release review;
  - the web's lockfile gets an advisory check (the tool is Internal's choice);
  - Dependabot security updates for `cargo`, npm/yarn and `github-actions`, all supported;
  - a triage clock once real people's data exists: a critical advisory in code that production reaches is fixed or mitigated within 72 hours of alpha opening; anything else by the next slot;
  - a new crate that touches crypto, parsing, the network or auth gets a security look on top of the approved-crate rules.
- Both `cargo-deny` and `cargo audit` read the RustSec database, so one tool is enough.

**4. Incident response basics.** Owner: security (the runbook), you (the incident lead), whoever owns deployment (the levers), moderation (lockdown), compliance (notification duties). Lands by: **anyone else can reach a build**, with one tabletop. A drill of the watcher path runs before **the directory switch**.
- **The reference.** NIST SP 800-61 Revision 3 (April 2025) sets incident response inside the CSF 2.0 functions.
- **A one-page runbook:** who decides (you), who assists (the Orchestrator and security), where reports arrive (the security contact), a severity scale, and the first moves for:
  - an exposed root or custody key;
  - an operation the directory watcher can't explain. Inside 72 hours "a higher authority rotation key can 'rewrite' history" (the PLC spec), so the alarm is only useful if someone acts within that window;
  - an exposed session store or OAuth tokens: revoke every session, revoke at each PDS, rotate the vault key;
  - a leaked CI, deploy or PDS admin credential;
  - a CSAM match (moderation's drill);
  - a vulnerable dependency in production;
  - a Private node served to a stranger.
- **The levers that must exist:**
  - revoke every session at once;
  - rotate each secret in the inventory;
  - restore from backup;
  - a "writes off" mode. Moderation's lockdown takes abilities away, so I ask moderation whether one of its four switches serves a security incident.
- **A security event log** (question 5): sign-ins, sessions created and revoked, key operations, handle changes and moderator acts (moderation's audit log covers the last). It never holds tokens, cookies or handles at `info`. OWASP allows a session id only as a hash: "consider replacing with a hashed value".
- **Notification clocks.** The duty to notify people of a breach is compliance's to name.

**5. Backups as a security property.** Owner: whoever owns deployment (database and blobs; architects plan it at the last P), identity (hosted-repo exports, its P6), compliance (retention). Lands by: **alpha opens**, with a restore watched once.
- **The standard.** NIST CSF 2.0 PR.DS-11: "Backups of data are created, protected, maintained, and tested."
- **Today:**
  - there are no backups;
  - blobs sit unencrypted in PostgreSQL;
  - nothing escrows the root key, so losing it loses every custodial key and every stored session.
- **Requirements:**
  - encrypted;
  - held under credentials the app host doesn't have, so a compromised API can neither read nor delete them;
  - a restore tested before alpha and at each stage gate, in an isolated environment that is wiped afterwards;
  - restoring is a privileged, audited act, because backups hold Private data;
  - wrapped keys in a backup are useless without KMS. So the KMS root is the single point of failure, and its deletion protection is part of the backup story.
- **Two points that need a ruling or a check:**
  - Hard-deleted data lives on in backups until they expire. The window is compliance's to set, and the privacy policy must state it (question 12).
  - Moderation's evidence store holds suspected CSAM. Whether it may sit in routine backups is a legal question, for compliance with moderation.
- **A PDS copy is publication, not backup** (The Den). The nightly export of hosted repos (identity's question 14) is what makes a restore possible.

**6. Session and token hardening.** Owner: identity (most of it), web (the cookie in the BFF), moderation (the console). Lands by: **anyone else can reach a build**. Tokens sealed under their own key: before **the first write-scope token** (identity P7).
- **Today on `main`:**
  - `zurfur.sid` is HttpOnly and SameSite=Lax, Secure only in staging and production, and host-only, with no `__Host-` prefix;
  - it expires after 7 days idle and has no absolute cap;
  - the id is rotated at sign-in;
  - sign-out deletes the session row but leaves the person's OAuth tokens stored and unrevoked;
  - abandoned sign-ins are never reaped.
  - The sign-in stack adds the browser-bound `__Host-zurfur.signin` cookie, revocation on an account mismatch, and the identity-only `atproto` scope.
- **Before anyone else can reach a build:**
  1. **`__Host-` on the session cookie** (question 4). OWASP lists it as "Recommended for session IDs". MDN: it needs `Secure`, no `Domain`, and `Path=/`.
  2. **An absolute lifetime beside the idle one** (question 4).
     - OWASP's examples (idle "15-30 minutes for low risk applications", absolute "between 4 and 8 hours") would sign artists out daily.
     - atproto lets a confidential client's session run longer: refresh tokens "limited to 180 days", access tokens "less than 30 minutes".
     - So the numbers are yours. A fresh sign-in guards the sensitive acts.
  3. **Sign-out** revokes and deletes the stored PDS tokens (identity P7). Add "sign out everywhere".
  4. **OAuth tokens and DPoP keys** sealed under their own KMS-wrapped key. Google's guidance for server-held tokens matches: "encrypt them at rest".
  5. **The production OAuth client:** confidential, `private_key_jwt`, its key in the store, rotated through the published key set with an overlap (identity P8).
  6. **Housekeeping:** abandoned sign-ins and orphaned OAuth sessions reaped; a database statement timeout. Both are residuals from my sign-in review.
  7. **Rate limits** on sign-in and anonymous lookups (identity P5).
  8. **The command line.** Its identity file is a self-asserted DID until it signs in through the browser (identity P8). Until then, its in-process user commands refuse any database that isn't dev.
  9. **The moderator** is minted only from a fresh passkey proof (moderation P3).
  10. **Invite codes** are single-use and unguessable, and redemption is rate-limited (identity P8).

**Exit (before alpha opens):**
- Every row of the security card is built, reviewed, and on `main`.
- The six areas above hold, each with a test, a file reference or a drill record.
- The threat model is current, and the alpha-readiness stage gate has no open must-fix.
- The directory switch's prerequisites are met: KMS, the watcher, mint ordering, race cleanup, tombstone durability, BYO checks and handle edges. The switch itself waits for its own review at A1.

## Alpha

- **A1: The directory switch, reviewed before it flips.**
  - Features: did-ops (submission on); security-asap (closing). Proposal: the security contact on the real domain.
  - Security builds:
    - the door review of identity's A1;
    - a live drill: a forged operation on a test DID in the local directory raises the watcher's alarm, and the runbook's recovery step is walked through.
    - the security contact on the real domain (question 6):
      - `/.well-known/security.txt`, served over `https`;
      - `Contact` and `Expires` are required, and `Expires` "less than a year into the future" is recommended;
      - an OpenPGP signature is recommended (RFC 9116).
  - Depends on: identity (A1), whoever owns deployment (the real domain).
  - Done when:
    1. the door review says "ship";
    2. the drill's alarm reached you;
    3. `curl https://<domain>/.well-known/security.txt` returns a valid file whose `Expires` is less than a year away.
- **Each alpha slot:** end-of-release reviews; a door review for the first hosted repo on the real domain (identity and architects).
- **Last A: Beta readiness** (the stage gate).
  - Proposal: the threat-model refresh for the beta's new boundaries (plugins, other sign-in sources, external mounts).
  - Done when:
    1. the refreshed model names each beta boundary with its prerequisites below;
    2. the plugin design study (plugin's last-alpha proposal) carries my review;
    3. the five mount rulings (question 10) are answered, or the mounts slot is not placed.

Exit: every alpha card's security pieces have passed review. The watcher has run a whole slot with every alarm explained (identity's exit too). No end-of-release must-fix is open.

## Beta

- **The plugin slot: first-party plugins only, pulled, with a kill switch** (plugin B1, the plugin-api card).
  - What must be true before it opens:
    1. one credential per first-party Golem, kept in the secret store and exchanged for short-lived, scoped tokens; revocation checked on every call; rotation an audited admin act (the decided token model; plugin's question 4);
    2. **pull only**, so nothing leaves Zurfur unsigned (plugin's question 3 (a));
    3. out of reach looks exactly like nonexistent, with per-key limits on lookups by DID (Auth Surfaces' threat pass, finding F1);
    4. `changelog:write` passes its mandatory security review (Plugin Security), covering spoofing, quota exhaustion, payload-cap bypass, sentence injection and cross-commission reach;
    5. the MVP-only "Account membership confers view" rule is gone before any Golem joins an Account (commission's and plugin's note);
    6. **a "suspend a Golem" moderator action** ships in the same slot (plugin's proposal). Moderation's publisher ban at its B3 is too late for Golems acting since B1;
    7. one Zurfur-held rotation key per Golem; the first Golem DID is a door (plugin B1);
    8. no plugin code in the first-party page; the Portal stays parked.
  - Done when: the plugin-surface review says "ship", with each of the eight lines tested or referenced.
- **The mounts slot: external storage** (den B2, proposed; alpha only if you pull it, at the earliest in the last alpha slot).
  - What must be decided before the slot (question 10), with the infrastructure that must exist first:
    1. **What Private means on storage Zurfur doesn't control.** Zurfur can enforce only what it serves. My proposal:
       - "Private" promises "Zurfur won't show it" (den's P5 wording);
       - an external node can't be Listed or Public until its bytes are copied into the gated blob store as a scanned snapshot;
       - so Zurfur never serves live third-party bytes to anyone but the owner.
    2. **Which endpoints:**
       - named providers with fixed API hosts only. OWASP's SSRF guidance allowlists when "the destination is known", and warns that domain allowlisting "alone does not prevent DNS rebinding";
       - no address a user types (NAS, WebDAV) until a network-layer egress guard exists. My sign-in review lists its absence as a residual.
    3. **Which scopes.** For Google Drive, `drive.file` is "Non-sensitive (recommended)". `drive` and `drive.readonly` are "Restricted": storing their data on servers needs a CASA security assessment "at least every 12 months". Whether `drive.file` can back a whole-folder mount is *unverified*.
    4. **Token custody:**
       - tokens sealed under their own KMS-wrapped key, one per mount;
       - revoked at the provider on unmount, Account deletion or suspension ("Revoke tokens as soon as they are no longer needed", Google);
       - never sent to the browser, a plugin or a log.
    5. **Proxy, not redirect.** Bytes pass through Atta after the hash check, so a viewer's address never reaches the provider. Each new hash is scanned before anyone but the owner sees it. Whether proxying to the owner alone carries a scanning duty is for compliance.
  - Also needed: an amendment to the boundary contract's class test for a third destination (den's ask). The fetches use the shared guarded client only.
  - Done when: the third-party-token door review says "ship"; den's done-when list passes; a mount whose provider revokes the token shows "unreachable" and holds no live token.
- **Other sign-in sources** (identity B1, proposal):
  - attach only after a fresh atproto sign-in;
  - one outside account attaches to at most one User;
  - never match on email (identity's rules).
  - Security reviews it as a takeover path and refreshes the threat model.
- **The changelog hash chain** (commission B2): reviewed as tamper evidence. The chain covers alpha entries (architects).
- **Last B: Release readiness** (the stage gate, with the external test). See question 7.
  - Scope: the web, the API, sign-in, the Den's access check, byte serving, custody and the moderator console.
  - Method: OWASP's testing guide (WSTG).
  - The bar is ASVS 5.0 level 2: "Most applications should be striving to achieve this level."
  - Done when: the testers' report is in; each finding is fixed, or carries your written risk acceptance; and the disclosure policy is drafted with compliance.

Exit: no third-party plugin, third-party push delivery or third-party token exists unless its prerequisites above shipped. The external test's must-fixes are closed.

## Release

- **R1: Open doors, with a way to tell us what's broken.**
  - Features: webhook-signing, marketplace (prerequisites only). Proposals: the disclosure policy, the external retest.
  - What must be true:
    1. **A disclosure policy** linked from `security.txt` as its `Policy`. It states:
       - scope, and what testers must not do;
       - response times;
       - safe harbor. OWASP: "Legal provisions such as safe harbor policies", and "Not threaten legal action against researchers".
       - The process follows ISO/IEC 29147:2018 (disclosure) and 30111:2019 (handling). iso.org marks both as "to be revised".
       - GitHub's private vulnerability reporting stays on, and GitHub can assign a CVE through a draft advisory.
    2. **The external retest is clean,** or each remaining finding carries your written acceptance.
    3. **Before any third-party plugin installs or any push delivery** (plugin R1 and R3, together):
       - per-install keys;
       - `standard-webhooks` HMAC-SHA256 signatures with a timestamp and id bound in;
       - SSRF-hardened delivery through the shared guarded client and the egress guard;
       - the trust ladder: self-serve Listed, Trusted as a Seal on the publisher, the publisher ban, reports on listings;
       - a leaked-key runbook;
       - rate-limit values set.
    4. **No bug bounty at 1.0** (question 8). OWASP: bounties "should only be used by organizations that already have a mature vulnerability disclosure process".
  - Done when:
    1. a test report sent through the policy's channel reaches you and is acknowledged within the stated time;
    2. the release review and the retest show no open must-fix.
- **After R1:** the same cadence per release. The threat model is refreshed at least once a year even without a new boundary (my proposal). The bounty question comes back once the disclosure policy has run and someone can triage.

Exit: the public can reach Zurfur, and also reach us.

## One-way doors, and the slot that closes each

| Door | Why it can't be undone | Owner | Built | Closes |
|---|---|---|---|---|
| Submitting to plc.directory, and the key list in each genesis operation | Every operation is public for good | identity | P2 to P8 | A1, after its door review |
| Real people's custody: the sealed format, key ids, and the KMS root | A deleted KMS key "cannot be recovered", and every custodial key goes with it | identity, architects | P2 to P8 | A1 |
| The hosted PDS's rotation key at rank 2 on every hosted DID | It lands in public operations, and one key would span every hosted identity | identity (the Hosted Repos decision) | P6 | A1 |
| The user-content domain, the moderator console's domain, the PDS's domain | Published names are permanent. Each must be its own registrable domain to be a boundary | posts, moderation, identity (web hosts them) | P3 to the last P | A1 |
| The first stored third-party OAuth token | Zurfur starts holding other companies' credentials, with their assessments and revocation duties | den, identity | B2 | B2 |
| The first Golem DID | Public and permanent | plugin, identity | B1 | B1 |
| A published disclosure policy with safe harbor | A public legal promise | security, compliance | last B | R1 |

**Not doors:** the KMS provider (keys can be re-wrapped), session lifetimes, the `__Host-` rename (it signs everyone out once), SHA pins, the advisory policy, the `security.txt` contact.

## Cross-team dependencies (what you need, from whom, by which slot)

| What security needs | From | By |
|---|---|---|
| The CSRF origin policy at the first writes: the web tier's check (dev included) and the relay check | web | P2 |
| The API guard refusing a cookie-surface write with no `Origin` | identity or the tech lead (the consolidator assigns) | P2 |
| The soft-delete gate in the slice that makes Account facts real; mint ordering, race cleanup, tombstone durability | identity | P2 |
| The outbox and the worker that mint ordering and tombstones run on | architects (and identity) | P2 to P3 |
| The scan gate, the report path, lockdown, the console's passkey, and two database roles | moderation | P2 to P5 |
| An answer: does one of lockdown's switches serve as "writes off" in a security incident? | moderation | the last P |
| The gated blob store, and public bytes on a separate registrable domain | posts (the tech lead confirms ownership) | P2, P3 |
| KMS custody with one key per purpose, the watcher, BYO checks, handle edges, token sealing and revocation, the production client, command-line login, invite codes | identity, with architects for the custody port | P2 to P8 |
| The answer on the hosted PDS's shared rotation key | identity (the Hosted Repos decision), from architects' finding | before P6 |
| CSP and headers on every response; the session cookie in the BFF | web | the last P |
| SHA-pinned actions with enforcement, Dependabot, the advisory policy, the `deny` required check, an advisory check on the web's lockfile | internal | before the first secret in CI; the rest before alpha |
| The secret store, OIDC deploys, protected environments, the deployment checklist, encrypted off-host backups with a watched restore, the egress guard | **no one owns deployment yet** (architects and identity also flag this) | the first staging deploy; the last P |
| Breach-notification clocks, backup retention against deletion, the evidence store in backups, the safe-harbor text, scanning duty on proxied bytes, Google's assessment | compliance | the last P; the last B for safe harbor |
| The mounts' five rulings carried in den's design; the guarded client kept shared | den, identity | before the mounts slot |
| Per-install keys, signing and the trust ladder together, before any third-party plugin | plugin, moderation | R1 |
| Budget for the external test, which may need a contracting entity (*unverified*) | you | the last B |

## Not on the board (proposals, each with why)

1. **The threat model** (P2), so every later review reads against one picture, not slice by slice.
2. **The secrets inventory, and one key per purpose** (P2 to the last P). Today one key does two jobs, and nobody has listed what Zurfur will hold.
3. **The incident runbook and a tabletop** (the last P). The 72-hour recovery window is only worth having if someone knows the moves.
4. **A security event log** (the last P). Without one, an incident can't be reconstructed. Its privacy limits are question 5.
5. **The security contact before release.** GitHub's private vulnerability reporting now, because the repository is already public and anyone can read the code. `security.txt` at A1. The full policy at R1.
6. **CSRF at P2 instead of P9** (a re-timing, not a new item). It guards every write route built from P2 on.
7. **`__Host-` on the session cookie, and an absolute lifetime** (the last P).
8. **Dependabot, an advisory check on the web's lockfile, and `deny` as a required check** (before alpha).
9. **Encrypted, off-host backups with a watched restore**, as a security property and not only an operations one (the last P). Architects carry the build.
10. **An automated scan of staging before alpha** (the last P). It's cheap, and it catches header and configuration slips before strangers do. The tool is the deployment owner's choice.

## Questions for the Engineer (each pre-answered)

1. **Gate each security item by the event it guards, and land the CSRF origin policy and the soft-delete gate with the first writes (P2)?** The card says the items are "sequenced ahead of feature work". I read that as ahead of the work each one guards. Identity's order fits that reading. Web's P9 for CSRF doesn't, because writes start at P2. **Proposed: yes.**
2. **The CSRF origin rule: a single exact origin, or an allowlist?** **Proposed:** a single exact origin per environment, from config, under the one-origin ruling.
   - A write with no `Origin` is refused, unless `Sec-Fetch-Site` is `same-origin`. This holds in `just dev` too.
   - OWASP makes Fetch Metadata a primary defense and says an `Origin` fallback "is a mandatory requirement". SameSite "does not replace a proper CSRF defense".
   - Then close the ticket with a design-sync of the Auth Surfaces page.
3. **Internal's question 3: should advisories block only PRs that change `Cargo.lock`?** **Proposed: yes**, with these conditions:
   - the daily run reaches you;
   - `deny` becomes a required check on `main` before alpha;
   - every ignored advisory carries a reason and a review date;
   - the web's lockfile gets the same treatment;
   - after alpha opens, the 72-hour clock applies to critical advisories in code that production reaches.
4. **Session lifetimes and the cookie name?** **Proposed:**
   - rename to `__Host-zurfur.sid`;
   - keep 7 days idle, and add a 30-day absolute cap;
   - require a fresh sign-in for enrolling or changing an owner key, changing a handle, deleting an Account, transferring ownership, and moderator acts (by passkey).
5. **What does the security event log hold, and for how long?** **Proposed:**
   - sign-ins, session creation and revocation, key operations, handle changes and moderator acts;
   - keyed by DID, never by handle;
   - no token or cookie, and a session id only as a hash;
   - no IP address beyond the window compliance allows. Compliance sets the retention.
6. **When does the security contact go live?** **Proposed:**
   - GitHub's private vulnerability reporting now (one setting on the public repo);
   - `security.txt` at A1, on the real domain;
   - the full disclosure policy with safe harbor at R1, its text drafted with compliance in the last beta slot.
7. **The external test: when, what, and with what budget?** **Proposed:**
   - the last beta slot, against ASVS 5.0 level 2, with a retest before R1;
   - if there's no budget, an internal ASVS level 2 self-assessment by this team at max effort, plus your written risk acceptance for the gap.
8. **A bug bounty at 1.0?** **Proposed: no.** Run the disclosure policy first, and revisit after R1, once there is budget and someone to triage.
9. **Where does the threat model live?** **Proposed:** a corpus page under `project/`, because design lives in the corpus. The release reports cite it.
10. **External-storage mounts: the five security rulings** (needed before the mounts slot, wherever you place it). **Proposed:**
    - (1) Private means "Zurfur won't show it", and an external node widens only as a scanned snapshot in the gated store;
    - (2) named providers only, no typed addresses, until an egress guard exists;
    - (3) the narrowest scope; for Drive, `drive.file` if it can back the mount, otherwise a decision on the restricted-scope assessment;
    - (4) tokens under their own key, revoked at unmount, deletion and suspension;
    - (5) proxy through Atta, never redirect.
11. **The beta's plugin credential and its kill switch?** **Proposed:**
    - accept one credential per first-party Golem, pull only (plugin's questions 3 and 4);
    - ship "suspend a Golem" in the same slot;
    - no third-party plugin until per-install keys, signing and the trust ladder all ship at R1.
12. **How long do deleted data live in backups?** **Proposed:** the shortest window that still allows the restore drill at each stage gate. Compliance names the number, and the privacy policy states it.
13. **Who owns deployment?** Many security prerequisites hang on it: the secret store, OIDC, the environments, the checklist, backups, the egress guard. It isn't a domain fork, but it blocks the runway. **Proposed:** internal owns the pipeline, and a named owner holds the environments and operations from the first staging deploy. The tech lead names that owner when the consolidated plan is cut.

## Sources (fetched 2026-10-10)

- RFC 9116, security.txt: https://www.rfc-editor.org/rfc/rfc9116.txt
- ISO/IEC 29147:2018: https://www.iso.org/standard/72311.html · ISO/IEC 30111:2019: https://committee.iso.org/standard/69725.html
- OWASP Vulnerability Disclosure Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Vulnerability_Disclosure_Cheat_Sheet.html
- NIST SP 800-61 Rev. 3: https://csrc.nist.gov/pubs/sp/800/61/r3/final · NIST CSF 2.0 (PR.DS-11): https://nvlpubs.nist.gov/nistpubs/CSWP/NIST.CSWP.29.pdf
- Threat Modeling Manifesto: https://www.threatmodelingmanifesto.org/ · OWASP Threat Modeling Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Threat_Modeling_Cheat_Sheet.html · STRIDE: https://learn.microsoft.com/en-us/azure/security/develop/threat-modeling-tool-threats · LINDDUN: https://linddun.org/
- GitHub, secure use of Actions (SHA pinning): https://docs.github.com/en/actions/reference/security/secure-use · SHA-pinning policy (2025-08-15): https://github.blog/changelog/2025-08-15-github-actions-policy-now-supports-blocking-and-sha-pinning-actions/ · OIDC: https://docs.github.com/en/actions/concepts/security/openid-connect · environments: https://docs.github.com/en/actions/reference/workflows-and-actions/deployments-and-environments · Dependabot ecosystems: https://docs.github.com/en/code-security/dependabot/ecosystems-supported-by-dependabot/supported-ecosystems-and-repositories · private vulnerability reporting: https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/configuring-private-vulnerability-reporting-for-a-repository
- CVE-2025-30066 (tj-actions/changed-files): https://services.nvd.nist.gov/rest/json/cves/2.0?cveId=CVE-2025-30066
- RustSec: https://rustsec.org/ · cargo-deny advisories: https://embarkstudios.github.io/cargo-deny/checks/advisories/index.html
- OWASP ASVS 5.0 levels: https://github.com/OWASP/ASVS/blob/master/5.0/en/0x03-What-is-the-ASVS.md (release 5.0.0, 2025-05-30: https://github.com/OWASP/ASVS/releases)
- OWASP cheat sheets: Session Management https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html · CSRF https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html · SSRF https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html · Secrets Management https://cheatsheetseries.owasp.org/cheatsheets/Secrets_Management_Cheat_Sheet.html · Cryptographic Storage https://cheatsheetseries.owasp.org/cheatsheets/Cryptographic_Storage_Cheat_Sheet.html · Key Management https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html · Logging https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html
- MDN, Set-Cookie (`__Host-`): https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie
- RFC 9700, OAuth 2.0 Security BCP: https://www.rfc-editor.org/rfc/rfc9700.html
- atproto OAuth: https://atproto.com/specs/oauth · permissions: https://atproto.com/specs/permission · did:plc: https://web.plc.directory/spec/v0.1/did-plc
- AWS KMS key specs: https://docs.aws.amazon.com/kms/latest/developerguide/symm-asymm-choose-key-spec.html · Sign (DER): https://docs.aws.amazon.com/kms/latest/APIReference/API_Sign.html · key deletion: https://docs.aws.amazon.com/kms/latest/developerguide/deleting-keys.html · Google Cloud KMS algorithms: https://docs.cloud.google.com/kms/docs/algorithms
- AWS IMDSv2: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/configuring-instance-metadata-service.html · https://aws.amazon.com/blogs/security/defense-in-depth-open-firewalls-reverse-proxies-ssrf-vulnerabilities-ec2-instance-metadata-service/
- Let's Encrypt challenge types: https://letsencrypt.org/docs/challenge-types/
- Google Drive scopes: https://developers.google.com/workspace/drive/api/guides/api-specific-auth · restricted-scope verification: https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification · OAuth best practices: https://developers.google.com/identity/protocols/oauth2/resources/best-practices
- Bluesky reference PDS admin script: https://raw.githubusercontent.com/bluesky-social/pds/main/pdsadmin/account.sh
