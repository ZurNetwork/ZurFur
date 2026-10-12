# Roadmap: web

Lead: Fennec · Stages covered: pre-alpha from P2 (P1 = 0.1, fixed), alpha, beta, release · QA: Crow · 2026-10-10

## In one screen

- **The rule throughout.** A web screen ships in the same slot as the backend capability behind it. It is built first on mock data shaped by the approved contract, then wired to the real API in that slot. If a capability moves, its screen moves with it, never ahead of it.
- **End of pre-alpha (P9).** The closed group's whole loop works in the Explorer frame, in Zurfur's own look (the Claude Design pass). You can make commissions, characters and boards, upload files, pin and comment, invite a client, slot characters, approve, and invoice. Characters and Accounts go public with a key only you hold. Your public page, portfolio and the Gallery show mature art blurred by default. On the real domain there are invite codes, a report button on every piece of content, legal pages and a lockdown banner. Nothing waits on a blank screen, and an accessibility audit with real screen readers is recorded.
- **End of alpha (A1, A2).** The last four alpha cards get their screens: friendship, linking your alts, Ask-for-Art and reputation. The MVP's frontend is done, and every public address is stable, with redirects for anything that moves.
- **End of beta (B1 to B4).** Posting your work publicly, with credits and replies; commission comments; phases, YCH templates and commission trees; Seals and achievements; then the plugin screens, with plugin UI on its own domain.
- **Release (R1 to R3).** 1.0 adds notifications, the openings feed, collections and capacity guards. It also brings what real work needs: search engines let in, a data export, and an accessibility statement. After that come disputes and subscriptions, then the marketplace and the Telegram, Trello and plugin-key screens.
- **From you, now:** four questions before P2 (below), and three one-way doors flagged early. The first question asks for your Claude Design session (your sona's colours, your taste, a chosen direction) before P2 starts.

## Pre-alpha

Every slot, at no extra charge:
- mock first, wired in the same slot, with golden-response tests;
- Crow's keyboard walk and 320 px run;
- the contrast test on every gate;
- a security review when the slot touches sessions, the private/public boundary or DIDs.

The privacy rules from 0.1 stay as they are: no caching of signed-in pages, nothing in browser storage, route-only logs, generic titles, one cookie.

- **P2: You make things in your Den, in Zurfur's own look, and a slow load never looks broken.** (web: L)
  - Features:
    - `frontend`: the Claude Design pass (token values and core components, not names) and the loading states you deferred (skeleton rows from 300 ms, "Still loading…" at 3 s, the tree's "Couldn't load. Retry." row, a quiet progress bar);
    - `commission-core`: create a commission, change its lifecycle and status, shown as a calm status pill;
    - `changelog`: a timeline, a page at a time;
    - `surfaces`: the brief as it stands, read-only;
    - `characters`: create, rename and delete (Private only);
    - `workflows`: create the one board;
    - `closed-door`: every form starts Private, and widening is a deliberate choice;
    - `fact-deletion`: "Archive" and "Delete" said plainly, with what each keeps;
    - `maturity-ui`: the required rating picker on a commission.
    - Proposal: stream the Den read so the frame paints before the backend answers, if the stale-token and ended-session cases can be shown in the page.
  - Depends on:
    - you: the Claude Design session;
    - den: named actions in den.proto, and how a type's content reaches the web (question 3);
    - commission: create, status, archive and delete, the changelog page read, the brief read;
    - identity: Characters stored on PostgreSQL;
    - the Workflows' owner: creating a Workflow.
  - Done when, on `just dev`, signed in:
    - "New commission" asks a title and a rating and starts Private. It appears in the tree; a status change shows a pill and a new changelog line.
    - Archived, it shows only with the toggle.
    - A new Character appears under `characters`.
    - With DevTools throttled to "Slow 4G", a folder shows skeleton rows, then "Still loading…", never a blank pane.
    - The fonts and colours are the Claude Design system's, and the gate's contrast test passes.
  - Security review: yes (the first writes, and the level picker).
- **P3: You see your files, talk on pins, and keep your own settings.** (web: M)
  - Features:
    - `markup`: pins on an image, each opening a thread, with replies;
    - `commission-core`: upload to `attachments` and `products`, and see an image;
    - `maturity-ui`: the rating on a product;
    - `frontend`: the light theme and your viewer settings (text size, contrast, motion), "Show hidden" for dotfiles, and "Upload configuration".
  - Depends on:
    - moderation: the scan gate on every upload route;
    - commission: uploads, file bytes through the gated blob store, pins, threads and replies;
    - den: dotfiles, "Upload configuration", reading and writing your private settings file, the "show hidden" listing option;
    - whoever serves file bytes: user bytes are never served as live content from Zurfur's own address. An SVG can carry script; this is reasoning, to confirm in the review.
  - Done when:
    - Upload a PNG and open it: the image shows.
    - Drop a pin, write, reply; the thread holds both.
    - Pick the light theme, reload, then sign in from a second browser: light in both.
    - DevTools → Application shows no browser storage.
  - Security review: yes.
- **P4: You run your studio: members, and the board.** (web: L)
  - Features:
    - `accounts`: invite a member, accept, set roles, leave;
    - `workflows`: columns, and commissions placed on them;
    - `board-ordering`: move a card by dragging, and also by keyboard or a "Move to…" menu with no dragging;
    - opening links: a card on the board opens its commission.
    - Proposal: fold today's separate Accounts pages into the Den, ending the duplicate you accepted when the Accounts pages went in the frame.
  - Depends on:
    - identity: Account invitations, accept, roles, leave;
    - den and commission: bringing a commission into an Account, columns as nodes, placements as links, opening a link;
    - the Workflows' owner: placing and moving by "after" anchors.
  - Done when:
    - On the dev profile, alice invites bob to Supreme Arts and bob accepts.
    - alice adds a column, places a commission, and moves it once with the mouse and once with the keyboard only.
    - bob sees the same order after a reload.
  - Accessibility: WCAG 2.2 "Dragging Movements" (AA) requires a non-dragging way for every drag, and "Target Size (Minimum)" requires 24 by 24 CSS pixels (W3C Recommendation of 12 December 2024, read 2026-10-10).
  - Security review: yes, because membership now opens commissions brought into the Account.
- **P5: You work on a commission with your client.** (web: L)
  - Features:
    - `seats-handshake`: invite into a seat, apply to one, accept, with seat ceilings shown;
    - `slots`: put a Character in a slot with its Keeper's consent;
    - `view-grants`: give a view grant (a card, the description, or everything) and take it back;
    - the commission's `participants` entry.
    - Proposal: a "Requests" page listing what waits on you (question parked to P5).
  - Depends on:
    - commission: seats and the handshake, seat ceilings, slots and Keeper consent, view grants, participants, a read of the pending items addressed to me;
    - den: a grant shows up as a mount.
  - Done when:
    - alice invites bob as client. bob finds it under Requests and accepts, and the commission appears in his `commissions`.
    - bob's character fills a slot only after bob consents.
    - carol, given a card grant, sees a card and nothing more.
  - Security review: yes.
- **P6: You ask for approval and get paid.** (web: S)
  - Features:
    - `request-approval`;
    - `client-progress`: the client sees where the work stands without asking;
    - `invoices`: issue one prefilled from the slots, and mark it paid.
  - Depends on: commission (request and approve, the status-only view, invoices).
  - Done when:
    - alice asks for approval and bob approves.
    - A status-only viewer sees the status and nothing else.
    - alice issues an invoice and marks it paid.
    - The changelog shows all three.
  - Security review: yes (the status-only view is a boundary).
- **P7: Your characters and Accounts go public, with a key only you hold.** (web: L)
  - Features:
    - `did-ops`: the owner-key ceremony. You make your own key and save it, and you see it first in the rotation list.
    - `character-presence`: publicize a Character, with its public page; unpublish it.
    - `claims`: claim a character; it reads "claimed, unconfirmed" until both halves exist.
    - `account-pds`: an Account's public repo, with your owner key.
    - `handle-change`: rename an Account's handle, or prove your own domain.
  - Depends on:
    - identity: attaching an owner key, hosted repos, publicize and unpublish, claims, the handle change flow, own-domain checks;
    - vulpes: the claim halves, if Zurfur uses its library;
    - question 7: where the key is made.
  - Done when:
    - Publicize Ember. You save a key file, and the page shows its fingerprint first in Ember's rotation list.
    - Ember's public page opens signed out.
    - An Account's handle moves to your own domain after the domain check.
  - Security review: yes. It covers keys, and the correlation between DIDs and handles.
- **P8: Your public page, your portfolio and the Gallery, with mature art blurred by default.** (web: L)
  - Features:
    - `user-pages`: the public User Page, a view of your tree at the visitor's level;
    - `portfolios`;
    - `gallery-view`: minimal;
    - `tags`;
    - `maturity-ui`: the blur cover with "I'm 18+", and your sensitivity setting (Safe, Blurred or None), Blurred for everyone by default, signed-out visitors included;
    - other people's avatars, loaded through Zurfur, which 0.1 parked to the User Page.
  - Depends on:
    - identity: profile records, avatars through Zurfur, rate limits on anonymous reads;
    - posts: the Gallery's view and Portfolios;
    - the owners of tags;
    - den: public views resolved at the visitor's level, and the rating on public items.
  - Done when:
    - Signed out, your public page lists only Public items.
    - An Adult piece is blurred until "I'm 18+".
    - Signed in with "Safe", labelled items are hidden.
    - DevTools shows no request to another host.
  - Security review: yes. Public pages are the private/public boundary.
- **P9: Ready for a real domain and a closed group.** (web: L)
  - Features:
    - `deployment`: the production build behind the one origin;
    - `login`: the production sign-in, and screens to enter an invite code and see the codes you've earned;
    - `security-asap`, the web's part: an origin check on every form post, a full content-security policy, and the report button and form;
    - `blocks`: block and unblock; a blocked person's replies, credits and applications stop showing;
    - `frontend`: the accessibility audit with real screen readers.
    - Proposals: the lockdown banner, the moderator console, error reports only through Zurfur's address, search engines kept out, signed-out bug reports. The legal pages are in the corpus but not a card.
  - Depends on:
    - whoever owns deployment: proxy, TLS, hosting;
    - identity: the production OAuth client, invite codes;
    - moderation: the report path, lockdown, the console and its address, blocks, and its floor before anyone else can reach a build;
    - compliance: the legal pages' text, what "I'm 18+" must say, any cookie notice;
    - the tech lead: anti-abuse for signed-out bug reports, and who reads them.
  - Done when:
    - On the real domain, a second person signs in with a fresh invite code.
    - Every response carries the policy headers.
    - A report from any content reaches moderation.
    - Turning lockdown on shows the banner.
    - Crow's check file holds walks with NVDA on Firefox and VoiceOver on Safari iOS.
  - Security review: yes, the stage gate.

Exit, before alpha opens:
- every screen on `main` is backed by the real backend, and mock mode still refuses a production build;
- no web must-fix is open in any security review;
- the Claude Design look and the light theme ship;
- nothing waits without a skeleton or a time limit;
- on the real domain every page carries a full content-security policy and makes no request to another host (your own avatar now comes through Zurfur too);
- a WCAG 2.2 AA audit is recorded;
- every alpha card except friendship, user-linking, Ask-for-Art and reputation has its screens.

## Alpha

- **A1: Friends, and your alts linked on purpose.** (web: M)
  - Features: `friendship` (request, accept; the friendship shows only when both sides exist) and `user-linking` (prove control of your second handle, so both public pages point at each other; undo it).
  - Proposal: a "Your reports" page, now that someone other than you files bug reports.
  - Depends on:
    - identity: friendship records, and the second-handle proof (a second sign-in that must never mix two sessions);
    - moderation: blocks, already in, sever a friendship's rendering.
  - Done when:
    - alice and bob friend each other, and it shows only after both accept.
    - You link two of your handles, and each page names the other.
    - Undo it, and both references are gone.
  - Security review: yes. This is correlation by your own choice, with two identities in one browser.
- **A2: Ask for art, and say how it went.** (web: M)
  - Features: `ask-for-art` (open an ask, apply to one, the ask feed) and `reputation` (rate a finished commission from your side, 1 to 10, positive only; both ratings on profiles).
  - Depends on:
    - commission: asks as tags, applications into an ask, ratings tied to membership;
    - the tags' owner: the ask feed's query;
    - blocks: a blocked creator can't apply.
  - Done when:
    - bob opens an ask, alice finds it in the feed and applies, and bob accepts.
    - After completion each rates the other, and the ratings show on both profiles.
- **Follows den, if it lands in alpha:** the screens for external-storage mounts (your note): connecting a storage, and a plain "offline" state for a mount that stops answering.

Exit: all 35 alpha cards have their screens, and the `frontend` card is done. Public addresses are stable, with redirects for anything that moved. The accessibility audit is re-run on the alpha's screens.

## Beta

- **B1: Publish your work, and talk in public.**
  - Features:
    - `gallery-posts`: post a product, with its snapshot, its credits, and its rating carried as labels;
    - `replies`: public reply posts and shouts;
    - `commission-comments`: the commission page's own comment surface.
  - Depends on:
    - posts: post records, snapshot, credits, labels;
    - commission: the comment surface;
    - moderation: holds on public replies.
  - Done when you publish a product, see it in the Gallery with its credits, and reply to it from another user.
- **B2: Commissions with stages, templates and families.**
  - Features:
    - `phases`: the stage list, and approval per stage;
    - `ych`: author a template, and clone it;
    - `trees`: parent and child commissions, with their semantic edges;
    - `hash-chain`: a quiet "record intact" line, only if commission serves a check.
  - Depends on: commission.
  - Done when a cloned YCH opens with its stages and its parent shown.
- **B3: Seals and achievements.**
  - Features:
    - `seals`: badges from labelers, peer grants between mutuals, the Trusted mark, the convention QR flow;
    - `achievements`: EXP and levels, shown without streaks, leaderboards or red counters, as the design language rules.
  - Depends on: whoever owns Seals and EXP.
  - Done when a peer grant from a mutual shows on your page, and the QR flow works phone to phone.
- **B4: Plugins.**
  - Features: `plugin-api`.
    - Install a Golem or a Portal on your User or Account, and grant its scopes in plain words.
    - Plugin types render through a declarative-view renderer.
    - Portal UI runs on its own registrable domain.
    - `kafka` has no web part.
  - Proposals: the Finder-style column view as a view you pick; space themes for visitors, which make the token names a one-way door.
  - Depends on: plugin (install, scopes, the declarative view vocabulary, the Portal's domain).
  - Done when you install a first-party Golem, revoke one scope, and see a plugin-typed file render.

Exit: every beta card has its screens. Plugin UI never runs on Zurfur's own origin.

## Release

- **R1 (1.0): Everyone, for real work.**
  - Features:
    - `notifications`: an inbox you pull, the seen cursor, your rules;
    - `openings-feed`: the feed, with its RSS link;
    - `collections`: favourites, wishlist, will-not-commission;
    - `capacity-guards`: the board says when you've taken on more than you set.
  - Also what real work needs:
    - search engines let in, if you kept them out;
    - a "take everything with you" export page;
    - an accessibility statement after an outside audit;
    - a browser-support policy;
    - a performance budget you set.
  - Depends on:
    - the owners of notifications, the openings feed and collections;
    - the Workflows' owner;
    - compliance: accessibility law, and the privacy policy at scale.
- **R2: Disputes and supporters.**
  - Features: `disputes` (flag, then direct, mediation and arbitration; the wronged party controls publicity) and `subscriptions` (tiers, patron access, patron-priority commissions; Zurfur holds no money).
  - Depends on: the owners of disputes and subscriptions, and the payments decision.
- **R3: The plugin ecosystem.**
  - Features:
    - `marketplace`: listings, ratings, reports;
    - `telegram`: connect the bot by a link;
    - `trello-import`: the import wizard;
    - `webhook-signing`: plugin keys shown once, and rotated.
  - Depends on: plugin.

## One-way doors, and the slot that closes each

| Door | Why it's one-way | Closes |
| --- | --- | --- |
| **Public page addresses**: the User Page, a public Character or Account, a Portfolio, a Gallery item | They are shared on Bluesky the day they exist, so an old address must redirect forever. Proposed shape: `zurfur.app/profile/<handle>`, the literal swap of Bluesky's `bsky.app/profile/<handle>` (checked 2026-10-10). A bare `zurfur.app/<handle>` collides with files such as `/robots.txt`, which is a valid handle: two segments (atproto handle spec, read 2026-10-10). | Designed in P8; for real when the alpha opens |
| **Search engines on public pages** | Another site's cached copy of a page can't be recalled. | The first public page on the real domain, unless kept out (question 6) |
| **The owner key**: which kind, how it's made, what file you keep | A real mint on the public PLC directory is permanent and public. | Designed in P7; for real at the alpha's first real Account or public Character |
| **Your viewer settings' keys** (theme, text size, contrast, motion, maturity sensitivity) in your private settings file | Stored data in real users' Dens. | When the alpha opens |
| **Where user-uploaded bytes are served from** | A public file's address gets shared from P8 on. Serving user bytes from Zurfur's own address risks script running there. | Designed in P3; for real in P8 |
| **How a type's content reaches the web** (question 3) | den.proto's content shapes freeze with den.proto when the alpha opens. This is the den team's door; the web consumes it. | When the alpha opens |
| **Token names as the theme vocabulary** | Once space themes let people write token names into files, renaming a token breaks those files. | The beta slot that ships space themes, not before |
| **Plugin UI's origin** and **the moderator console's address** | A domain name, once published, is permanent. These are the plugin and moderation teams' doors; the web hosts them. | B4; P9 |

## Cross-team dependencies (what you need, from whom, by which slot)

| By | From | What the web needs |
| --- | --- | --- |
| P2 | the Engineer | The Claude Design session: your sona's colours and taste, a chosen direction |
| P2 | den | Named actions in den.proto with one error shape; how a type's content is shaped (question 3) |
| P2 | commission | Create, lifecycle and status, archive and delete, the changelog page read, the brief read |
| P2 | identity | Characters stored on PostgreSQL: create, rename, delete |
| P2 | Workflows' owner (to name) | Create a Workflow |
| P3 | moderation | The scan gate on every upload route |
| P3 | commission | Uploads, file bytes through the gated blob store, pins, threads, replies |
| P3 | den | Dotfiles, "Upload configuration", read and write of your private settings file, the "show hidden" option |
| P4 | identity | Account invitations, accept, roles, leave |
| P4 | den, commission | Bringing a commission into an Account; columns as nodes; placements as links; opening a link |
| P5 | commission | Seats and the handshake, ceilings, slots with Keeper consent, view grants, `participants`, a read of pending items addressed to me |
| P6 | commission | Request and approve, the status-only view, invoices |
| P7 | identity (vulpes if used) | Attaching an owner key; hosted repos; publicize and unpublish; claims; the handle change flow and own-domain checks |
| P8 | identity, posts, den, tags' owner | Profile records; avatars through Zurfur; anonymous rate limits; the Gallery's view; Portfolios; public views at the visitor's level; tags |
| P9 | deployment owner, identity, moderation, compliance, tech lead | Hosting and TLS; production OAuth client; invite codes; report path, lockdown, console, blocks; legal text; bug reports from signed-out visitors |
| A1 | identity, moderation | Friendship records; the second-handle proof; block severance |
| A2 | commission, tags' owner | Asks as tags; applications into an ask; ratings; the ask feed |
| B1 to B4 | posts, commission, Seals' owner, plugin | Each beta card's capability, as listed above |
| R1 to R3 | each card's owner, compliance | As listed above |

## Not on the board (proposals, each with why)

1. **The Claude Design pass**: alpha users see the product. It changes token values, not names, so it's cheapest before more screens exist.
2. **Loading states and skeletons**: your words, "so that people don't just LEAVE if nothing loads". You deferred them in 0.1.
3. **Streamed Den reads**: today a full page load shows nothing until the backend answers.
4. **The light theme and your viewer settings**: comfort and accessibility. They live in your private settings file (The Den, the dotfiles ruling).
5. **A pulled "Requests" page**: notifications are a release item, yet a pending invitation gets no card, and the My Den rulings say notifications announce it. Without this page, nobody learns they were invited.
6. **Folding today's Accounts pages into the Den**: it removes the duplicate that can disagree with the tree.
7. **Invite-code screens**: your stages ruling, not a card.
8. **The lockdown banner and the moderator console**: your moderation rulings of 2026-10-08. **Legal pages** are already in the corpus (Inventory Closures, the four moderation slivers).
9. **Error reports and visitor counts only through Zurfur's address**: the corpus puts Sentry and Plausible in the alpha, while 0.1 guarantees no request to another host.
10. **Search engines kept out through the beta**: alpha and beta are closed groups.
11. **Signed-out bug reports, and a "Your reports" page**: parked in 0.1 to the alpha.
12. **An accessibility audit with real screen readers before the alpha, and a statement at release**: real people arrive at the alpha.
13. **The Finder-style column view**: your note on the tree's keyboard behaviour, "as a theme in the future".
14. **Space themes for visitors**: The Den's dotfiles ruling. They make the token names a door.
15. **External-storage mount screens**: your note of 2026-10-10. They follow den's slot.
16. **A data export page, a browser-support policy and a performance budget at 1.0**: these are what "used for real work" asks of a web app.

## Questions for the Engineer (each pre-answered)

**Needed now: P2, and the roadmap's shape.**

1. **Will you run the Claude Design session before P2 starts?** That means filling your sona's colours and your taste into the brief and picking a direction. *Proposed:* yes, during 0.1's last units, and the web syncs the values at the start of P2. If not, P2 builds its forms on the placeholders, and the pass moves to P3, no later than P9.
2. **Do the loading states land in P2?** You deferred them in 0.1 to "after the Claude Design sync". *Proposed:* yes, in the same slot, right after the sync, so every new screen is born with them.
3. **How does a type's content reach the web?** The first answer is needed in P2, for a commission's status and the changelog. The den team co-owns this.
   - (a) Each core type has its own message in the contract and its own screen in the web.
   - (b) Atta renders a generic declarative view, and the web draws any type from it.
   - (c) (a) for core types through the alpha; (b) arrives with plugin types in the beta.
   - *Proposed: (c).* A board you drag, pins on art and forms outgrow any view vocabulary designed this early. Typed messages are testable against golden responses. The generic renderer gets designed when its first outside user, plugins, exists.
4. **When does the alpha open?** *Proposed:* after P9. At that point the commission loop works end to end and the safety floor stands. A1 and A2 then add friendship, alts, Ask-for-Art and reputation, which are thin web work. Blocks land before strangers can apply to anyone.

**One-way doors flagged early.** They are asked at their slot; the Orchestrator decides whether they reach you now.

5. **Public page addresses are `zurfur.app/profile/<handle>`** (P8). *Proposed:* yes. Swapping `bsky.app` for `zurfur.app` then lands on the same person, and no handle can collide with a file such as `robots.txt`.
6. **Search engines are kept out of public pages through the beta** (P9). *Proposed:* yes. Pages carry "noindex", and indexing starts at 1.0.
7. **Where is your owner key made?** (P7)
   - (a) In the browser.
   - (b) Outside Zurfur; you bring only its public half.
   - (c) The browser by default, with bring-your-own offered.
   - *Proposed: (c).* What the choice turns on:
     - A key made by a script Zurfur serves is only as independent of Zurfur as that script. So (b) is the path that keeps your exit open even if Zurfur is compromised.
     - The browser's built-in cryptography makes P-256 keys (MDN, read 2026-10-10).
     - The PLC directory accepts P-256 and secp256k1 rotation keys (PLC spec v0.1, read 2026-10-10).
8. **At the alpha, do error reports and visitor counts go only through Zurfur's own address?** (P9) *Proposed:* yes.
   - No third-party script runs in the browser.
   - Error reports go out with DIDs, handles, Den paths and page tokens removed.
   - Visitor counts are taken on signed-out pages only, as the corpus says.

**Parked, asked at their slot:**
- P4: fold today's Accounts pages into the Den? *Proposed:* yes.
- P5: a pulled "Requests" page in place of notifications? *Proposed:* yes, with no counters.
- P8: your maturity setting lives in your private settings file. A signed-out "I'm 18+" is remembered for that page only, with nothing stored. *Proposed:* yes.
- B4: the Finder-style column view, as a view you choose in your settings? *Proposed:* yes.
