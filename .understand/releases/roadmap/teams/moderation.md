# Roadmap: moderation

Lead: Meerkat · QA: Lapwing · Programmers: Mastiff, Goose, Alpaca · Stages covered: all four

**Standing rule.** Every intervention is a named, audited moderator action, and nobody edits the database by hand. Only the app's own account writes to the database; the operator's `migrate` command is the one exception. (Your rulings of 2026-10-08. They aren't in the corpus yet.)

**What this file builds on.**
- The moderation study (`.understand/20261008-moderation-study.md`). Its 20 questions are still open, and each is asked at the slot named in "Questions for the Engineer".
- Your ruling on moderation's first deadline (D-26), which says the floor is due at whichever comes first:
  - the first build that anyone but you can reach;
  - the first slice that changes an upload path.
- This ask adds four things that must also be in place before real people arrive: blocks, kill switches, lockdown and the console.

## In one screen

- **Pre-alpha, P2 to P5: the whole floor, before anyone but you can reach a build.**
  - P2: the scan gate.
  - P3: Moderator Actions, the audit log, the console and the database roles.
  - P4: the report path, lockdown and suspension.
  - P5: blocks, the kill switches for public views, the legal pages and a live scanner.
  - Moderation's programmers have no feature work, so all four slots run alongside the other teams' work.
- **End of pre-alpha:**
  - No byte is served before a live vendor has scanned it, and anyone can report anything.
  - From a console behind a passkey, you can hide, take down, suspend and lock the platform down.
  - Every act goes into an append-only log that the app's own credentials can't rewrite.
  - A block made in any Bluesky client holds in Zurfur.
- **End of alpha:**
  - Take-down reaches the hosted PDS.
  - Retention rules run, and invite codes obey bans and lockdown.
  - Every alpha feature's moderation hook is live.
  - The full moderation design that Inventory Closures deferred is ruled, so the beta can build it.
- **End of beta:**
  - More than one moderator, working from a queue.
  - Public posts and replies are moderated like files.
  - The plugin and Seals kill switches ship with their features.
  - Appeals exist.
- **Release:** the doors open to everyone. That needs sign-up abuse controls, plus whatever legal duties counsel names for a public NSFW host.
- **The biggest risk is the scanning vendor.** PhotoDNA goes through third-party vetting [read, 2026-10-09], and Zurfur has no legal entity (the Payments decision). Whether a vetted service needs one is [unverified]. If it does, the legal entity is on the alpha's critical path, so counsel and the vendor applications start at P2.

## Pre-alpha

**P1 (0.1, fixed).** No moderation work of its own: moderation's programmers build the bug-report backend.
- **Confirmed for the tech lead** (the handoff asked): the commission host arms (WP-18) don't trigger the scan gate under your D-26 ruling. They change which file names an upload accepts, not where the bytes go or when a file becomes a node. The file name still lands in the "file added" changelog entry. That is P2's one-way door, not 0.1's.
- **Still open from 0.1:** the tech lead hasn't yet added moderation's floor to the deployment checklist in the hardened-resolver design. I checked on 2026-10-10.

- **P2: an upload is served only after a scan says it's clear.**
  - **Features:** security-asap (CSAM scanning).
  - **What it builds:**
    - **The scanner port,** with a fake scanner for dev, mock and tests.
    - **A pending area that moderation owns.** Bytes wait there pinned, and they aren't tied to the commission. A file becomes a node only once the scan says "clear" (study question 14).
    - **What a match does:** it holds the bytes by content hash, everywhere they appear, and moves them to an encrypted evidence store.
    - **A re-upload of held bytes** opens a new incident, and its response looks like any other upload's.
    - **When the scanner is down,** uploads wait, nothing is served and the founder is alerted.
    - **The "file added" changelog entry** carries the file's id and never its name (study question 6, the prevention half).
  - **Depends on:**
    - commission: the upload calls the gate, and the entry carries the id;
    - den: a node over held bytes is the one not-found, through the host check, in paths and in listings;
    - the tech lead: the gated blob store, which is another security-asap item.
  - **Done when** you run `just dev` and:
    1. an image uploads as "processing", then appears for alice and bob;
    2. the fake scanner's match file never appears for anyone, by path or in a listing, and its incident row exists (read it with `just db-shell`);
    3. re-uploading that file opens a second incident, with the same response as any other upload;
    4. with the scanner stopped, uploads stay "processing" and nothing is served;
    5. deleting the commission while a file is pending leaves the pending bytes in place;
    6. the changelog shows the cleared file by id, with its current name, and no entry stores a file name.

- **P3: every intervention is a named, audited moderator action.**
  - **Features:** proposal (your rulings of 2026-10-08).
  - **Who acts, and how they sign in:**
    - The founder is the one moderator, named in config (study question 1).
    - A passkey is added on top of Bluesky sign-in. The first one is enrolled with a one-time operator code (question 2).
    - The console lives on its own origin (question 3).
    - The moderator principal can only be minted from a fresh passkey proof, so the command line can never mint one.
  - **The audit log** is append-only, enforced by a trigger and by privileges.
  - **Two database roles** (question 11). The app's role can't change the schema, truncate tables, or update or delete rows in append-only tables. Only `zurfur migrate` migrates.
  - **The first actions:** take down, hide, restore, delete (refused while a legal hold exists), place or release a legal hold, record a legal report, and redact a changelog entry. A redaction is itself recorded, and the old entry's hash is kept (question 6).
  - **An act inside a commission** appends an entry signed "Zurfur moderation", citing a clause of the terms (question 7).
  - **Depends on:**
    - identity: the passkey step and its enrollment;
    - web: the console's screens;
    - commission: new changelog entry kinds, written through a port that takes the audit record;
    - den: the host check lets the moderator principal see held nodes;
    - the tech lead: the console's domain. Booting without migrating also changes CLAUDE.md.
  - **Done when:**
    1. you sign in to the console at its own address with Bluesky, then with your passkey;
    2. you take down a cleared file:
       - alice and bob get the one not-found;
       - the changelog says "Zurfur moderation took down a file" and cites a clause;
       - the audit log names you;
    3. restoring it brings it back, with its own changelog entry and its own audit row;
    4. as the app's database role, `UPDATE`, `DELETE` and `TRUNCATE` on the audit log are all refused;
    5. the `zurfur` command line has no command that acts as a moderator;
    6. the API boots without migrating, and `zurfur migrate` migrates.

- **P4: anyone can report anything, and you can close the doors.**
  - **Features:** security-asap (a report path). Proposals: lockdown (ruled 2026-10-08 as "to be designed"), suspension and ban, and notices to owners.
  - **Reports:**
    - Every user-facing item can be reported. The report is stored privately (question 4).
    - Categories include child safety and non-consensual intimate images. The second has a 48-hour clock (the lean in question 19).
    - The founder is notified. Closing a report is an action, and it tells the reporter the outcome.
    - The person reported never learns who reported them.
    - The owner is told why their item was acted on, except for suspected CSAM until counsel answers (question 5).
  - **Lockdown:**
    - Four switches, each of which only takes abilities away (question 9).
    - Any moderator can turn a switch on, and only you can turn one off. A config flag is the break-glass (question 10).
    - Lockdown never blocks take-down, evidence, sign-in, handle resolution, health checks, or a user leaving with their DID.
  - **Suspension:** a suspended user can't act but can still leave (question 12). A ban is a suspension with no end.
  - **Bug reports:** the operator read of bug reports moves into the console, over HTTP, and their retention follows the report rule. 0.1 parked both for this slot.
  - **Depends on:**
    - web: the report form and the lockdown banner;
    - identity: the suspended state at the session gate, and the first-sign-in gate;
    - den: the public read gate, at the host check.
  - **Done when:**
    1. as bob, you report alice's file as child safety, and the report appears in the console;
    2. you close it with an outcome: bob sees the outcome, and alice is never shown bob's name;
    3. "uploads off" in the console refuses uploads with a banner, and everything else still works;
    4. you set `ZURFUR_LOCKDOWN=public` and restart:
       - a signed-out read of a public node is refused;
       - `/.well-known/atproto-did` and sign-in still answer;
       - the lockdown log records both;
    5. a suspended bob can sign in read-only and export his data, but can't act.

- **P5: real people can protect themselves, and every public view obeys moderation.**
  - **Features:** blocks; the kill switches Inventory Closures says must work (tag moderation delete, Index suppression); the terms and legal pages.
  - **Blocks:**
    - Block records (`app.bsky.graph.block`) are read through identity's hardened resolver into a private cache.
    - They are checked wherever a block applies today: comments, both ways; friendship's edge; and new invitations and applications.
    - A block never touches business (the Blocks decision).
    - Zurfur has no block button of its own in the alpha (question 15).
  - **Kill switches:** the hide hold, which every public view filters; and deleting or banning a tag.
  - **The live scanner** goes behind the port. Counsel's answers decide the vendor. Sending files to it gets a security review, because those files cross the private boundary.
  - **Legal pages,** each rule with a stable clause number:
    - Terms of Service;
    - a content policy;
    - a privacy policy that names the scanner;
    - a copyright policy;
    - a child-safety contact.
  - **Depends on:**
    - identity: reading block records;
    - commission: invitations and applications check blocks;
    - web: the "also unfriend?" and "also leave?" prompts, and the legal pages;
    - the tech lead: naming who owns tags;
    - you, through counsel: the vendor and the legal text.
  - **Done when:**
    1. with two Bluesky accounts of yours, the first blocks the second in a Bluesky client. Within the cache's refresh:
       - their comments vanish for each other;
       - neither can invite the other to a new commission;
       - a commission they share still stands;
    2. a hidden public node leaves every public view that exists, and its owner still sees it;
    3. a banned tag disappears everywhere and can't be added again;
    4. the vendor's test procedure holds an upload. Whether vendors supply a harmless test hash is [unverified];
    5. the legal pages are live, and P3's actions cite their clauses.

**Exit: before the first build that anyone but you can reach, whatever its stage.**
- **The floor in your D-26 ruling is built:**
  - the scan gate, with a live vendor, in front of every upload route that exists;
  - the report path;
  - your answers on database roles and on text in changelog entries.
- **This ask's additions stand:** Moderator Actions with the audit log, the console, lockdown, suspension, blocks, the hide hold and the tag ban.
- **Every moderation slot passed `/security-review`.** Each one touches authentication, session handling or the private/public boundary.
- **Counsel is engaged**, and the reporting route is registered: NCMEC as Inventory Closures decided, or another route if you reopen it.
- **A drill has been run on the deployment:** one match, end to end, from the upload to a recorded legal report.

## Alpha

- **A1: take-down reaches the hosted PDS.**
  - **Features:** the moderation hooks of account-pds and character-presence.
  - **When:** no later than the first slot that writes a blob to a hosted repo. If that slot is in pre-alpha, this slot moves with it.
  - **What it builds:**
    - **The scan gate runs before every blob Zurfur writes to a hosted repo,** a character's reference sheet for example.
    - **Taking down a hosted record or blob** runs through a separate worker that holds the PDS admin credential, reached only through the outbox (study question 13). That amends the "attach/recovery only" rule, so it needs a security review.
    - **Hiding legal content** stays at Zurfur's own views.
    - **A suspension hides** the suspended account's hosted presence.
  - **Depends on:** identity, for the worker, the credential and the write path.
  - **Done when:**
    1. a public character's reference sheet is scanned before it reaches the PDS;
    2. after you take it down, the PDS stops serving the blob, and the evidence is kept.

    The admin call covers blobs [read: lexicon]. That it stops serving on the PDS we actually run is [unverified until we test it].

- **A2: retention, and invite codes that obey moderation.**
  - **Features:** proposal. Invite codes come from your stages ruling and aren't on the board.
  - **What it builds:**
    - **Retention timers,** set by counsel, for reports, evidence (a legal hold lasts until a person releases it) and bug reports.
    - **Invite codes and bans:** banning a user voids their unused codes, and "sign-ups off" stops codes from being redeemed.
    - **Who invited whom** is recorded privately, so a wave of abuse can be traced, and is never shown to anyone. It links personas, which is the kind of DID correlation the design forbids exposing.
  - **Depends on:** whoever builds invite codes; counsel.
  - **Done when:**
    1. a banned user's unused codes are refused;
    2. with "sign-ups off", a valid code is refused and known users still sign in;
    3. the console shows each report's retention date.

- **Also in the alpha:** each alpha card's moderation hook lands in the same slot as its card. The list is under "Cross-team dependencies".

**Exit: before the beta's bigger group arrives.**
- Every alpha card's moderation hook is live.
- The full moderation system that Inventory Closures deferred (report queues, triage, moderator roles, appeals) is written as a decision page and ruled.
- Counsel has answered the four legal questions.
- You have reviewed the alpha's report volume and set how many moderators the beta needs.

## Beta

- **B1: more than one moderator.**
  - **Features:** proposal. It reopens Inventory Closures' deferral of moderator roles, at the post-MVP stage that deferral points to.
  - **What it builds:**
    - A second platform role, Hearth. You, as the Bearer, grant and revoke it, and both are audited actions.
    - A report queue with assignment and triage.
    - Matched images stay blurred for every moderator by default.
  - **Done when:** you grant Hearth to a test moderator, who takes a report from the queue and acts on it, and the log names her. After you revoke the role, her session can't act.

- **B2: public posts are moderated like files.**
  - **Features:** the moderation hooks of gallery-posts, replies, commission-comments and hash-chain.
  - **What it builds:**
    - The scan gate covers post uploads.
    - Posts on hosted repos can be taken down through A1's worker.
    - Post and reply feeds filter the hide hold.
    - Blocks cut public replies.
    - Moderators can remove commission-page comments.
    - A redaction still lets the hash chain verify. The Eventlog decision says "scrub content, retain the hash", while P3 deletes the entry and keeps the old hash. Question 6 settles which.
  - **Done when:**
    1. a post upload waits for its scan;
    2. a post taken down stops on the PDS;
    3. the chain still verifies after a redaction.

- **B3: the beta's kill switches.**
  - **Features:** the moderation hooks of plugin-api and seals. Inventory Closures says the kill switches "must work".
  - **The publisher ban:**
    - It is a moderator action that writes one ban, keyed by the publisher's Account DID.
    - Plugin reads it at four points: listing reads, the key exchange, every plugin API call and webhook dispatch. This is the plugin lead's proposal on the board; it isn't agreed.
    - Unlisting a single listing stays with plugin.
  - **Seals:** a labeler blocklist, keyed by DID and read at hydration, plus negating a Zurfur label.
  - **Done when:**
    1. after you ban a test publisher, its listing disappears, and its next API call and next webhook are refused, with no sweep;
    2. after you block a labeler, its labels stop rendering.

- **B4: appeals.**
  - **Features:** proposal. Inventory Closures deferred appeals to after the MVP.
  - **What it builds:**
    - A user can contest a moderator action.
    - The decision on the appeal is itself an audited action.
    - The statement of reasons follows the EU's shape, if counsel says that law applies.
  - **Done when:** alice appeals a take-down, you uphold or restore it, and the log shows both steps.

**Exit: before release to everyone.**
- Appeals are live.
- The queue keeps up with the beta's volume, at a figure you set.
- The release cards' hooks are designed.
- Every duty in counsel's list for release is built.

## Release

- **R1: open doors.**
  - **Features:** proposal, plus whatever counsel names.
  - **Abuse controls:**
    - Rate limits on reports, sign-ups and uploads.
    - "Sign-ups off" tested under load.
    - Signed-out visitors can report, if the law asks for it.
  - **The legal duties counsel confirms.** Candidates from the study:
    - the 48-hour removal process for intimate images (the TAKE IT DOWN Act, if Zurfur is covered) [read];
    - a designated copyright agent and a repeat-infringer policy (17 U.S.C. § 512) [read];
    - an EU legal representative, if Zurfur is offered in the EU (DSA Art. 13) [read];
    - UK age assurance, if UK users can see adult work [excerpt]. That would clash with the "I'm 18+" click-through in Project MVP's maturity line.
  - **Done when:** every item on counsel's checklist is built and checked.

- **R2: the release cards' hooks.**
  - **Features:** the moderation hooks of notifications, marketplace, disputes, collections, openings-feed and subscriptions.
  - **What it builds:**
    - Notices to owners and reporters move onto the notification system.
    - Marketplace listings can be reported and unlisted.
    - Collections, lists and the openings feed filter hidden items and blocks.
    - Content behind a subscription tier passes the same scan gate and the same report path.
    - A dispute isn't a report. A dispute may cite a moderator action, but it never triggers one. That line is asked when disputes are designed.
  - **Done when:** each of these cards lists its hook in its own definition of done.

## One-way doors, and the slot that closes each

| Door | Why it can't be undone cheaply | Closes at |
|---|---|---|
| An upload becomes a node only once it's clear (question 14) | Under your D-26 ruling, an upload slice built the other way gets rebuilt | P2 |
| Changelog entries carry ids, never user text or file names | Once real people write, every name stays in an append-only record | The first build others can reach (built in P2) |
| How a redaction is recorded (question 6) | Entries are append-only, and the beta's hash chain must verify across a redaction | The first build others can reach; checked again at B2 |
| Database roles: the app isn't the superuser and doesn't migrate | A superuser bypasses every check [read: PostgreSQL 16, in the study] | The first build others can reach (built in P3) |
| The audit log's shape, and the terms clause and version each act cites | Append-only, so a wrong shape can only be added to | The first real moderator action |
| The terms' clause numbers | Real acts cite them | The alpha's opening (P5) |
| The report record, and who can read it | It holds accusations and reporters' names | The alpha's opening |
| The evidence store and how long it keeps things | Legal duty; evidence deleted early can't be recovered | The first real match |
| Holds: by content hash for bytes, and one moderation table for nodes (question 20) | Every view reads them | The alpha's opening |
| Moderation's changelog entry kinds | The list of kinds is closed, and real entries render them for good | The alpha's opening |
| "Zurfur moderation" as the public name on acts (question 7) | It is written into append-only entries | The first real act |
| Reserved handle names for moderation and legal | Once a real user holds `legal.zurfur.app`, taking it back is an act against that user | The alpha's opening (identity's list) |
| The console's origin | Passkeys are bound to their origin [unverified: confirm in the WebAuthn spec] | The first real enrollment (P3) |
| Take-down on the hosted PDS (question 13) | It amends the admin credential's "attach/recovery only" rule | A1 |

## Cross-team dependencies (what you need, from whom, by which slot)

| Need | From | By |
|---|---|---|
| Moderation's floor on the deployment checklist (0.1's objective 2, still open) | Tech lead | Before any build others can reach |
| The upload calls the gate; a file becomes a node only once clear; "file added" carries the id | Commission | P2 |
| The host check refuses held nodes as the one not-found, and lets the moderator principal through | Den | P2, P3 |
| The gated blob store, next to which the pending area and the evidence store sit | Tech lead (whoever owns that security-asap item) | P2 |
| The passkey step-up and its enrollment | Identity | P3 |
| Console screens, report form, lockdown banner, legal pages, block prompts | Web | P3, P4, P5 |
| Moderation's changelog entry kinds, through a port that takes the audit record | Commission | P3 |
| The console's domain, and who owns a mailer (new infrastructure and a new secret) | Tech lead | P3, P4 |
| The suspended state at the session gate, and the first-sign-in gate | Identity | P4 |
| Reading block records through the hardened resolver | Identity | P5 |
| Invitations and applications check blocks | Commission | P5 |
| Who owns tags | Tech lead | P5 |
| Add `moderation`, `legal`, `safety`, `trust`, `report`, `terms`, `privacy`, `copyright`, `dmca` and `appeals` to the reserved labels. Today's list already has `mod`, `moderator`, `abuse`, `security`, `support` and `staff` (`domain/src/elements/handle/reserved.rs`, read 2026-10-10) | Identity | Before the alpha |
| Invite codes: a ban voids unused codes, redemption obeys "sign-ups off", and who invited whom stays private | Whoever owns invite codes | A2 |
| The PDS take-down worker that holds the admin credential | Identity | A1 |
| CI tests: no use case outside moderation writes a hold, and the audit log refuses rewrites | Internal | P3 |
| A legal entity, counsel, vendor vetting and the reporting route | You; the compliance review at each release's end | Start at P2; done before any build others can reach |

**Rides with: each of these is part of the feature's own definition of done, and lands in that feature's slot.**

| Feature (cards) | Its moderation hook |
|---|---|
| Any new upload path: commission files in the Den, configuration uploads, character and account content, gallery-posts, bug-report screenshots, and external-storage mounts | Goes through the scan gate |
| Every public view: gallery-view, user-pages, portfolios, character-presence, the ask-for-art feed and tag queries; later openings-feed and collections | Filters the hide hold and suspended actors |
| Every social surface: markup (comments), friendship, seats-handshake, ask-for-art applications and reputation; later replies and seals | Checks blocks |
| Every user-facing item | Carries the report button |
| tags | Delete a tag, and ban a tag name |
| maturity-ui | An action that corrects a maturity rating |
| handle-change | Revoke an impersonating `*.zurfur.app` handle (whether that is a moderator act is asked of identity) |
| markup (comments) | A moderator can remove a comment. This amends the Comments & Markup decision on who may delete |
| fact-deletion | Delete is refused while a legal hold exists, and evidence survives a commission's hard delete |
| plugin-api, seals | The publisher ban and the labeler blocklist (B3) |

## Not on the board (proposals, each with why)

- **Moderator Actions, the audit log, and "take down isn't delete".** Ruled on 2026-10-08, but they aren't in the corpus and have no card. They are the standing rule. *Offer:* capture those rulings as a decision page with `/design-decision`.
- **Lockdown.** Ruled on 2026-10-08 as "to be designed". It is the safe state when the moderation tools themselves are broken.
- **A passkey for moderator sign-in, and a console on its own origin.** The moderator account is the most powerful credential. atproto OAuth leaves login strength to the moderator's PDS [read: the atproto OAuth spec, in the study].
- **Database roles.** Today the app connects as the database superuser.
- **The evidence store and legal holds.** Take-down isn't delete. US law preserves a reported item's contents for a year (18 U.S.C. § 2258A(h)) [read].
- **Suspension and ban.** Inventory Closures puts up terms pages "for ban grounds to point at", but names no ban.
- **A2's retention rules and invite-code controls.** Invite codes come from the stages ruling, and they are the alpha's front door.
- **B1's moderator roles and queue, and B4's appeals.** Inventory Closures deferred them to after the MVP. They are due before strangers arrive.
- **R1's release-time duties.** Counsel decides which apply.
- **The pre-alpha drill.** It is the only proof that the whole CSAM path works before it matters.

## Questions for the Engineer (each pre-answered)

1. **Does the alpha open only once a scanning vendor is live?** *Answer:* yes. If vetting needs a legal entity, the entity goes on the alpha's critical path. The only other option is opening with "uploads off", and I don't recommend it.
2. **When does counsel start?** *Answer:* at P2. All of these need counsel before any build others can reach:
   - the terms' clause numbers;
   - the reporting route;
   - retention;
   - the study's four legal questions (16 to 19).
3. **May a pre-alpha build go on the real domain before the floor is built, behind a sign-in allowlist that holds only your DID?** *Answer:* yes. Every write needs a session, so no one else can upload or write text, and readers see only your content. The floor becomes due the day the allowlist widens.
4. **One rule for every feature: its moderation hook is part of its definition of done** (the "Rides with" table)? *Answer:* yes. It extends the study's question 8 ("each kill switch ships with its feature") to scanning, the hide hold, blocks and the report button.
5. **Moderators beyond you: when?** *Answer:* you stay the one moderator through the alpha (study question 1, option A), and the question reopens at B1, before the beta's bigger group.
6. **Reserve the moderation and legal handle names before the alpha?** *Answer:* yes. Identity adds them to its list.
7. **External-storage mounts (your note 2), asked when their slot comes:** *Answer:* Zurfur serves bytes from storage it doesn't control to no one but their owner until those bytes pass the scan gate.

**When the study's 20 questions are asked:**

| Slot | Questions |
|---|---|
| P2 | 6 (the prevention half), 14 and 20 |
| P3 | 1, 2, 3, 6 (the record), 7 and 11 |
| P4 | 4, 5, 9, 10, 12 and 19's report category |
| P5 | 15; 16 to 18 through counsel |
| A1 | 13 |
| B3 | 8 (plugin backs answer B) |
