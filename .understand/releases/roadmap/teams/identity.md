# Roadmap: identity

Lead: Fox · QA: Magpie · Stages covered: pre-alpha (P2 to P8), alpha (A1 to A3), beta (B1 to B3), release (R1)

## In one screen

- **After 0.1:** you sign in with your real handle through one hardened resolver. Accounts are founded and renamed inside `*.zurfur.app`. Nothing Zurfur signs reaches the public DID directory.
- **End of pre-alpha:** every identity item on the "security ASAP" card is built and reviewed. A DID is published only after its row is stored, and a lost handle race leaves nothing behind. Tombstones survive outages, and a brought domain is checked both ways before anything is minted. Zurfur's root key sits in a KMS, and your own owner key ranks above every Zurfur key. Characters are real. Accounts get repos. Public Characters carry their claim pair. The public User Page and its rate limits run, and production sign-in with invite codes works on staging. All of it has run against a local directory and a local PDS.
- **End of alpha:** A1 turns on the real directory. The first DIDs Zurfur publishes already carry the final key list, so nothing is re-minted. Then come alts (user-linking), shared Characters, Character handles and the command-line login without a browser. Every identity card in the alpha column is done.
- **End of beta:** other sign-in sources attach to an existing User (your answers of 2026-10-07 and 10-08). Golems and Seals get their identities, and the hosted-repo credential is hardened.
- **End of release:** credible exit is proven end to end. A person can take an Account or a public Character to another PDS with their own key. Name history and data export ship, and a directory replica audits plc.directory.
- **What blocks the most:** four questions: how a person holds an owner key (Q2), when it's required (Q3), what an Account's repo holds (Q4), and where Zurfur's PDS lives (Q6). P3 and P6 can't start without your answers.

## Pre-alpha

Sizes are identity's share only, in the plan's units (S = 1, M = 2, L = 3), and rough. Every slot gets a security review by the Designer lane; identity's whole surface touches authentication, the data boundary or DID handling.

- **P2: Every DID Zurfur signs is safe to publish** (about 9 units).
  - **Features:**
    - `security-asap`: mint ordering, handle-race cleanup, tombstone durability. Also key custody, part 1: the root key moves behind a versioned seam, so keys can be re-wrapped later.
    - `did-ops`: the outbox's background worker (deferred in July), with a publish status per DID.
    - `fact-deletion`: the Account half of the soft-delete gate. Today the Account facts check always answers "none", so every Account delete is a hard delete and frees the handle at once.
    - `accounts`: `Account.id` removed from the Account contract (the 0.1 TDD parks this "after 0.1").
    - Design work, in the Designer lane, for your ruling: the pending decision page on hosted repos (attach, custody, the write credential), and the owner-key decision (Q2 to Q6).
  - **Findings that drive it** (from reading the code, not from running it):
    - Founding mints and submits the DID before it stores the Account (`application/src/account/create.rs`; the minter submits inside `mint`).
    - The founding pre-check skips soft-deleted Accounts (`adapter-pg/queries/account/find_did_by_handle.sql`), but the unique index covers them. So founding with a deactivated Account's handle mints a DID and then fails to store it. No race is needed.
    - Once submission is on, either path leaves a published DID that names a handle no Account holds.
    - A failed tombstone is logged and dropped (`application/src/account/delete.rs`).
    - A tombstone is sent before the delete's transaction commits, because the unit of work commits only when the use case returns. If that commit fails, the DID is dead and the Account lives on.
  - **Depends on:**
    - The tech lead or internal: a local PLC directory in the dev stack.
    - Commission: what counts as an Account's facts on their side (Q5).
  - **Done when**, on `just dev` with the local directory:
    1. Two foundings fired at once for one new handle make one Account, and the local directory holds one DID naming that handle.
    2. Founding with a deactivated Account's handle is refused, and nothing is minted.
    3. Stop the directory and found an Account: it shows on your Accounts page. Start the directory: within a minute its DID resolves there.
    4. The same holds for deleting an empty Account: its tombstone lands after the restart.
    5. An Account with any fact you rule (Q5) can only be soft-deleted, with a named test per fact kind that exists by then.
    6. The Account contract has no `id`, and the Accounts pages still work.

- **P3: Your keys, and real Characters** (about 10 units).
  - **Features:**
    - `did-ops`: the final key list, owner first; handle-less mints, with keys per Character.
    - `security-asap`: key custody, part 2. Your owner key is made in your browser, and Zurfur never stores it.
    - `accounts`: founding asks for your owner key.
    - `characters`: stored in PostgreSQL, created, and deleted by facts (tombstone, or hard delete). This replaces 0.1's pinned empty answer.
    - `fact-deletion`: the Character half of the gate.
  - **Depends on:**
    - Your rulings on Q2 and Q3, from P2's design work.
    - Web: the key screens and the Character form.
    - Den: the character node's payload.
    - Commission: the Keeper-consent handshake for slots, which reads the Keeper. Commission builds it.
  - **Done when:**
    1. Found an Account: your browser makes the key and you save it. The local directory lists your key first, then Zurfur's, and `just db-shell` finds no copy of yours.
    2. Create a Character in My Den: it shows under `characters`, Private, with the key list Q3 rules.
    3. Delete a Character with no facts: it's gone, and its DID is tombstoned in the local directory. One in an active slot can only be tombstoned, and it leaves your Den.

- **P4: Handles you can trust** (about 7 units).
  - **Features:**
    - `security-asap`: brought-domain (BYO) verification; the handle-namespace edge cases.
    - `handle-change`: renames to and from a brought domain, which are refused today. Success comes only once the directory has the new handle, and you rule what a quarantined handle answers.
    - `accounts`: founding with a brought domain.
  - **Finding:** founding stores any well-formed domain without checking who controls it. `ensure_handle_claimable` in `application/src/account.rs` checks only uniqueness and quarantine. So anyone can found an Account as someone else's domain, and the domain's real owner can then never claim it in Zurfur. Rename already refuses brought domains.
  - **Edge cases the slice settles, each with a test:**
    - `zurfur.app` itself, `a.b.zurfur.app`, reserved labels and `xn--` labels.
    - A deactivated Account's handle. The deletion decision says it "stays reserved and still resolves", but the well-known route answers 404 today.
    - A hard-deleted Account's handle. The deletion decision frees it at once; the handle-change decision says it stays reserved. Which one holds is your call, asked inside the slice.
  - **Depends on:**
    - 0.1's hardened resolver, on `main`.
    - Your answer to Q7: a dev-only way to reach the local directory.
    - Web: the "add this DNS record" step.
  - **Done when:**
    1. Found an Account as a domain you don't control: it's refused, and nothing is minted.
    2. With a test domain you control: Zurfur shows the record to add. Once you add it, founding completes, and the handle checks out both ways.
    3. Rename to a `*.zurfur.app` label and back: it works, and another Account can't take the vacated label during quarantine.
    4. The edge-case tests are named in the PR and pass.

- **P5: The public User Page** (about 8 units).
  - **Features:**
    - `user-pages`: `/profile/<handle or DID>`, for any handle that resolves, signed out too. It mirrors the Bluesky profile, and lists the memberships you chose to list (the "list on profile?" choice, made at founding and at accepting an invitation). Your public Characters appear once P7 lands.
    - `login`: rate limits and a cache for anonymous lookups and for sign-in, which your ruling of 2026-10-08 ties to the User Page.
    - Proposal: avatars served through Zurfur, so a viewer's address never reaches the subject's PDS host. The 0.1 TDD parks this until the User Page.
  - **Depends on:**
    - Den: `/profile` and the derived profile view (the My Den rulings on profiles).
    - Web: the page.
    - Whoever owns two-dimensional reputation: the ratings on the page.
  - **Done when:**
    1. Signed out, open a friend's handle: their name and avatar show, and the network tab shows the avatar coming from Zurfur.
    2. An unknown handle shows "Nothing here".
    3. Your page lists only the Accounts you ticked.
    4. Thirty sign-ins from one address in a minute meet the limit message.

- **P6: Accounts get a repo** (about 9 units).
  - **Features:**
    - `account-pds`: mint, then attach, in one founding flow. One directory update adds the PDS service and its keys. The per-repo password is sealed, and sessions are opened only when needed.
    - `security-asap`: key custody, part 3. The PDS admin credential lives outside the app process.
    - `accounts`: a soft delete deactivates the repo, and a hard delete removes it, then tombstones the DID. An ownership transfer moves the top key to the new Owner.
    - Proposal: a nightly export of every hosted repo (Q14).
  - **Depends on:**
    - The tech lead or internal: a Zurfur PDS in the dev stack. The adapter tests already run one.
    - Your answers to Q4 and Q6.
    - The tech lead: the storage-routing crate, if the dual writes here get convoluted (your note 1). I expect this is where the pain shows first.
  - **Done when:**
    1. Found an Account. The local directory lists your key, the PDS's key and Zurfur's key, in that order, with the local PDS as its service, and the PDS describes the repo.
    2. Soft-delete it: the PDS reports it deactivated.
    3. The API process's environment holds no PDS admin password.
    4. A restore from last night's export works once.

- **P7: Public Characters, and the claims that say whose they are** (about 12 units).
  - **Features:**
    - `character-presence`: publicize creates the repo, attaches it, and writes the `owned` half and nothing else. De-publicize tears the repo down, and the Character keeps its DID.
    - `claims`: the `owner` half goes in your own repo. A half alone shows "claimed, unconfirmed", and withdrawing means deleting your half.
    - `login`, in three parts:
      - The write-scope request at first publish, covering Zurfur's record types and the ACP's.
      - The full-access fallback, as its own consent moment.
      - Sign-out revokes the stored PDS tokens. This was parked until PDS writes use them, and now they do.
  - **Depends on:**
    - Vulpes: the paired-claim mint and the pair check, plus frozen canonical bytes. Vulpes's `src/acp.rs` still calls these "the next code lines on the roadmap" (read 2026-10-10).
    - Web: the publicize and consent screens.
    - Q7's dev route, so a test user on the local PDS stands in for your real Bluesky repo.
  - **Done when**, as a test user on the local PDS:
    1. Publicize a Character: the PDS asks once for write access. The Character's repo holds the `owned` half, yours holds the `owner` half, and Zurfur shows it as yours.
    2. Delete your half: it shows "claimed, unconfirmed".
    3. De-publicize: the repo is gone, and the Character is Private on the same DID.
    4. Sign out: the PDS lists no session for Zurfur.

- **P8: Ready for a real domain** (about 13 units).
  - **Features:**
    - `login`, in two parts:
      - The production OAuth client: its hosted metadata, a signing key and a public redirect.
      - The command line's sign-in through the browser, which you ruled for the alpha on 2026-10-08.
    - `security-asap`: key custody, part 4.
      - The KMS adapter, for whichever provider deployment picks.
      - The directory watcher, which alarms on any operation Zurfur didn't sign. The custody decision puts it in v1.
    - Proposal: the invite-code gate (Q11).
    - Proposal: moderation's identity states (Q13).
    - Then a security review of everything identity built since 0.1, in the Designer lane at xhigh.
  - **Depends on:**
    - The tech lead or deployment: a staging host on the real domain, the KMS provider, and the PDS on its own domain.
    - Moderation: which states they need.
    - You: Q11.
  - **Done when**, on staging:
    1. You sign in with your real handle through the production client.
    2. A new handle without an invite code is turned away politely, and one with a code gets in.
    3. `zurfur session login` opens your browser and signs the command line in.
    4. An operation that Zurfur didn't sign, on a DID in the local directory, raises the alarm.
    5. The review lists no open must-fix.

**Exit: what holds before alpha opens.**
- Every identity item on the security card is built and has passed its review:
  - key custody (the KMS, and owner keys above Zurfur's);
  - mint ordering;
  - BYO-domain verification;
  - handle-race cleanup;
  - the https-only PDS redirect, which 0.1 closes under the answer that rode on your signature;
  - tombstone durability;
  - the handle-namespace edge cases;
  - identity's halves of the soft-delete gate (Accounts and Characters).
- Minting, attaching and claims have all run against a local directory and PDS. Pointing them at plc.directory is a setting, plus your review.
- Identity's contract is what the first alpha tag freezes, with `Account.id` already gone.
- Production sign-in, invite codes and the anonymous rate limits work on staging.

## Alpha

- **A1: Identity live on the real domain** (about 4 units).
  - **Features:**
    - `did-ops`: directory submission on, against plc.directory, with the watcher live.
    - `account-pds` and `character-presence` go public: the PDS on its own domain, with the nightly export running.
    - The lexicons identity ships, published from Zurfur's lexicon DID when their feature ships (the lexicon registry's "publish late" rule).
  - **Depends on:** deployment, and moderation's floor (your ruling that no build anyone else can reach ships before that floor stands).
  - **Done when:**
    1. Found an Account on the real domain: plc.directory lists your key first and names the handle.
    2. `https://<handle>/.well-known/atproto-did` returns the same DID.
    3. A public Character's claim pair checks out with vulpes's checker, with no Zurfur code in the loop.
- **A2: Alts, and Characters an Account looks after** (about 6 units).
  - **Features:**
    - `user-linking` (Q10).
    - `characters`: shared Characters. The Account holds the top key, the claims name the real owner, and the Character is mounted into each member's Account.
  - **Depends on:** web, and den (the Account's `characters` mounts).
  - **Done when:**
    1. Link two of your test handles. Each User Page shows the other only after both sides confirm, and when either side deletes its half, both stop showing it.
    2. A shared Character shows up in every member's Account folder.
- **A3: Character handles, and the command line without a browser** (about 5 units).
  - **Features:**
    - `character-presence`: Character handles, if Q9 rules them cheap ("cheap, alpha; otherwise beta").
    - `login`: the command line signs in without a browser, which you asked for "right after" the browser version. Whether Bluesky's servers support the device-code flow is unverified; the fallback is a code shown on another device.
  - **Done when:**
    1. A public Character answers at its `*.zurfur.app` handle, both ways.
    2. `zurfur session login` signs in from an SSH session with no browser.

**Exit:**
- Every identity alpha card is done: `login`, `did-ops`, `user-pages`, `accounts`, `handle-change`, `characters`, `claims`, `character-presence`, `account-pds`, `user-linking`.
- The watcher has run a whole slot with no operation it couldn't explain.
- A hosted repo has been restored once from its nightly export on the real domain.

## Beta

- **B1: More ways to sign in, attached to you** (proposal).
  - Google, Discord, Telegram or email attach to an existing User, then sign that User in. This follows your answers of 2026-10-07 and 10-08 (Q3 (b) and Q5 (c) of the unified-identity investigation).
  - A private link list is kept for sign-in. A public "this account is this DID" claim, attested by Zurfur, exists only when the person publishes it.
  - **Rules:** attach only with a fresh atproto sign-in; one outside account attaches to at most one User; never match on email.
  - **Depends on:** vulpes, for the claim's payload, and a ruling on who attests.
  - **Done when:** attach a test Discord account, sign out, sign in with Discord, and land as the same DID. Attaching that Discord account to a second User is refused.
- **B2: Identities for plugins and Seals.**
  - **Features:** `plugin-api` (Golem DIDs, the Golem's credential, the act-as boundary for its admin); `seals` (Zurfur's attesting DID and its key custody; verification as a Seal); `achievements`, for Character levels if you rule them in.
  - **Depends on:** plugin and the Seals owner, who lead these features. Identity supplies minting and custody.
  - **Done when:** a test Golem gets its own DID, signs in with its own credential, and can't act as its admin.
- **B3: Keys you can lean on** (proposals).
  - The hosted-repo credential's follow-up hardening: keep refresh tokens and discard the password.
  - Character transfer and sale: key rotation plus a claim swap, final after the 72-hour window.
  - A screen for more than one person administering a Character.
  - Confusable-name checks in place of the `xn--` ban (the punycode decision's upgrade path).
  - **Done when:** a Character moves to another User, and the old Keeper's undo works within 72 hours and fails after.

**Exit:** sign-in sources attach without opening a takeover path, and the beta's plugin and Seals features have the identities they need.

## Release

- **R1: Credible exit, proven** (proposals: what "used for real work" demands).
  - Take an Account or a public Character to another PDS with your own key, tested end to end on the real network.
  - Account name history, read from the directory's log, and an export of your own data. Both are on the handle-change decision's backlog for after v1.
  - A read replica that audits plc.directory (the custody decision's deferred item).
  - An independently operated PDS option, which restores the "two independent failures" property the alpha gives up.
  - A support runbook for lost owner keys.
  - The compliance review's identity items. The main one: the public directory keeps every operation forever, including old handles, and erasure requests have to be answered against that.
  - **Done when:** a test Account leaves Zurfur's PDS, keeps its DID and handle elsewhere, and Zurfur's copy goes quiet.

## One-way doors, and the slot that closes each

| Door | Why it can't be undone | Built | Closes |
|---|---|---|---|
| The key list in every published genesis operation, and how owner keys are made (Q2, Q3) | The public directory keeps every operation forever. A wrong list means re-minting, and with it a new identity | P3 | A1 |
| Turning on submission to plc.directory at all | Each DID's creation time is public forever, deleted DIDs included | P2 to P8 | A1 |
| Where Zurfur's PDS lives (Q6) | Its address goes into every hosted DID document. Moving it costs one operation per DID, plus a repo move | P6 | A1 |
| The handle namespace: the labels issued under `*.zurfur.app`, the reserved list, the quarantine rule, and whether Characters share it (Q9) | A label given to a real person can't be taken back without breaking their links | P4 | A1 for Accounts; A3 for Characters |
| Identity's contract spellings: Account, session, `/me`, Character, handle change, the User Page, invite codes | Your ruling ends the pre-release period at the first alpha tag | P2 to P8 | A1 |
| Stored identity data: actors, Accounts, Characters, custody (the sealed keys and their format), handle changes and quarantine, the outbox | Real people's data from A1 on. A lost custody key can't be recovered | P2 to P6 | A1 |
| The claim halves as Zurfur writes them: field shape and write order (both still open rows; the record type is the ACP's) | The network archives published records even after they're deleted | P7 | A1 |
| What an Account's repo holds, and its lexicon (Q4) | Lexicons change only by addition once published | P6 | A1 |
| The user-link record and its lexicon (Q10) | Same reason | A2 | A2 |
| Starting the alpha on an empty database (Q8) | It deletes your pre-alpha Accounts and Characters | — | A1, after your review |

**Not doors:** the hosted-repo password (rotated by reset), the KMS provider (keys can be re-wrapped), rate-limit numbers, the invite rule.

## Cross-team dependencies (what you need, from whom, by which slot)

| What identity needs | From | By |
|---|---|---|
| A local PLC directory, then a local PDS, in the dev stack and in CI | Internal or the tech lead | P2 (directory); P6 (PDS) |
| What counts as an Account's fact on their side (a commission brought into the Account, invoices), behind a read port | Commission | P2 |
| The Keeper-consent handshake for slots (commission builds it; identity supplies the Keeper and the Character's facts) | Commission | P3 |
| The key screens, the Character form, the DNS step, the User Page, publicize and consent, claims shown, invite redemption | Web | P3, P4, P5, P7, P8 |
| `/profile` and the derived profile view; the character node; shared-Character mounts | Den | P5; P3; A2 |
| Ratings on the User Page | The owner of two-dimensional reputation (unassigned, as I read the board) | P5 |
| The paired-claim mint, the pair check, frozen canonical bytes, the custody read | Vulpes | P7 |
| The storage-routing crate, if hosted-repo dual writes get convoluted | Tech lead | P6 |
| A staging host on the real domain, the `*.zurfur.app` wildcard certificate, the KMS provider, the PDS on its own domain, an egress firewall | Tech lead, deployment | P8; live at A1 |
| Which states and hooks moderation needs (Q13) | Moderation | P8 |
| A security review of each slot, and the stage-end review | Security (Designer lane) | Every slot; P8 |
| Who holds Zurfur's labeler and attester keys | The Seals owner | B2 |

**What others need from identity**, for the consolidator:
- **Characters (P3):** commission's slots.
- **Write scopes at first use (P7):** friendship and blocks, whose records live in users' own repos. Friendship needs the "graph" permission set that the publish-consent decision names.
- **Moderation's identity states (P8):** moderation's floor.
- **Golem identities (B2):** the beta's plugin surface.

**Security-card items I don't own:**
- The CSRF origin policy: web and the tech lead. The API's guard lets a request with no `Origin` through, and it sits on the sign-in routes, so I'll take the API half in P8 if the consolidator assigns it.
- General hardening: the tech lead.
- CSAM scanning, a report path and the gated blob store: moderation and posts.
- The commission half of the soft-delete gate: commission.

## Not on the board (proposals, each with why)

1. **The invite-code gate (P8).** Your stage ruling makes alpha invite-only through earned codes, and no card holds it. A User is created at first sign-in, so the gate is identity's.
2. **The command line's sign-in: through the browser in P8, without one in A3.** You ruled it on 2026-10-08; no card holds it.
3. **A local directory and PDS in the dev stack, plus a dev-only way to reach them (P2, P4, P6; Q7).** Every publishing path should run before it becomes a door, without writing junk into your real Bluesky repo.
4. **Avatars served through Zurfur (P5).** The 0.1 TDD parks the viewer-address leak until the User Page.
5. **A nightly export of every hosted repo (P6; Q14).** "Routine CAR export" is a hard requirement of hosted repos (the Characters on ATProto decision).
6. **Moderation's identity states (P8; Q13):** moderator sign-in, a suspended state, and a takedown on hosted repos.
7. **More sign-in sources (B1).** Your answers place them after the alpha.
8. **The hardening tier (B3):** refresh-token custody, Character transfer, a screen for more than one administrator, confusable checks.
9. **Credible exit, proven (R1):** name history, data export, a directory replica, an independent PDS. Each is a deferred item from a decided page.

## Questions for the Engineer (each pre-answered)

- **Q1. Do I read the security card's undefined items right?**
  - The card names them without defining them:
    - **Mint ordering:** a DID is published only after the row it belongs to is stored.
    - **Handle-race cleanup:** a founding that loses a handle, by a race or by the soft-deleted gap above, leaves no published DID behind.
    - **BYO-domain verification:** a brought domain is checked both ways before anything is minted or stored, at founding and at rename.
  - Tombstone durability (the deletion decision's follow-up), the handle-namespace edge cases, and the Account and Character halves of the soft-delete gate are identity's too.
  - **Proposed answer:** yes, as read.
- **Q2. How does a person hold an owner key?**
  - (a) One recovery phrase per User, made in the browser and shown once. Every Account and Character key is derived from it in a way that keeps the public keys from being tied together. The decision page verifies that property before anyone relies on it.
  - (b) One key file per Account or Character, downloaded.
  - (c) Passkeys. They likely can't sign a directory operation (unverified).
  - **Proposed answer:** (a). One thing to back up, and the per-Character rule (no two Characters share a key) still holds.
- **Q3. When is the owner key required?**
  - **Proposed answer:** at Account founding, because the repo attaches in the same flow, and at a Character's publicize.
  - A Private Character mints with Zurfur's keys only, and its owner key joins at publicize. The exit guarantee binds public Characters only (the Characters on ATProto decision).
  - The alternative is a key for every Character at creation, which reads the My Den rulings' per-Character key literally, at a real cost in sign-up friction.
- **Q4. What does an Account's repo hold at the alpha?** It's an open row on the Account page.
  - **Proposed answer:** one profile record (name, description, avatar), and nothing else until the Gallery's posts land in the beta.
- **Q5. What counts as an Account's fact, now that Users own commissions?**
  - **Proposed answer:** any of these: a commission ever brought into the Account, an invoice issued under it, a member besides its founder at any time, or a record published in its repo. Any one of them makes the Account soft-delete only.
- **Q6. Where does Zurfur's PDS live?**
  - **Proposed answer:** on its own registered domain, never a `zurfur.app` subdomain.
  - A PDS serves people's files, and the deployment checklist keeps user content off `zurfur.app` subdomains, which are same-site with the session cookie.
  - This is a door at A1 (see the table).
- **Q7. Should the dev stack get its own way to reach the local directory and PDS?**
  - The resolver refuses http and loopback everywhere except in tests. So the dev stack can't verify a brought domain against the local directory, or sign in a test user on the local PDS.
  - **Proposed answer:** yes: a dev-only exception that refuses to start outside `ZURFUR_ENV=dev`, like the dev profile's own lock. Otherwise claims can only be tried against your real Bluesky repo, which would publish junk records.
- **Q8. Does the alpha start on an empty database?**
  - **Proposed answer:** yes. Pre-alpha DIDs were never published and carry the old key list. Carrying them over means publishing them as they are. You re-make your own Accounts and Characters by hand.
- **Q9. Which namespace do Character handles use?** It's still an open ruling, under "cheap, alpha; otherwise beta".
  - **Proposed answer:** the shared, flat `*.zurfur.app`, for public Characters only. One claim check covers Accounts and Characters, as a note in the claim check already asks. It lands in A3.
- **Q10. What shape is user-linking?**
  - **Proposed answer:** Friendship's double-sided shape.
    - Each of the two Users writes a link record naming the other, in their own repo.
    - The link exists only while both records exist, and either side deletes theirs to unlink.
    - Profiles show it only while both exist.
  - It needs its own small decision page and a lexicon, which is a door at A2.
- **Q11. Who builds the invite codes, and what are they?**
  - **Proposed answer:** identity builds the gate in P8. No new User gets in without a valid single-use code, and your handle is allowlisted. Identity also builds the code store.
  - The earning rule is yours. My suggestion: 5 codes for each new User, plus 2 for each commission they complete, since both sides of a commission bring people in.
- **Q12. Does Zurfur keep its own minter, or mint through vulpes?**
  - **Proposed answer:** keep Zurfur's own. The Characters on ATProto decision says "our minter", and vulpes's minter lacks four things Zurfur needs (the unified-identity investigation). Vulpes supplies the claim halves, their checks and the custody read.
- **Q13. Does identity build moderation's identity asks?** These are moderator sign-in, a suspended state for Users, Accounts and Characters, and a takedown on hosted repos.
  - **Proposed answer:** yes, in P8, before the first build anyone else can reach. Moderation builds the actions.
- **Q14. Does "routine CAR export" bind at the alpha?**
  - **Proposed answer:** yes, with hosted repos: a nightly export of each hosted repo to storage the PDS host doesn't control, and one restore tested before A1.
