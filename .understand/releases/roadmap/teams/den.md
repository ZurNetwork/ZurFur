# Roadmap: den

Lead: Badger · QA: Owl · Stages covered: pre-alpha (from P2), alpha, beta, release

The Den isn't a card on the Release Board. It is the tree every other card's data lives in, and Atta is its engine. So each den slot names the cards it carries. The work that has no card is marked as a proposal.

## In one screen

- **P1 (0.1, fixed):** My Den is read-only, with names and levels and no payloads. den.proto enters the contract.
- **Pre-alpha:** the Den becomes writable through Atta (P2); each node shows its content, checked against its hash (P3); boards, grants, Accounts and characters appear as links and mounts (P4); the Den is safe to show to strangers (P5).
- **When alpha opens:** nothing reads or writes a node except through Atta. Moderation's holds and lockdown sit in the resolver.
- **den.proto:** it gains payloads (P3) and links (P4), you review the whole file at P5, and it closes at the first alpha tag. After that it only grows.
- **Alpha:** tags become node attributes (A1); tombstoned characters and public records go in their places (A2).
- **Beta:** plugin types (B1); external-storage mounts, your note 2 (B2, proposed); your own folders and dotfiles (B3); a bigger group's load (B4).
- **Release:** ready for real work (R1); collections and the openings feed (R2); subscription tiers (R3); Atta on its own (R4, proposal).
- **For you now:** questions 2 to 4 (the node store, where writes sit on the wire, what the private-data MAC covers). P2 waits on them.

## Pre-alpha

- **P2: You change your Den through Atta.**
  - *What it builds.* Atta's write side:
    - Every change is a named action that passes Atta's generic checks, then reaches its type's handler through the host's exhaustive match. The checks:
      - a new node starts Private unless its creator picks a wider level;
      - a node is never wider than its parent;
      - "make public" widens whatever ancestors it needs, in one act;
      - the depth cap;
      - no generic create, move, copy, edit or delete.
    - The node store is settled, as you rule (question 2).
    - Each write stores its payload's hash and the private-data MAC (the keyed check The Den requires on every private payload) in the same unit of work. They are stored now and served from P3.
    - Today's metadata writes move behind the gate: found and delete an Account; create, retitle and archive a commission; declare a slot.
    - A node over held bytes is the one not-found in paths and listings, through the host check that 0.1 builds. A moderator's principal is let through.
    - Uploads keep their current path until P3.
  - *Features:* `commission-core`, `closed-door`, `slots`, `accounts`, `fact-deletion` (archive through the Den), `security-asap` (holds).
  - *Depends on:*
    - commission and identity: they hand their write use cases to the gate (question 3);
    - identity: the MAC's key, under the same custody as the did:plc keys;
    - web: actions in the Den view;
    - moderation: the hold, which commission's arms read;
    - tech lead: a `just gate` test that no route writes a node except through the gate.
  - *Done when,* in My Den on `just dev`:
    1. Create a commission. It shows Private.
    2. Rename it, make it Public, then Private again, and archive it. Each change shows at once, and in the changelog wherever commission logs that change.
    3. Found an Account and delete it.
    4. A planted route that writes a commission row without the gate turns `just gate` red.
- **P3: Your Den shows what it holds, and every byte is checked.**
  - *What it builds.*
    - The first payloads are served: a commission's own fields, a slot's state and character, a file's bytes, and the changelog a page at a time.
    - Atta serves a payload only after its hash and MAC verify. A mismatch fails closed and is reported (question 4).
    - Uploads move behind the gate:
      - a file's hash is its raw-bytes content id;
      - the file goes through moderation's scan gate, and becomes a node only once clear;
      - an upload quota applies (question 7).
    - den.proto grows, additively: the hash on the open part (openers only), a file's content, and each core type's payload.
  - *Features:* `security-asap` (the gated store: no byte is served except through Atta, after it verifies), `changelog`, `slots`, `commission-core`, `markup` (the file content that pins sit on). Proposal: the upload quota.
  - *Depends on:*
    - commission: each type's payload message; the file store computing the content id; the changelog served as the content of its `changelog` file; where the brief and surfaces go;
    - moderation: the scan gate;
    - web: rendering each core type and images;
    - identity: custody of the MAC's key, as in P2.
  - *Done when:*
    1. Open "Untitled" and see its deadline, status and maturity.
    2. Open an attachment and see the image.
    3. Page through the changelog.
    4. Upload a file in the Den. It shows "processing", then appears.
    5. Change one payload byte by hand in `just db-shell`. That node isn't served, and the log reports the mismatch.
- **P4: Links and mounts beyond your own.**
  - *What it builds.*
    - Symlinks in Atta:
      - a link shows at the lowest of its own level, its folder's and its target's, so a link to a Private target is absent;
      - opening a link lands on the target's real path. That brings the read by real path onto the wire, with its own security review;
      - a link to an archived node gets its rule (question 5).
    - Mounts from more than your own standing:
      - a view grant: Presentation gives a card, Description opens the node, Total opens its subtree;
      - a commission brought into an Account (the MVP-only exception);
      - an Account's `commissions` and `characters`;
      - a keeper's characters, once identity stores them.
    - A board's columns are nodes, and its placements are symlinks in them.
    - Seat ceilings take their place in the formula (question 6).
  - *Features:* `workflows`, `board-ordering`, `view-grants`, `characters`, `seats-handshake` (ceilings), `commission-core` (bringing a commission into an Account).
  - *Depends on:*
    - commission: columns, placements, grants, the bring-into-Account record;
    - identity: Character storage in PostgreSQL, replacing the keeper read's empty answer;
    - web: the link landing;
    - security: Heron's review of the read by real path.
  - *Done when,* on the dev profile:
    1. As alice, place "Untitled" on Supreme Arts's board and see it in its column.
    2. Give bob a Presentation grant. As bob, see a card.
    3. Raise the grant to Description. bob opens the node and nothing below it.
    4. Revoke it. The mount leaves bob's `commissions` without a word.
    5. A link to a Private commission is absent for bob.
- **P5: Your Den is safe to show to others.**
  - *What it builds.*
    - The first reads by viewers with no relationship to a node, signed-out viewers included:
      - Listed cards and Public nodes, through the read by real path;
      - the derived views (a public User-Profile, the Gallery, Portfolios), which resolve each item through Atta at the visitor's level.
    - Built first, before any of those reads:
      - lockdown's "public pages off" gate in the resolver;
      - folders bounded, and keyset page tokens;
      - the same database statements for every pair of not-found causes on public paths;
      - the maturity a card needs.
    - Profile records under `/profile` are served after their signed-commit check.
    - den.proto reaches its alpha shape. You review the whole file, and the type names are settled or accepted (question 8).
  - *Features:* `user-pages`, `gallery-view`, `portfolios`, `client-progress`, `maturity-ui`, `character-presence`, `security-asap` (the public read gate, timing).
  - *Depends on:*
    - moderation: the lockdown switch. Its own lockdown check needs these signed-out reads to exist;
    - identity: `/profile` and the signed-commit check, through the hardened resolver;
    - posts: the Gallery over `/post`, and where a Portfolio lives;
    - web: the public pages;
    - security: Heron reviews the Den as strangers are served it.
  - *Done when:*
    1. Signed out, open alice's profile. Only what's Public shows.
    2. As a moderator, hold a Public commission. It vanishes for everyone else.
    3. Turn "public pages off" on. Public reads stop, and your own Den still works.
    4. Add entries to a folder while paging through it past the bound. No entry repeats.

**Exit (before alpha opens):**
- Nothing reads or writes a node except through Atta, and a test proves each.
- Every payload served verifies.
- The node store, the hash and the MAC's scope are ruled and built.
- den.proto is at its alpha shape and reviewed by you.
- Holds and the public read gate sit in the resolver.
- Heron's review of the whole Den path lists no open must-fix.
- Every Atta rule maps to a named test.

## Alpha

- **A1: Tags and status tags as node attributes.**
  - *What it builds.*
    - Atta's attributes (The Den, decision 26):
      - one row per attribute, in the `community.`, `owner.` and `system.` namespaces;
      - written only through the subject type's named action;
      - read only through Atta, so a hidden node's tags are hidden too.
    - Queries over a derived index pass each result through Atta. They filter, count and sort only by what the viewer can see; the ask feed is the first.
    - den.proto adds a node's visible attributes. The change is additive.
  - *Features:* `tags`, `ask-for-art`.
  - *Depends on:*
    - tech lead: who owns Tags (the moderation study asked: den or posts);
    - commission: asks as status tags;
    - web.
  - *Done when:*
    1. Tag your commission, and set "Commissions Open" on your Account.
    2. bob sees the tag only where he can see the node.
    3. The ask feed lists only what bob may see, and no count includes the rest.
- **A2: Tombstones and public records in their places.**
  - *What it builds.*
    - A tombstoned character shows only its name, under "show archived and deactivated". That is a new access case in den.proto: additive, and older clients show a card.
    - Claims, Friendship, Blocks and the Account repo's records each get their place among the always-public directories, and are served after their signed-commit check.
  - *Features:* `fact-deletion`, `claims`, `friendship`, `blocks`, `account-pds`.
  - *Depends on:* identity: Character delete; the records; where they sit; what the Hosted Repos decision puts in an Account's repo.
  - *Done when:*
    1. Delete a character that has facts. It leaves your `characters`.
    2. Toggle "show archived and deactivated". Only its name shows.
    3. A friendship record opens from a profile only while Public.

**Exit:**
- Every alpha card whose data lives in the tree has its nodes.
- Since the first alpha tag, den.proto and the node store have only grown.
- No real person's data was ever served around Atta.

## Beta

- **B1: Plugin types in the Den.**
  - *What it builds.*
    - The `Plugin` arm of the type match, with a registry scoped to plugins only (My Den, open row).
    - A plugin renders its type as a declarative view, and Atta renders that view.
    - A Golem reads the Den as itself. Its scope refuses nodes through the host check, as the one not-found.
    - den.proto is served on the plugin surface. The versioning rules' tolerant-reader clause then has to name that surface.
  - *Features:* `plugin-api`.
  - *Depends on:* plugin (the forms, scopes and credential); web (rendering declarative views); identity (Golems as Users).
  - *Done when:*
    1. A first-party test type installed on your Account renders in your Den.
    2. A Golem scoped to one commission sees only that commission. Every other path is "Nothing here".
- **B2: External-storage mounts (your note 2).**
  - *What it builds.*
    - A system folder in your root, and in an Account's, holds mounts you make to storage Zurfur doesn't control.
    - First-party adapters sit behind one port, with one provider first and read-only first.
    - Every byte is served through Atta, after it verifies against the hash taken when it was indexed.
    - Openers see an honest "unreachable" state.
    - Fetches go only through the guarded client, and the provider's token sits under custody.
  - *Features:* proposal.
  - *Depends on:*
    - identity: the guarded HTTP client; custody;
    - moderation: the scan gate on every byte Zurfur serves;
    - compliance: holding third-party tokens, and serving their bytes;
    - you: an amendment to the boundary contract's class test, since this is a third destination.
  - *Done when:*
    1. Mount a folder. Its files show in your Den, Private.
    2. Open one.
    3. Edit it at the provider. Zurfur reports it changed, and doesn't serve it.
    4. Take the provider offline. Only you see "unreachable" on that mount.
  - *Must not be closed off before it.* Each line names the slot that could close it:
    - **P2, P3 (the hash):** a hash is a content id over the bytes or the canonical payload, never a key into Zurfur's own store. Then fetched bytes verify the same way, and moderation's holds by hash reach them.
    - **P4 (mounts):** Atta takes the viewer's mounts as a list from the host and never assumes each comes from an access row. Then an owner-made, stored mount can join as a second source.
    - **P4, P5 (the top level):** the top-level directory set stays host-supplied (it already grew by `/profile`). Then a system directory for external stores can join, and the mount guard still holds.
    - **P5 (words):** the web's text for Private promises only "Zurfur won't show it". No level can be enforced on storage we don't control.
    - **The first alpha tag (den.proto):** content stays optional, and an unknown content case reads "content not shown yet" (already in revision 2.1). Then "unreachable" can be added. A viewer who can't open the node still gets the one not-found.
    - **Before alpha (security-asap's custody and KMS):** custody is built for any secret, not just signing keys.
    - **Always:**
      - the guarded HTTP client stays shared, so no second fetcher is written;
      - the scan gate takes bytes from any source, not only uploads;
      - the storage-routing crate (your note 1), if built, leaves room for a third destination.
- **B3: Your own folders, dotfiles and theming.**
  - *What it builds.*
    - User-made folders in your root and in an Account's. The rule that holds them to system folders until after the MVP is relaxed, under the depth cap.
    - Dotfiles: "Upload configuration" (through the scan gate), `showHidden` (additive), the cascade along the real path, `/.theme.yaml`, your private `.preferences.yaml`, and a board column's dotfile.
    - The MVP-only exception that lets Account membership confer view is removed, as ruled.
  - *Features:* proposal. The exception's removal is ruled (My Den, decision 8: "to be removed after the MVP").
  - *Depends on:* web (a form over a dotfile; the theme); commission (gift commissions get per-User view back; the columns' dotfile); moderation (the scan gate).
  - *Done when:*
    1. Make a folder and move a file into it, by its named action.
    2. Upload a `.theme.yaml`. A visitor sees your space restyled, and your large-text preference stays yours.
- **B4: The Den at a bigger group's scale.**
  - *What it builds.*
    - Cached access decisions, dropped the moment an ancestor narrows (The Den, decision 27).
    - Resolve cost at depth, measured against the recorded fallback where each node answers alone.
    - The changelog's hash becomes the head of its hash chain.
    - `/post` nodes mirror Gallery Post records, with the post key carrying its collection.
  - *Features:* `hash-chain` and `gallery-posts` (den's part); `trees` (none, if child commissions relate through their payload; otherwise P4's links cover it).
  - *Depends on:* commission (the chain); posts (the records, and the key before the first stored post).
  - *Done when:*
    1. Narrow a folder. A viewer loses it on the next read, cache or not.
    2. A deep path stays within the time budget the tech lead sets.

**Exit:** the Den takes code it didn't ship (plugin types, external stores) without an Atta rule bending, and it holds up under the bigger group's load.

## Release

- **R1 (release-v1.0.0): The Den for real work.**
  - *What it builds:*
    - a full security re-review of Atta;
    - a rotation drill for the MAC's key, where every node still verifies after the key changes;
    - equal not-found timing re-measured at scale;
    - proposal: export your Den to a peer view, attributes included.
  - *Features:* proposal (the export); `security-asap`, carried on.
  - *Done when:*
    1. Rotate the key. Nothing fails to verify.
    2. Export your Den and open it as plain files.
- **R2: Collections and the openings feed through Atta.**
  - *What it builds.*
    - A static collection is a folder of ordered links.
    - Dynamic collections and the openings feed are queries over a derived index, with every result passed through Atta.
    - The proposed guard on aggregate counts gets its ruling here.
  - *Features:* `collections`, `openings-feed`.
  - *Depends on:* the Lens engine's owner; moderation (hidden items filtered).
  - *Done when:* the openings feed never lists, ranks or counts a commission you can't see.
- **R3: Subscription tiers as a grant.**
  - *What it builds:* a tier lifts a patron's level the way a view grant does. A mediator's access in a dispute could use the same mechanism, if commission designs it that way.
  - *Features:* `subscriptions`.
  - *Done when:* a patron sees tier content, and a lapsed patron gets "Nothing here".
- **R4: Atta on its own.**
  - *What it builds:* Atta is split into its own library, and built for the browser.
  - *Features:* proposal (The Den rules both directions for after the MVP, with no slot).
  - *Done when:* Atta builds outside the workspace and for the browser, with its tests unchanged.

## One-way doors, and the slot that closes each

| Door | Kind | Settled in | Closes at |
| --- | --- | --- | --- |
| den.proto's spellings: fields, rpcs, query parameters | Published contract | P1, grown in P3 to P5 | The first alpha tag. Additive only after. |
| The type names on the wire | Published contract | P5 (question 8) | The first alpha tag |
| The read by real path and the link landing | Contract and security boundary | P4 | The first alpha tag |
| Where node metadata lives (question 2) | Stored data | P2 | The alpha (real people's data) |
| What a hash is: a content id over the canonical payload, or over the raw bytes for files | Stored data | P2, P3 | The alpha |
| What the private-data MAC covers, and its key's custody (question 4) | Security boundary | P2 | The alpha. The key id lets the key rotate, not the scope. |
| One write path and one read path, through Atta | Security boundary | P2, P3 | The alpha |
| Holds and the public read gate in the resolver | Security boundary | P2, P5 | The first build anyone else can reach (your ruling on moderation's first deadline) |
| Attribute namespaces as stored names | Stored data | A1 | A1 |
| The post key, which lacks its collection | Stored data | posts | The first stored post (B4, or sooner if the alpha stores posts) |
| Holding third-party storage tokens | Security boundary | B2 | The first token stored |
| Dotfile names and their vocabulary | Stored data and contract | B3 | The first stored dotfile |

## Cross-team dependencies (what you need, from whom, by which slot)

| What den needs | From | By |
| --- | --- | --- |
| Write use cases handed to the den write gate (the Account ones) | identity | P2 |
| Write use cases handed to the den write gate (the commission ones) | commission | P2 |
| The MAC's key under the custody the did:plc keys use: today's envelope root key, then the KMS before alpha | identity, and the owner of the security-asap custody item | P2 |
| The hold, read by the arms; the moderator principal | moderation | P2 |
| An architecture test in `just gate`: no route reads or writes a node around Atta | tech lead | P2, P3 |
| Each core type's payload message; the file's content id at upload; the changelog as the content of its file node | commission | P3 |
| Where a commission's brief and surfaces go in the tree. Den's suggestion: as child nodes, because anything narrower than the commission is its own node (My Den, decision 9). | commission, with den | P3 |
| The scan gate, before an upload becomes a node | moderation | P3 |
| Board columns, placements, view grants, the bring-into-Account record | commission | P4 |
| Character storage in PostgreSQL, and keeper mounts | identity | P4 |
| `/profile` records, and the signed-commit check through the hardened resolver | identity | P5 |
| Lockdown's switch. Moderation's own lockdown check needs P5's signed-out reads. | moderation | P5 |
| The Gallery over `/post` through Atta; where a Portfolio lives (an open row of The Den) | posts, identity | P5 |
| Who owns Tags | tech lead | A1 |
| Where Claims, Friendship and Blocks records sit. Den's suggestion: keyed by the full record address (DID, collection, record key), which also fixes the post key. | identity, posts | A2 |
| Plugin forms, scopes and credentials; Golems as Users | plugin, identity | B1 |
| The guarded HTTP client kept shared; custody for third-party tokens; a compliance review | identity, compliance | B2 |
| Your note 1 (storage routing): Atta stays above the router. It checks the tree's rules and routes to the type's action, and never learns which store holds a node. | tech lead, if the crate is built | Any slot |

## Not on the board (proposals, each with why)

- **The node store (P2).** It isn't a feature. But the alpha makes stored shape a one-way door, and today's nodes are projected from each type's own rows.
- **An upload quota (P3).** Real people upload from the alpha. Files are capped at 25 MiB each with no total, which is an open row on The Den.
- **External-storage mounts (B2).** This is your note 2. The Den holds user and Account directories to system folders until after the MVP, so it lands after the alpha.
- **Your own folders and dotfiles (B3).** The Den decided both, and no card carries them. Nothing in the alpha needs them unless board columns must be configured by a dotfile. If so, the dotfile mechanics move to an alpha slot.
- **Export your Den (R1).** Views are peers. An export must carry attributes explicitly (The Den, decision 26), and it serves credible exit. Whether data-portability law also requires it is unverified; that is for compliance.
- **Atta as its own library, and in the browser (R4).** The Den rules both for after the MVP. To keep them possible, Atta stays free of anything that won't build for the browser.

## Questions for the Engineer (each pre-answered)

1. **External-storage mounts: alpha or beta?** *Needed for: placing the slot.* *Proposed: beta, B2, right after plugin types.*
   - The Den holds user and Account directories to system folders until after the MVP.
   - The mounts need the hash (P3), the write side (P2), custody for third-party tokens and the scan gate first.
   - The alpha already carries 35 cards.
   - If you want it in the alpha, the earliest version is the last alpha slot: read-only, owner-only, one provider, in one new system folder.
2. **Where does a node's metadata live?** *Needed by: P2.* Your ruling to project from today's rows held until the first payload. *Proposed: one node table for Atta's fixed metadata, with each type's table keyed to it, and attributes keyed to it.*
   - The metadata is the same for every node.
   - Columns, links and dotfiles have no entity row.
   - Attributes need one key.
   - A resolve that reads one table keeps the equal-statements check simple.
   - It is the shared super-table that the identities decision (34013187) documented but didn't build ("revisit for a truly open union"), and plugin types make the Den one.
   - Cost: two writes per create, in one unit of work.
3. **Where do writes sit on the wire?** *Needed by: P2.* *Proposed: in each type's own service, as today.*
   - Each such route hands its action to one den write use case, which runs Atta's checks and dispatches through the host's match.
   - den.proto gains no write rpc.
   - Each team keeps reviewing its own contract, and the gate test keeps the path single.
4. **What does the private-data MAC cover, and what does a mismatch show?** *Needed by: P2.* *Proposed:*
   - The MAC covers the real path, the type, the hash, and the name (a commission's title is user content outside the payload).
   - Not the level: the MAC proves content, not access, and a database writer could grant access through other rows anyway.
   - No version number until something outside the database anchors it.
   - A mismatch is the one not-found, reported to you.
   - This is reasoning, not a checked source, so Heron checks it at P2.
5. **How do links behave?** *Needed by: P4.* *Proposed:*
   - A link shows its own name, copied when it is made.
   - Only a node you can open can be linked.
   - A link to an archived node acts like a typed path under your hiding ruling: it opens, marked, only for someone with their own relationship to the node, and is absent for everyone else.
6. **Seat ceilings in the formula: confirm?** *Needed by: P4.* *Proposed:*
   - A ceiling caps the standing a seat gives.
   - Grants lift as before (Seat Visibility Ceilings: "ceilings bound this seat's view", grants unchanged).
   - Atta keeps "the highest of own level, standing and grant", with the standing capped first.
7. **The upload quota.** *Needed by: P3.* *Proposed:*
   - One total per User, across every file they upload. You set the number.
   - An over-quota upload is refused before any byte is stored.
   - It is charged in full whether or not the bytes were known.
8. **The type names on the wire.** *Needed by: P5.* *Proposed:* ship the provisional names (`commission`, `commission.attachments`, …) unless the type-catalog decision lands by P5. A rename after the alpha shows older clients a generic icon rather than breaking them.
