# Roadmap: plugin

Lead: Octopus (programmers Hermit, Remora, Cuttle · QA Mantis) · Stages covered: all four; build work starts in the beta

## In one screen

- **Pre-alpha:** no plugin code and no slot of our own; our people can be lent (the tech lead's call). We review four seams other teams build: den.proto's three plugin openings, a reserved plugin-type name, a CSRF guard scoped to the cookie routes, and impersonation names on the reserved-handle list.
- **Alpha:** still no build, and no alpha card is ours. The alpha opens with every line of our "keep open" list intact. In its last slot we write the plugin design study on paper, so the beta starts with its questions answered.
- **End of beta (the plugin-api card, first-party):** a Golem is a User the Zurfur team registers. It joins a commission by accepting an invitation over `/plugin/v1`, reads the changelog at its seat's ceiling, acts within its scope profile and appends `plugin_recorded` entries. It joins Accounts once the MVP-only "membership confers view" exception is gone. First-party plugin types render in the Den. Golems pull and nothing is pushed, so nothing leaves unsigned. `/plugin/v1` stays pre-GA.
- **End of release:** signed, SSRF-hardened push deliveries with per-install keys, the Telegram bot, the public marketplace (self-serve listings, the Trusted publisher Seal, the publisher ban, reports) and the Trello importer. `/plugin/v1` goes GA. The sandboxed JS/HTML Portal stays parked.
- **External-storage mounts (note 2):** the den team should carry them. We review the adapter seam and the threat model (Question 2).
- **Due before the alpha:** one ruling, how a plugin's type shows on the wire (Question 1). Every other question waits for its slot.

## Pre-alpha

No slot of our own from P2 on. Each item is a review that rides another team's slot. None is build work.

- **The plugin-type rule goes into the type-catalog decision.** Features: none on the board. It is part of den's type catalog. The TDD's parked row "The type catalog" doesn't carry it yet, and the 0.1 carry-over's row 35, which asks to add it there, is still open. Depends on: den. Done when: den.proto's comment on a node's `type` names the reserved value and shape, and no core type uses either (Question 1).
- **Tolerant-reader tests for an unknown type and an unknown content case** (the 0.1 carry-over, row 34, still open). Features: none. Depends on: web, in whichever slot next touches den.proto's client, if 0.1 doesn't land them. Done when: `just gate` runs two web tests. One decodes a node of an unknown type and shows the generic icon. The other decodes an open node with an unknown content case and shows "content not shown yet".
- **den.proto keeps its openings until it freezes.** Features: none. Depends on: den, on every den.proto PR. Done when: every den.proto PR before the alpha carries our review line, and its two requests still take only the path, `includeDeleted` and `pageToken`.
- **The CSRF origin guard covers the cookie routes only.** It belongs to security-asap. Auth Surfaces (24543244) D1 says the guard is never a blanket cross-origin block, because a blanket block would refuse the bearer API's legitimate callers. Depends on: whoever builds the guard. Done when: a test sends a cross-origin request to a route outside the cookie routes and the CSRF guard doesn't refuse it, and no web page or API route sits under `/plugin/`.
- **Impersonation names go on the reserved-handle list.** This belongs to security-asap's "handle-namespace edge cases". The Account Handle decision (24870914) leaves the reserved set open. Golems act and speak as themselves, so a Golem called `support` is the impersonation case. Plugin Security (30572603) D5 already reserves "Zurfur" for listings. Depends on: identity. Done when: a test refuses `zurfur`, `support`, `staff`, `official`, `moderator`, `admin`, `help`, `security` and `team` as `*.zurfur.app` labels. These are our proposed additions. Your list wins.

People: Hermit, Remora and Cuttle are free for other teams' slots (the tech lead's call). Mantis keeps reviewing bug reports' security.

Exit: before the alpha opens, Question 1 is ruled and written into den.proto's comments, the two tolerant-reader tests are green, the CSRF guard covers the cookie routes only, and the reserved-handle list has the impersonation names.

## Alpha

No slot of our own. No alpha card is ours, so "every alpha card has a home" asks nothing of us.

**What must stay open through pre-alpha and alpha, so the beta can land plugins.** Each line names whose ground it is on.

1. **den.proto's three openings** (den, web). den.proto freezes when the alpha ships. An unknown type gets a generic icon. An unknown content case shows "content not shown yet". The requests carry no viewer, no root and no "on behalf of", so a Golem can later read as itself.
2. **No core type takes the reserved plugin value or shape** (den).
3. **A Golem is a User** (identity). There is no `golem` actor kind. A User row must not need an OAuth session, a PDS of its own or a handle in order to exist. This isn't a hard door, since an additive migration would fix it, but it is cheaper never to close it.
4. **Every use case takes its actor from the command, and none reads the cookie** (every backend team). The Application Layer decision (55836674) D7 already says so. It lets a Golem accept an invitation through the same use case a human uses.
5. **Seat ceilings are enforced on the one read path, not per endpoint** (commission, as part of the seats-handshake card). A Golem's whole read boundary is its seat ceiling (Seat Visibility Ceilings, 30277634, D3).
6. **The Den's one not-found and its hiding seam** (den). The seam is the My Den rulings' one not-found (2026100701, D21) and `admit`, the host method your 0.1 ruling on hidden nodes (D-15) placed after access resolves, which refuses a node without saying why. That is where a plugin's scope will refuse a node.
7. **The session cookie authenticates only the cookie routes, and `/plugin/` stays free** (whoever owns routing).
8. **The changelog's kinds stay a closed vocabulary, and no core kind is named `plugin_recorded`** (commission). The Changelog as the Only Record (59310081) D3 reserves that kind for plugins.
9. **The MVP-only "brought into an Account" exception stays marked FIXME** (den, commission; My Den rulings, D8). Nothing new is built on "membership confers view", because Golem members would inherit it.
10. **Invite codes earned through usage, and reputation, can tell a Golem from a human** (whoever builds invite codes; commission for reputation). With no Golems in the alpha this costs nothing, and it keeps a Golem farm from earning either one later.
11. **The outbox and the rate limiter stay general** (identity, internal). That means keyed by an opaque principal, not only by IP address or only for PLC operations. Per-Golem limits and deliveries then reuse them. This is reuse, not a door.

- **A1 (the alpha opens): the door check.** Features: none. Depends on: every team named above. Done when: each of the eleven lines has a test or a file reference beside it, signed off by Mantis before the alpha tag.
- **An (the last alpha slot): the plugin design study, on paper** (proposal). Features: plugin-api (preparation). Depends on: none. Done when: the study is in `.understand/`, with options and pre-answered questions for B1 and B2, and you have one question sheet. No code.

Exit: the alpha ships with all eleven lines intact, and the study's B1 questions are answered.

## Beta

- **B1: A Golem joins a commission and reads it.** Features: plugin-api (Golems, the `read` profile). About 0.1's size; split it at the study if it is bigger.
  - **Registration.** The Zurfur team registers a Golem: a User with a DID from the shared minter. Its admins sit in their own table, and their acts (rotate the key, edit the profile, archive) are audited by name.
  - **The surface.** `/plugin/v1` opens. The Golem's credential is exchanged for short-lived, scoped tokens tagged with the plugin and the principal, and is never a long-lived bearer (Auth Surfaces, D3). Revocation is checked on every call.
  - **Reading.** The Golem lists its pending invitations and accepts one. It pages its commission's changelog at its seat's ceiling. This is the read feed the changelog decision allows (59310081, D8).
  - **Out of reach looks like nonexistent.** A DID or node out of its reach answers exactly like one that doesn't exist, and every lookup by DID is rate-limited per key. Auth Surfaces' threat pass makes this an acceptance criterion (finding F1).
  - **The rest.** The web shows a Golem as a Golem. Moderation can suspend a Golem (proposal). `/security-review` is mandatory.

  Depends on:
  - identity: minting, and a user store that takes a Golem;
  - commission: accepting an invitation through `/plugin/v1`, seat ceilings and the paginated changelog read;
  - moderation: the suspend action;
  - web: the Golem's pages and how it is shown;
  - internal: contract CI for the plugin package, and the versioning contract's tolerant-reader clause extended to name the plugin surface;
  - den: the hiding seam, for Den reads.

  Done when:
  - you register a Golem, invite it with a Description ceiling, and page the changelog with `curl` and its token, seeing only what a Description seat sees;
  - a real DID out of its reach and a made-up DID get byte-identical answers;
  - a request with a valid `zurfur.sid` cookie and no token is refused;
  - after "rotate key", the old key's next call fails.
- **B2: Golems act, and append to the record.** Features: plugin-api (`act`).
  - **Acting.** The cumulative profiles `notify`, `act` and `manage` arrive. In an invoked act the clicking human is the principal; in a reactive act the Golem itself is. Every act passes the domain's own rules and lands in the changelog under the Golem's name. Widening a Golem's profile asks for consent again.
  - **Appending.** The `changelog:write` scope arrives (Plugin Security, D1): one opaque kind, `plugin_recorded`, with a required sentence and a size-capped opaque payload. It is append-only, with a tighter limit per plugin per commission. `/security-review` is mandatory.

  Depends on:
  - commission: `plugin_recorded` in the changelog contract, and the hash chain (a beta card) covering plugin entries. The chain lands before this slot or with it, or else covers earlier entries when it arrives.
  - web: the consent-again screen, and plugin sentences shown as plain text.

  Done when:
  - a Golem with `act` moves a commission's status, and the changelog says it did;
  - the same call with only `read` is refused;
  - a move the lifecycle refuses is refused for a Golem too;
  - a sentence holding markup renders as literal text;
  - appends past the per-commission limit get 429 with Retry-After.
- **B3: Plugin types in the Den.** Features: plugin-api (the renderer form, which is how the Den decision reads "Portals"). A first-party plugin defines a type, and files of that type render as declarative views. The type travels on the wire as Question 1 rules. Its dotfiles merge through the plugin that defined them (The Den, 2026093001, D28). A viewer without the plugin sees the generic icon and "content not shown yet". Depends on: den (one enum variant for plugin types, with a registry scoped to plugins, the direction the My Den rulings record) and web (the declarative view renderer). Done when: a file of the plugin's type shows its view in My Den, and without the plugin it shows the generic icon. Under Question 8's answer, the plugin's host also receives no file content, which you can check in its logs.
- **B4: Golems in Accounts; installs by Users and Accounts.** Features: plugin-api (Account context through membership, install bindings, and scope grants by an Account admin). Depends on: den and commission removing the MVP-only membership exception (Question 14), and identity (Account roles). Done when:
  - an Account admin adds a Golem as a member, and it sees the Account's workflow and nothing more than the ruling allows;
  - removing the Golem ends its reach at once;
  - a User installs a type renderer for themselves and revokes it without touching anyone else's install.
- **B5 (proposal): beta testers register their own Golems.** Features: plugin-api (Golem as User, 28049410, D3: any User may register one, free). Depends on: B1's suspend action, a registration cap (Golem as User's open row "Registration anti-abuse"), and moderation. Done when: a tester registers a Golem, the cap refuses one past the limit, and a moderator suspends the Golem so that its next call fails.
- **B6 (only if you pull it in, Question 9): the Trello importer.** Features: trello-import, moved from the Release lane. It is a Golem with the `act` profile. It creates commissions from a Trello export and invites the artist, whose acceptance is the consent, then hands over ownership (Plugin Forms, 29458464, D8). Depends on: B2, and commission (ownership transfer). Done when: you import a Trello export, each card becomes a commission you're invited to, accepting makes you its owner, and the changelog says the importer did it.

Exit: the plugin-api card is done for first-party plugins. Golems register, join, read, act and append. Plugin types render. Golems are in Accounts as you ruled. Every slot passed its security review, and a Golem can be suspended. `/plugin/v1` is still pre-GA, so breaking it costs nothing yet.

## Release

- **R1: Signed push deliveries and per-install keys.** Features: webhook-signing. What it holds, from Auth Surfaces D5 and Plugin Security D2–D3:
  - event scopes deliver domain events, filtered by what the Golem can see;
  - `standard-webhooks` signatures: HMAC-SHA256 over the raw bytes, with a timestamp and a stable id bound in;
  - one secret per install, with overlapping rotation;
  - SSRF defense: resolve, check, then pin and dial; metadata addresses refused, IPv6 included; no redirects; bodies never reflected;
  - delivery at least once, with backoff, jitter and a dead-letter queue, as its own retryable outbox step;
  - payloads that carry only the DIDs the scope needs;
  - install copy saying that uninstalling stops future access but doesn't retract identities already sent (finding F2).

  Per-install keys replace the beta's one key per Golem. Depends on: domain events as code (the open fork that notifications and EXP share); the outbox or the broker backbone (the kafka beta card); web, for the install copy. Done when:
  - you point a Golem's webhook at a local receiver and verify a delivery with the standard-webhooks reference library;
  - a webhook address that resolves to 127.0.0.1, 169.254.169.254 or ::1 is refused;
  - deliveries verify under both secrets during a rotation;
  - a receiver that is down gets retries, then a dead-letter entry.
- **R2: The Telegram bot.** Features: telegram. The reference Golem, whose outside face is the bot. It installs through a deep link carrying a one-shot token that expires (First-party plugins, 3244063), and it reads the notification rules' output. Depends on: R1, and notifications (a Release card). Done when: the deep link adds the bot to a Telegram group, a watched commission's status change posts there, revoking stops it, and a reused link token fails.
- **R3: The marketplace, and `/plugin/v1` goes GA.** Features: marketplace. What it holds (Plugin — Listing Lifecycle, 41189377; Plugin Security, D4–D5):
  - the listing lifecycle, Draft, Listed (self-serve), Deprecated and Archived, with visibility Private, Unlisted or Public;
  - the storefront: author snapshot, scopes, download count, and 1–5 star reviews the author can answer;
  - reports that open an investigation;
  - deletion only while no facts exist;
  - Trusted, as a Zurfur Seal on the publisher's Account DID;
  - the publisher ban: every listing dies and every runtime stops, while its Golems stay Users;
  - registration open to any User, under the cap.

  Breaking `/plugin/v1` after this mints a `/plugin/v2`, with at least 12 months' notice (Plugin — API Stability & Versioning; the versioning contract, R10). Depends on:
  - Seals (a beta card, owner unassigned): the Trusted label;
  - moderation: banning a publisher, unlisting a listing, plugin investigations;
  - compliance: the ban grounds, disclosure duties and reserved branding in the terms;
  - web: the storefront and install screens;
  - internal: per-major usage telemetry, which the versioning contract requires before the first deprecation.

  Done when: a test publisher lists a plugin self-serve, and you install it from its storefront and see its scopes. A moderator then bans the publisher: every listing disappears and every runtime stops, while its Golems still show in old changelogs.
- **R4: The Trello importer**, unless it went to the beta as B6. Features: trello-import. Same as B6.

What "used for real work" asks of us at release:
- **Set the build-time values.** Rate limits, payload caps and the Trusted review time are all "set at build" in Plugin Security; they get set.
- **A leaked-key runbook:** revoke, rotate, tell the installer.
- **A developer kit** (proposal).
- **Subscriptions** (a Release card, not ours) names "the plugin surface's recurring-billing primitive". Plugin acquisition and pricing wait for a future decision, so we build nothing for it until that decision exists.
- **The JS/HTML Portal stays parked.** Unparking it needs a second registrable domain, the opaque sandbox, a message bridge and per-frame tokens (Auth Surfaces, D4).

## One-way doors, and the slot that closes each

| Door | What becomes permanent | Closes at |
| --- | --- | --- |
| Core type names in den.proto, and the plugin-type reservation | The alpha publishes the names; renaming them later is breaking at review | A1 |
| den.proto's requests carry no viewer, root or "on behalf of" | A field added there binds every client | A1 |
| Impersonation names on the reserved-handle list | A real user can claim `support.zurfur.app` on day one | A1 |
| The first Golem's DID | did:plc is public and permanent: its first operation and rotation keys sit in the public directory | B1 |
| Scope names and profiles in stored grants and consent screens | Consent stored against real data | B1 |
| `plugin_recorded` entries in real changelogs | Append-only and chained; only a recorded redaction removes one, so the sentence and payload rules must be right first | B2 |
| A plugin type's wire form | Stored in real Dens | B3 |
| DIDs delivered to infrastructure Zurfur doesn't run | Can't be retracted (Auth Surfaces' threat pass, finding F2) | R1 |
| The webhook payload and signature scheme | Outside receivers verify against it | R1 |
| `/plugin/v1` GA | The 12-month deprecation floor binds | R3 |
| Public listings, reviews and the Trusted mark | Real publishers' public data | R3 |

## Cross-team dependencies (what you need, from whom, by which slot)

| From | What we need | By |
| --- | --- | --- |
| den (Badger) | The plugin-type reservation in the type catalog, and den.proto's openings kept until the freeze | A1 |
| den | The hiding seam (`admit`) and the one not-found, usable by a plugin's scope | B1 |
| den | One enum variant for plugin types, a registry scoped to plugins, and the dotfile merge handed to its plugin | B3 |
| den | Carry external-storage mounts, if you take Question 2's answer | Its slot |
| web (Fennec) | The unknown-type and unknown-content tests | The first slot after 0.1 that touches den.proto's client; A1 at the latest |
| web | Golem pages and how a Golem is shown (B1); consent again and plain-text plugin sentences (B2); the declarative view renderer (B3); install screens and the storefront (R3) | B1–R3 |
| identity (Fox) | Impersonation names on the reserved-handle list | A1 |
| identity | A user store that takes a non-human User; Golem DIDs from the shared minter, with one Zurfur-held key per Golem | Reviewed by A1, built by B1 |
| commission (Wolf) | Seat ceilings on the one read path; invitations any actor accepts through the same use case; the paginated, visibility-filtered changelog read | The alpha; used at B1 |
| commission | `plugin_recorded` in the changelog contract, and the hash chain covering plugin entries | B2 |
| commission, den | The MVP-only membership exception removed | B4 |
| commission | Ownership transfer, for the importer's hand-over | B6 or R4 |
| commission (slot pairing) | Commission's file puts its whole half of plugin-api in its B5: seats, the write gate, changelog appends and Golem members. Ours spreads over B1 (seats and reads), B2 (acts and appends) and B4 (Accounts). We agree on removing the membership exception before Golems join Accounts (their question 8). For the consolidator: either commission's half splits across our three slots, or ours moves together into one later slot. | Consolidation |
| moderation (Meerkat) | A "suspend a Golem" action (proposal) | B1 |
| moderation | The publisher ban, keyed by the publisher's Account DID. It is read at the token exchange and on every `/plugin/v1` call from B1, at webhook dispatch from R1 and at listing reads from R3. Moderation's file places it at their B3 and tests it on a listing and a webhook. In our plan neither exists before R1, and the beta's only publisher is Zurfur, so a ban has nothing to stop before B5 (testers' own Golems). Until then, suspending a Golem is the switch that works. | B5 at the earliest; R3 in full |
| moderation | Ban a publisher, unlist a listing, plugin reports into investigations. We back recommendation B of the moderation study's question 8, "Kill switches for beta features": each switch ships with its feature. You haven't answered it yet. | R3 |
| Whoever builds invite codes; commission (reputation); the achievements owner | Golem activity earns no invite codes, reputation or EXP unless you rule otherwise | B1 |
| internal (Beaver) | Contract CI over the plugin package (B1); per-major usage telemetry (R3) | B1, R3 |
| Whoever owns domain events, notifications and the broker | Domain events as code, and a delivery backbone (R1); the notification rules' output (R2) | R1, R2 |
| The Seals owner (unassigned) | The Trusted Seal, as Zurfur's label on the publisher's Account DID | R3 |
| compliance | Plugin terms (ban grounds, disclosure duties, reserved branding), and a review of the install copy about disclosed DIDs | R1, R3 |

## Not on the board (proposals, each with why)

1. **A moderator action to suspend a Golem (B1).** Beta Golems have no listing, so the publisher ban can't reach them, and a Golem must be stoppable before it can act.
2. **The plugin design study in the last alpha slot (paper).** B1 alone needs about six rulings. The alpha has no plugin work, so the study costs no build time and the beta doesn't stall on questions.
3. **Beta testers register their own Golems (B5).** It brings real outside pressure on `/plugin/v1` while the surface is pre-GA and breaking it is free. The corpus already lets any User register one.
4. **One shared, guarded web client for addresses users supply.** Webhooks (R1), external-storage mounts (note 2) and identity's resolver all fetch addresses that users supply. Identity already built a guarded client, `GuardedHttp`, on its resolver branch; per its slice description it allows https and port 443 only, refuses IP literals, filters every DNS answer and follows no redirects. Lift it to a shared place when its second user arrives, and review it once. Webhooks may need other ports, which is a policy question for R1.
5. **A developer kit at release:** docs, a local harness that signs and verifies deliveries, and a delivery log showing the dead letters. Third-party developers can't do real work without it.

## Questions for the Engineer (each pre-answered)

**Asked now**

1. **How does a plugin's type show on the wire?** This comes from 0.1, and the TDD's "type catalog" row still doesn't carry it. (a) A reserved name shape, carrying the plugin's own type name. (b) One `type` value for every plugin node, with the plugin's type name in a field only openers see. *Proposed: (b)*, decided with the type catalog. It keeps a plugin from passing for a core type, and a card says only "a plugin's file". It is due before A1, because the alpha publishes the core names. If the catalog slips, the minimum is to reserve both the value `plugin` and a prefix, and pick one at B3.
2. **Should the plugin team carry external-storage mounts (note 2)?** *Proposed: no, not as owner. The den team carries them, and we review.*
   - What's new is a mount whose bytes live outside Zurfur. That means a storage backend behind Atta's host trait, an honest offline state in the tree, and the hash check before serving (The Den, D25). All of that is den's.
   - It isn't a plugin as the corpus defines one. A plugin acts (a Golem) or renders a type, and "nothing else" (Plugin Forms, D1; The Den, D3). A storage adapter would be a third form, and that needs a ruling of its own.
   - You place mounts in the alpha, maybe the beta. The plugin surface doesn't exist before the beta, so an alpha mount can't be a plugin.
   - What we would carry: a review of the adapter seam, so a third party could supply an adapter later, and the threat model where it overlaps our webhook work (addresses users supply, and third-party OAuth tokens Zurfur would hold).
   - *Alternative:* if you want third-party storage adapters from the start, the plugin team owns the adapter's contract, credential and listing, and the den team owns the mount. That needs the third form ruled first.

**Asked before the beta's design study (An), listed now so nothing is lost**

3. **Push or pull in the beta?** (a) Pull only: Golems page the read feed and act through the API, and push lands with signing at R1. (b) Push in the beta, with webhook signing moved forward. *Proposed: (a).* It keeps the board's placement of webhook signing at Release, and nothing is ever sent unsigned.
4. **What credential does the beta's surface use?** *Proposed: the decided token model in full, on one credential per Golem:* exchanged for short-lived scoped tokens, revocation checked on every call, rotation as an admin act. Per-install keys stay with webhook signing at R1, where installs by others and pushes first appear. Auth Surfaces makes this the only plugin credential model, and security doesn't relax with the stage.
5. **Who may register a Golem in the beta?** *Proposed:* the Zurfur team only through B4, since the card says first-party. Beta testers follow from B5, once the suspend action and a registration cap exist.
6. **Is `/plugin/v1` GA in the beta?** *Proposed: no.* It stays pre-GA until R3. The 12-month floor then binds only once third parties publish; pre-GA surfaces get no notice window under the versioning contract.
7. **"Golems & Portals" on the beta card: which Portal?** *Proposed:* the declarative type renderer. The JS/HTML Portal stays parked, as the Den decision says.
8. **How does a plugin render its type?** (a) Zurfur renders a declarative template the plugin ships, so no file leaves Zurfur. (b) Zurfur sends the file to the plugin's host, which returns the view. *Proposed: (a) first.* (b) sends private file content to infrastructure Zurfur doesn't run, on every render and for every viewer. Needed by B3.
9. **Pull the Trello importer into the beta as the first customer?** *Proposed: yes, as B6.*
   - It needs only `act`: no push, no signing.
   - It is a real way in for artists leaving Trello.
   - The Plugin page wants the API proven by a real first-party plugin.

   *Alternative:* keep it at R4, and the beta surface ships without a customer outside the team.
10. **What does an install bind, now that a Golem is a User and Users install plugins?** The corpus says "per plugin and Account" (Plugin Security, S2) and "per Golem" (the Golem page), and the Den decision leaves a User's install open. *Proposed:* a Golem holds its own credential and acts as itself. An install is a binding per plugin and installer, where the installer is a User or an Account, and each binding gets its own key from R1. Needed by B4.
11. **Who holds a Golem's rotation keys?** *Proposed:* one Zurfur-held key per Golem, never shared (did:plc Custody, 26804226, D1). An admin's own key can be added later, through custody's opt-in (D2). Needed by B1, because the first Golem DID is permanent.
12. **Does a Golem get a hosted repo?** The corpus disagrees. Golem as User D2 and the Golem page say no repo; Plugin Security D6's table says a repo on Zurfur's PDS. *Proposed:* no repo in the beta. Its profile is a view Zurfur derives, and a repo can come later. Needed by B1.
13. **Do Golems earn invite codes, reputation or EXP?** *Proposed: no*, until you rule otherwise. Golem as User leaves rating semantics open. Needed by B1.
14. **May Golems join Accounts while "membership confers view" stands?** *Proposed: no.* B4 waits for that MVP-only exception's removal, which is due after the MVP anyway.
15. **Is the second authenticated surface ours?** Peer views, such as a third-party app or an Obsidian vault reading your private Den, need one (the Den decision's accepted tradeoff). *Proposed: yes,* on `/plugin/v1`, as a client whose every call is invoked by its human. Unplaced, after R3.
16. **Offer a design-sync of the plugin pages before the study?** *Proposed: yes, at An, with your go-ahead.* It would fix:
    - the Plugin, Golem and First-party plugins pages, which still place plugins in the MVP;
    - Golem as User calling a Golem "Account-blind", while Plugin Forms gives it Account context through membership;
    - the contradictions behind Questions 10 and 12.
