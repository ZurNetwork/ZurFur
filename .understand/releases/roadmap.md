# Zurfur roadmap: pre-alpha to release

Consolidated by Kestrel on 2026-10-10 from eleven teams' roadmap files, for the Engineer's approval. Revised the same day after Magpie's QA (the challenges and their resolutions are at the end) and your rulings R-1 to R-50. Only this file was written: no team file, corpus page or code changed.

**How to read it.** The top section is one screen. Everything after it is reference: one table of versions, then each version, then gaps, conflicts, questions and the design-sync list. Version numbers follow your rulings: one continuous count, with the stage as the tag prefix (`pre-alpha-v0.2.0` … `alpha-v0.10.0` … `beta-v0.14.0` … `release-v1.0.0`). You approved the sequence (R-21), with staging moved to 0.4 (R-47). Versions have no dates. Each is one demoable increment, built by several teams in parallel, as 0.1 was.

## In one screen

- **Pre-alpha, 0.1 to 0.9 (only you).** By 0.9 a whole commission runs inside the Den, from PostgreSQL:
  - writes go through Atta, and files go through a scan gate into a content-addressed byte store;
  - you get pins and threads, seats and invitations, characters in slots with their keeper's consent, the board and view grants;
  - there is a public User Page, and a minimal Gallery where Adult work shows only as a label, with no image, until the viewer has passed the age check.

  Accounts get their own repos and public Characters carry their claim pair, against a private directory and Zurfur's own PDS. Every write that leaves PostgreSQL goes through one outbox. Moderation's floor stands (the scan gate, audited moderator actions, reports, lockdown, blocks, legal pages), and the security card is closed. **From 0.4 a staging copy runs that only you can reach** (R-47), on the hosting you pick in 0.3. 0.8 adds the age check and invite codes, and 0.9 builds production after you watch staging's backup restore.
- **Alpha, 0.10 to 0.13 (a closed group on the real domain).** 0.10 opens invite-only and turns on submission to the public DID directory. Then come approval, invoices, ratings and Portfolios (0.11), and Ask-for-Art, tags, friends and alts (0.12). Every alpha card has its home by 0.12. 0.13 adds Character handles and command-line sign-in without a browser, and readies the beta.
- **Beta, 0.14 to 0.20 (a bigger group).** Public Gallery posts and replies, phases, commission comments, the hash chain, templates (YCH) and commission trees. Golems join, read, act and render types, then join Accounts. Seals and achievements, Kafka, and external-storage mounts (a proposal). An outside security test closes the stage.
- **Release, 1.0 to 1.3 (everyone).** 1.0 brings:
  - notifications, capacity guards, the openings feed and collections;
  - open sign-up with age checks, a data export, and a generally available v1 contract.

  Disputes, signed deliveries and the Telegram bot follow in 1.1 (R-48), the plugin marketplace and Trello import in 1.2, and subscriptions in 1.3.
- **The five biggest risks:**
  1. **You operate the alpha as an individual** (R-5). The scanning vendor or the child-abuse reporting body may refuse an individual (unverified). Moderation applies in 0.2 to find out. If refused, the alpha opens with uploads off (R-9). The lawyer starts in 0.3.
  2. **The age check is new work that reaches every layer.** You ruled a vendor check everywhere, 18 and over (R-6, R-7). It has no card, and it needs a vendor and a cost (question 104). By 0.8 it must reach the server and the cookieless image domain, not just the screens (questions 101 and 102). If every vendor refuses an individual, the alpha opens with Adult work shown to no one and 18 and over self-declared in the Terms (R-45).
  3. **Permanent writes pile up at the alpha's opening.** At 0.10 the directory switch, the relay mesh, hosted repos, claims, lexicons and every stored shape become permanent together, with EU and UK users in the public directory from day one (question 106). Two problems must be fixed first:
     - the hosted PDS's one server-wide rotation key would outrank Zurfur's per-DID keys (question 45);
     - the ruled publish permission set can't carry the claim records (question 59).
  4. **Identity, web and Internal are the critical path.** Identity carries seven pre-alpha versions (about 68 units by its own count) plus what the merge added: the cross-site guard, Terms acceptance, the age check's stored result, friendship and more. Staging at 0.4 makes that version identity's heaviest (about 13 units, with a cut order). Fox re-sizes, and question 110 moves the age-check vendor work off identity. Internal's first deploy is now due at 0.4, with no roadmap file to size it against. Web ships every backend capability's screen in the same version.
  5. **Your queue.** 71 open questions. None blocks 0.2. Two are needed by 0.3: the hosting choice (question 65) and when deployment is designed (question 66). You also review every door and the full text of the contract files, and you run the environments from 0.4 (R-3, R-47).

## Versions at a glance

Pre-alpha has 9 versions (0.1 to 0.9), alpha 4 (0.10 to 0.13), beta 7 (0.14 to 0.20) and release 4 (1.0 to 1.3).

| Version | Headline | Teams with work landing |
| --- | --- | --- |
| **0.1** (signed) | Sign in with Bluesky and browse My Den, read-only | identity, web, den, commission, security, tech lead |
| **0.2** | Make and change things in your Den | architects, den, commission, identity, web, security, moderation, compliance, vulpes, internal |
| **0.3** | Upload files safely, pin and discuss them; your own keys and real Characters; you pick the hosting | posts, moderation, den, commission, identity, architects, internal, web, compliance |
| **0.4** | Bring people in; handles you can trust; every intervention audited; staging, reachable only by you | commission, den, identity, moderation, posts, web, vulpes, internal, architects, security |
| **0.5** | Characters in slots; Accounts get their own repo | commission, den, identity, moderation, architects, web, security |
| **0.6** | Your public face (User Page, Gallery), reports and lockdown; the board | den, identity, moderation, posts, commission, web, security |
| **0.7** | Who else sees it: view grants, public Characters and claims, blocks, legal pages | commission, den, identity, moderation, architects, web |
| **0.8** | Ready for real people: the age check, invite codes and the directory watcher | identity, den, posts, web, moderation, security |
| **0.9** | The alpha cut: production, and every door ready to close | architects, internal, web, moderation, security, compliance, den, commission, identity, plugin |
| **0.10** (alpha) | The alpha opens on the real domain | identity, architects, security, moderation, plugin, vulpes, web |
| **0.11** | Close the loop: approval, invoices, ratings, Portfolios | commission, posts, den, identity, web, compliance |
| **0.12** | Ask for art, tags, friends and alts | den, commission, identity, moderation, web |
| **0.13** | Character handles, command line without a browser, beta readiness | identity, den, plugin, architects, moderation, security, compliance, vulpes |
| **0.14** (beta) | Post your work publicly; phases and commission comments; more moderators | posts, commission, moderation, architects, web, compliance |
| **0.15** | From commission to post; tamper-evident history | posts, commission, den, moderation, web, security |
| **0.16** | Golems join commissions and read; replies; YCH | plugin, identity, commission, moderation, posts, architects, web, security |
| **0.17** | Golems act and append; commission trees; domain events as code | plugin, commission, den, architects, web |
| **0.18** | Plugin types in the Den; your own folders; Seals and achievements | den, plugin, commission, posts, moderation, identity, web |
| **0.19** | Golems in Accounts; Kafka; external storage (proposal); more sign-in sources | plugin, internal, architects, den, identity, posts, moderation, web, security |
| **0.20** | Release readiness: the outside security test | security, compliance, plugin, identity, moderation, vulpes |
| **1.0** (release) | Everyone, for real work | every team |
| **1.1** | Disputes, signed deliveries, the Telegram bot, dynamic collections | commission, plugin, posts, den, moderation, web |
| **1.2** | The plugin marketplace and Trello import | plugin, moderation, posts, web, internal |
| **1.3** | Subscriptions | den, web, moderation, compliance, you |

**In every version, without repeating it below:**
- a security review of each slice that touches sign-in, the private/public boundary, DID or handle correlation, sessions or tokens;
- the end-of-release reviews by security, compliance and architects (your note 4), where a must-fix blocks the tag unless you accept it in writing;
- each feature's moderation hook (scanning, the hide hold, blocks, the report button and its kill switch) in that feature's definition of done (R-35);
- web's screen in the same version as its backend: mocked first, wired by the end.

## Per version

Each version lists what lands by team, the exit check you can do by hand, and the one-way doors decided in it. Pre-alpha may break things, so most pre-alpha doors are *decided* in their version and *close* at the first alpha tag (0.10). It is cheaper to decide them where they are built.

### 0.1: Sign in with Bluesky and browse My Den (signed, building)

As the signed plan says: hardened sign-in, the Explorer shell, My Den read-only on the real backend, the time limits and bug reports. **Carried to 0.2 if cut:** bug reports (cut line at plan unit 9), refusing a PDS address that isn't a bare origin, and clearing the binding cookie only on a match.

### 0.2: Make and change things in your Den (the first writes)

**It starts with the domain crate split** (R-23), at the quiet seam right after 0.1 merges. No 0.2 feature branch is cut until the split lands, and the two open work-in-progress branches restack once.

| Team | What lands |
| --- | --- |
| architects | The crate split. The application layer's last five driver-opened transactions become use cases, with the clippy ban extended to `api`, `cli` and `composition`. **The outbox and a separate worker process** (R-24), pulled forward from architects' third slot. It carries the three writes that leave PostgreSQL today: founding an Account's DID, the handle change, and the tombstone at deletion. |
| den | Atta's write side: named actions; the generic checks (born Private, never wider than the parent, "make public" widens ancestors, the depth cap); one node table (R-13); hash plus MAC stored in the same unit of work (R-33). Today's metadata writes move behind the gate. Held bytes answer the one not-found, and moderators are let through. **Pulled forward from den's third slot (R-44):** serving the core payloads (a commission's fields, a slot's state, the changelog a page at a time) only after the hash and MAC verify. Den confirms the size; anything that doesn't fit moves to 0.3, and you're told. |
| commission | Lifecycle, both status axes (including the plain "Disputed" move, R-32), deadline, and archive and delete (hard delete only while fact-free). The changelog a page at a time; born Private. The commission acts enter the contract as typed calls (R-14). The decision page recording where a commission's content lives (R-15). |
| identity | DIDs safe to publish (R-43): mint ordering, handle-race cleanup and tombstone durability on the outbox, plus **the handle change moved onto the outbox** (today it sends the directory update, then writes PostgreSQL, with no retry). Key custody part 1, which is architects' one custody port with a key id per wrapped secret and one key per purpose. **The soft-delete gate, both halves** (R-20, R-31): the facts check with the four facts you ruled, and the store refusing every write to a soft-deleted Account. `Account.id` removed from the contract. **The API's cross-site write guard** (R-20). On paper: the hosted-repos decision, and the owner-key decision page recording R-29 and R-30, which verifies the key derivation. |
| web | The Claude Design pass after your session (R-22), then the loading states. Create a commission; the status pill; the changelog timeline; plain Archive and Delete; forms born Private; the maturity rating picker. The web tier's cross-site check (R-20) and the https-only relay check. The tolerant-reader tests plugin needs (0.1's carry-over row 34). |
| security | Threat model version 1, as a corpus page (R-36). The hardening baseline and the secrets inventory. GitHub's private vulnerability reporting turned on. |
| moderation | Applies to the scanning vendors as an individual (R-9), with the age-check vendors alongside (R-45). Builds the scan gate on its own branch, to land in 0.3. Lists the study's scan-gate answers in its first PR for your review (R-38). |
| posts | Builds the byte store on its own branch, to land in 0.3. |
| compliance | Most of its questions are ruled. The operator is you, as an individual (R-5), so your name and a postal address go on the legal pages and registrations. The operator's country is your country of residence (R-49); compliance uses it to know which representatives, registrations and regulator apply. |
| vulpes | A pure library on `main`: its three cut branches pushed (R-37), PRs #30 and #33 merged, and a tag Zurfur can pin. The claim record caught up with the four-shapes ruling, with `did` for the other party (R-28). |
| internal | Actions pinned to commit SHAs, `deny` required, Dependabot (R-2). A local PLC directory in the dev stack. Team path globs follow the crate split. |
| plugin | Reviews only: the cross-site guard covers cookie routes only, and den.proto keeps its plugin openings. |

**If 0.2 runs over, this moves first:**
1. Den's payloads move to 0.3 (R-44).
2. Identity's directory-safety slices follow (mint ordering, race cleanup, tombstone durability). Nothing reaches the public directory before 0.10, so they can wait.

The crate split, the outbox (R-24), Atta's writes, commission's lifecycle and the cross-site and soft-delete items stay (R-20). You're told of any move.

**Exit check, on `just dev` after your real sign-in:**
1. In My Den, create "Test 1". It shows Private and Draft. Rename it, move it to Active, set a deadline, and page back through its changelog.
2. Archive it, and it leaves `commissions` and returns with the toggle. Create "Test 2" and delete it.
3. A write to someone else's commission answers "Nothing here", the same as a read.
4. Change one payload byte by hand. That node isn't served, and the log reports the mismatch.
5. Fire two foundings at once for one new handle: one Account results, and the local directory holds one DID. Founding with a deactivated Account's handle is refused, and nothing is minted.
6. With the directory stopped:
   - found an Account;
   - delete an empty one;
   - change a handle; it shows as pending.

   Restart the directory. Within a minute the DID resolves, the tombstone lands and the handle change completes.
7. Accepting an invitation into a soft-deleted Account answers 404 and creates no membership.
8. A cross-site form post to any write route is refused, `just dev` included. A planted route that writes around the gate turns `just gate` red.
9. `ls backend/crates` shows the per-domain crates.

**Doors.** These are ruled and close at 0.10:
- one node table (R-13);
- typed write calls (R-14);
- what the MAC covers (R-33);
- how a type's content reaches the web (R-34).

Still decided here: the custody envelope's format.

### 0.3: Upload files safely, pin and discuss them; your own keys and real Characters (the first upload change)

| Team | What lands |
| --- | --- |
| posts | The gated byte store, built to your ruling (R-25): one port with an S3-compatible adapter and a local one; both hashes from the first byte; every backend untrusted and checked on read; encryption at rest; bytes stored once; an upload that looks the same either way; mark-and-sweep after a 30-day grace window (R-26); pins for moderation. The mock byte table is retired. Architects review the shape. |
| moderation | The scan gate with a fake scanner, and a pending area. A match holds the bytes by hash and moves them to an encrypted evidence store. With the scanner down, nothing is served and you're alerted. The "file added" entry carries the file's id, never its name. |
| den | Uploads behind the gate: the raw-bytes content id as the hash; a file becomes a node only once clear; 2 GiB per User (R-26). File bytes are served through Atta after the check. den.proto gains file content. |
| commission | Files and pins: uploads into `attachments` and `products`, a pin opens a thread, notes become comments, the submission prompt, and a product registered as a fact. The old drawn markup and notes tables are dropped (that PR comes to you, under your migrations ruling). |
| identity | Your keys and real Characters. The owner key is made from your recovery phrase in the browser and never stored (R-29), and founding asks for it (R-30). The final key list puts yours first. Handle-less mints. Characters are stored, created and deleted. **The Character facts check is a stub here, answering "no facts"**: the first Character fact is real once slots land in 0.5. |
| architects | The contract's generated Rust types in their own leaf crate, so the command line can call the API over HTTP. **The hosting design** (question 65), so you can pick the provider and region in this version: staging runs on it from 0.4 (R-47). One provider gives the object store and KMS, which also picks the KMS that identity builds against. |
| you, internal | **You pick the hosting provider and region** (question 65, a door), and the KMS comes with it. Internal starts the deploy pipeline for staging, designed in 0.4's plan (question 66). The choice also fills in compliance's processor and region list. |
| web | Upload and image view, pins and threads, the rating on a product. The Character form, moved here from web's first slot. The owner-key screens. The light theme follows your system setting (R-39). |
| compliance | Counsel holds drafts of the four policies; the record of processing, the impact assessment, the retention schedule and the two runbooks exist. The UK Online Safety Act paperwork is drafted, naming you (R-46), for counsel's review. The reporting body's registration email is sent, and the copyright agent designated. The stored fields that harden at the alpha are reviewed. **The lawyer is hired once drafts exist** (R-10). The fixed scope gains four points raised since:<br>• whether one age check per account meets Brazil's "at every access" rule;<br>• whether the UK relief for an adults-only service holds with signed-out public reads (R-50; if it does, 0.9's children's risk assessment is dropped);<br>• whether a registered-agent or mail-forwarding address can stand in for your home on the legal pages;<br>• the license's enforceability on mostly AI-written code (R-27). |

**Exit check:**
1. Upload `sketch-02.png`. It shows "processing", then appears in `attachments`. The changelog says a file was added and doesn't print its name.
2. The same image in two commissions is one stored object.
3. The fake scanner's match file never appears for anyone, and its incident row exists. With the scanner stopped, uploads stay "processing".
4. Pin the file and reply twice. Delete the first comment, and it shows "deleted". With a final in `products`, delete refuses and offers archive.
5. Found an Account: your browser makes the key, the local directory lists it first, and `just db-shell` finds no copy.
6. Create a Character, then delete one with no facts: its DID is tombstoned locally.

**Doors.** Ruled here:
- how private bytes are named (R-25);
- how the owner key is made and held (R-29); it closes at the first real mint, 0.10.

Decided here:
- an upload becomes a node only once clear (moderation's door, which closes here);
- changelog entries carry ids, never names;
- comments replace the old markup and notes;
- the hosting provider and region (question 65), which staging uses from 0.4.

### 0.4: Bring people in; handles you can trust; every intervention audited; staging, reachable only by you

**Staging moved here from 0.8 (R-47).** It brings the KMS adapter and the production sign-in client with it, and its hosting is picked in 0.3. Here is what moved and what it weighs (S = 1 unit, M = 2, L = 3, the 0.1 plan's sizes; estimates where a team gave none):

| Moved | From → to | Who | Size |
| --- | --- | --- | --- |
| The hosting design, and your pick of provider and region | 0.8 → 0.3 | architects; you | M |
| The staging host, from the pipeline's first deploy | 0.8 → 0.4 | internal, with you as ops owner | L (new for Internal, whose project was due at the alpha cut) |
| The KMS adapter, with one key per purpose | 0.8 → 0.4 | identity builds; architects review | M |
| The production sign-in client (confidential, private-key signed, public metadata) | 0.8 → 0.4 | identity | M |
| Production sign-in screens | 0.8 → 0.4 | web | S |
| The deploy pipeline's security rules and the secrets inventory for staging's secrets | 0.8 → 0.4 | security, internal | M |
| The deployment checklist's staging items, and a wildcard certificate for staging's handles | 0.8 and 0.9 → 0.4 | internal, architects | S |
| Zurfur's PDS on staging | 0.8 → 0.5, with Account repos | architects, identity | S |
| The live scanner wired on staging | 0.8 → 0.7, with the scanner | moderation | S |

Identity's 0.4 grows from about 9 units to about 13, its heaviest version, and its 0.8 shrinks by the same amount.

| Team | What lands |
| --- | --- |
| commission | Seats with requirements; invite → accept; apply → accept → accept (10 applications a day). The Creator, Client and Commission Admin roles, and `participants` at the viewer's standing. **Requests** (R-42), the pending list of invitations, slot requests and approval requests. Moderation's entry kinds written through commission's changelog port. |
| den | Links and mounts in Atta: symlinks (shown at the lowest of three levels, opening on the target's real path); **the read by real path on the wire, with Heron's review**; mounts taken as a list from the host; seat ceilings in the formula; keeper mounts. Den's board and grant checks run in 0.6 and 0.7, when commission's records exist. |
| identity | Brought-domain verification both ways, and the handle edge cases. The reserved list gains moderation's and plugin's names. Renames to and from a brought domain. What a hard-deleted Account's handle does is question 111. A dev-only route to the local directory and PDS. Finding a DID from a handle, for invitations. **For staging (R-47):** the production sign-in client, and the KMS adapter (key custody part 4, reviewed by architects). |
| architects, internal, you | **Staging** (R-47), on the hosting you picked in 0.3, deployed by Internal's pipeline. You hold the environment (R-3). It runs the API, web, worker, PostgreSQL and its own private PLC directory; nothing reaches plc.directory before 0.10. It refuses to boot without the KMS key, and KMS key deletion is denied to the app's and the deploy's roles. A wildcard certificate covers staging's handles, issued with a narrowly scoped DNS token or from a separate host, because Zurfur's resolver checks handles over https only. The deployment checklist's staging items apply: an egress address nothing internal trusts, instance metadata locked down, cookie redaction in the proxy's logs, and the sign-in callback routed straight to the API. **How staging is reached is question 107**: the client's metadata must be public, so a network block alone breaks sign-in. |
| security | The gate "the first staging deploy" arrives here: the secrets inventory complete for every secret staging holds; deploys with short-lived credentials; a protected staging environment, with you as the required reviewer; container images pinned by digest. An automated scan of staging (a proposal), re-run at 0.8. |
| moderation | Moderator actions, the append-only audit log, the console on its own origin, two database roles, and the API booting without migrating. **Moderation builds the passkey step-up and identity reviews it.** That move off identity's last slot still needs Meerkat's and Fox's agreement, and it's security-reviewed either way. A passkey enrolled on staging's console is a rehearsal; the door closes at the first enrollment on production's. |
| posts | Public bytes from their own cookieless domain: raster images re-encoded with metadata removed, every other type served as a download, and nothing served without a "clear" verdict. Commission adds "make public" on a product, with the rating required. **Adult bytes aren't served on this domain** until the age check exists (0.8). How they're linked then is question 102. Staging gives this domain its first real TLS host. |
| vulpes | Claims ready: the pair check and its report, the ownership kind published, the DNS records on got-paws.net (a worksheet for you), and the JSON boundary. Zurfur keeps its own minter (R-16). |
| web | Seat and invitation screens, Requests, `participants`, and Account members (invite, accept, roles, leave). The Accounts pages folded into the Den (R-39). The handle-change and DNS-record screens. The console's screens. **The production sign-in screens**, for staging. |
| internal | The pipeline's first deploy, to staging. CI tests that only moderation writes a hold, and that the audit log refuses rewrites. |

**If 0.4 runs over** (a proposal): identity's renames to and from a brought domain move to 0.5. Verification at founding stays, because it's on the security card. Staging's own pieces (the KMS adapter, the production client, the pipeline) can't move without moving staging.

**Exit check:**
1. As alice, declare a Client seat and invite bob. bob finds it under Requests and accepts, and the commission shows in his `commissions`.
2. carol applies to a Creator seat and is seated. Her 11th application that day is refused.
3. Found an Account as a domain you don't control: it's refused, and nothing is minted. With a test domain, Zurfur shows the record to add, and founding completes.
4. Sign in to the console with Bluesky, then your passkey. Take down a cleared file: alice and bob get "Nothing here", and the changelog says "Zurfur moderation took down a file".
5. As the app's database role, `UPDATE` on the audit log is refused.
6. A signed-out browser loads a Public product from the user-content domain without sending `zurfur.sid`. It carries no EXIF, an SVG downloads instead of rendering, and an Adult product's image isn't served.
7. **On staging:** sign in with your real handle through the production client. Anyone else is kept out, as question 107 rules.
8. **On staging:** it boots against the KMS key and refuses the shipped dev key. A deploy waits for your approval as the environment's reviewer.

**Doors decided here:**
- the handle namespace and reserved list, and a hard-deleted handle (question 111);
- the user-content domain's name, and its link scheme for Adult bytes (question 102);
- the hosting provider and region (question 65, picked in 0.3);
- the production client's published metadata address (question 107);
- the console's origin, which closes at the first enrollment on production;
- the audit log's shape, and "Zurfur moderation" as the public name on acts;
- the storage of seats, applications and roles;
- the read by real path.

### 0.5: Characters in slots; Accounts get their own repo

| Team | What lands |
| --- | --- |
| commission | Slots: open, a character on Zurfur, an off-platform character, a slot's own files, and its note as a comment. The keeper's request → accept, and "This character is private" with Request Access. A slot in a live commission becomes the first real Character fact. |
| den | Slot folders and their files, within the depth cap. The Account repo's records get their place in the tree (moved here from den's second alpha slot). |
| identity | **Accounts get a repo** (identity's sixth slot, moved ahead of the User Page). Mint and attach in one founding flow, with the hosted-PDS admin steps in an operator-only process (R-24). Soft delete deactivates the repo, and ownership transfer moves the top key. A nightly export (a proposal). A local PDS joins the dev stack. The slot fact joins the Character facts check. |
| moderation | **Take-down reaches the hosted PDS** (moderation's first alpha slot, moved here). Identity's proposed Account repo holds an avatar, so this version writes the first blob to a hosted repo. Every such blob is scanned first, and take-down runs through a separate worker that holds the admin credential and is reached through the outbox. |
| architects | Zurfur runs a PDS in dev and tests, **and on staging** (moved from 0.8 with R-47). Its rotation key is under KMS, and its admin steps run in an operator-only process (R-24). The storage-routing crate is watched; identity expects the pain here first. |
| security | The door review of the hosted-repo attach, including the shared rotation key, before the first attach. |
| web | Slot screens with the keeper's consent; an Account's public repo. |

**Exit check:**
1. alice adds slots "Hero" and "Kael", then asks for bob's "Abco". bob accepts under Requests. When bob makes Abco Private, alice's slot shows "This character is private".
2. bob tries to delete Abco while it sits in a live slot. It can only be tombstoned, and it leaves his Den.
3. Found an Account. The local directory lists your key, then the PDS's key, then Zurfur's, with the local PDS as its service.
4. Soft-delete the Account: the PDS reports it deactivated. The API process's environment holds no PDS admin password.
5. Restore from last night's export once.
6. Take down an Account's avatar: the local PDS stops serving it, and the evidence is kept.

**Doors decided here:**
- what an Account's repo holds, and its lexicon;
- where Zurfur's PDS lives;
- the hosted repos' rotation list;
- slots and the keeper-consent record;
- take-down on the hosted PDS, which amends the admin credential's "attach/recovery only" rule.

### 0.6: Your public face, reports and lockdown; the board

Den's public reads, moderation's lockdown and reports, identity's User Page and posts' Gallery land **together**. Moderation's check "a signed-out read is refused under lockdown" can only run once signed-out reads exist.

| Team | What lands |
| --- | --- |
| den | Reads by people with no relationship to a node, signed-out visitors included: Listed cards and Public nodes, plus the derived User-Profile and Gallery views. Built first:<br>• lockdown's gate in the resolver;<br>• **the hide hold, filtered on every public view** (moved here from 0.7, under R-35);<br>• bounded folders and keyset page tokens;<br>• the same statements for each pair of not-found causes on public paths;<br>• the maturity a card needs;<br>• **Adult content withheld (a label, no image) from any viewer without a passed age check**, with the check stubbed as "not passed" until 0.8. Where this rule lives is question 101.<br>`/profile` records are served after their signed-commit check. **den.proto reaches its alpha shape, and you review the whole file**, type names included. Your stored viewer settings land (R-39); where they live is question 109. |
| identity | The public User Page at `/profile/<handle or DID>`, listing only the memberships you chose to list. Rate limits and a cache for anonymous lookups and sign-in; avatars served through Zurfur. For moderation, the suspended state at the session gate, and the first-sign-in gate that lockdown's sign-ups switch needs. |
| moderation | Anyone can report anything, with a 48-hour clock for intimate images; lockdown's four switches; suspension and ban; notices to owners. Notices stay inside Zurfur until the mailer lands in 0.9. The bug-report reader moves into the console. |
| posts | A minimal Gallery: Public products, newest first. Adult work is a label with no image (stubbed until 0.8), and other ratings above Safe blur by your setting. It applies the hide hold, never shows a Listed card, counts nothing the viewer can't see, and has a report link on every item. |
| commission | The board: Workflows with columns as child nodes, one board per Account, ordering by fractional keys, and each card's own alias and archived state. |
| web | The User Page and the Gallery. Adult work as a label with no image (stubbed until 0.8), the blur for other ratings, and your sensitivity setting. The report button and form, and the lockdown banner. **The board, moved here from web's first and fourth slots**, with dragging plus a keyboard "Move to…". |
| security | Heron reviews the Den as strangers are served it. |

**Exit check:**
1. Signed out, open alice's profile. Only Public things show, and only the Accounts she chose to list. An unknown handle shows "Nothing here". Thirty sign-ins from one address in a minute meet the limit.
2. As a moderator, hide a Public commission: it vanishes from every public view for everyone else.
3. Turn on "public pages off". Public reads stop, and your own Den still works. `/.well-known/atproto-did` and sign-in still answer.
4. As bob, report alice's file as child safety; it shows in the console. Close it, and bob sees the outcome while alice never sees his name. A suspended bob signs in read-only.
5. A product made Public appears in the Gallery, blurred if it's above Safe. An Adult one shows only a label, to everyone until 0.8. Make it Private and it's gone.
6. In Supreme Arts, create "Queue" with three columns. Place two commissions, drag one up, move one by keyboard only, and reload: the order holds.

**Doors decided here:**
- public page addresses (`zurfur.app/profile/<handle>`);
- den.proto's alpha shape and the type names, with the plugin-type rule;
- card storage (the link, the position key's format, the alias);
- the report record and who reads it;
- your viewer settings' keys (question 109).

### 0.7: Who else sees it: view grants, public Characters and claims, blocks, legal pages

| Team | What lands |
| --- | --- |
| commission | View grants: Presentation gives a card, Description opens the node, Total opens everything, and revoking is quiet. Bringing a commission into an Account (the exception that lasts only for the MVP), and an Account's bookmarks of outside commissions. The brief, frozen where you ruled (R-15). Client-visible progress on a public board. "Make Listed or Public" asks for the rating first. Invitations, applications and slot requests check blocks. |
| den | Grant mounts; an Account's `commissions` folder with members' mounts. Den's grant checks. The claim records' places in the tree (moved here from den's second alpha slot). |
| identity | **Public Characters** (publicize creates and attaches a repo and writes the `owned` half; de-publicize tears it down) **and claims** (the `owner` half in your own repo; "claimed, unconfirmed" when only one half exists). The write scope at first publish, plus the full-access fallback. Sign-out revokes the stored PDS tokens. Reading Bluesky block records for moderation. A Public Account. |
| moderation | Blocks: Bluesky block records read through the hardened resolver into a private cache, with no Zurfur block button. The live scanner behind the port, if a vendor accepted an individual, wired on staging (a security review, since files cross the private boundary). The legal pages, live, with stable clause numbers. |
| architects | The outbox's "waiting on the person" state, for writes into a person's own repo, which claims start. **The storage-routing checkpoint at the end of 0.7** (the second kind of public record); the crate is built only if its trigger is met. |
| web | Grant screens; client-visible progress; the publicize, consent and claim screens; Ember's public page; the legal pages. **The brief view, moved here from web's first slot.** |

**Exit check:**
1. Grant dave Description: he sees the header and open seats, not the files. Raise it to Total and he sees the files. Revoke it and the commission leaves his Den without a word.
2. Bring the commission into Supreme Arts: every member sees it. With Supreme Arts Public, a signed-out browser sees the card on "Queue" and can't open it.
3. As a test user on the local PDS, publicize a Character: the PDS asks once for write access. Delete your half, and it shows "claimed, unconfirmed".
4. Sign out: the PDS lists no session for Zurfur.
5. Block one of your Bluesky accounts from the other in a Bluesky client. Their comments vanish for each other, neither can invite the other, and a commission they share stands.
6. The vendor's test procedure holds an upload, and the legal pages are live.

**Doors decided here:**
- the claim halves' shape and write order, which close at the first real claim;
- the publish permission set's contents;
- grants and the bring-into-Account record;
- the Terms' clause numbers.

### 0.8: Ready for real people: the age check, invite codes and the directory watcher

Staging has run since 0.4 (R-47), so this version is the last one before the cut that adds features, and its work is all checked on staging.

| Team | What lands |
| --- | --- |
| identity | Command-line sign-in through the browser. The directory watcher. The invite-code gate and store (a proposal). Moderator sign-in states. **The age check (R-6, R-7): the first-sign-in step and the stored result, 18 and over, nothing but the result kept.** Who builds the vendor integration is question 110. The accepted Terms version on the User. Then a review of everything identity built since 0.1. |
| den | The real age result replaces 0.6's stub in the rule of question 101. |
| posts | Adult bytes on the user-content domain, as question 102 rules. |
| web | Entering an invite code and seeing the codes you've earned; the age-check and Terms-acceptance screens. Adult work shows a label with no image to signed-out visitors and to anyone who hasn't passed. |
| moderation | A rehearsal of the drill on staging. |
| security | The automated scan of staging re-run (a proposal), and the review of identity's whole surface. |

**Exit check, on staging:**
1. A new handle without an invite code is turned away politely, and one with a code gets in after the age check.
2. Signed out, an Adult Public product shows only a label, and no address serves its image to you. After you pass the check, it shows.
3. `zurfur session login` opens your browser and signs the command line in to staging.
4. An operation Zurfur didn't sign, on a DID in staging's private directory, raises the alarm.
5. Identity's review lists no open must-fix.

**Doors decided here:** the stored age result and Terms version.

### 0.9: The alpha cut: production, and every door ready to close

| Team | What lands |
| --- | --- |
| architects | Production:<br>• managed PostgreSQL with point-in-time recovery, and encrypted backups away from the app host, with a restore drill;<br>• alerts to you, and the deployment checklist;<br>• **Zurfur's PDS on its own domain, its rotation key in KMS, and its data directory and blobs in the backup set**;<br>• the wildcard certificate for production's handles;<br>• production's own KMS key, with deletion protection, an alarm on any scheduled deletion and a written recovery procedure.<br>The watcher running in the worker. The stored shapes reviewed as doors and ruled by you. **One baseline migration**, plus gates that refuse an edit to a released migration. The contract-freeze machinery and the final cleanup, including the CI assertion over `lifecycle.toml` for the baseline-reset hatch (R-40). **Lexicon tooling and its additive-only gate only**: Zurfur's lexicon DID is minted at 0.10, after the directory switch. |
| internal | The pipeline's production half, completing the deployment card (staging's half since 0.4). An npm license gate. Agents that read real data running under commercial terms (R-12). The mailer, for moderation's notices. |
| identity, commission, den | Export and erasure of a person's data as named operator actions, never hand-written SQL, with one drill. |
| web | The production build behind one origin, with a full content-security policy. The report button everywhere, footer links to the legal pages, and a license-notices page. An accessibility audit with real screen readers, and a short accessibility statement. Error reports and visitor counts only through Zurfur's address. Pages kept out of search engines. `__Host-zurfur.sid` with an absolute lifetime. |
| moderation | The drill on the deployment: one match, end to end, to a recorded legal report. The reporting route registered. |
| security | The alpha-readiness stage gate: every row of the security card built and reviewed, the incident runbook walked once, and the security event log. |
| compliance | The alpha gate audit: every row met with evidence, or accepted by you in writing. EU and UK representatives appointed (R-11). The UK Online Safety Act paperwork signed, with you named (R-46). The children's risk assessment is done here unless counsel said in 0.3 that the UK relief holds (R-50). Compliance's review of what goes into a PLC operation, which is a precondition of the switch, plus identity's answer on erasure against the public log (question 106). |
| den, commission | You review `commission.proto` as a whole, and den.proto's final additions. |
| plugin | Prepares the door check: the eleven "keep open" lines, each with a test. |

**Exit check:**
1. Watch a restore of last night's staging backup reach a given minute, then sign in to the restored copy.
2. Stop the PLC directory longer than the outbox's patience: the alert reaches you.
3. `just db-reset` runs exactly one baseline migration.
4. A PR that edits a released migration, renames a proto field or removes a lexicon field fails CI.
5. Run one access request and one erasure request through the named operator actions.
6. Tick the deployment checklist on production.
7. Compliance's report and security's stage gate list no open must-fix.

**Doors decided here (they close at 0.10):**
- starting the alpha on an empty database;
- the changelog's entry encoding;
- sessions;
- what goes into a PLC operation (question 106);
- production's KMS root (deletion protection and a recovery procedure);
- the contract's final cleanup (`Account.id` already gone, the changelog's bare array, the type names).

### 0.10: The alpha opens on the real domain (`alpha-v0.10.0`)

| Team | What lands |
| --- | --- |
| architects | The first production deploy from the alpha tag. v1 becomes additive-only. The restore drill re-run on production. **Joining the relay mesh**: a deliberate switch, thrown only after take-down reaches the hosted PDS (0.5). Zurfur's lexicon DID minted and `_lexicon.zurfur.app` published, after the directory switch. |
| identity | Submission to the public directory turned on, after security's door review and compliance's review of what goes into an operation, with the watcher live. The PDS goes public on its own domain, with the nightly export running. Lexicons published when their feature ships. |
| security | The directory switch's door review; a live watcher drill; `security.txt` on the real domain. |
| moderation | Invite codes obey moderation: a ban voids unused codes, "sign-ups off" stops redemption, and who invited whom stays private. Retention timers set by counsel. **If no scanning vendor accepted an individual, the alpha opens with uploads off** (R-9), and the fallback comes back to you. Since the alpha starts on an empty database (question 69), nothing with an image would exist: no attachments or products, no Account avatars, an empty Gallery and Portfolios without images. **If no age-check vendor accepted** (R-45), Adult work shows to no one, 18 and over is self-declared in the Terms, and the UK children's risk assessments become real work; that fallback comes back to you too. |
| plugin | The door check, signed off by Mantis. |
| vulpes | Frozen from the first real claim. Zurfur pins a tag. |
| web | Public addresses stable, with redirects for anything that moved. |

**Exit check:**
1. Found an Account on the real domain. plc.directory lists your key first and names the handle.
2. `https://<handle>/.well-known/atproto-did` returns the same DID.
3. A public Character's claim pair checks out with Vulpes's checker, with no Zurfur code in the loop.
4. A second person signs in with a fresh invite code, after the age check.
5. `security.txt` is valid and expires within a year.

**Doors that close here:**
- every pre-alpha door above;
- turning on directory submission, and what goes into each operation;
- joining the relay mesh;
- the first published lexicons and Zurfur's lexicon DID;
- the first real mint's key list;
- the claim record.

### 0.11: Close the loop: approval, invoices, ratings, Portfolios

| Team | What lands |
| --- | --- |
| commission | The Request for Approval. Invoices: lines prefilled from slots, voided and reissued but never edited, the issuer's "paid" mark a fact. **Reputation** (owner: commission, R-41): both sides rate on Completed, 1 to 10, positive only. |
| posts, den | Portfolios: a new always-public `/portfolio` directory of links to commissions, each shown at the visitor's level. |
| identity | Ratings on the User Page. |
| web | **The approval and invoice screens (moved here from web's sixth slot)**, the rating screens and Portfolios. |
| compliance | The invoice wording; how long ratings are kept, and what erasure does to them. |

**Exit check:**
1. "Request approval" sets "Waiting for Approval", and "Approve" clears it.
2. An invoice prefills two lines from two filled slots. Editing it is refused. Once it's marked paid, delete refuses.
3. Completed asks both sides to rate, and both ratings show on both profiles.
4. alice's Portfolio of two commissions shows to bob. When she makes one Private, it vanishes for him.

**Doors decided here:** invoices and the approval act; keeping ratings; where a Portfolio lives.

### 0.12: Ask for art, tags, friends and alts

| Team | What lands |
| --- | --- |
| den | **Tags** (owner: den, R-41): node attributes in the `community.`, `owner.` and `system.` namespaces, read only through Atta. Queries over a derived index pass each result through Atta, the ask feed first; this is the Lens engine's first piece (owner: den, R-41). Places in the tree for friendship and user-link records. |
| commission | Ask-for-Art: the client creates the commission, with a vacant Creator seat and "Looking for Artist". Owner tags and the reserved status tags. |
| identity | "Commissions Open" on an Account. **Friendship** (owner: identity, R-41) **and user-linking** (question 83), both double-sided. Shared Characters. |
| moderation | Deleting and banning a tag; this kill switch comes with the tags card. Blocks on ask applications and on friendship. |
| web | The ask screens and feed; friendship and user-linking; "Your reports" (a proposal). |

**Exit check:**
1. Tag your commission and set "Commissions Open". bob sees the tag only where he can see the node, and the ask feed counts nothing he can't see.
2. Create "Need a ref sheet" and send the link. A second account applies, and the handshake seats them while you stay owner.
3. alice and bob friend each other. It shows only after both accept.
4. Link two of your handles: each page names the other. Undo it, and both references go.

**Doors decided here:** the status tags' names; attribute namespaces; the user-link and friendship records and their lexicons.

### 0.13: Character handles, command line without a browser, beta readiness (last alpha)

| Team | What lands |
| --- | --- |
| identity | Character handles, if you rule them cheap. Command-line sign-in without a browser. Whether Bluesky's servers support the device-code flow is unverified; the fallback is a code shown on another device. |
| den | A tombstoned character shows its name under the toggle (an additive access case). |
| plugin | The plugin design study, on paper, plus a design-sync of the plugin pages. |
| architects | Groundwork for external mounts: the availability field reserved, the guarded client made general plus a network egress guard, and custody for third-party tokens. The storage-routing checkpoint decided. |
| moderation | The full moderation design (queues, triage, roles, appeals) written and ruled. You set the beta's moderator count. |
| security, compliance | The beta-readiness stage gate and the threat-model refresh. A request drill and a breach drill. The UK and US size tests measured on alpha data. |
| vulpes | The watcher helper and the expiry groundwork. |

**Exit check:**
1. A public Character answers at its `*.zurfur.app` handle, both ways.
2. `zurfur session login` works from an SSH session with no browser.
3. The watcher ran the whole version with every alarm explained.
4. A hosted repo was restored once from its nightly export on the real domain.
5. Every alpha card is built (see the card map).

**Doors decided here:** the Character handle namespace. If you pull external mounts into the alpha, this is their earliest version: read-only, owner-only, one provider.

### Beta, 0.14 to 0.20

The beta gets its own release planning. These versions show order and dependencies; each will be re-cut then.

| Version | What lands, by team | Exit check (short) | Doors |
| --- | --- | --- | --- |
| **0.14** (beta opens) | **moderation:** a second platform role and a triaged report queue; the scan gate on post uploads. **architects:** the API and worker on two or more instances. **posts:** your first Gallery Post in your own PDS, with the publish consent; scanned and re-encoded before it leaves; alt text and rating required; the post lexicon published. **commission:** phases (check-off, client approval reusing the approval act) and commission-page comments. **compliance:** re-measure the size tests; **self-serve export and deletion** (see conflicts). **web:** compose, posts, phases, comments. | A Safe post with alt text shows in your PDS and in the Gallery, with no metadata in its bytes; deleting it in Zurfur removes both. A commission created from "Queue" starts with its phases. A second moderator takes a report from the queue. | Posts are always public; the post key; the post lexicon |
| **0.15** | **posts:** publish a product as a post (snapshot, credits, deduplication, community tags). **commission:** the hash chain (alpha entries chained once; an operator check; redaction keeps the hash). **den:** the changelog's hash as the chain's head. **moderation:** redaction across the chain; credit disavowal. **web:** publish from a product; "record intact". | Two people publish the same final: one Gallery item with two sources. Alter one entry: the check names it. Redact one: the chain still verifies. | The snapshot's shape; the chain's input |
| **0.16** | **plugin:** a Golem registered by the Zurfur team, `/plugin/v1` opens, the Golem accepts an invitation and reads the changelog at its seat's ceiling. **identity:** Golem DIDs, with one Zurfur-held key each. **commission:** Golem seats and the ceiling-bounded read (its half of the plugin API, part 1). **moderation:** suspend a Golem. **architects:** the second sign-in zone. **posts:** public replies and shouts. **commission:** YCH templates. **security:** the plugin-surface review. | A Golem in a Description seat reads only what Description shows; an out-of-reach DID and a made-up one get identical answers; a cookie without a token is refused. bob's reply lives in his PDS; alice hides it. Claiming "Winter YCH" makes a new commission. | The first Golem DID; scope names |
| **0.17** | **plugin:** Golems act (notify, act, manage) and append `plugin_recorded` entries. **commission:** plugin entries in the changelog contract (part 2); commission trees. **architects, commission:** domain events as code, which achievements, Kafka, notifications and webhooks all consume. **web:** consent-again; plugin sentences as plain text; trees. | A Golem with `act` moves a status, the changelog says so, and the same call with `read` is refused. A child commission "Lineart" sits under "Big piece". | `plugin_recorded` entries; tree storage |
| **0.18** | **den with plugin:** plugin types render in the Den. **den:** your own folders and dotfiles; **the membership-confers-view exception removed**, as already ruled for after the MVP. **commission:** its half of that removal. **posts (owner, R-41):** Seals and achievements. **identity:** Zurfur's attesting DID. **moderation:** the labeler blocklist. **web:** the declarative-view renderer, themes, Seals. | A first-party test type renders, and without the plugin it shows the generic icon. A `.theme.yaml` restyles your space for visitors. A member no longer sees a brought commission without a grant. A peer grant from a mutual shows on your page. | A plugin type's wire form; dotfile names |
| **0.19** | **plugin:** Golems in Accounts, and installs by Users and Accounts. **internal (owner, R-41) with architects:** Kafka behind the outbox. **den:** external-storage mounts (a proposal), plus the Den at scale. **identity:** more ways to sign in (a proposal). **posts:** posts written in other apps, scanned at ingest (a proposal). **moderation:** appeals. **security:** the third-party-token door review. | An Account admin adds a Golem, and removing it ends its reach at once. A mounted folder's changed file isn't served, and an offline mount shows "unreachable" only to you. Attach Discord and sign in with it as the same DID. alice appeals a take-down. | The first stored third-party token |
| **0.20** (last beta) | **security:** the outside test against ASVS level 2, and the release-readiness gate. **plugin:** testers register their own Golems, and the Trello importer if pulled into the beta (both proposals); the publisher ban (moderation). **identity:** the hardening tier. **compliance:** the company formed (R-5: at beta or release) before subscriptions. **vulpes:** the attestor toolkit, if placed. | The testers' report is in, with every finding fixed or accepted in writing. A Character moves to another User, and the undo works within 72 hours. | The disclosure policy's text |

### Release, 1.0 to 1.3

| Version | What lands, by team | Exit check (short) | Doors |
| --- | --- | --- | --- |
| **1.0** (`release-v1.0.0`) | **notifications** (owner: posts, R-41), with commission's domain events as one intake and den's access check. **capacity guards** and **the openings feed** (commission, on the Lens engine). **Static collections** (posts, den). **identity:** credible exit proven. **den:** a full re-review of Atta and a drill that rotates the MAC key. **architects:** v1 generally available, per-version telemetry and a cold-start recovery drill. **moderation:** abuse controls and counsel's duties. **compliance:** open sign-up, age checks everywhere adult art shows, geo-blocks. **security:** the disclosure policy and a clean retest. **web:** an inbox, search engines let in, and the accessibility statement after the outside audit. **Everyone:** one "take everything with you" export. | A test Account leaves Zurfur's PDS and keeps its DID and handle. The openings feed never lists or counts a commission you can't see. Accepting past an artist's capacity warns. | v1's 12-month notice window; search engines let in; the published disclosure policy |
| **1.1** | **disputes**, with tiers and warnings (R-48), after notifications in 1.0 so the other side hears; commission builds them, with mediators from moderation. **webhook-signing:** per-install keys and signed, SSRF-hardened deliveries, sent only to first-party Golems until 1.2. **telegram** (plugin). **Dynamic collections** (posts, on the Lens engine). Moderation's hooks for these release cards. | Raise a dispute: the deadline freezes, "paid" is refused, and the other side is notified. A delivery verifies with the reference library. The Telegram bot posts a watched status change. | DIDs delivered to hosts Zurfur doesn't run; the webhook signature scheme |
| **1.2** | **marketplace**, and `/plugin/v1` goes generally available (plugin; moderation's publisher ban; security's trust ladder before any third-party install). **trello-import** (unless done in 0.20). **posts:** durable for real work. | A test publisher lists a plugin; a ban removes every listing and stops every runtime. | `/plugin/v1` generally available; public listings and reviews |
| **1.3** | **subscriptions:** a tier lifts a patron's level as a grant does (den, web). It needs the company and a payment processor that accepts adult content (compliance). Atta as its own library (a proposal). | A patron sees tier content, and a lapsed patron gets "Nothing here". | Taking money: tiers and the processor |

## Gaps and owners

You ruled the owners of the areas that had none (R-41); Internal may re-shape them later from data. The rest are for the tech lead to confirm.

| Gap | Owner | Note |
| --- | --- | --- |
| **Deployment** | **Ruled** (R-3, R-47): Internal owns the pipeline, with staging due at 0.4 and production at the alpha cut (0.9); architects design the hosting, which you pick in 0.3; you hold the running environments. | Internal has no roadmap file, so the 0.4 deploy isn't sized against its capacity. |
| **Notifications** (release card) | **posts** (R-41), with commission's domain events (0.17) as one intake and den's access check filtering every row | The second intake, posts written in other apps, is a 0.19 proposal. |
| **Reputation** (alpha card) | **commission** (R-41); identity shows totals on the User Page | — |
| **The Lens engine** | **den** (R-41) | Its first piece is the ask feed in 0.12. |
| **The pending list** | **Ruled: "Requests"** (R-42). Commission builds the read, web the page, in 0.4. | — |
| **Friendship** (alpha card) | **identity** (R-41), in 0.12 | — |
| **Seals and achievements** (beta cards) | **posts** (R-41), with identity's attesting DID, commission's events for EXP and moderation's labeler blocklist | — |
| **Tags** (alpha card) | **den** (R-41) for the mechanism; commission for status tags; posts for community tags | — |
| **Kafka** (beta card) | **Internal** (R-41) builds it, **architects** design it, and you run it | — |
| **The API's cross-site write guard** | **identity**, in 0.2 (R-20) | — |
| **The KMS adapter** | **identity** builds it (custody part 4) in 0.4, for staging (R-47), and **architects** review it | Both teams had listed it. |
| **The moderator passkey** | **moderation** builds it in 0.4, and identity reviews it. Pending Meerkat's and Fox's agreement; security-reviewed either way. | Identity had it in its last slot, three versions after moderation needs it. |
| **The age check** (no card) | identity stores the result and gates the first sign-in; den and posts apply it on the server (questions 101 and 102), as a stub in 0.6 and for real in 0.8 | Who builds the vendor integration is question 110. |
| **Workflows' code path** | **commission**, with `application/src/account/workflow` added to its charter | — |
| **The mailer** (moderation's notices) | **Internal**, in 0.9; you hold the account | Notices stay inside Zurfur until then. |
| **Domain events as code** | **architects** design them, and **commission** builds them first, in 0.17 | — |
| **Who reads alpha bug reports** | **You**, until agents run under commercial terms (R-12) | — |

## Cross-team conflicts, resolved

| Conflict | Resolution | Why it satisfies both |
| --- | --- | --- |
| **KMS:** identity's last pre-alpha slot vs architects' third | The **custody port** (one port, a key id per wrapped secret, one key per purpose) lands in **0.2**. **Ruled (R-47):** the KMS adapter, the production sign-in client and staging land together in **0.4**, with identity building the adapter and architects reviewing it. | Den's MAC key needs custody in 0.2, and security asks for one key per purpose. KMS must come before staging, and the boot guard already refuses staging without it. Staging five versions before production is architects' answer to a big-bang first deploy. |
| **Staging at 0.4 vs "CD is designed at alpha planning"** (R-2) | A conflict the move created. Staging's deploy is designed in **0.4's plan**; production's half (its protected environment, deploys from alpha tags) at alpha planning (question 66). | Staging can't run without a pipeline, and the production design can still wait for the alpha. |
| **Staging at 0.4, before moderation's floor** | A conflict the move created. Allowed while no one but you can reach it (question 68), with sign-in gated as question 107 rules. The floor is due the day anyone else can. | Your ruling on moderation's first deadline (D-26) is about who can reach a build, not where it runs. |
| **Identity's 0.4 grows to about 13 units** | A conflict the move created. If it runs over, renames to and from a brought domain move to 0.5; staging's pieces and verification at founding stay. Fox re-sizes. | The KMS adapter and the production client can't move without moving staging. |
| **The security gate "the first staging deploy"** moved from 0.8 to 0.4 | The secrets inventory for staging's secrets, short-lived deploy credentials, a protected environment and pinned images land in **0.4** | Security's rule is that these come before the first secret in a deploy, wherever that falls. |
| **Cross-site write protection:** web's last slot and identity's last vs security's first | **Ruled (R-20): 0.2.** Web builds the web tier's check and identity the API guard, on cookie routes only. | Every write route born after it is guarded by construction. Plugin's condition keeps the bearer API open. |
| **Moderation's lockdown check vs den's public reads** | **Both in 0.6**, with identity's User Page and posts' Gallery | "A signed-out read is refused" can only be shown once signed-out reads exist. |
| **The outbox:** identity's first slot vs architects' third | **Ruled (R-24): 0.2**, carrying founding, the handle change and the tombstone | Mint ordering and tombstone durability depend on it, and the handle change is today's write with no retry. |
| **The byte store:** posts', architects' and moderation's first slots vs den's and commission's second | Built during 0.2, it **lands in 0.3** with the first upload change. Posts owns the store and the byte route, commission owns file nodes and uploads, moderation owns the scan gate, and architects review the shape. | Uploads move once, onto the gate, the store and the scan together, which honours your ruling on moderation's first deadline. |
| **Payloads:** commission and web want commission fields and changelog pages in the first slot; den serves payloads in its second | **Ruled (R-44): core payloads in 0.2**; den confirms the size, and overflow moves to 0.3 with you told. File bytes and uploads wait for 0.3. | Writes you can't see aren't demoable. |
| **Web's first slot ran ahead of the backend** (Characters, the board, the brief) | Character screens go to **0.3**, the board to **0.6** and the brief to **0.7**. The approval and invoice screens (web's sixth slot) go to **0.11**. | This is web's own rule: a screen ships with its capability. |
| **Viewer settings and dotfiles:** web's second slot vs den's beta | **Ruled (R-39):** the light theme follows your system setting in **0.3**, and stored settings land in **0.6**. Where they live is question 109. The rest of dotfiles waits for **0.18**. | Nothing goes into browser storage, the 0.1 rule. |
| **Adult work in 0.6 vs the age check in 0.8** | 0.6 builds the ruled shape, a label with no image, with the check stubbed as "not passed". 0.8 makes it real. | Nothing ever shows Adult work by self-declaration (R-6). |
| **Moderation's hide hold (its fifth slot) vs public views (0.6)** | The hide hold is filtered on every public view from **0.6** | Under R-35, a hook ships with its feature. |
| **Seat ceilings:** den's third slot vs commission's third | **Both in 0.4.** Den's board and grant checks run in 0.6 and 0.7. | Den's links-and-mounts slot needs records that commission ships across three versions. |
| **The User Page needs den's public reads** | Identity's Account repos move **ahead, to 0.5**, and the User Page goes to **0.6** | Repos don't need the User Page, and public Characters (0.7) need both. |
| **Character facts before slots** | 0.3's Character facts check is a stub; the slot fact lands with slots in **0.5** | It works the way R-20 handles Accounts: each fact lands with the slice that makes it real. |
| **Take-down on the hosted PDS:** moderation's first alpha slot | It moves to **0.5** with Account repos, as moderation's own rule says | Identity's proposed Account repo holds an avatar, the first blob in a hosted repo. |
| **Den's alpha slot for public records vs identity's pre-alpha repos and claims** | Each record gets its place in the version that publishes it: Account repo records in 0.5, claims in 0.7, friendship and links in 0.12 | Records must be served from somewhere when they ship. |
| **Moderation's identity asks** (passkey, suspended state, first-sign-in gate) in identity's last slot | Passkey in **0.4** (moderation builds it, pending both leads' agreement); suspended state and the first-sign-in gate in **0.6** (identity); moderator sign-in states in **0.8** | Each lands with the moderation version that needs it. |
| **The production PDS, the wildcard certificate and the relay switch** were in no version | Staging's wildcard certificate in **0.4** and Zurfur's PDS on staging in **0.5**; both in production in **0.9**; the relay switch at **0.10**, after take-down reaches the hosted PDS. Zurfur's lexicon DID is minted at **0.10**, after the directory switch. | 0.10 opens them to the public, so they must exist first, in order. Zurfur's resolver checks handles over https only, so staging's handles need the certificate from day one. |
| **Tag ban (moderation's fourth slot) before tags exist (0.12)** | The tag kill switch ships with tags, in **0.12** | Moderation's own rule: a hook comes with its feature. |
| **Commission's half of the plugin API all in one slot vs plugin's three** | Split across plugin's versions: seats and reads in **0.16**, appends in **0.17**, Accounts in **0.19**, with the membership exception removed in **0.18** | Each Golem capability lands with its commission side. The exception goes before Golems join Accounts. |
| **The membership exception's removal:** den's third beta slot and commission's fifth | **0.18**, done jointly | Its removal is already ruled. Plugin's Accounts slot (0.19) needs it gone first. |
| **The publisher ban (moderation's third beta slot) has nothing to stop** | "Suspend a Golem" in **0.16**; the publisher ban with testers' Golems (0.20) and the marketplace (1.2) | Beta Golems are the Zurfur team's own, so the switch that works then is suspension. |
| **"Take down posts on hosted repos" (moderation's second beta slot)** | Dropped. A post's take-down is a scan before upload plus suppression on Zurfur's side. | Only Users author posts, on PDSes Zurfur doesn't host. |
| **Moderation's 0.1 objectives quote "the Gallery checks holds itself"** | The Gallery and the byte route read through Atta, so `admit` applies holds | The Den's rule is that nothing reads a node except through Atta. |
| **Disputes:** commission's first release slot vs web's second | **Ruled (R-48): 1.1**, after notifications in 1.0. The plain "Disputed" move covers 0.2 to 1.1. | The other side must hear of a dispute, so notifications come first. |
| **Signed deliveries vs the trust ladder:** security wants all prerequisites together | Signed deliveries in **1.1** go only to first-party Golems. No third-party install happens before the trust ladder in **1.2**. | Nothing leaves unsigned, and no third party runs early. |
| **Export:** compliance wants self-serve in the beta; four teams put export at release | Export and erasure as named operator actions in **0.9** (identity, commission and den, with a drill); **self-serve** at **0.14**; the credible-exit exports (a repo's CAR file, the Den as plain files, leaving for another PDS) at **1.0** | EU users from 0.10 start the one-month legal clock. Moving to another PDS is a separate, larger feature. |
| **Accessibility statement:** compliance's alpha gate vs web's release | A short statement with a contact address in **0.9**; the audited statement in **1.0** | Both duties are met. |
| **Block button:** web's last slot has block and unblock; moderation says no Zurfur button in the alpha | **No Zurfur button** unless you rule otherwise (question 61); Bluesky blocks are honoured from 0.7 | It follows moderation's pending study question. |
| **When the rating is required:** web asks at creation; the Maturity Vocabulary decision requires it before widening | It is **required before Listed or Public**, as the corpus says. Whether the create form asks early is web's UI call. | — |

## Questions, by version

Your rulings R-1 to R-50 are marked **Ruled** in place (R-1 and R-17 to R-19 concern the build and the board, not this list). **No open question blocks 0.2.** 71 questions are open: 63 of the original hundred, and 8 of the 13 new from QA. Two are needed by 0.3. Every answer is pre-filled and yours to change. **Door** marks a one-way door. Numbers are kept from earlier drafts, so a few run out of order where staging's move (R-47) changed when they're needed.

### New from QA, for the Engineer

The must-fixes come first. Each carries the answer its owning lead's file supports, and the version that needs it.

| # | Question | Needed by | Proposed answer | Door | From |
| --- | --- | --- | --- | --- | --- |
| 101 | **Where does the age check bite on the server?** Your ruling says "everywhere", but only identity and web had 0.8 work. | 0.6 (stub), 0.8 (real) | One rule in Atta's host check, where holds already sit: an Adult-rated node's content and bytes open only to someone whose age result is "passed". Signed-out visitors and Golems never pass. Every view inherits the rule (den's signed-out reads, the Gallery, Portfolios, the byte route, commission files), so no view carries its own check. In the alpha every signed-in person has passed (18 and over at first sign-in, on an empty database). So the rule bites on signed-out reads from 0.6, and on Golems from 0.16. | | den, compliance, posts |
| 102 | **How are Adult images served from the cookieless user-content domain?** That domain can't see the session, so a stable link to an Adult image would skip the check. | 0.4 (its link scheme) | Never at a stable cookieless address. An Adult image gets a short-lived link that the byte route issues only to a session that has passed; other ratings keep stable addresses. Until 0.8, Adult bytes aren't served on that domain at all. | **Door** (shared links) | posts, security |
| 103 | **What if every age-check vendor refuses an individual?** | 0.3 (applications); 0.10 | **Ruled (R-45):** apply to age-check vendors now, alongside the scanning vendors. If all refuse, Adult work shows to no one, 18 and over is self-declared in the Terms, the UK children's risk assessments become real work, and the fallback comes back to you. | | compliance, moderation |
| 104 | **Which age-check vendor, at what cost?** | 0.6 (your pick) | Compliance shortlists the vendors that accept an individual after vetting in 0.3, with each one's price per check and whether it keeps only the result. You pick by 0.6, so 0.8 can build against it. | | compliance |
| 105 | **Who is the named person under the UK Online Safety Act?** | 0.3 (drafts), 0.9 (signed) | **Ruled (R-46):** you. Compliance drafts in 0.3, counsel reviews, and you sign before the alpha. | | compliance |
| 106 | **What goes into a public directory operation, and how is an erasure request answered against that permanent log?** EU users' handles enter it from 0.10. | 0.9; closes at 0.10 | Compliance's review is a precondition of the directory switch. An operation carries only keys, the service address and the handle. Founding says plainly that an Account's or Character's handle stays in a public log for good, and suggests a name that isn't a person's. Identity's answer on erasure moves from 1.0 to 0.9: tombstone the DID, take reasonable steps to tell relays, and say in the Privacy Policy what can't be erased. The impact assessment covers it before the first real submission. | **Door** | compliance, identity |
| 107 | **How is staging reached?** Its production sign-in client must publish its metadata on the public web, where Bluesky's servers fetch it (atproto's OAuth spec, as Magpie read it today), so a network block alone breaks sign-in. | 0.4 (staging, R-47) | Publish only the client metadata document and its public keys at a public address. Everything else on staging stays network-restricted to you, with moderation's sign-in allowlist (only your DID) as a second lock. Whether the metadata must sit on the redirect's origin is unverified; architects check the spec before building. | **Door** (the published metadata address) | architects, moderation |
| 108 | **Staging earlier, as architects proposed?** | 0.3 | **Ruled (R-47):** staging at 0.4, not 0.8. The hosting choice, the KMS adapter and the production sign-in client move to 0.4, which forces the hosting and vendor choice in 0.3. It amends R-21's placement of staging. | **Door** (hosting and region) | architects |
| 109 | **Where do your stored viewer settings live from 0.6** (theme, text size, contrast, motion, maturity sensitivity)? | 0.6 | (a) The one private settings file that The Den's dotfiles ruling names, pulled forward from den's beta slot with no cascade and no other dotfiles. Its name and keys become a door at 0.10. (b) A settings record beside the User, next to the age result, with dotfiles left in the beta and a move into the file then. Proposed: (a), because The Den already decided where these settings live, and (b) means moving them later. Den hasn't confirmed the size. | **Door** (settings keys) | den, web |
| 110 | **Who builds the age-check vendor integration?** Identity is already the critical path. | 0.6 | Moderation's programmers, behind a port, as they build the scanning vendor's; they have no feature work after 0.7. Identity keeps the stored result and the first-sign-in step. Fox re-sizes identity's rows either way. | | consolidator, from identity's and moderation's files |
| 111 | **Is a hard-deleted Account's handle freed at once, or held?** The deletion decision frees it; the handle-change decision keeps it reserved. | 0.4 | A hard-deleted Account's `*.zurfur.app` label goes into the same quarantine as a renamed Account's, then frees. A brought domain is released at once, since its owner controls it. Then sync both pages. (Identity asks this inside its slice without an answer, so this one is the consolidator's.) | **Door** (handle namespace) | identity |
| 112 | **Which country is the operator based in?** | 0.3 | **Ruled (R-49):** your country of residence. It isn't recorded in this file. | **Door** (compliance's) | compliance |
| 113 | **Does the UK relief for an adults-only service still hold when signed-out visitors read public pages with no check?** | 0.3 (counsel) | **Ruled (R-50):** counsel answers in 0.3. The UK children's risk assessment stays planned for 0.9, and is dropped only if counsel says the relief holds. | | compliance |

### Already ruled

| Question | Ruling | From |
| --- | --- | --- |
| Who owns deployment? | Internal owns the pipeline, as a project due at the alpha cut; architects design the hosting; you are the ops owner (R-3). | architects, security |
| Do advisories block only PRs that change `Cargo.lock`? | Yes, with `deny` required, a daily run on `main`, auto-merge only with all nine jobs required, and no required checks on `feature/**` (R-2). | security, internal |
| Version numbering | One continuous count through alpha and beta; the release starts at `release-v1.0.0` (R-4). | consolidator |
| Owners for unowned areas | Notifications to posts; reputation to commission; the Lens engine to den; friendship to identity; Seals and achievements to posts; tags to den; Kafka to Internal (R-41). | consolidator |
| Den payloads in 0.2 | Accepted; den confirms the size, and overflow moves to 0.3 with you told (R-44). | den, commission, web |
| The membership-confers-view exception | Its removal after the MVP is already ruled (My Den rulings, decision 8). Only its timing and mechanism are open: question 93. | den, commission, plugin |
| v1 additive-only from the first alpha tag | Already ruled (0.1 rulings, D-19). Question 70 asks only what "generally available" means. | architects |
| A PLC-log watcher in the alpha? | Answered by the corpus: the did:plc custody decision puts the watcher in v1. Identity builds it in 0.8, and Vulpes's helper comes in 0.13. | vulpes, identity |

### Now: before 0.2 starts

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 1 | Run the Claude Design session before 0.2, with the loading states right after the sync? | **Ruled (R-22):** yes, during 0.1's last units; web syncs at 0.2's start, then builds the loading states. | | web |
| 2 | Approve this version sequence? | **Ruled (R-21):** approved. Pre-alpha is 0.1 to 0.9 (staging, only you, since moved to 0.4 by R-47; 0.9 production), alpha 0.10 to 0.13, beta 0.14 to 0.20, release from 1.0. | | web, consolidator |
| 3 | The domain crate split: shape and timing | **Ruled (R-23):** split at 0.2's start: a small base crate, then `identity`, `workflow`, `gallery` and `moderation`, with `plugin` later, behind a thin `domain` façade. | | architects |
| 4 | Land each security item by the gate it guards, with cross-site protection and the soft-delete gate in 0.2? | **Ruled (R-20):** both in 0.2. The soft-delete gate ships in the slice that makes Account facts real. | | security, identity, plugin |
| 5 | The cross-site origin rule | **Ruled (R-20):** one exact origin per environment, from config. A write with no `Origin` is refused unless `Sec-Fetch-Site` is `same-origin`, in dev too. | | security |

### 0.2

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 6 | Do I read the security card's undefined items right? | **Ruled (R-43):** yes, as identity reads them. | | identity |
| 7 | Where does a node's metadata live? | **Ruled (R-13):** one node table for Atta's fixed metadata, with each type's table and the attributes keyed to it; two writes per create, in one unit of work. | **Door** (stored data) | den |
| 8 | Where do writes sit on the wire? | **Ruled (R-14):** typed calls in each type's own service, one per named action, each handed to one den write use case that runs Atta's checks. den.proto gains no write call. | **Door** (contract) | den, commission |
| 9 | What does the private-data MAC cover, and what does a mismatch show? | **Ruled (R-33):** the real path, type, content hash and name, but not the level. No version number until something outside the database anchors it. A mismatch is the one not-found, reported to you. Heron checks the reasoning at 0.2. | **Door** (security) | den |
| 10 | How does a type's content reach the web? | **Ruled (R-34):** typed now, generic later. Typed messages per core type through the alpha; a declarative view arrives with plugin types in the beta. | **Door** (den.proto) | web, den |
| 11 | What counts as an Account's fact? | **Ruled (R-31):** a commission ever brought into it, an invoice issued under it, a member besides its founder at any time, or a record published in its repo. | | identity |
| 12 | Can a commission move to "Disputed" in the alpha? | **Ruled (R-32):** yes, as a plain move any participant can set or clear, which freezes the deadline. Tiers and warnings wait for the release: disputes land in 1.1 (R-48). | | commission |
| 13 | A separate worker process? | **Ruled (R-24):** one durable outbox in 0.2, run by a separate worker process; the hosted-PDS admin steps run in an operator-only process. | | architects |
| 14 | Where do a commission's brief, budget and seats live? | **Ruled (R-15):** by visibility level. Card-level content is the card; Description-level content, vacant seats included, is the node's payload; the brief, price, references and linked channel are a sixth fixed entry, a Private `brief` file shown at the viewer's standing. Recorded as a decision page in 0.2. | **Door** (stored shape) | commission, den |
| 15 | The byte store: your CrowFS design or the corpus's plain port? | **Ruled (R-25):** the corpus's port, built to CrowFS's byte rules (both hashes, every backend checked on read, mark-and-sweep, encryption at rest). A decision page records it. | **Door** (how bytes are named) | posts |
| 16 | The grace window and the upload quota | **Ruled (R-26):** 30 days, and 2 GiB per User beside the 25 MiB file cap; an over-quota upload is refused before any byte is stored. | **Door** (grace window) | posts, den, commission |
| 17 | A feature's moderation hook in its definition of done? | **Ruled (R-35):** yes. | | moderation |
| 18 | When does counsel start? | **Ruled (R-10):** a lawyer is hired in 0.3, once drafts exist, for a fixed-scope review. | | moderation, compliance |
| 19 | A company before the alpha? | **Ruled (R-5):** stay personal for the alpha, accepting the personal liability. The company forms at beta or release. The operator's country is your country of residence (question 112, R-49). | **Door** | compliance |
| 20 | How does adult art meet age-check law in the alpha? | **Ruled (R-6):** a vendor age check everywhere. Adult work opens only to signed-in users who passed it, signed-out visitors see a label with no image, and only the result is stored. A design-sync of User-Profiles is owed. | **Door** (stored data) | compliance |
| 21 | Minimum age | **Ruled (R-7):** 18 and over for alpha and beta, enforced by the same check at first sign-in. | | compliance |
| 22 | Sexual content of real people | **Ruled (R-8):** banned, everywhere and in every medium. | | compliance |
| 23 | Sexual depictions of characters who are, or look like, minors | **Ruled (R-8):** banned everywhere, in every medium, whatever age the character is given. Counsel reviews the wording. | | compliance, moderation |
| 24 | Which license? | **Ruled (R-27):** AGPL-3.0-or-later for the server and web app, Apache-2.0 for the contract. Enforceability goes to the lawyer. | **Door** | compliance |
| 25 | The ownership claim's other party | **Ruled (R-28):** `did`. | **Door** (first real claim) | vulpes |
| 26 | Push Vulpes's three cut branches now? | **Ruled (R-37):** yes, with no PRs; the PRs open in 0.2. | | vulpes |
| 27 | Where does the threat model live? | **Ruled (R-36):** a corpus page under `project/`; Heron writes version 1 in 0.2. | | security |
| 28 | How does a person hold an owner key? | **Ruled (R-29):** one recovery phrase per User, made in the browser and shown once, with every key derived so their public keys can't be tied together; the decision page verifies that property. Bring-your-own key is offered too. | **Door** (first real mint) | identity, web |
| 29 | When is the owner key required? | **Ruled (R-30):** at Account founding and at a Character's publicize. A Private Character mints with Zurfur's keys only. | **Door** | identity |
| 30 | Zurfur's minter or Vulpes's? | **Ruled (R-16):** keep Zurfur's minter. | **Door** (first real mint) | identity, vulpes |
| 31 | Light theme in 0.3? | **Ruled (R-39):** yes, following the OS setting; stored settings wait for 0.6. | | web, den |
| 32 | Fold the Accounts pages into the Den in 0.4? | **Ruled (R-39):** yes. | | web |
| 33 | The moderation study's scan-gate questions (6, 14 and 20) | **Ruled (R-38):** take the study's answers; moderation lists them in its first 0.2 PR for your review. | **Door** | moderation |

### 0.3

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 65 | Hosting, and how much data you can afford to lose | Needed in 0.3 now that staging runs from 0.4 (R-47). Managed PostgreSQL with point-in-time recovery, one container host, and an object store and KMS from one provider. At most 15 minutes of data lost, and service back within 4 hours. Vendor, region and cost are yours. | **Door** (deploy target and region) | architects, internal |
| 66 | When is deployment designed, now that staging runs from 0.4? Your CI/CD ruling said continuous deployment is designed "at alpha planning" (R-2). | Staging's half (its pipeline, environment and approvals) is designed in 0.4's plan, which forces it; production's half (its protected environment, deploys from alpha tags) at alpha planning, in 0.9's plan. | | consolidator, internal |

### 0.4

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 68 | May staging run before moderation's floor? | Yes, from 0.4 (R-47), while no one but you can reach a build (how it's reached: question 107). The floor is due the day anyone else can. | | moderation, architects |
| 34 | A dev-only route to the local directory and PDS? | Yes, refusing to start outside `ZURFUR_ENV=dev`. | | identity |
| 35 | Public bytes on their own domain? | Yes: Public bytes only, outside `*.zurfur.app`, with no cookies. Private files stay downloads on the main origin. Adult bytes follow question 102. | **Door** | posts |
| 36 | Reserve moderation, legal and impersonation names? | **Merged**: add moderation, legal, safety, trust, report, terms, privacy, copyright, dmca, appeals, zurfur, official, admin, help and team to the existing list. | **Door** | moderation, plugin |
| 37 | Seat ceilings in the formula: confirm? | Yes. A ceiling caps the standing a seat gives, and grants lift as before. | | den |
| 38 | How do links behave? | A link shows a name copied when it is made. Only a node you can open can be linked. A link to an archived node acts like a typed path. | | den |
| 39 | The pending list's name | **Ruled (R-42):** Requests. | | commission, web |
| 40 | Moderator sign-in and the console (study questions 1, 2, 3, 7 and 11) | You are the one moderator, named in config. A passkey on top of Bluesky sign-in, the first one enrolled with a one-time operator code. The console on its own origin. Acts signed "Zurfur moderation", citing a Terms clause. Two database roles. | **Door** (console origin, public name) | moderation |
| 41 | How a redaction is recorded (study question 6) | **Merged**: scrub the content and keep the hash, as the Eventlog decision says and compliance's privacy sources favour, rather than deleting the entry. | **Door** | moderation, compliance |
| 42 | Publish Vulpes's DNS records on got-paws.net? | Yes, three records (resolution isn't hierarchical); Vulpes sends a worksheet. | **Door** (the domain stays registered) | vulpes |

### 0.5

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 43 | What does an Account's repo hold at the alpha? | One profile record (name, description, avatar), and nothing else until Gallery posts. | **Door** (lexicon) | identity |
| 44 | Where does Zurfur's PDS live? | On its own registered domain, never a `zurfur.app` subdomain. | **Door** | identity |
| 45 | The hosted PDS's one server-wide rotation key | Leave it out of each hosted DID's rotation list, or rank it below Zurfur's per-DID key, after a security review. This weighs against your layout ruling of 2026-09-12. | **Door** (first attach) | architects, security |
| 46 | Does the nightly repo export bind at the alpha? | Yes, to storage the PDS host doesn't control, with one restore tested before the alpha. | | identity |
| 47 | How is the keeper's consent stored, and when is a slot a character fact? | A slot request is its own record beside the seat application. A slot in a non-archived commission that is Draft, Batched, Active or Disputed counts as a fact. | **Door** | commission |
| 48 | What fills layout D's "vulpes_recovery" slot? | Name it by role, "an independent recovery custodian". | | vulpes |
| 49 | Take-down on the hosted PDS (study question 13) | A separate worker holds the admin credential and is reached through the outbox. This amends the "attach/recovery only" rule. | **Door** | moderation |

### 0.6

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 50 | Public page addresses | `zurfur.app/profile/<handle>`, so swapping `bsky.app` for `zurfur.app` lands on the same person. | **Door** | web |
| 51 | Type names on the wire, and a plugin's type | **Merged**: ship the provisional names unless the type catalog lands. Every plugin node carries one `type` value, with its own type name in a field only openers see. At least reserve `plugin` and a prefix. | **Door** | den, plugin, architects |
| 52 | What does the alpha Gallery show? | Public products, then Public Characters that have an image. | | posts |
| 53 | Who sees the Gallery during the alpha? | Signed-in members only; signed-out visitors from 1.0. | | posts |
| 54 | Does a Gallery item name its artist? | Only when the commission's `participants` file is Public. | | posts |
| 55 | Your maturity sensitivity for ratings below Adult | Safe, Blurred or None, Blurred by default, kept with your other stored settings (where: question 109). Adult work isn't covered by this setting: it follows the age check (R-6). | | web |
| 56 | User Pages for people who never signed up | Add the privacy notice and an opt-out that compliance asks for. | | compliance |
| 57 | Reports, lockdown and suspension (study questions 4, 5, 9, 10, 12 and 19) | As the study pre-answers them. Reports are stored privately. Owners are told why, except for suspected child abuse until counsel answers. Lockdown has four switches that only take abilities away; any moderator turns one on, and only you turn it off. A suspended person can still leave. There is an intimate-images category on a 48-hour clock. Security asks that one switch double as "writes off" in an incident: yes. | **Door** (report record) | moderation, security |
| 58 | Where do a card's position, alias and archived state live? | In the card link's own payload. | **Door** | commission |

### 0.7

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 59 | How does Zurfur get consent to write the `owner` half of a claim? | **Merged** (architects prefer a Vulpes permission set; Vulpes prefers a plain scope): request the plain scope `repo:net.got-paws.acp.claim?action=create&action=delete` at the first claim, beside Zurfur's own set. A Vulpes permission set comes later and is optional, because a set that can't be resolved fails sign-in. Magpie re-read atproto's permission spec today: partial wildcards are refused, and a set can't reach a sibling namespace. Fix Zurfur's own set (`repo:app.zurfur.*`) the same way. | **Door** (permission set) | vulpes, architects |
| 60 | What do the two ownership halves carry in the alpha? | A far-future expiry from the kind, and no status or witnesses. | **Door** | vulpes |
| 61 | A Zurfur block button in the alpha (study question 15)? | No. Bluesky blocks are honoured, and web drops its block and unblock screens. | | moderation, web |
| 62 | The storage-routing crate's rules, if built | The outbox's front door. It picks the store by the boundary contract's class test, never makes a write atomic across stores, and is built only when its trigger is met. | | architects |
| 63 | The reporting route and how child-abuse material is detected (study questions 16 and 17; 18 is ruled) | Keep NCMEC as the reporting route, as Inventory Closures decided. Compliance reads UK law as counting reports already made to NCMEC. Counsel confirms that an individual can register and whether a person looks before a report goes out. Detection: hash matching from whichever vetted vendor accepts an individual (PhotoDNA, Arachnid Shield or Safer), plus a classifier that ranks human review where the vendor offers one. | | moderation, compliance |

### 0.8

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 64 | Invite codes: who builds them, and what are they? | **Merged**: identity builds the gate and store in 0.8. Codes are single-use and unguessable, redemption is rate-limited, and your handle is allowlisted. A ban voids unused codes, and Golems earn none. The earning rule is yours; suggested: 5 per new User plus 2 per completed commission. | | identity, moderation, plugin, security |
| 67 | Does the alpha open only once a scanning vendor is live? | **Ruled (R-9):** apply now, as an individual. If refused, the alpha opens with uploads off until the company exists or another vendor accepts, and that fallback comes back to you. | | moderation |

### 0.9

| # | Question | Proposed answer | Door | From |
| --- | --- | --- | --- | --- |
| 69 | Does the alpha start on an empty database, from one baseline migration? | **Merged** (identity and architects agree): yes. Your local data stays on your machine, and table names stay as they are. | **Door** | identity, architects |
| 70 | What does "generally available" mean for v1? | The alpha and beta stay pre-GA. v1 becomes generally available at 1.0, when the 12-month notice window binds. The breaking check compares against the latest stage tag. | | architects |
| 71 | Fix the changelog's entry encoding before the alpha? | Yes: canonical (DAG-CBOR, like the Den's payload hash), with a version per entry. | **Door** | architects |
| 72 | Session lifetimes and the cookie name | `__Host-zurfur.sid`, with 7 days idle and a 30-day absolute cap. A fresh sign-in is required for key, handle, Account-delete, transfer and moderator acts. | | security |
| 73 | What does the security event log hold? | Sign-ins, sessions, key operations, handle changes and moderator acts, keyed by DID. It never holds tokens or cookies, and session ids only hashed. | | security |
| 74 | How long do deleted data live in backups? | 35 days, so each monthly restore drill always has a backup and an erased person's data leaves every backup within about five weeks. The Privacy Policy states it. This number is the consolidator's; compliance and counsel confirm it. | | security, compliance |
| 75 | Keep search engines out through the beta? | Yes: "noindex" until 1.0. Keeping them out can be undone; the door is letting them in, at 1.0. | | web |
| 76 | Error reports and visitor counts only through Zurfur's own address? | Yes. No third-party script, and reports are scrubbed of DIDs, handles and Den paths. | | web |
| 77 | EU and UK users in the closed alpha? | **Ruled (R-11):** admit them, with EU and UK representatives appointed before any EU or UK user joins. Consequences: questions 105 and 106. | | compliance |
| 78 | Which AI tools may touch real users' data? | **Ruled (R-12):** before the alpha, any agent that may read real users' data runs under commercial terms with a data processing agreement. Until then you, not the Orchestrator, read alpha bug reports. No agent ever reads the evidence store. | | compliance |
| 79 | When does the security contact go live? | GitHub's private reporting now, `security.txt` at 0.10, and the full policy at 1.0. | | security |

### Alpha (0.10 to 0.13)

| # | Question | Needed by | Proposed answer | Door | From |
| --- | --- | --- | --- | --- | --- |
| 80 | What is the Request for Approval? | 0.11 | The Creator's request sets "Waiting for Approval" and the Client's approval clears it. Both are changelog entries, and neither is a fact. Phases reuse it. | **Door** | commission |
| 81 | What does an invoice say? | 0.11 | Keep the name, and show "Issued by <artist>. Zurfur does not issue invoices or collect payment." | | compliance |
| 82 | What is an alpha Portfolio? | 0.11 | Hand-picked links under a new always-public `/portfolio`. This settles "Portfolio Live vs Static" as option 3 with 3A. | **Door** | posts |
| 83 | What shape is user-linking? | 0.12 | Both teams agree on two records, one in each person's repo, valid only while both exist. They differ on whose record type. (a) Identity: its own decision page and a Zurfur lexicon, independent of other projects. (b) Vulpes: Vulpes's generic paired relationship kind, reusing the pair check Zurfur runs for claims, but depending on Vulpes's namespace and on question 59's plain scopes. Proposed: (a), the owning lead's answer. | **Door** (lexicon) | identity, vulpes |
| 84 | Which namespace do Character handles use? | 0.13 | The shared, flat `*.zurfur.app`, for public Characters only. | **Door** | identity |
| 85 | External-storage mounts: alpha or beta? | 0.13 | **Merged**: beta (0.19), owned by den with plugin reviewing the seam. The earliest alpha fallback is 0.13: read-only, owner-only, one provider. | | den, architects, plugin, your note |
| 86 | Hold ACP's public announcement until real claims? | 0.10 | Yes. | | vulpes |
| 87 | Moderators beyond you: when? | 0.13 | One moderator through the alpha; reopened as the beta opens. | | moderation |
| 88 | A design-sync of the plugin pages before the study? | 0.13 | Yes. | | plugin |

### Beta (0.14 to 0.20)

| # | Question | Needed by | Proposed answer | Door | From |
| --- | --- | --- | --- | --- | --- |
| 89 | Which card is the beta "replies"? | 0.14 | **Merged**: the public reply post. Private pin replies are already in 0.3. | | commission, posts |
| 90 | Are posts always public? | 0.14 | Yes. | **Door** | posts |
| 91 | The post key | 0.14 | **Merged** (den wanted the full record address): keep `/post/<did>/<record key>`, with one top-level directory per published collection, so the directory names the collection. | **Door** | posts, den |
| 92 | The notifications that posting and replies promise | 0.14 | Ship the beta without them; they arrive at 1.0. | | posts |
| 93 | Membership-confers-view: what replaces it? | 0.18 | Each member gets a per-user grant at the level the owner picked, logged once; Golems get none. | | commission, plugin |
| 94 | The plugin study's questions: push or pull, the credential, who registers, which Portal, how types render, the install binding, Golem keys, a Golem repo, Golems earning codes or reputation, the Trello importer in the beta, and Golems in seats on Adult commissions | 0.13 to 0.16 | As plugin pre-answers them: pull only; one credential per Golem, exchanged for short-lived tokens; the Zurfur team registers Golems until 0.20; the declarative renderer; a template the plugin ships; per plugin and installer; one Zurfur-held key per Golem; no repo; Golems earn nothing; Trello in the beta, yes. The second authenticated surface, for peer views, is plugin's, after 1.2. New since QA: a Golem never passes an age check, so under question 101 it reads no Adult content, even in a Total seat. | **Door** (first Golem DID) | plugin, security |
| 95 | The five security rulings for mounts | 0.19 | As security pre-answers them: Private means "Zurfur won't show it"; named providers only; the narrowest scope; tokens under their own key, revoked on unmount; bytes proxied through Atta, never redirected. | **Door** (first stored token) | security, moderation |
| 96 | The outside security test | 0.20 | Last beta version, ASVS 5.0 level 2, retested before 1.0. Without a budget, an internal self-assessment plus your written risk acceptance. | | security |
| 97 | The Finder-style column view | 0.18 | Yes, as a view you choose. | | web |

### Release (1.0 to 1.3)

| # | Question | Needed by | Proposed answer | From |
| --- | --- | --- | --- | --- |
| 98 | A bug bounty at 1.0? | 1.0 | No; revisit after the disclosure policy has run. | security |
| 99 | The 10-minute dispute withdrawal vs the append-only changelog | 1.1 | The entry stays, marked as a quick withdrawal, and never becomes a warning. | commission |
| 100 | Do Vulpes's tags use Zurfur's stage names? | any | No; plain semver, with Zurfur pinning exact tags. | vulpes |

## Design-sync list

Contradictions the leads found. Each is fixed by one `/design-sync` once its question is answered. Nothing here was edited.

**Corpus against the protocol, or against itself:**
1. **Publish Consent & OAuth Scopes** (29687820) and **Characters on ATProto** (54427650): `app.zurfur.authPublish` carrying `repo:net.got-paws.acp.*`, and `repo:app.zurfur.*` itself, conflict with atproto's permission spec. After question 59.
2. **Account Deletion** (23003138) frees a hard-deleted Account's handle at once, while **Account Handle Change Flow** (27852802) keeps it reserved. After question 111.
3. **Commission** (3276807): a dispute withdrawn within 10 minutes "leaves no permanent changelog entry", while **The Changelog as the Only Record** (59310081) is append-only. After question 99.
4. **Project MVP** (589826) puts "Disputed" in the alpha lifecycle, while the Commission page leaves that move to disputes. **Owed now** (R-32).
5. **Project MVP**: the beta "replies" card is ambiguous, since **Comments & Markup** (65110018) already moved pin replies into the alpha. After question 89.
6. **Project MVP**: Sentry and Plausible in the alpha conflict with 0.1's "no request to another host". After question 76.
7. **User-Profiles, the Handle Swap & Content Maturity** (21594113), decision 10, and Project MVP's maturity line: "I'm 18+" self-declaration. **Owed now** (R-6).
8. **The Eventlog** (32178178) says "scrub content, retain the hash", while moderation's proposal deletes the entry. After question 41.
9. **Golem as User** (28049410) and **Golem** (12451841) give a Golem no repo, while **Plugin Security** (30572603) gives it one on Zurfur's PDS. After question 94.
10. **Plugin Security** binds an install "per plugin and Account", the Golem page "per Golem", and **The Den** (2026093001) leaves a User's install open. After question 94.
11. **Plugin** (3047451), **Golem** and **First-party plugins** (3244063) still place plugins in the MVP. Golem as User calls a Golem "Account-blind", while **Plugin Forms** (29458464) gives it Account context. After question 88.
12. **The Application Layer** (55836674) and **Domains and Applications** (11763713) cite "The API Contract, decision 11" for the leaf-crate move; that decision is about committed generated code, and the move is recorded nowhere.
13. **Inventory Closures** (30343170), item 6: a plain port with reference counting. **Owed now** (R-25), plus a decision page for the byte store.
14. **Post** (8978492): "Unlisted/Private as courtesies" closes as no. After question 90.
15. **Gallery Posts** (29949954), decision 4, and **The Replyable Trait** (30572573), decision 7, promise notifications the board places at release. After question 92.
16. **Comments & Markup**: who may delete a comment. Moderation's "remove a comment" amends it.
17. **Characters on ATProto** names Vulpes's `docs/characters-atproto.md` as its source of record, which is stale (Vulpes PR #30). The claim's field is `did` (R-28); layout D's slot name follows question 48.
18. **Project MVP**, "Account public presence": the PDS rotation key at rank 2. After question 45, through the pending Hosted Repos decision.
19. **Auth Surfaces & CSRF** (24543244): the origin rule. **Owed now** (R-20), closing the cross-site ticket.

**Rulings not yet in the corpus** (offer `/design-decision` or `/design-sync`):
- your moderation rulings of 2026-10-08 (Moderator Actions, take-down isn't delete, nobody edits the database by hand, lockdown);
- your deployment ruling, today's version numbering and the approved sequence (R-3, R-4, R-21);
- decision pages owed:
  - where a commission's content lives (R-15);
  - the byte store (R-25);
  - the owner key (R-29, R-30);
- The Den's open rows these close:
  - the node table (R-13);
  - typed write calls (R-14);
  - the MAC (R-33);
  - content to the web (R-34);
  - the grace window and quota (R-26);
- the security card's meanings (R-43), the moderation-hook rule (R-35), the threat model page (R-36), the owners (R-41) and Requests (R-42);
- the compliance rulings (R-5 to R-12): operating as an individual, the age check, 18 and over, the two content bans, the vendor fallback, the lawyer, representatives, AI tools;
- "Portfolio Live vs Static" (8978501), after question 82.

**Repository docs, not the corpus:**
- `backend/crates/domain/README.md` names `api` as the composition root;
- the versioning contract's example tag `api/v1.4.0`;
- moderation's 0.1 objectives quote the old "Gallery checks holds itself";
- the LICENSE files (R-27).

## Appendix A: card-to-version map

The **home** is the version where the card is complete. Earlier parts are listed in brackets.

**Alpha cards (35; all built by 0.12, the last two pieces of character-presence and login in 0.13):**

| Card | Home | Owner | Parts |
| --- | --- | --- | --- |
| login | 0.8 | identity, web | [0.1 hardened sign-in · 0.4 production client, on staging · 0.6 rate limits · 0.7 write scope, sign-out revokes tokens · 0.13 command line without a browser] |
| did-ops | 0.10 | identity | [0.2 outbox, mint ordering, handle change · 0.3 key list] |
| user-pages | 0.6 | identity, den, web | [0.11 ratings shown] |
| accounts | 0.4 | identity, web | [0.2 soft-delete gate, `Account.id` gone · 0.3 owner key at founding] |
| workflows | 0.6 | commission | |
| board-ordering | 0.6 | commission, web | |
| commission-core | 0.2 | commission, den | [0.3 files · 0.7 bringing a commission into an Account] |
| slots | 0.5 | commission | [0.2 slot state served] |
| seats-handshake | 0.4 | commission, den | |
| markup | 0.3 | commission | |
| changelog | 0.2 | commission | [0.3 "file added" by id] |
| surfaces | 0.7 | commission | |
| view-grants | 0.7 | commission, den | |
| client-progress | 0.7 | commission | |
| characters | 0.3 | identity | [0.5 the slot fact · 0.12 shared Characters] |
| claims | 0.7 | identity, vulpes | [live at 0.10] |
| tags | 0.12 | den (R-41) | |
| invoices | 0.11 | commission | |
| request-approval | 0.11 | commission | |
| gallery-view | 0.6 | posts | |
| frontend | 0.9 | web | [screens in every version · 0.2 design pass] |
| deployment | 0.9 | internal, architects, you | [0.3 hosting picked · 0.4 staging · 0.10 first production deploy] |
| security-asap | 0.9 | security, with owners per item | [0.2 to 0.9; directory-switch review at 0.10] |
| closed-door | 0.2 | den, commission | |
| fact-deletion | 0.5 | identity, commission | [0.2 Account and commission halves · 0.3 Character half, stubbed] |
| handle-change | 0.4 | identity | [0.2 on the outbox] |
| maturity-ui | 0.8 | web, den, posts | [0.2 picker · 0.3 on products · 0.6 label and blur, check stubbed] |
| character-presence | 0.7 | identity | [live at 0.10 · 0.13 Character handles] |
| account-pds | 0.5 | identity | [live at 0.10] |
| ask-for-art | 0.12 | commission | |
| blocks | 0.7 | moderation | |
| friendship | 0.12 | identity (R-41) | |
| reputation | 0.11 | commission (R-41) | |
| portfolios | 0.11 | posts, den | |
| user-linking | 0.12 | identity | |

**Beta cards (11):** gallery-posts 0.15 [0.14 standalone posts] · phases 0.14 · ych 0.16 · trees 0.17 · hash-chain 0.15 · commission-comments 0.14 · replies 0.16 · seals 0.18 (posts, R-41) · achievements 0.18 (posts, R-41) · plugin-api 0.19 [0.16 join and read · 0.17 act and append · 0.18 types] · kafka 0.19 (Internal, R-41).

**Release cards (10):** capacity-guards 1.0 · notifications 1.0 (posts, R-41) · openings-feed 1.0 (Lens engine: den, R-41) · disputes 1.0 (R-32) · collections 1.0 [1.1 dynamic] · telegram 1.1 · webhook-signing 1.1 · marketplace 1.2 · trello-import 1.2 (0.20 if pulled into the beta) · subscriptions 1.3.

**Unplaced:**
- gift, fan and surprise art, and a keeper's standing permission: after 1.1, because each gives up a keeper's protection;
- the second authenticated surface for peer views: after 1.2;
- Vulpes's private lane;
- the JavaScript and HTML Portal: parked.

**Proposals (not on the board), by version:**

| Version | Proposals |
| --- | --- |
| 0.2 | The outbox and worker; the custody port with one key per purpose; the node store; the threat model and secrets inventory; streamed Den reads; the Claude Design pass and loading states; the local directory in the dev stack; Vulpes as a pure library; legal footing (the operator named; counsel in 0.3) |
| 0.3 | The leaf contract crate; the hosting design and your pick; light theme by system setting; legal drafts, privacy records and the UK online-safety paperwork |
| 0.4 | Staging, with its wildcard certificate and a staging scan; Requests; folding the Accounts pages into the Den; a cookieless user-content domain and re-encoding; Moderator Actions, the audit log, the passkey console and database roles; the evidence store |
| 0.5 | The nightly export of hosted repos; a local PDS in the dev stack; Zurfur's PDS on staging |
| 0.6 | Lockdown, suspension and ban; avatars through Zurfur; stored viewer settings |
| 0.7 | Legal pages live |
| 0.8 | Invite codes; command-line sign-in through the browser; the age check and Terms acceptance |
| 0.9 | The migration baseline and gates; the lexicon gate; the changelog's canonical encoding; the incident runbook and tabletop; the security event log; `__Host-` sessions; noindex; reports through Zurfur's address; license notices; operator export and erasure; the mailer; the plugin door check |
| 0.10 | `security.txt`; invite codes obeying moderation; retention |
| 0.12 | "Your reports" |
| 0.13 | The plugin design study; Vulpes's watcher helper; the mounts groundwork |
| 0.14 to 0.20 | More moderators; self-serve export and deletion; external mounts; your own folders, dotfiles and themes; posts written elsewhere; more sign-in sources; the identity hardening tier; testers' Golems; appeals; the Finder-style column view; the outside security test |
| 1.0 and later | Credible exit; Den export; the disclosure policy; a developer kit; durable posts (a blob replica); Atta as its own library |

## Appendix B: sources

- **Team files**, in `.understand/releases/roadmap/teams/`:
  - `den.md` (Badger), `identity.md` (Fox), `commission.md` (Wolf), `web.md` (Fennec);
  - `posts.md` (Bowerbird), `moderation.md` (Meerkat), `plugin.md` (Octopus), `vulpes.md` (Reynard);
  - `architects.md` (Termite), `security.md` (Heron), `compliance.md` (Tortoise).
- **The frame:** `.understand/releases/roadmap/ASK.md`.
- **Inputs:** `.understand/releases/roadmap/inputs/release-board.md` (the scope of record), `engineer-notes.md` and `orchestrator-notes.md`.
- **Today's rulings:** `.understand/releases/roadmap/rulings-2026-10-10.md`, R-1 to R-50, all read before this revision.
- **QA:** Magpie's challenges, below, and her log in `.understand/retros/20261010-roadmap-qa.md`.
- **Stages:** `.understand/releases/stages.md`.
- **0.1:**
  - `.understand/releases/0.1/plan.typ` (signed), for what 0.1 holds and what may be carried to 0.2;
  - `rulings-2026-10-09.md` (D-6, D-15, D-19, D-26, D-36).
- **Internal's CI/CD study,** as ruled: `~/code/zurfur-workspaces/beaver/.understand/internal/ci-cd-study.md`.
- **The moderation study**, for its questions 16 and 17: `.understand/20261008-moderation-study.md`.
- **Corpus pages read for this file:** Project MVP (589826) and the Notifications decision (30605313). Every other page is cited as the teams cited it.
- **Security and protocol facts** (the reference PDS's rotation key, the KMS curves, the age-check laws) are each team's citations of sources they read on 2026-10-10; I didn't re-check them. Magpie re-checked the permission-set rule and the OAuth client-metadata rule today.

## QA challenges

2026-10-10 · Magpie (identity QA). The question put to every claim: how did you reach it, and shouldn't the Engineer (as PM) or the tech lead decide it? Line numbers below are this file's lines before this section was appended.

**Verdict: not ready for the Engineer until the 8 must-fixes close.** 35 challenges: **8 must-fix**, **15 should-fix**, **12 notes**.

**What holds:**
- Identity's file is carried faithfully, slot by slot, and every move is explained: Account repos before the User Page, the API's cross-site guard to 0.2, the moderator passkey to moderation.
- Every Release Board card has a home: 35 alpha, 11 beta, 10 release.
- The rulings on the node table, typed write calls, where a commission's content lives and keeping Zurfur's minter (R-13 to R-16) are marked Ruled in place. Operating as an individual, the vendor fallback, the lawyer at 0.3, the AI-tools rule and the EU and UK representatives before the alpha (R-5, R-9, R-10, R-11, R-12) are carried into compliance's and moderation's rows.
- Risk 3's protocol facts check out. I re-read atproto's permission spec today: partial wildcards like `app.zurfur.*` are refused, and a permission set can't reach a sibling namespace.
- The summary is one screen, in plain words.

**What doesn't hold:**
- The age-check ruling (R-6) is contradicted in 0.6, and its reach on the server isn't scheduled (M1, M2). Its vendor may refuse an individual, and nothing says what happens then (M3).
- Admitting EU and UK users (R-11) brings UK safety paperwork and a problem with erasing entries from the public DID log. Neither has a version (M4).
- Staging in 0.8 can't sign in through the production client while only your network can reach it (M5).
- The handle change, the one cross-store write running today with no retry, never moves onto the outbox (M6).
- 0.10 makes a PDS and `*.zurfur.app` public, but no version deploys them (M7).
- The CSRF and soft-delete ruling (R-20) is still listed as open, and the question counts are stale (M8).

**Spot-checked in depth:** identity and compliance. Moderation, den, architects and commission were checked against their rows; security was checked for KMS, CSRF and the soft-delete gate. Posts, plugin, vulpes and web were checked only where a finding touched them.

| # | Severity | Where | Challenge | Evidence | Suggested change |
|---|---|---|---|---|---|
| M1 | must-fix | 0.6 web row; question 55 | 0.6 builds "the blur with 'I'm 18+'", and question 55 proposes that a signed-out "I'm 18+" is remembered for the page. The age-check ruling retired self-declaration: signed-out visitors see a label with no image. Both lines come from web's file, written before that ruling. How did a retired mechanism survive the merge? | roadmap.md:176, :499; rulings-2026-10-10.md:12; teams/web.md:132, :358 | Build the ruled shape in 0.6: Adult work is a label with no image for anyone without a passed check, with the check stubbed as "not passed" until 0.8. Rewrite question 55 without "I'm 18+". |
| M2 | must-fix | 0.8; gaps, "Age checks"; 0.4 posts row; question 35 | The ruling says "everywhere", but only identity and web have 0.8 rows. The gaps table says "posts, den and commission apply them to views", with no version. Two consequences have no home. (a) Den's signed-out reads and the Gallery must withhold Adult images on the server, not only in the web. (b) The cookieless user-content domain (question 35, a door) can't tell whether a viewer passed, so a shared link to an Adult Public image skips the check. Den's per-file check matters only if a signed-in principal can lack a passed check: accounts made before 0.8, or Golems from 0.16. | roadmap.md:129, :215–216, :370, :469, :638; teams/compliance.md:92 ("including Adult-rated commission files"), :258; rulings:12–13 | Add posts and den rows to 0.8. Add an exit check: "signed out, an Adult Public product shows a label, and its image link refuses without a passed session." Ask posts and security to answer question 35 for Adult bytes. Proposed: a short-lived link issued only to a passed session, never a stable cookieless URL. Ask den and compliance whether Atta's file read checks the age result. Move maturity-ui's home to 0.8. |
| M3 | must-fix | Summary, risk 2; questions | Risk 2 says the age-check vendor may refuse an individual, but no question asks what happens then. The scanning vendor has a partial fallback (uploads off). This one doesn't, because the 18-and-over ruling is enforced by this check at first sign-in, so without a vendor nobody can be admitted as ruled. No question asks which vendor or what it costs either. Compliance already planned this application alongside the scanning vendor's. | roadmap.md:15, :108; rulings:11, :13, :15; teams/compliance.md:51 | Add a 0.3 question with a proposed answer: apply to an age-assurance vendor as an individual, alongside the scanning vendors. If every vendor refuses, either the alpha waits for the company, or you rule another way to enforce 18 and over in the alpha. Name it as a blocker of 0.10 in risk 2. |
| M4 | must-fix | R-11's consequences | Admitting EU and UK users is carried only as representatives at 0.9. Two consequences in compliance's file have no version. (a) The UK Online Safety Act paperwork: an illegal-content risk assessment, a children's access assessment, written records, and a named person responsible, who is you. (b) The public DID log is permanent. Compliance makes "what goes into a PLC operation" a door that closes at the first real submission, covered by the impact assessment. Identity puts answering erasure requests against that log at 1.0, but EU users' handles enter the log from 0.10. | teams/compliance.md:93, :108–112, :221; teams/identity.md:226; roadmap.md:238, :271, :348; rulings:17 | Add (a) to 0.3 (drafts) and 0.9 (signed, with you named). Add "what goes into a PLC operation" to 0.10's closing doors, with compliance's review as a precondition of the directory switch. Bring identity's answer on erasure against the log forward to 0.9. |
| M5 | must-fix | Summary; 0.8 architects row and exit check 1; question 68 | Staging is "network-restricted to you" and signs in "through the production client". atproto's OAuth spec, read today: "Clients must publish a 'client metadata' JSON file on the public web", and "Authorization Servers need to fetch client metadata documents from the public web". Bluesky's servers can't fetch metadata from an address that answers only your network, so exit check 1 fails as written. Question 68 credits moderation, but moderation proposed a different gate, a sign-in allowlist holding only your DID. The merge kept architects' network block and dropped moderation's allowlist without saying so. | roadmap.md:9, :214, :221, :522; teams/architects.md:72–73, :82; teams/moderation.md:352; atproto.com/specs/oauth | Ask architects and moderation. Proposed: publish the client metadata and its keys at a public address, and gate staging by DID (0.8's invite gate, or moderation's allowlist), instead of or beside the network block. Then rewrite question 68 and exit check 1. |
| M6 | must-fix | 0.2 identity and architects rows | Today the handle change sends the directory update, then writes PostgreSQL, with no record and no retry. Architects list it as one of the three writes the outbox must carry. The roadmap moves founding and the tombstone onto the outbox, but never the handle change, in any version. | `backend/crates/application/src/account/change_handle.rs:74–82`; teams/architects.md:26, :63, :79; roadmap.md:74, :77 | Add the handle change to 0.2's outbox work. Add architects' check to 0.2's exit check: "with the directory stopped, a handle change shows as pending; restart it, and it completes." |
| M7 | must-fix | 0.8 to 0.10 | 0.10 puts "the PDS ... public on its own domain" and checks `https://<handle>/.well-known/atproto-did`. No version deploys either one: the production PDS (its rotation key in KMS, its data and blobs in the backup set), the `*.zurfur.app` wildcard certificate with a narrowly scoped DNS token, or the relay-mesh switch that architects call a door. Also, 0.9's lexicon tooling mints Zurfur's lexicon DID and writes schema records "on Zurfur's PDS" before 0.10 turns on directory submission. Identity and security both named these. How did they drop? | roadmap.md:233, :257, :271; teams/architects.md:115, :161–165, :244; teams/identity.md:258; teams/security.md:152 | Add the production PDS, the wildcard certificate and the relay switch to 0.8 (staging) and 0.9 (production), under architects and internal. Name the relay switch in 0.10's doors. Either mint the lexicon DID at 0.10, after the switch, or say 0.9 builds only the tooling and its gate. |
| M8 | must-fix | Questions 4 and 5; appendix B; summary; 0.2 | The CSRF and soft-delete ruling (R-20) answers questions 4 and 5, but both are still listed open. Appendix B still cites "R-1 to R-16", and the counts are stale: 85 open and 21 for 0.2, not 87 and 23. The ruling also ships the soft-delete gate with the slice that makes Account facts real. In security's terms, that gate is the store refusing every write to a soft-deleted Account. 0.2 says only "the Account half of the soft-delete gate", which is identity's facts check, and the exit check covers neither. | roadmap.md:21, :77, :86–93, :428–429, :691; rulings:26; teams/security.md:104, :128; `backend/crates/application/src/account/facts/exist.rs:16` (always "no facts") | Mark questions 4 and 5 Ruled, cite R-1 to R-20, and recount. In 0.2, name both halves: the facts check, and the store refusing writes to a soft-deleted Account. Add security's test: accepting an invitation into a soft-deleted Account answers 404 and creates no membership. |
| S1 | should-fix | Conflict "Export"; 0.9 | Export and erasure as named operator actions are due "by 0.9", but no version gives them a team row. Compliance assigns them to identity, commission and den. With EU users from 0.10, the one-month legal clock runs from the first request. | roadmap.md:401, :676; teams/compliance.md:98, :250, :257 | Add them to the identity, commission and den rows of 0.9, and add a drill to 0.9's exit check. |
| S2 | should-fix | 0.8 | The KMS adapter is listed under both architects and identity. Which team builds it? | roadmap.md:214–215 | Name one builder; the other reviews. |
| S3 | should-fix | 0.2 | The frame sizes a version at "about the size of 0.1 or smaller". 0.2 holds the crate split plus three pull-forwards: the outbox (architects' third slot), payloads (den's third) and the API guard (identity's last). It also chains work inside one version: the split, then the custody port, then the MAC, then serving payloads; and the outbox, then identity's DID safety. Identity's findings aren't live while directory submission is off, and architects need the outbox only before the first PDS write (0.5). Was 0.2's size checked? | ASK.md:26; roadmap.md:70–84; teams/architects.md:58; inputs/orchestrator-notes.md:54 | Ask the tech lead whether the outbox and identity's DID-safety work move to 0.3 together. 0.2 would keep the split, Atta's writes, commission's lifecycle and the CSRF and soft-delete items. |
| S4 | should-fix | 0.2 den row; conflict "Payloads" | The payload pull-forward is written as landing, while the conflict row says "Den should confirm the size". The tamper check that proves verification stays in 0.3's exit check, although verification starts in 0.2. | roadmap.md:75, :117, :385; teams/den.md:47–48, :65 | Mark it "pending den's confirmation" until Badger answers, and move the tamper check to 0.2. |
| S5 | should-fix | 0.6 den row; conflict "Viewer settings"; question 55 | Web asked for the private settings file early, and den placed dotfiles in the beta, unless board columns need one sooner. Pulling the file to 0.6 makes dotfile names a stored-data door at 0.10, for a single preference. Since the age-check ruling, the gate that counts is the age result on the User (0.8), so only the blur preference would live in the file. Did den agree? | roadmap.md:171, :387, :499; teams/den.md:205, :270, :300; teams/web.md:54, :58 | Ask den. Proposed alternative: store the sensitivity beside the age result on the User in 0.8, and leave dotfiles in the beta. |
| S6 | should-fix | 0.8; conflict "KMS" | The case for staging at 0.8 rests on "after the hosting and continuous-deployment designs your CI/CD ruling places at alpha planning", which assumes the answer to question 66. Architects put staging early (their third slot) to avoid a big-bang deploy. 0.8 now holds staging, KMS, the CD design, the production client, invite codes, the age check, Terms acceptance, moderator sign-in states, the watcher and identity's full review. It is the heaviest version, and it sits right before the cut. | roadmap.md:214–218, :380, :519–520; teams/architects.md:57, :273; Internal's CI/CD study, Q10 (ci-cd-study.md:143–146) | Put architects' early-staging option beside question 2 or 66, with its cost: the hosting choice (question 65) moves earlier. |
| S7 | should-fix | Summary, risk 4; gaps | The "about 68 units" are identity's own file. The merge then added to identity: the API cross-site guard (0.2), finding a DID from a handle (0.4), reading Bluesky blocks and a Public Account (0.7), the age check and Terms acceptance (0.8), and "Commissions Open" plus the unclaimed friendship card (0.12). The summary names identity the critical path and then loads it further. Who checked identity's capacity? | roadmap.md:20, :77, :127, :195, :215, :297, :364, :370; teams/identity.md:18–138 | Re-size identity's rows with Fox, or give the age-check integration to a less-loaded team. Then restate risk 4. |
| S8 | should-fix | 0.2 exit check | Two of identity's 0.2 checks were dropped. (1) Founding with a deactivated Account's handle is refused and mints nothing; this bug needs no race. (2) A tombstone lands after the directory restarts. Today a failed tombstone is logged and dropped. | teams/identity.md:27, :36, :38; `backend/crates/application/src/account/delete.rs:67–73`; roadmap.md:86–93 | Add both to 0.2's exit check. |
| S9 | should-fix | 0.6, 0.7; gaps "Mailer" | Three moderation dependencies slipped. (a) Identity's "first-sign-in gate", which lockdown's sign-ups switch needs, is missing from 0.6. (b) "The hide hold on every public view" sits in 0.7, though public views ship in 0.6 and this roadmap's own rule puts each hook in its feature's definition of done. (c) The mailer has an owner but no version, while moderation's notices to owners start at 0.6. | teams/moderation.md:111, :305, :321; roadmap.md:57, :172–173, :196, :372 | Add (a) to identity's 0.6 row. Move (b) into den's and posts' 0.6 work. Place (c) in 0.6, or say that notices stay in-app until it lands. |
| S10 | should-fix | Question 83 | "Merged (both teams agree ...)" overstates the agreement. Both agree on two records, one in each repo, but not on whose lexicon. Identity proposed its own decision page and lexicon; Vulpes proposed a generic Vulpes kind. Picking Vulpes's makes user-linking depend on another project's namespace and on question 59's plain scopes. | teams/identity.md:320–325; teams/vulpes.md:154; roadmap.md:547 | Show both as options, with the trade-off, and let the Engineer pick. |
| S11 | should-fix | Design-sync item 2; questions 63 and 74 | Whether a hard-deleted Account's handle is freed at once or stays reserved is "ruled inside 0.4", with no numbered question and no proposed answer. Identity asks it inside the slice. Question 63 ("Through counsel") and question 74 ("compliance sets the number") carry no answer either. | roadmap.md:513, :533, :582; teams/identity.md:68 | Add a 0.4 question with a proposed answer, and pre-fill questions 63 and 74. |
| S12 | should-fix | Door marks and door lists | Question 65 (hosting) isn't marked as a door, though Internal's study calls the deploy target and region one-way. 0.8's door list leaves out the deploy target and region, and compliance's door for the stored age result and Terms version. 0.10's list leaves out the relay-mesh switch and "what goes into a PLC operation" (see M4 and M7). | roadmap.md:227, :271, :519; ci-cd-study.md:146; teams/compliance.md:223 | Mark question 65 as a door, and add the missing doors to both lists. |
| S13 | should-fix | Question 19 | The row says the ruling on operating as an individual (R-5) answers "Where will the operator be based, and a company before the alpha?". It answered only the company part. Compliance says many duties turn on the country, including whether EU or UK representatives are needed at all. | roadmap.md:448; rulings:11; teams/compliance.md:10, :220 | Split the question, and ask the Engineer to state the operator's country. It is probably a fact, not a decision. |
| S14 | should-fix | Gaps, "The moderator passkey"; 0.4 | Moderation asked identity to build the passkey step-up. Identity had it in its last slot. The merge gives it to moderation, with identity reviewing. Sign-in strength for the most powerful account is authentication work. Did Meerkat and Fox agree to the move? | teams/moderation.md:79, :302; teams/identity.md:137, :331; roadmap.md:128, :369 | Confirm the owner with both leads. Either way, it gets a security review. |
| S15 | should-fix | 0.3 identity row | "Characters ... deleted by facts" and "the Character half of the soft-delete gate" land in 0.3. A Character's facts come from slots (keeper consent, a slot in a live commission), which land in 0.5. Until then the check can only answer "no facts", and identity's own check, "one in an active slot can only be tombstoned", can't run. | roadmap.md:105, :148; teams/identity.md:47–48, :57; question 47 | Say that 0.3's facts check is a stub, and land each Character fact kind with the slice that makes it real (slots, 0.5), as the CSRF and soft-delete ruling does for Accounts. |
| N1 | note | Age-check ruling and Brazil | Compliance cites Brazil's rule that adult content needs a check "at every access". The 18-and-over ruling checks once, at first sign-in. Does one check per account satisfy it, if Brazilian users are admitted? A question for counsel. | teams/compliance.md:190 (Brazil, source S27); rulings:13 | Add it to counsel's fixed-scope review. |
| N2 | note | 18 and over, with signed-out reads | Compliance says the 18-and-over rule takes the UK children's duties off the table, if an age check makes the service adults-only. From 0.6, signed-out visitors read public pages without any check. Does the relief still hold? Unverified; a question for compliance and counsel. | teams/compliance.md:93, :322; roadmap.md:171 | Ask compliance. If the relief doesn't hold, M4's children's assessments become real work. |
| N3 | note | 0.2 compliance row | Operating as an individual puts your name and a postal address on the public legal pages of an adult-art platform. Could a registered-agent or mail-forwarding address serve instead of your home? The DMCA filing needs a street address. Unverified; ask counsel. | roadmap.md:81; teams/compliance.md:87, :419 | Add it to counsel's review. |
| N4 | note | Summary | "0.9 builds production, with a backup restore you have watched." 0.9's exit check restores staging's backup. The production restore drill is in 0.10. | roadmap.md:9, :243, :256 | Say "a restore of staging's backup". |
| N5 | note | Question 17 | The roadmap states the moderation-hook rule as settled for every version, but question 17 still asks it. | roadmap.md:57, :446 | Mark the rule "if you agree with question 17". |
| N6 | note | 0.7 commission row | Commission's "an Account's bookmarks of outside commissions" was dropped without a note. | teams/commission.md:42; roadmap.md:193 | Place it, or say why it was cut. |
| N7 | note | 0.10 moderation row | The fallback "the alpha opens with uploads off" doesn't say what goes dark: Account avatars (0.5), products and attachments, the Gallery and Portfolios. | roadmap.md:259; rulings:15 | List them, so the fallback is a decision with its cost in view. |
| N8 | note | Question 75 | "Keep search engines out" is marked as a door. Keeping them out can be undone; letting them in is the step that can't. | roadmap.md:534 | Move the door mark to 1.0's "search engines let in". |
| N9 | note | 0.6 identity row | The User Page lists only the memberships you chose to list (the "list on profile?" choice), and that choice was dropped. | teams/identity.md:81, :91 | Restore it to 0.6 with its check. |
| N10 | note | Gaps, "Notifications" | Posts as the owner rests on the second intake, the ingestion of posts written in other apps, which is a 0.19 proposal. The first intake is commission's domain events (0.17). | roadmap.md:360 | Rest the case on something already placed, or say it's weak. |
| N11 | note | 0.2 commission row | The decision page on where a commission's brief, budget and seats live is still written as "on paper (it blocks 0.4)". It is ruled (R-15), so all that's left is recording it. | roadmap.md:76, :443 | Say "recorded as a decision page". |
| N12 | note | 0.16 | Golems are Users and never pass an age check. A Golem in a Total seat on an Adult commission reads Adult files. That is a beta question under the age-check ruling. | roadmap.md:338; rulings:12 | Add it to plugin's study questions (question 94). |

**For Kestrel:** resolve each row with a short answer (fixed, declined with why, or put to the Engineer), as the 0.1 QA did. M1 to M5 and S5, S6 and S10 need a lead's or the Engineer's answer, not just an edit.

## QA resolutions

2026-10-10 · Kestrel. One line per challenge. "Put to the Engineer" means the row became a question in "New from QA, for the Engineer", carrying the owning lead's answer. Every edit is in the body above.

| # | Resolution |
| --- | --- |
| M1 | **Fixed.** 0.6 builds the ruled shape (a label with no image, the check stubbed "not passed" until 0.8) in the den, posts and web rows and exit check 5. Question 55 is rewritten without "I'm 18+". The summary and a new conflict row say the same. |
| M2 | **Put to the Engineer:** question 101 (the server-side rule, 0.6 stub, 0.8 real) and question 102 (Adult bytes on the cookieless domain, 0.4, a door). **Also fixed:** den and posts rows in 0.8; 0.8 exit check 4; maturity-ui's home moved to 0.8; 0.4's posts row and exit check 6 keep Adult bytes off that domain until 0.8. |
| M3 | **Put to the Engineer:** question 103 (the fallback; 0.3; blocks 0.10) and question 104 (which vendor, at what cost; 0.6). Risk 2 names it as a blocker of 0.10. |
| M4 | **Put to the Engineer:** question 105 (the UK paperwork with you named; drafts 0.3, signed 0.9) and question 106 (what goes into a directory operation, and erasure against the log; 0.9; a door). **Also fixed:** compliance's 0.3 and 0.9 rows; the door lists for 0.9 and 0.10; compliance's review as a precondition of the switch in identity's 0.10 row. |
| M5 | **Put to the Engineer:** question 107 (publish only the metadata and keys; keep the rest restricted; moderation's DID allowlist as a second lock; 0.8; a door). Question 68 and 0.8's exit check 1 now point to it. |
| M6 | **Fixed.** The handle change moves onto the outbox in 0.2 (architects and identity rows). Architects' check is 0.2 exit check 6: a handle change shows pending while the directory is down and completes after the restart. |
| M7 | **Fixed.** Zurfur's PDS and the wildcard certificate (scoped DNS token, or a separate host) go on staging in 0.8 and into production in 0.9. The relay switch is at 0.10, after take-down reaches the hosted PDS, and is named in 0.10's doors. 0.9 builds only the lexicon tooling and its gate; Zurfur's lexicon DID is minted at 0.10, after the directory switch. A new conflict row records the order. |
| M8 | **Fixed.** I read R-17 to R-44, and every answered question is marked Ruled (R-1 and R-17 to R-19 concern the build and the board). Counts are redone: 76 open, none for 0.2. Appendix B cites R-1 to R-44. 0.2 names both halves of the soft-delete gate (the facts check, and the store refusing writes), and security's 404 test is 0.2 exit check 7. |
| S1 | **Fixed.** 0.9 gains an identity, commission and den row for named export and erasure actions, with a drill as exit check 5. The export conflict row is updated. |
| S2 | **Fixed.** Identity builds the KMS adapter and architects review it (0.8 rows, the gaps table, the KMS conflict row). |
| S3 | **Fixed in part.** 0.2 gains a cut order: den's payloads go first (R-44), then identity's directory-safety slices. **Declined:** moving the outbox to 0.3, because you ruled it into 0.2 (R-24). |
| S4 | **Fixed.** The payload pull-forward reads "accepted (R-44); den confirms the size; overflow moves to 0.3 and you're told". The tamper check moved to 0.2 exit check 4. |
| S5 | **Put to the Engineer:** question 109 (the settings file pulled forward, or a record beside the User), noting that den hasn't confirmed. R-39 already settled that stored settings land in 0.6. |
| S6 | **Put to the Engineer:** question 108 (staging at 0.4, as architects proposed), with its cost and the rulings it reopens (R-21, R-2). Question 66 notes the alternative. |
| S7 | **Put to the Engineer:** question 110 (moderation's programmers build the age-check vendor integration). Risk 4 now lists what the merge added to identity, and says Fox re-sizes those rows. |
| S8 | **Fixed.** 0.2 exit checks 5 and 6 cover the refused founding with a deactivated Account's handle, and the tombstone landing after a restart. |
| S9 | **Fixed.**<br>(a) The first-sign-in gate is in identity's 0.6 row.<br>(b) The hide hold is filtered on every public view from 0.6, in den's and posts' rows. It is gone from 0.7, and a conflict row records the move.<br>(c) The mailer is in Internal's 0.9 row, and notices stay inside Zurfur until then. |
| S10 | **Fixed.** Question 83 shows both options with the trade-off, pre-filled with identity's answer (the owning lead's). |
| S11 | **Fixed.** Question 111 covers a hard-deleted handle (0.4, a door), with a proposed answer marked as the consolidator's, since identity left it open. Questions 63 and 74 are pre-filled; 74's number is marked as mine, for compliance and counsel to confirm. Design-sync item 2 now points to question 111. |
| S12 | **Fixed.** Question 65 is marked a door (deploy target and region). 0.8's doors add the deploy target and region, plus the stored age result and Terms version. 0.10's doors add the relay switch and what goes into a directory operation. |
| S13 | **Fixed.** Question 19 now covers only the company, which is ruled. The operator's country is question 112, flagged as probably a fact. |
| S14 | **Put to the tech lead,** not the Engineer. The passkey reads "pending Meerkat's and Fox's agreement; security-reviewed either way" in 0.4, the gaps table and the conflict row. |
| S15 | **Fixed.** 0.3's Character facts check is a stub, and the slot fact lands with slots in 0.5. This is in commission's and identity's rows, 0.5 exit check 2 and a conflict row, and fact-deletion's home moves to 0.5. |
| N1 | **Fixed.** Brazil's "at every access" rule is added to counsel's scope in 0.3. |
| N2 | **Put to the Engineer:** question 113 (counsel answers in 0.3; meanwhile the children's assessment is planned for 0.9). |
| N3 | **Fixed.** A registered-agent or mail-forwarding address is added to counsel's scope in 0.3. |
| N4 | **Fixed.** The summary says 0.9 builds production "after you watch staging's backup restore". |
| N5 | **Fixed.** The moderation-hook rule is ruled (R-35), cited where it's stated, and question 17 is marked Ruled. |
| N6 | **Fixed.** An Account's bookmarks of outside commissions are placed in 0.7's commission row. |
| N7 | **Fixed.** 0.10's moderation row lists what goes dark under the uploads-off fallback. |
| N8 | **Fixed.** The door mark moved from question 75 to 1.0's "search engines let in". |
| N9 | **Fixed.** The "list on profile?" choice is restored to identity's 0.6 row and exit check 1. |
| N10 | **Fixed.** The notifications owner is ruled (R-41). The gaps row names commission's domain events (0.17) as the first intake, and posts' ingestion as a 0.19 proposal. |
| N11 | **Fixed.** 0.2 says the decision page records the ruling (R-15). |
| N12 | **Fixed.** Golems and Adult files are added to question 94. |

**Changed beyond QA, from the rulings:**
- ~~**Disputes move from 1.1 to 1.0.**~~ Superseded: R-48 put disputes back in 1.1, after notifications in 1.0.
- **The R-41 owners and the name "Requests" (R-42)** are written in wherever "proposed" stood.

**Final pass, from the rulings:** applied R-45 to R-50.
- Questions 103, 105, 108, 112 and 113 are marked Ruled, and their consequences are carried: R-45's fallback in 0.10, R-46's named person in 0.3 and 0.9, and R-50's children's risk assessment in 0.9.
- Staging moved to 0.4 (R-47), re-flowing 0.3, 0.4, 0.5, 0.7, 0.8 and 0.9. The 0.4 section has a table of what moved and its size, and four new conflict rows record what the move created.
- Disputes went back to 1.1 (R-48).
