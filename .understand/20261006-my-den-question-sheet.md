# My Den, read-only — question sheet

2026-10-06 · merged by the Designer for the tech lead (`claude`)

Four teams studied your ask "My Den, read-only" and each wrote a design file: den (lead Badger, QA Owl), identity (Fox, Magpie), commission (Wolf, Jay) and web (Fennec, Crow). The files are `den.md`, `identity.md`, `commission.md` and `web.md` in the session scratchpad under `den-ask/`. This sheet merges their questions without duplicates and cites each item by file and id, so the tech lead can check it. **Write your answer on the `**Engineer:**` line under each question.** "rec" accepts the recommendation; where teams differ, name the team or the option. Section 2 says which questions to answer first, section 3 holds every answer line, and sections 4–8 are for your information.

Citations: "the Den DD" is 2026093001; D1–D28 are its Decisions and "ruling n" its numbered rulings. "den Q1", "identity PM4", "commission Q0" and "web Q7" are the teams' question ids, "den QA 14" is item 14 of a team's QA section, and "board #58" is a board message. Terms: **Atta** is the Den's engine crate. **Standing** is access a viewer holds by who they are at a node (themselves, an Account member, a character's keeper, a commission's Participant). A **lift** is D10's rule that standing or a grant only raises a node's level for that viewer. A **mount** is an entry the system makes in one viewer's Den from their access (D11). A **card** is a Listed node, shown but not openable (D8). The **Class B MAC** is D25's keyed check over a private node's key, type and payload hash.

## 2. Rulings that unblock the most (answer these first)

Each count is the interface rows and questions, across the four files, that wait on the ruling or are carried by it; a row mirrored in two files counts twice. The answer lines are in section 3.

1. **P1, the Den read's contract.** Unblocks 16: web I4, I5, I6, I9, I14, I17, T2, T7; den I-W1, I-W2, I-W3, I-W6, I-W10, I-W11, I-W13, T7. Until you rule, every wire row the leads "agreed" is a proposal (web.md preamble).
2. **P2, the slice's scope.** Unblocks 12: den I-F3, I-F10, I-W-f, I-W14; identity I5, I14, PM5; commission I-5, I-10; web I3, I12, I13.
3. **P3, where Atta sits and how it reaches Zurfur's types for reads.** Unblocks 11: den I-F2, I-W-B4, I-W-g, T1; identity I2, I15, TL1; commission I-14, Q11, T1; web I4. The files contradict each other on routing (section 4, item 1).
4. **P4, the level an entry shows.** Unblocks 10: den I-F4, I-F12, I-W-B5, I-W5; identity I6, TL8; commission I-11; web I5, Q1, T7.
5. **P5, standing covers the real subtree.** Unblocks 9: den I-F2, I-W-B1, I-W-c; identity I3, I11, PM3; commission I-1, I-6, Q0. The tech lead didn't expect it among the blockers; the count puts it here.
6. **P6, the node key, and the MAC binding the real path.** Unblocks 7: den I-F1, I-F13, I-W-B3; identity I1, TL3, PM11; commission I-2.
7. **P7, an in-memory runtime or tests only.** Unblocks 5: den T10, G8; identity PM15, G10; web T1. Fewest items, but it decides whether a dev profile and demo sign-in get built at all, and the files disagree on whose call it is (section 4, item 5).

## 3. All PM questions

Ordered by how much each unblocks, in three groups.

### Blocking the slice

**P1. Will you rule on the Den read's contract, `den.proto`, as one decision before code is generated?**

- _Raised by:_ web Q7 (from web QA 2, 10, 11). Unblocks 16.
- _Hinges on:_ CLAUDE.md (an API contract is yours); 40992770 D1; the Den DD's Open row "Contract impact" ("decided at implementation" says when, not who). The ruling also moves the nine-endpoint pin in `api/tests/contract_routes.rs` (your scope ruling of 2026-07-25) and sets aside, for the Den, the `(session)` group's internal rule "F1" (no fetch-by-id read in v1).
- _Options:_ (a) the leads draft `den.proto` and you rule on it once; (b) you rule field by field; (c) you delegate the shape to the tech lead within the privacy rules (no counts, no hash, the own level only to openers, page tokens without names).
- _Recommendation:_ (a) (web; den co-authors).

**Engineer:** I think (a) is the correct option, since proto grows by version but we have nothing out there yet---so each proto will be v1 until the alpha happens. Everything we do right now is pre v1, so we do not need versioning YET.

**P2. Confirm the slice's scope.**

- _Raised by:_ commission Q0; den Q6 items 1–4; identity PM5 (carried by den Q6). Unblocks 12.
- _Hinges on:_ the ask's "Scope as offered"; D13, D15, D16; ruling 16 and the worked example (`posts` is a mount); the Open rows "Shape of the Commission directory", "Recording 'brought into an Account'", "P/D/T vs. the three hidden levels" and "Placements: workflow payload or links?".
- _Proposed:_ (1) `/user/<did>` shows `accounts`, `characters`, `commissions` and a `posts` mount that lists empty; (2) an Account's `characters` and `commissions` list empty, and `workflows` lists each Workflow with no children (section 4, item 11); (3) a character's `art` is omitted; (4) a commission shows only `attachments`, `changelog`, `products` and `slots`; (5) Participants only: no grants, no bring-in (D21), no payload.
- _Options (commission Q0):_ (a) as proposed; (b) plus view grants (P21, P22); (c) plus rendered boards (P28); (d) as (a) without Workflows, so no folder opens empty.
- _Recommendations:_ commission: (a) with Workflows flagged "content not shown yet" (P9), or (d) if an empty board is unacceptable. Den: confirm every edge, then fix D13's `posts` wording through design-sync. Identity concurs.

**Engineer:** I agree on this, since it's a great skeleton to start on. Since the process will be iterative, we can change this later. Make sure `/user/<did>` has the correct permissions (user can see its own and not others')

**P3. Where does Atta sit among the layers, and how do Zurfur's types answer Atta's reads?**

- _Raised by:_ den Q4; commission Q11 (the layer half, asked twice) and T1; identity TL1 (moved to den Q4). Unblocks 11.
- _Hinges on:_ the Open row "Atta among the layers"; 11763713 (no row for Atta); D4 ("routes each operation to the handler registered for the node's type"), against ruling 34's second condition and the 55836674 banner, which apply that routing to named actions, and 55836674's "No mediator, no handler registry" and D1 ("Dispatch-by-type erases the call graph").
- _Layer options:_ (a) a library below `application`, with Den reads as `#[use_case]` Queries; (b) above `application`; (c) a layer of its own.
- _Routing options:_ (i) one host trait, implemented by an exhaustive `match` over Zurfur's type enum that delegates to each team's module; (ii) a runtime registry of handlers per type, reads included; (iii) separate storage, standing and type traits.
- _Recommendations:_ all teams: layer (a). Routing: den and identity (i) (den 1.2; identity I15 refinement, board #67); commission's file still describes (ii) (I-14; Q11 (a)). See section 4, item 1.

**Engineer:** Atta is a library. It is completely decoupled and should not speak Zurfur's language. It should be consumed by `application`, so `a`

_Follow-up (claude): your answer settles the layer (a) and the drawing conflict: the API never touches Atta, and `application` turns path pieces into Atta's segments. The **routing** half is still open: (i) one host trait plus an exhaustive `match` on Zurfur's side, or (ii) a runtime registry of handlers per type. Which?_ I lean i.

**P4. Which level does a listing entry show: the viewer's resolved level, the node's own, or both?**

- _Raised by:_ den Q2; web Q1; identity I6, TL8; commission I-11. Unblocks 10.
- _Hinges on:_ the ask ("its level"); D8, D10, D24; D25 (the hash goes only to viewers who can open the node). The corpus is silent on serving the own level.
- _Options:_ (a) the resolved level only (card or open); (b) both, to everyone; (c) the resolved level always, plus the own level only to viewers who can open the node. Included: a mount has no level of its own, so an opener sees its target's; a symlink shows its own.
- _Recommendation:_ (c) (all four teams). Under (a) alice's Private commission reads "Public" to her; under (b) a card viewer learns the level set for others.

**Engineer:** I believe c. This is literally what is needed, since it doesn't make sense for everybody else.

**P5. Does a viewer's standing at a node cover its whole real subtree, never crossing a mount or a link?**

- _Raised by:_ den Q3 (narrowed to standing by den QA 2); identity I3, I11; commission I-1, I-6. Identity PM3 (the Account case) was withdrawn as corpus-answered. Unblocks 9.
- _Hinges on:_ ruling 29 ("a participant gets everything"); 3276807 ("a Participant stays at Total regardless of a lesser key"); 29130754 D8; D10, stated per node; D12 (children are born Private); D9.
- _Options:_ (a) yes, along the real path only; (b) no, each node answers alone; (c) yes, along the path the viewer came by.
- _Recommendation:_ (a) (all teams). Without it a Participant can't open `attachments`; (c) lets standing on your own root open everything mounted in it. Grants are left to P22.

**Engineer:** Along the real path only. This one is tedious. `b` remains as a backup (annotate).

**P6. Is your draft `Key(Uuid)` a node id beside the path segment, or should the key be the segment itself?**

- _Raised by:_ den Q1; identity I1, TL3, PM11; commission I-2; web gap 3. Unblocks 7.
- _Hinges on:_ D6 and ruling 15; 57081857 D1 and D4 (its accepted tradeoff lets a surrogate be "re-added additively"); D24 ("the node's key" beside "its real parent"); D25; the Open row "What the Class B MAC covers". Most keys are already answered (corpus list below).
- _Options:_ (a) the key is the segment, unique within its real parent, so a node's identity is its real path and the draft's `Key` changes; (b) a minted UUID beside the segment, which adds a field to D24's closed set and gives each actor node a UUID beside its DID.
- _Consequence of (a):_ the Class B MAC must bind the real path, or a payload replays onto a sibling with the same key and type (every user's `characters`). Under (b) a UUID is unique alone.
- _Recommendation:_ (a) (den, identity, commission).

**Engineer:** `a`

**P7. Must My Den run as a clickable app on seeded in-memory data, or is a slice proven in tests enough?**

- _Raised by:_ identity PM15 (from identity TL5, QA 31); web T1 (filed for the tech lead); den T10. Unblocks 5.
- _Hinges on:_ the ask ("A signed-in User opens My Den in the web app"; data from `adapter-mem`). Today `composition` doesn't wire `adapter-mem`, a real sign-in brings a DID the seed lacks, and `MemAuthenticator` signs everyone in as one fixed DID.
- _Options:_ (a) the web's mock data layer only; (b) a seeded `adapter-mem` dev profile that signs in as fixture alice; (c) both.
- _Recommendation:_ (c) (web; identity concurs; den co-signs that someone must own it).

**Engineer:** `c`

### Shaping this slice (buildable against the recommendation if unanswered)

**P8. Do archived commissions and soft-deleted Accounts show in My Den?**

- _Raised by:_ commission Q10; identity PM8; den I-F9, I-W-i; web I11. Unblocks 5.
- _Hinges on:_ 3276807 ("Archived = soft delete"); 1966081 ("the same treatment as commissions"); D11 (a mount disappears when access ends); 23003138 D2 (soft delete hides the Account, recoverably) and D6 (a reference to it renders "deactivated"), which commission Q10 doesn't cite.
- _Options:_ (a) both stay, marked; (b) both hidden; (c) archived commissions stay, soft-deleted Accounts are hidden.
- _Recommendations:_ commission (c), den agrees. Identity: hidden now, shown to the Owner once a restore action exists.

**Engineer:** Under a filter. Archived commissions stay but are hidden from view. They may be toggled in the UI. Including soft deleted commissions requires a special parameter in the lookup, such as `include-deleted: true`. This is for BOTH things.

_Follow-up (claude): the Commission page (3276807) says "Archived = soft delete". Your answer treats them as two states: archived means listed but hidden behind a UI toggle; soft-deleted appears only with `include-deleted: true`. Are they two states now (the page gets amended), or is archiving the soft delete?_ archiving = soft delete

**P9. Should a node whose content this slice doesn't render say "content not shown yet" instead of looking empty?**

- _Raised by:_ web Q2a; den I-W10; web I13, I14; commission Q0. Unblocks 5.
- _Hinges on:_ the ask (only folders open); D16; D2 (the frontend never decides). The corpus is silent. It applies to a Workflow, an Account's `commissions` and the empty `posts`.
- _Options:_ (a) yes, from a backend flag fixed per type or system folder, never computed from data; (b) the ordinary empty state; (c) leave such nodes out (commission Q0 (d)).
- _Recommendation:_ (a) (web, den, commission).

**Engineer:** `a` is fine

**P10. How are node types named?**

- _Raised by:_ den Q5, I-W7; identity I8; commission I-3. Unblocks 4.
- _Hinges on:_ D2, D3; 45514754 D10 (the type catalog gets its own DD; "Enum vs free text — decide with the catalog"); 29622283 D6 (published names are one-way doors); 10354710 (`app.zurfur.*`).
- _Options:_ (a) NSID-style, `app.zurfur.commission`; (b) bare, `commission`; (c) media-type style.
- _Recommendation:_ provisional names in this slice, opaque to Atta and the web; the catalog DD rules later, with (a) as den's recommendation.

**Engineer:** We can use provisional names in this slice. Otherwise, a display name that is not the key is shown. Each node has a name set by what it holds. A commission node has the name of the commission to display; characters have names; etc.

**P11. Is a commission's title its node `name` (metadata) or a payload field?**

- _Raised by:_ commission Q1, I-8; den I-W-a; web I10. Unblocks 4.
- _Hinges on:_ D6 and D24 (names are metadata) against D15 ("Its own fields (title, deadline, status, maturity) are its payload"); the worked example's `{ name: "Untitled" }`.
- _Options:_ (a) metadata `name` only; (b) both, written in one unit of work; (c) payload only, with `name` something else.
- _Recommendation:_ (a) (commission). Note that `name` sits outside D25's hash and MAC (commission R-3).

**Engineer:** It needs to be part of the `metadata` to render in a UI. `a`

**P12. Is a commission's changelog a file or a folder of entries?**

- _Raised by:_ web Q9, I12; den Q6 item 5. Commission treats it as a file and D15's "four fixed folders" as a wording slip (I-3, I-10, R-5); web's board #58 asking commission to carry it went unanswered. Unblocks 4.
- _Hinges on:_ for a file, ruling 22, D15 ("an append-only file"), and the Open row "Where the changelog sits" ("Resolved by ruling 22"), which no team cited. For a folder, ruling 21 and D15 ("four fixed folders"), banners on 3276807, 589826 and 45514754, and 65110018 (notes are Comments).
- _Options:_ (a) a file whose handler renders the entries; (b) a directory of entry nodes, each with its own level; (c) defer: an entry that doesn't open, ruled before it opens.
- _Recommendations:_ den: confirm (a), then design-sync D15. Commission: (a), as a wording fix. Web: (c) now and (a) later, though (b) fits per-entry visibility.

**Engineer:** A changelog is something we use to derive onto something else (the event view includes stuff from plugins), which means that a changelog is SET and APPEND ONLY. Because of this initial "read", I think the changelog should be a file in interface only. Rather, it should exist in a database in the backend so that pagination works, unless the changelog happens completely on the client side.

**P13. May the slice serve projected nodes before D25's verification exists?**

- _Raised by:_ den Q7 (from den QA 5), I-F5; identity I13. Unblocks 3.
- _Hinges on:_ D25 ("Before serving a node, Atta asks a host trait to verify it"; a mismatch fails closed); the Open row "What the Class B MAC covers" (not the name or level).
- _Options:_ (a) yes: metadata only, D25 recorded as not yet exercised, and a verifier that refuses every payload; (b) first store nodes in `adapter-mem` with a hash and MAC; (c) serve nothing until (b).
- _Recommendation:_ (a) (den). Full D25 wouldn't cover the name, type and level served here.

**Engineer:** `a`

**P14. May the slice project nodes from today's rows, leaving the lasting storage shape open?**

- _Raised by:_ identity PM12 (from identity TL4); den T9, G1 (filed for the tech lead); commission §1 item 1. Unblocks 3.
- _Hinges on:_ the Open row "Metadata storage shape" (lists "today's pattern"); the weighing ("the tree is a read model"); against D7 and D27 (metadata in PostgreSQL, written in one unit of work).
- _Options:_ (a) project for this slice only and settle the shape with the first payload; (b) store node rows now.
- _Recommendation:_ (a) (identity, commission). Den asks the tech lead to confirm the handler interface ties storage to no layout.

**Engineer:** `a`

**P15. May today's reads that bypass Atta stay beside the Den for now?**

- _Raised by:_ commission Q12; identity PM14; den T3 (filed for the tech lead). Unblocks 3.
- _Hinges on:_ D4 ("Nothing reads a node except through Atta"); ruling 34. The reads are `accounts().list`, `users().me`, `commissions().list` and the commission projections. They already disagree with the Den: a non-participant gets 404 for a Listed commission, where the Den shows a card.
- _Options:_ (a) a temporary breach with a named follow-up that has an owner and a deadline; (b) move them behind Atta first.
- _Recommendation:_ (a) (all three teams), with both paths on the security review list.

**Engineer:** `a`

**P16. What is a Workflow node's own level?**

- _Raised by:_ den Q8, I-W-e. Commission reads it from `Workflow.visibility` without asking (§1 table, I-14); den's board #62 to commission went unanswered. Unblocks 3.
- _Hinges on:_ 29130754 D10: `Workflow.visibility` is a creation-time form default with "no ongoing influence"; 9895957 ("A list holds no privacy of its own"); D12; D16.
- _Options:_ (a) born Private like any node, opened by members through the Account's standing; (b) `Workflow.visibility` doubles as its level; (c) a level field of its own.
- _Recommendations:_ den (a); commission's file builds (b). Members are unaffected; an opener's view under P4 (c) changes. Whether a board's level gates its cards is part of P28.

**Engineer:** Agreed on the recommendation.

**P17. What is a User node's `name`?**

- _Raised by:_ identity PM4, I8; den I-F6; web I2. Unblocks 3. Low stakes: the root's breadcrumb reads "My Den".
- _Hinges on:_ D6; the User row holds no name, since handle and display name come from the user's PDS (their atproto data server); the worked example's `{ name: "alice" }`.
- _Options:_ (a) the presented handle (verified both ways, else the DID), as `/me` does; (b) the display name, falling back to (a); (c) the DID.
- _Recommendation:_ (a) (identity, web, den).

**Engineer:** The name is `a`. Display Name > Username > DID.

_Follow-up (claude): you wrote (a), but "Display Name > Username > DID" is option (b): the display name, falling back to the handle, then the DID. Confirm (b)?_ `b` confirmed

**P18. Must the slice show a real card, or is a card built and tested against fixtures enough?**

- _Raised by:_ web Q8, gap 13. Unblocks 1.
- _Hinges on:_ the ask ("A node the viewer can't open but may know about is a card"). Every path in My Den runs under the viewer's own standing, so nothing produces a card (commission "Out of this slice"; identity 1.1 item 6).
- _Options:_ (a) fixtures only; (b) one real source now, such as a Presentation grant or D17's private-character card; (c) drop cards.
- _Recommendation:_ (a) (web), saying up front that the demo shows only openable nodes.

**Engineer:**
`a`

**P19. Where does My Den live in the web app?**

- _Raised by:_ web Q6. Unblocks nothing else.
- _Hinges on:_ the corpus is silent; the web's internal ruling "9b" makes `/` the signed-in landing page.
- _Options:_ (a) `/den` plus a header link; (b) `/` becomes My Den.
- _Recommendation:_ (a) for this slice (web).

**Engineer:** The den IS the web app. I think the den should either be `/` or a subdomain.

_Follow-up (claude): `/` or a subdomain for this slice? Making `/` the Den replaces today's signed-in landing page (the web's internal ruling "9b")._ Let's do subdomain

_Follow-up 2 (claude): does the **whole web app** move to the subdomain (for example `den.zurfur.app`, still one origin, nothing else changes), or does the Den sit on a subdomain **beside** the rest? Beside means sharing the session cookie across subdomains, which widens where it's sent and reopens the CSRF assumptions in "Auth Surfaces" (24543244). Which, and what's the subdomain's name?_ Hm... Good point. I think this changes my answer to `a`

### Before later slices (not blocking My Den)

**P20. What does an Account's `commissions` folder hold?** (commission Q5; web Q4. Unblocks 4; empty in this slice.)

- _Hinges on:_ D13 (contents unnamed); D21; 29130754 D2 (bookmarks are consent-free).
- _Options:_ (a) commissions brought in (D21), mounted for each member; (b) (a) plus external bookmarks as symlinks, which D10 then handles; (c) nothing until after the MVP.
- _Recommendation:_ (b) (commission).

**Engineer:** `b`

**P21. Does a view-grant holder who isn't a Participant get a mount?** (commission Q2; identity PM13, I10; den I-W-d. Unblocks 3.)

- _Hinges on:_ D1 ("Gaining access shows up as new nodes"); Objective 1; the Open row "How access shows up as nodes" ("Revokes stay quiet (29130754 D5)", the grant-revoke clause); against D11's "(keeper, member, participant)".
- _Options:_ (a) yes, at the grant's level; (b) no, only by link or path.
- _Recommendation:_ (a) (commission; identity concurs).

**Engineer:** `a`

**P22. How do Presentation, Description and Total grants map onto the Den?** (commission Q3; den I-W-d, G12. Unblocks 2.)

- _Hinges on:_ the Open row "P/D/T vs. the three hidden levels"; 29130754 D3; 3276807 _Visibility_.
- _Options:_ (a) Presentation gives a card, Description opens the commission node only, Total lifts the subtree like a Participant; (b) every key lifts the subtree, capped at its level; (c) keep `min(tab, surface, element)` inside, the Den formula only at the commission.
- _Recommendation:_ (a) (commission).

**Engineer:** `a`

**P23. How is "brought into an Account" (D21) recorded, and at what level?** (commission Q6. Unblocks 2.)

- _Hinges on:_ D21 ("logged like a grant"); its Open row; 3276807 ("no association table"); 29130754 D3, D4 and the 2026-09-04 amendment ("Accounts hold no keys").
- _Options:_ storage (s1) a commission-side record logged in the changelog, reviving account-held view; (s2) a symlink (breaks D11); (s3) a placement flag (breaks D21). Level (l1) the owner picks; (l2) always Total.
- _Recommendation:_ (s1) with (l1) (commission). Also to rule: Golem members, owner membership, ownership transfer, undo, Account soft delete, Commission Admins, several Accounts.

**Engineer:** Agreed.

**P24. Does a participant's Den show the canonical title or their own custom title?** (commission Q15, R-9; den I-W-a. Unblocks 2.)

- _Hinges on:_ 3276807 _Rules_ (every Participant can have a custom title); D6 (one `name`); 29130754 D7's board alias is separate. Custom titles aren't built.
- _Options:_ (a) the canonical `name`; (b) the viewer's title as an overlay on their own mount.
- _Recommendation:_ (b) once custom titles exist (commission; den agrees).

**Engineer:** `b`, but defaults to the canonical name if none is given. The canonical name is also suggested on addition through a pre-filled input.

**P25. How does a public User-Profile render, when actor directories are born Private?** (identity PM1; den I-F7. Unblocks 2.)

- _Hinges on:_ 21594113 D1–D5 (public profiles and Account rosters); D12, D13; the Open row "Public User-Profiles".
- _Options:_ (a) a view derived from the tree, like the Gallery (D19), each item resolved at the visitor's level; (b) the directory itself, widened.
- _Recommendation:_ (a) (identity). Den asks that P27 be ruled with it.

**Engineer:** `a`

**P26. What fills an Account's `characters` folder?** (identity PM7; den I-F8; web Q4. Unblocks 2; empty in this slice.)

- _Hinges on:_ D13, D14; 54427650 (an Account administers a shared Character); 2026091901 D7; D11 (mounts are per viewer). The corpus is silent.
- _Options:_ (a) mounts for each member from the Account's custody record; (b) the same as cards, opened only by the keeper; (c) symlinks written by a named action.
- _Recommendation:_ (a) (identity).

_Soft answer from your rulings: (a). P20 and P21 say access shows up as mounts for each member or grant holder, so members of an Account get its characters the same way._

**Engineer:** I believe `a`

**P27. Can a board be public while its Account is Private?** (den Q9, I-W-j; commission R-12, as a risk only. Unblocks 2.)

- _Hinges on:_ 9895957 (one board is both the private working board and the public progress board); D9, D12, D13. The Open table doesn't list it.
- _Options:_ (a) a public board widens its Account directory and `workflows` folder; (b) boards are reached by query or link, never by traversal (a new rule); (c) wait until after the MVP.
- _Recommendation:_ den: rule with P25; (a) needs no new rule; add an Open row. Commission gives none.

_Soft answer from your rulings: (b). P25 (a) makes a public profile a view derived from the tree rather than a widened directory; a public board would follow the same pattern and be reached by query or link._

**Engineer:** Yes, `b`

**P28. Are board placements payload or links, do columns keep their visibility, and does a board's or column's level gate its cards?** (commission Q9; identity TL9. Unblocks 2.)

- _Hinges on:_ the Open rows on placements and columns; D16; D9; 9895957 ("A list holds no privacy of its own"); 62488577.
- _Options:_ (a) payload projected per viewer (conflicts with D25; raw payload leaks Private ids); (b) symlinks under column nodes (position, alias and archived state need a home, and every public placement needs a widen); (c) payload never served, only a rendered board.
- _Recommendation:_ (b), amending D16 (commission).

_Soft answer from your rulings: leans (b). P20 (b) already makes external bookmarks symlinks, so placements as symlinks fits. Still open: where a placement's position, alias and archived state live._

**Engineer:** Yup. `b` is what I believe.

**P29. When a symlink opens, where does the viewer land?** (web Q3, I4, I7. Unblocks 2; no symlink exists yet.)

- _Hinges on:_ ruling 9; the Open row "How a link resolves"; Objective 7 (where access is checked, not where navigation lands); D11.
- _Options:_ (a) the target's real path, outside the Den; (b) under the link's own path.
- _Recommendation:_ (a) (web), so a node has one address.

**Engineer:**`b` would be so weird to implement. I think `a` is the one that makes the most sense.

**P30. Do dotfiles appear in a listing?** (web Q5; den I-W9; commission T7. Unblocks 2; none exist yet.)

- _Hinges on:_ D13; D28 (a dotfile is defined by its leading dot). The corpus is silent on listings.
- _Options:_ (a) hidden by default; (b) shown; (c) behind a "show hidden" toggle.
- _Recommendation:_ (c) (web), since hiding by name would let an uploaded `.notes.png` hide from other participants; commission and den instead have uploads refuse a leading dot (section 4, item 12).

**Engineer:** Hidden by default, but behind a show hidden toggle, like a real file system. So `c`.

_Follow-up (claude): should uploads refuse names that start with a dot? Under D28 an uploaded `.theme.yaml` in a commission folder is a real dotfile and would re-theme the commission for every participant. Commission and den recommend refusing; so do I._ Uploading them? Yes. Rather, we should have a special button called "Upload configuration", so that the action becomes explicit.

**P31. May a Character be Listed?** (identity PM2. Unblocks 1.)

- _Hinges on:_ D8; 5668866 and 54427650 (private or publicized, a one-way door); 29622283 D5 (no quiet hiding of Class A); D20.
- _Options:_ (a) Private or Public only; (b) Listed as Index-only, with no repo; (c) Listed with a repo.
- _Recommendation:_ (a) for the alpha (identity).

**Engineer:** Private is fine. I think both are going to be available on development because of visibility, so `a` works right now.

**P32. What does a Listed card of an Account or Character show?** (identity PM9. Unblocks 1; needed before slots show characters.)

- _Hinges on:_ D23; no clause fixes actor cards; 3276807's commission card as precedent.
- _Options:_ (a) name, type and effective level; (b) plus an Account's handle; (c) richer (member count, species).
- _Recommendation:_ (a) now (identity); never the keeper or members.

_Soft answer from your rulings: (a). P10 and P17 give every node a display name from what it holds, and P4 (c) gives card viewers only the resolved level._

**Engineer:** `a`

**P33. What does a Listed commission's card carry?** (commission Q4; web gap 2. Unblocks 1.)

- _Hinges on:_ D23; 3276807 ("display title/alias, stage, position, maturity"; never brief, client, price or files), where stage and position are board terms.
- _Options:_ (a) as written: off a board, title and maturity; (b) plus lifecycle step and Status; (c) plus the deadline.
- _Recommendation:_ (a) (commission).

_Soft answer from your rulings: partly (a). The title is the node's `name` (P11). Whether maturity shows on the card waits on P36._

**Engineer:** I believe `a`.

**P34. Where do a commission's participants and roles live in the node model?** (commission Q7. Unblocks 1.)

- _Hinges on:_ D15; 9895957 and 3276807 (client identity is Total-only); D23; the Open row "Shape of the Commission directory".
- _Options:_ (a) in the payload, visible to strangers on a Public commission; (b) a fifth fixed entry, `participants`, born Private; (c) standing data only.
- _Recommendation:_ (b), amending D15 (commission).

**Engineer:** `b` seems fair to me.

**P35. Which payload fields may a stranger see when opening a Public commission?** (commission Q8, R-2. Unblocks 1.)

- _Hinges on:_ D23, D25 (the whole payload goes to every opener); 3276807 (Public shows Description-visible content); today's row holds `linked_channel` and `owner_id`.
- _Options:_ (a) only stranger-safe fields, with the rest as child nodes; (b) a per-viewer projection, which breaks D25 if served as the payload.
- _Recommendation:_ (a) (commission).

**Engineer:** `a`

**P36. Does the maturity gate apply to Den entries, your own included?** (commission Q13. Unblocks 1.)

- _Hinges on:_ 21594113 D10; D23 (listings read metadata; maturity is payload); D26.
- _Options:_ (a) the handler applies it; (b) maturity becomes an attribute; (c) no gate on your own Den.
- _Recommendation:_ (c) now, (a) later (commission, as its own reading).

**Engineer:** c -> a

**P37. Where does a profile record sit in the tree?** (identity PM1b. Unblocks 1.)

- _Hinges on:_ 29622283 D1 (profiles are Class A); D20; the Open row "Other Class A records".
- _Options:_ (a) a new always-public top-level directory, amending D5; (b) no node, breaking Objective 1.
- _Recommendation:_ (a), ruled with "Other Class A records" (identity).

_Soft answer from your rulings: (a). A derived public view (P25 (a)) needs an always-public home, as the Gallery derives from `/post`._

**Engineer:** Agreed on `a`

**P38. What does a keeper see for a tombstoned Character?** (identity PM16; den I-F9. Unblocks 1; moot while delete is `todo!()`.)

- _Hinges on:_ 2026091901 D3 ("the DID and the name, and nothing else"); D11.
- _Options:_ (a) the mount disappears; (b) a name-only entry marked deleted.
- _Recommendation:_ (a) (identity).

_Soft answer from your rulings: leans hidden by default, with a name-only entry shown under `include-deleted` (P8's pattern). But a tombstone is permanent, not a soft delete, so confirm._

**Engineer:** Yes. Hidden by default.

**P39. Does a pending seat invitation or an application give a mount or a card?** (commission Q14.)

- _Hinges on:_ 3276807 (participants "may be ... simply invited") against _Seats & Slots_ (a Seat fills on acceptance); D11.
- _Options:_ (a) no mount; notifications announce it; (b) a card until they answer.
- _Recommendation:_ (a) (commission).

**Engineer:** No! A person needs to be part. We can add the "Accept" on a card later, but it doesn't make sense right now.

**P40. Is the user-held owner key per Character, and is a shared Character's Account key per Character?** (identity PM10; open as ZMVP-52.)

- _Hinges on:_ 57081857 D2; 54427650; the did:plc spec (reused keys "could reveal non-public identity details or relationships"; checked by identity 2026-10-06).
- _Options:_ (a) per Character for both; (b) per keeper and per Account.
- _Recommendation:_ (a) (identity), or the public PLC log ties a keeper's Private characters together.

**Engineer:** This is the main reason why users used to have one exact account before. Commissions would be part of an account again (albeit personal). I am willing to talk about users having exactly one personal account they cannot delete.

_Follow-up (claude): this doesn't settle the keys. The risk is that reused keys on the public PLC log tie a keeper's Private characters together, whatever account they sit under. Keep (a), per-Character keys, for now, and take "one undeletable personal Account per user" as its own design decision after this slice? (It would reverse 26247170's "no default account".)_ Alright, I agree with `a` then.

### Answered by the corpus (veto any)

1. **Any Account role is standing on the Account's subtree** (identity PM3, withdrawn after identity QA 9): 29130754 D8; the Den banner on 1966081. The general rule stays P5.
2. **A Private actor is absent from Zurfur's tree, not nonexistent** (identity PM6, withdrawn after identity QA 27): 57081857 D2; 29622283 D1, D5; 26607618; the atproto handle spec. Only UI copy remains (identity G4).
3. **Slot, file and Workflow names are metadata `name`** (commission Q1, narrowed after commission QA 16): ruling 23 as amended by D24; D24.
4. **Actors are keyed by DID, commissions by UUID, posts by AT-URI parts, blobs by CID** (den Q1, after den QA 22): D6; ruling 15; 57081857 D1, D4.
5. **A commission file is keyed by its opaque `FileKey`, never its name or CID** (den Q1; commission I-2): D6 (names don't address); D25 (a CID would show card viewers the hash); D18 (uploads must look the same whether or not the bytes exist).
6. **Only folders open in this slice** (web Q2, after web QA 19): the ask.
7. **Every mount's target sits directly under a top-level directory** (an earlier den question, moved to den T8 after den QA 27): rulings 16, 19, 20 and F1. Only the guard remains, for the tech lead.

**Engineer (name any you veto):**

## 4. Contradictions between the team files

Items 1–8 and 12 fold into PM questions; items 9–11 and 13 go to the tech lead.

1. **How reads reach Zurfur's types (P3).** Den (1.2, I-W-g, Q4) and identity (I15 refinement, board #67): one host trait, an exhaustive `match`, no runtime registry, citing 55836674. Commission (§1 item 5, I-14, Q11 (a), T1): "Routing by folder type, inside Atta (D4). There is no Zurfur-side dispatcher", with handlers the composition root registers; it never mentions #67. Both cite D4. Older wording survives in identity I2–I4 (separate `NodeSource`, `Access`, `MountSource`, `TypeRegistry`) and web I14 ("set by the type registry").
2. **The changelog (P12).** Den Q6 asks you to confirm "file"; commission (I-3, I-10, R-5) calls D15's "four fixed folders" a slip needing no ruling; web (I12, Q9) calls it a live fork and recommends deferring.
3. **A Workflow's level (P16).** Den (Q8, I-W-e): ask, recommending Private. Commission's §1 table and I-14: `Workflow.visibility`; commission QA 8 moved the meaning into Q9 but the table stayed.
4. **Unanswered board messages to commission (P8, P12, P16, P24, P27).** Board #58 (Fennec: carry the changelog question) isn't mentioned in commission.md, which keeps R-5. Board #62 (Badger, four points) isn't either: the Workflow level is unchanged; the custom title became commission Q15 through its own QA, but den's resolution table (row 19) says the title and the board alias were added to commission Q1, which has neither; the public board stays risk R-12; 23003138 D6 wasn't added to Q10.
5. **Whose call the in-memory runtime is (P7).** Identity PM15 says web T1 carries it to you as scope. Web T1 is filed for the tech lead, and web QA 25 says the ask already answers "clickable". Den T10 asks only who owns it.
6. **Whose call bypassing reads is (P15).** Den T3 files it for the tech lead and says identity TL2 and commission T2 ask the same; both moved it to you (PM14, Q12), since D4 is decided.
7. **Whose call projection is (P14).** Identity PM12 asks you; den T9 asks the tech lead and makes it yours only if the interface isn't layout-neutral.
8. **What D25 means here (P13).** Identity I13: no payload is served, "so nothing needs verifying", for the tech lead. Den after den QA 5: D25 covers serving any node, nothing _can_ be verified, and deferring it is yours (den Q7).
9. **Timing in the uniform not-found (security finding 7).** Den T5 and G6 put timing in the review; identity TL11 and G3 hold it as later hardening; web gap 5 leaves it to the backend.
10. **Listing order.** Identity G13: system folders first, then name, then segment. Den I-W2 and web I5 (board #59): name, then segment, no special case.
11. **Whether a Workflow opens.** Commission Q0 (a): "listed but not opened". Commission §1 item 4, den 1.5 and web I13: it opens and lists no children.
12. **Dotfiles by type (P30).** Web Q5 says commission T7 proposes typing dotfiles; commission T7's final text keeps D28's by-name rule and declines to ask.
13. **Stale pointers.** Den Q6 says identity PM5 asks who owns `/post` (that moved to identity TL10); web points `posts` at identity PM5 (carried by den Q6); den T3 cites identity TL2 and commission T2 (both moved); den I-F9 says commission Q10 cites 23003138 D6 (it doesn't).

## 5. Security findings for the security review

1. **Web path traversal.** SvelteKit decodes `%2F` in a route parameter and `fetch`'s URL parser resolves `..` and `%2e%2e`, so `/den/..%2Faccounts` would call `/api/v1/accounts` with the viewer's cookie before Atta's segment parser runs; the web must refuse empty, `.` and `..` segments first (web I9, T8, QA 8, verified on Node v25.6.0 and SvelteKit 2.70.1; den I-W6).
2. **The same shape today.** No `[...rest]` route exists yet. The account delete (`frontend/web/src/lib/server/api/zurfur-api.ts:341`) uses `encodeURIComponent`, which leaves a bare `..` intact, and `/api/v1/accounts/..` resolves to `/api/v1/` (my check in Node). Browsers normalize such URLs, so I found no cross-user path; the review should confirm (web T8).
3. **Split the raw path before decoding**, because axum decodes `%2F` inside a wildcard capture (web I9; den I-W6).
4. **Absolute hrefs only**: a relative href to a `did:` segment parses as a `did:` URL (web §1 item 4, QA 8).
5. **The Class B MAC must bind the real path**, or a payload replays onto a same-key, same-type sibling; the name and level are also outside it (den Q1, I-F13; identity PM11; commission R-3).
6. **A Workflow payload names Private commissions** and must never be served raw; a commission payload goes whole to every opener, including `linked_channel` and `owner_id` (commission R-1, R-2, I-12; den I-W-B6; identity TL9).
7. **Uniform not-found** for absent, nonexistent, unopenable-ancestor, failed-parse, unknown-DID and another user's root: one code path, status and body; a Listed node answers with its card. Timing is disputed (section 4, item 9) (den I-W3, T5; identity G3, TL11; web I6; commission R-11).
8. **Pagination after filtering**, never a count, and tokens without names (den 1.3, G11, QA 30; web I17).
9. **Name validation.** `NodeName` validates nothing; `FileName` accepts a leading dot, so an uploaded `.theme.yaml` would cascade under D28; titles accept control characters (commission T7, QA 33; den I-W-h; web gap 12). Additional: `FileName` uses `char::is_control`, which matches Unicode category Cc only, while bidi controls such as U+202E and U+2066 are Cf, so they pass today (checked in the Rust 1.94.1 core docs and Python `unicodedata` 16.0.0).
10. **No `Cache-Control` on signed-in pages**, so `(session)/accounts` already ships cacheable (web T9, QA 29).
11. **Den paths in logs** pair a User with Private-character DIDs; log route templates only (web T4, QA 27; identity G3).
12. **Session principal only**: standing, mounts and the root never come from a DID in the path (identity G3; den 1.2; web I1; 55836674 D7).
13. **Standing never crosses a mount or link**, with a named test (commission I-1, QA 2; den Q3).
14. **Two authorization paths** until reads converge, already disagreeing on Listed commissions (den T3; commission R-6, Q12).
15. **Top-level directories never list** (D5), against enumeration (identity TL11, G3).
16. **Card bodies** could leak a field later; cards never carry keepers or members (den G3; identity 1.3).
17. **Owner-key reuse** would correlate Characters through the public PLC log (identity PM10).
18. **A fixed-identity dev sign-in.** Under P7 (b), `MemAuthenticator` signs everyone in as alice (`backend/crates/adapter-mem/src/lib.rs:828`), so that profile must be unreachable in a deployed build (my addition, from web T1).
19. **Fail-closed defaults to confirm**: a segment naming both a child and a mount, an entry with no access case, and the verifier refusing every payload (den 1.3, T2; web §1 item 5).

## 6. Bugs found outside the slice

1. **`Commission::is_archived` is inverted**: it returns `archived_at.is_none()` (`backend/crates/domain/src/elements/commission/entity.rs:91`; no callers, rechecked) (commission T4).
2. **`Account.id` drift**: `contract/zurfur/api/v1/account.proto` still declares `string id` (lines 25, 57, 69, 100, 108) and `/api/v1/accounts/{id}` (134, 143), filled with the DID in `backend/crates/api/src/routes/accounts.rs` (99, 116, 140), though 57081857 D1 and D8 removed it; `frontend/web/src/lib/types/brand.ts:34` still calls `AccountId` "UUIDv7, minted by Postgres" (identity TL6; web T5, QA 28).
3. **Signed-in pages lack `Cache-Control: no-store`** (security finding 10; web T9).
4. **`ProfileSource::fetch` says "The handle always resolves"** (`backend/crates/domain/src/ports/profile.rs:10`), but the adapter presents the DID for an unverified handle (identity QA 25).
5. **DID-less docs survive** on `ActorIdentityWrites::create` (`backend/crates/domain/src/ports/actor_identity.rs:29`) and the `ActorIdentity` constructor, though 57081857 retired that path (identity G6).
6. **Name types are under-validated** (security finding 9; commission T7).
7. **Known code gaps, not bugs**: `Presence::Public { handle }` against handle-less characters; `CharacterStore` has no reads and the Postgres adapter is `unimplemented!`; `Characters::delete` is `todo!()`; no participant listing or slot read port (identity G6; den G7; commission §1 item 6).

## 7. Tech-lead questions (Claude answers these)

1. Spell Atta's host trait: associated types or opaque values, `async-trait`, and `Send` or `?Send` for the browser build (den T1, QA 28).
2. The in-memory verifier refuses every payload (den T2; identity I13).
3. The follow-up that routes or retires bypassing reads, after P15 (den T3; commission T2).
4. Cap values: write depth (8), Den-path length (16), segment length and `NodeName` length, passed in by the host (den T4, QA 4, 31).
5. The security review's checklist and timing, including whether not-found timing is a gate (den T5; identity TL11; commission T7; web T3).
6. Golden responses as contract-shaped fixtures under 40992770 D1 (den T6; web I16).
7. Ownership of the two route bindings, the `contract_routes.rs` wildcard, and a page token from the start (den T7; web T2, I17).
8. A guard on mount targets (den T8).
9. The handler interface is layout-neutral (den T9).
10. Who owns an `adapter-mem` runtime profile, after P7 (den T10; web T1).
11. The `application` to `atta` edge, with handlers per team in `application`, after P3 (commission T1).
12. New reads implemented in both adapters (commission T3).
13. A metadata-only `FileStore` read for file names (commission T5).
14. Projected nodes without a stored level are `Private` (commission T6).
15. `NodeName` validation, tightening titles and `FileName`, and uploads refusing a leading dot (commission T7; den I-W-h).
16. Not-found as a 404 or the house's 200 with a problem body (web T6).
17. The entry wire shape `oneof access { open { own_level } | card {} }`, unset rendering as a card (web T7).
18. The web's dot-segment guard as an agreed exception (web T8).
19. `Cache-Control: private, no-store` on the `(session)` group, now or first as its own fix (web T9).
20. Logs carry route templates, never Den paths or tokens (web T4).
21. When `Account.id` is removed (identity TL6; web T5).
22. Whether the fixture's repo-less Public characters stay (identity TL7).
23. Who owns `/post` and `/blob` (identity TL10).
24. A Workflow opens empty and serves no payload; fix commission Q0's wording (identity TL9; commission I-12; section 4, item 11).
25. Align identity G13's listing order with den and web (section 4, item 10).

## 8. Process lessons from the round

1. Run QA after the interfaces settle: commission's QA found four items already resolved by its lead's later revision, and den's draft cited an interface id that had been renamed (den QA 8).
2. Use one status vocabulary, "agreed", "agreed, pending Qn" and "open": two leads agreeing never settles a domain question (den QA 14; web's preamble; commission still writes plain "agreed").
3. Keep "asked once", but the carrier confirms on the board and citing files use the carrier's final id; this round duplicated den Q4 as commission Q11 and left stale pointers (section 4, items 5 and 13).
4. Leads propose the API contract; the Engineer approves the `.proto` as one ruling before codegen (web Q7).
5. Agents can wait only in short `sleep`s, so board #58 and #62 closed unanswered after about five minutes; re-check the inbox before closing a file, and an unanswered request stays with the asker, who says so.
6. Concurrent writes (observed by the tech lead): a lead and their QA shouldn't edit one file at once; QA appends after the lead's pass, or the writer takes the board lock.
7. Scratch files (observed by the tech lead): keep working files inside the round's folder or the session scratchpad, and delete scratch at hand-off.
8. After a board agreement changes, search your own file for the old wording: commission I-14 predates board #67, identity G13 predates board #59.
9. Stamp dates from the clock: identity's resolution pass is dated 2026-10-07.
10. QA that opens every cited page pays off: it caught citation slips in every file.

## 9. Tech-lead answers (claude, 2026-10-07)

These are implementation calls inside your rulings. Anything that turned out to be a domain question is marked **→ Engineer**. Overturn any of them in review.

### The four contradictions that were mine

- **9. Timing in the uniform not-found.**
  - The gate for this slice is a **uniform shape**: absent, nonexistent and "inside a node you can't open" all return the same status, body and headers.
  - Equal _timing_ is not a gate yet. Over `adapter-mem` the differences are negligible.
  - It goes on the security-review list as a risk to re-test when the Den first reads from PostgreSQL.
- **10. Listing order.** Name, then segment, with no special case for system folders (den and web, board #59). Identity aligns. The web may group system folders visually by type, since that's presentation, not order.
- **11. Whether a Workflow opens.** It **opens**, lists no children, and carries P9's "content not shown yet" flag. That follows from P2, P9 and P16 (a). Commission Q0's "listed but not opened" wording is retired.
- **13. Stale pointers.** From now on, the sheet's P numbers are the only references. The team files are scratch and are not corrected.

### The 25 questions

1. **Atta's host trait.**
   - It is object-safe and used through `&dyn`, following 55836674's "dynamic dispatch over generics". It uses Atta's own value types (`Segment`, `NodeName`, `Level`, `Metadata`, an opaque type key) and never Zurfur types. It is async through `async-trait`, and its futures are `Send`.
   - The later browser build (`?Send`) is not designed for now. When it comes, it is a `cfg` variant.
2. **Verifier.** Yes. `adapter-mem`'s verifier refuses every payload, so serving a payload fails closed until D25 lands (P13 (a)).
3. **Bypassing reads (P15).**
   - The follow-up is owned by identity (`accounts().list`, `users().me`) and commission (`commissions().list` and its projections).
   - It is due before the first Den slice that reads from PostgreSQL. It gets its own ledger entry.
4. **Caps.**
   - The host passes them in. Read path ≤ 16 segments; write depth ≤ 8, to be used once writes exist.
   - A segment's cap must fit the longest key D6 allows. That's a DID; the atproto specs bound handles and DIDs, and the den lead verifies and cites the exact number at build time.
   - A `NodeName` is capped for display; the den lead proposes the number with its source.
5. **Security review.**
   - **Before code:** a short `/security-review` on the path-segment design (web guard, API raw split, Atta's `Segment` parser), because that's where the traversal finding lives.
   - **At the end of the slice:** a full review (`designer`, max) with section 5's 19 findings as the checklist, plus the bypassing reads and the existing account-delete traversal shape.
   - Not-found timing per item 9.
6. **Golden responses.** Generated from the `adapter-mem` fixture world and serialized through the proto types (canonical JSON), committed, and consumed by the web's tests.
   - Card fixtures can't be recorded, since nothing produces a card (P18 (a)). They are hand-written but still built and validated through the proto types, and marked as hand-written.
7. **Routes and the page token.**
   - The Den route group (`api/src/routes/den/**`) and `application`'s den module become den-owned. I'll add both to the den team's charter.
   - The `contract_routes.rs` pin moves in the same PR as `den.proto` (P1).
   - Page tokens are in from the start: opaque, carrying no names.
8. **Mount guard.** Yes. Atta refuses a mount whose target isn't directly under a top-level directory, and fails closed.
9. **Layout-neutral.** Yes. The host trait speaks Atta's node model, never storage rows. Projection from today's rows lives in each team's module (P14 (a)).
10. **The `adapter-mem` dev profile (P7 (c)).**
    - I own it, on shared ground, built by a Spawn. Identity seeds the fixture world; the web keeps its mock layer.
    - Hard rule: the profile is behind a Cargo feature and refuses to start unless the environment is `dev`. It signs everyone in as alice, so it must be impossible in a deployed build.
11. **The `application` → `atta` edge (P3 (a), (i)).**
    - `application` depends on `atta`. The single host implementation and its exhaustive `match` live in `application`'s den module.
    - Each branch calls a function in the owning team's `application` module.
    - The API never imports `atta`; it hands raw segments to the den use case.
12. **New reads go into both adapters.** A port method has to compile in `adapter-pg` too, and stubs aren't allowed. The PostgreSQL reads are simple, and parity keeps both tested.
13. **File names.** Yes. A metadata-only `FileStore` read (key and name, no bytes), in both adapters.
14. **No stored level means `Private`.** Yes, fail closed, except where a rule sets the level: D5's top level, and the user root under D13.
15. **Names.**
    - `NodeName`, `FileName` and titles refuse control characters _and_ format characters. That includes the bidi overrides the merge found (Unicode category Cf, not only Cc). They also refuse empty names and `/`.
    - Ordinary uploads refuse a leading dot. Dotfiles come only through the explicit "Upload configuration" action (P30).
    - Owners: den for `NodeName`, commission for `FileName` and titles.
16. **Not-found.** A 404 with a problem document, per 23592962 (HTTP status plus RFC 9457 errors). It has one type, the same for every case.
17. **Entry wire shape.** I endorse `oneof access { open { own_level } | card {} }` for the leads' `den.proto` draft. An unset field renders as a card, which fails closed. It's yours to rule once, under P1.
18. **The web's dot-segment guard.** Yes. It's input sanitizing, not a visibility decision, so it doesn't break D2.
19. **`Cache-Control: private, no-store` on the `(session)` group.** **Now, as its own small fix before the slice.** It also covers today's account pages. Owner: web.
20. **Logs** carry route templates only, never Den paths, segments or page tokens, because paths contain DIDs and names.
21. **`Account.id` drift.** It's removed in a separate identity cleanup after the slice, not inside it.
22. **Fixture characters.** A Public character is shown only if the in-memory fake models its repo (54427650); otherwise the fixture makes it Private.
23. **Who owns `/post` and `/blob`.** No team owns them yet, and the slice only shows an empty `posts` mount. **→ Engineer:** a Gallery or posts team is needed before the Gallery is built. That's a team-structure call. **Resolved 2026-10-07: the Engineer created the posts team (lead Bowerbird), owning `/post`, `/blob` and the Gallery (PR #231).**
24. **A Workflow** opens empty and serves no payload, per item 11.
25. **Listing order** is aligned, per item 10.

### Bugs found outside the slice (section 6)

Each goes to its owning team as a small separate PR after the slice, except the `Cache-Control` fix, which goes first (19). `Commission::is_archived` is inverted and has no callers (verified 2026-10-06); it goes to commission.

## 10. Follow-ups found by redrawing the diagrams (2026-10-07)

### For the Engineer

**P41. Does P8's filter hide an archived commission everywhere, or only in listings?** (architecture §6 item 15)

- If the filter runs everywhere, alice's own archived commission at a typed `/den/commissions/<uuid>` returns the uniform not-found until she turns on the toggle.
- _Options:_ (a) listings only: a direct path still opens it, marked archived; (b) everywhere, until `include-deleted` is set.
- _Recommendation (claude):_ (a). The toggle is about clutter. A participant who has the path still has standing, and "not found" for your own commission reads as data loss.

**Engineer:** I'd say `a`, since it depends on each user's.

**P42. Who may turn on `include-deleted` for soft-deleted Accounts, and does an included Account open?** (architecture §6 item 16)

- P8 sets no role limit. Today's account reads drop soft-deleted rows, so this needs a new read.
- _Options:_ (a) the Owner only, and the Account opens read-only; (b) any member, as a card; (c) any member, and it opens.
- _Recommendation (claude):_ (a). Recovering a soft-deleted Account is the Owner's act under 23003138, and members of a deleted Account have lost the access a mount reflects (D11).

**Engineer:** `include-deleted` is only local to the user currently checking (and dynamic, not persisted).

_Follow-up (claude): so the flag is a per-request view option that any viewer may set, and (C6) it never shows anything they couldn't otherwise open. One point remains: while a soft-deleted Account can still be restored, do its **members** keep their access, so they see it with the flag on, or does only its **Owner**? Rec: the Owner only. Under 23003138 a soft delete hides the Account, and restoring it is the Owner's act._ Owner only. This may be prompt to change in the future. `include-deleted` is a query param.

### Recorded for later slices (no answer needed now)

- **P34's `participants` entry:** whether it's a file or a folder, and what it holds. Each participant's root opens only to its own user, so mounts of user roots would show nothing (§6 item 14). This slice keeps P2's four entries; `participants` arrives with the slice that builds it.
- **P26 against P5 (a):** an Account's shared Character arrives as a mount, and standing stops at a mount. So members need a lift on the character node itself (§6 item 18). The folder is empty in this slice.
- **P38:** a tombstone shows as a name-only entry under `include-deleted`, as confirmed. It's moot while character delete is `todo!()`.

### Tech-lead answers (claude)

- **Display names that fail `NodeName` (§6 item 19):** fall back along P17 (b)'s chain to the handle, then the DID. Never clean the name up, and never fail the read.
- **The Den page's status (§6 item 20):** follow the web's existing convention, as in `accounts/[id]`, which renders a problem state in the page rather than throwing. What security needs is that every not-found case looks the same, not any particular status code. The API stays 404 (9.16).
- **"Show hidden" (§6 item 21):** the backend filters dotfiles unless a `show-hidden` request flag is set, and the web toggle sets that flag (D2). It's moot in this slice, since nothing seeds a dotfile.
- **One Den in the dev profile (§6 item 22):** the dev profile gets a dev-only "sign in as" picker over the fixture users, so bob's Den can be demoed too. It's behind the same feature gate.
- **Who seeds (§6 item 9):** each team seeds its own entities: identity the actors, commission the commissions and Workflows (including the missing Workflow, §6 item 7). This corrects 9.10's "identity seeds the fixture world".

## 11. Questions from the corpus draft (zurfur-design PR #46, draft)

The draft records your rulings as a new decision page, 2026100701 (D1–D20), and amends 19 pages. These are the places where the drafter needed your word to write it down faithfully. Each has my recommendation.

**C1. (P6)** The draft records "the Class B MAC binds the real path" as part of (a), since the sheet gave it as (a)'s consequence. _Rec:_ confirm.

**Engineer:** Confirm

**C2. (P21)** A grant holder's mount goes in their `commissions` folder. _Rec:_ confirm. That's where a participant's mounts already live.

**Engineer:** Confirm

**C3. (P23)** "Brought into an Account" (s1) gives view to the Account's members, but the corpus says "Accounts hold no keys" (29130754, as amended). _Rec:_ make it an MVP exception for D21 only. The record isn't a key, but it confers view, and the "no keys" line gets a note saying so.

**Engineer:** Alright, MVP gets an exception. But it is a big FIXME.

**C4. (P27)** Is a Public board inside a Private Account an exception to traversal (D9), or a derived view? _Rec:_ a derived view, the same pattern as your P25 and P27. Members still reach boards through their Den; others reach a Public board only by query or link. That's how the draft words it.

**Engineer:** Let's treat boards as `/board` rather than `/account/board`. This implies an account gets a `symlink` or a `mount` and it keeps it's private status. ... However, this is functionally optimal, but semantically strange. Let's do it this way anyways, but it doesn't make sense to have a public board to a private account; it is too many gears moving. Let's use `/account/board` for the MVP and change it to `/board` if needed. A board being public can only be seen if the account that owns it is public (this doesn't apply to commissions in the same way)

**C5. (P28)** Placements as symlinks under column nodes means columns are nodes with their own level, which caps their placements. So the Workflow page's "a list holds no privacy of its own" is struck. _Rec:_ confirm, since it follows from columns being nodes.

**Engineer:** Columns are nodes. Confirm. They just happen to be more user-configurable (dotfile)

**C6. (P8)** `include-deleted` is only a filter: it never shows a viewer anything they couldn't otherwise open. _Rec:_ confirm. P42 decides who may use it for Accounts.

**Engineer:** Confirm.

**C7. (P38)** Under `include-deleted`, a tombstoned Character shows as a name-only entry. That's the second half of the soft answer you said yes to. _Rec:_ confirm.

**Engineer:** Confirm.

**C8. (P12)** "Derive onto something else" is read as the Eventlog composing the changelog stream, so 59310081 is unchanged, and "unless client-side" isn't recorded. _Rec:_ confirm.

**Engineer:** Confirm.

**C9. (P34)** Is `participants` a folder or a file? _Rec:_ a **file**, born Private, whose handler renders the participants at the viewer's standing, like the changelog. A folder of mounts to each participant's root would list nothing, because every user root opens only to its own user. This is for a later slice; the draft leaves it as an Open row.

**Engineer:** A file. Eventually, we could even discuss making these kind of things into dotfiles post-MVP.

**C10. (P37)** Name the profile directory, and confirm it's Public, system-owned and traverse-only like the other top-level directories. _Rec:_ `/profile`.

**Engineer:** Rule over here is to match what bsky does. `/profile` is the correct resource path.

**C11.** Keep my name-validation rule (9.15: control and format characters refused, including bidi overrides) in the corpus as D19? _Rec:_ yes. It's a security invariant, not just a code style.

**Engineer:** Yup.

**C12.** Which other section-9 rules belong in the corpus? _Rec:_ the security invariants go in the corpus: uniform not-found, "no stored level means Private", the mount-target guard, and page tokens that carry no names. Listing order and the logging rule stay as code conventions in `NODE.json`.

**Engineer:** I agree with the rec.

**C13. (P1)** "v1 until the alpha" against the corpus's "until the first published client" (40992770 R9): are those the same moment? _Rec:_ yes. The alpha is the first published client.

**Engineer:** Yes.

**C14.** How is a plugin's own type (D3) reached without a registry? _Rec:_ record it as an Open row with a direction. When plugin types arrive (after the MVP), the `match` gets one branch that hands off to a registry scoped to plugins, and core types stay checked at compile time. This is what I suggested for P3.

**Engineer:** Yes, what I am guessing you are saying is to have something like `Plugin(String)` or `Plugin(PluginType)` where `struct PluginType(String);` (for example).

## 12. The path-handling security review (pre-code, 2026-10-07): tech-lead decisions

Full review, experiments and about 70 test cases: session scratchpad `sec-paths/review.md`. The experiments ran against the installed SvelteKit 2.70.1, Node 25.6, axum 0.8.9 and matchit 0.8.4.

**Verdict:** the core design is sound. Atta checks the level at every node for the session's viewer, so reaching another *node* has real defence in depth. Reaching another *endpoint* rests on the web guard alone, because `fetch` resolves the URL before any API code runs. Four design changes are adopted, all inside existing rulings:
1. **One web-owned path builder.**
   - It refuses empty, `.` and `..` pieces and more than 16 pieces.
   - It encodes each piece, then checks that the parsed pathname equals the built string.
   - It is used only in `+page.server.ts`, and it also builds the hrefs.
   - Query strings come from an allowlist, never the incoming search string.
   - The response is decoded with its content `oneof` required.
   - The shared account-delete call is hardened with the same builder. It isn't exploitable today (four independent reasons, confirmed).
2. **Uniform not-found as a mechanism, not a hope.**
   - One problem type and one `NotFound` variant in the den Query.
   - No per-team error mapping in the den route, and no `Path` or `RawPathParams` extractors.
   - An explicit `/den/` route; the den routes sit inside the cookie surface; authentication comes before parsing.
   - No `todo!`, `unimplemented!`, `panic!` or `expect` on host arms.
   - Mounts are matched against the viewer's own mount set, never by looking up the requested target first.
3. **Logs.** A SvelteKit `handleError` that logs only the route id; the den contract errors carry the template path, never the real one.
4. **`Segment` is an ASCII allowlist,** `[A-Za-z0-9._:%~-]`, 1 to 2048 bytes, refusing `.` and `..`. This encodes D6 and changes no rule about what a key may be: DIDs, UUIDs, record keys, CIDs, file keys and the fixed folder names are all ASCII. It also closes the Unicode look-alikes of `..` and `/` (NFKC). The API's raw split decodes each piece exactly once and strictly, and caps the piece count before allocating.
5. **Page tokens:** a version byte plus an offset into the viewer's filtered listing. No names, no unkeyed hashes, no counts. No MAC needed in this slice; keyset pagination over PostgreSQL will need AEAD later.

**P42 read strictly:** "Owner only" applies everywhere. A non-Owner member who opens a soft-deleted Account by its typed path gets the uniform not-found. P41's "listings only" applies to archived commissions.

**Small additions to the slice:**
- `Referrer-Policy: same-origin` on signed-in pages.
- `den.proto` keeps real paths off the wire.
- Hrefs are absolute and encoded.

**Carried to the end-of-slice review:** SvelteKit's inner `fetch` re-adds every incoming cookie when the hostnames match, as in dev. That would defeat `api-proxy`'s "only `zurfur.sid`" filter. This is from the source only; the `credentials: 'omit'` fix is unverified.
