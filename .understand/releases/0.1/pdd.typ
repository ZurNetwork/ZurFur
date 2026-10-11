#import "template.typ": *

#show: doc.with(
  title: "Product Design Document",
  short: "Product Design Document",
  subtitle: [Release 0.1, `pre-alpha-v0.1.0`: "Sign in with Bluesky and browse My Den"],
  version: "5",
  date: "2026-10-10",
  status: "In review",
  author: "Kestrel, release planner, with the team leads: Badger (den), Fox (identity), Wolf (commission), Fennec (web), Reynard (vulpes), Lobo (wolven), Bowerbird (posts), Meerkat (moderation), Octopus (plugin)",
  approver: "The Engineer (Zuri), Product Owner",
)

// The team that raised a decision, shown after its title.
#let raised(team) = text(weight: "regular", size: 0.88em, fill: colors.muted)[ · raised by #team]

// A decision the Engineer has already ruled on. Same shape as the template's
// `decision`, with the ruling in place of Claude's answer.
#let decided(id: "D-?", title: [], date: "", question: none, options: (), why: none, ruling: [], after: none, source: none) = {
  let fg = colors.ok
  let bg = colors.ok-soft
  block(width: 100%, stroke: 0.8pt + fg, radius: 3pt, inset: 0pt, above: 1.1em, below: 1.1em, {
    block(width: 100%, fill: bg, inset: (x: 10pt, y: 6pt), below: 0pt, radius: (top: 3pt), grid(
      columns: (auto, 1fr, auto),
      column-gutter: 8pt,
      align: horizon,
      pill(text(size: 1.1em, id), fg: fg, bg: white), text(weight: "bold", title), pill([Decided by you · #date], fg: fg, bg: white),
    ))
    block(width: 100%, inset: (x: 10pt, top: 7pt, bottom: 9pt), {
      if question != none {
        block(below: 6pt)[#text(weight: "semibold")[Question.] #question]
      }
      if options.len() > 0 {
        block(below: 2pt, text(weight: "semibold")[Options])
        enum(numbering: "(a)", tight: true, ..options)
      }
      if why != none {
        block(above: 6pt)[#text(weight: "semibold")[Claude's proposal and why.] #why]
      }
      block(
        above: 8pt,
        width: 100%,
        fill: white,
        stroke: 0.7pt + fg,
        inset: 7pt,
        radius: 2pt,
      )[#text(weight: "bold", fill: fg)[Your ruling:] #ruling]
      if after != none {
        block(above: 6pt)[#after]
      }
      if source != none {
        v(4pt)
        set text(size: 8.5pt, fill: colors.muted)
        block(below: 0pt)[*Source:* #source]
      }
    })
  })
}

#summary[
  *What 0.1 is.* The first release you can click through, tagged `pre-alpha-v0.1.0`: only you check it, breaking changes may happen, and the proto stays tagged `v1` through them. Two things work end to end, together, as you ruled (D-1):
  - *Sign in with Bluesky, hardened.* Zurfur finds your account itself, never asks Bluesky's AppView, refuses lookups aimed at internal addresses, checks that you finished as the account you named in the browser you started in, and asks your PDS for your identity only (D-17). Built and security-reviewed; a real sign-in check, four small slices and a corpus update remain before `main`.
  - *My Den, read-only, on the real backend.* The Explorer frame in a neutral dark style, served by the real API and engine (Atta). You see it in two walks on your own machine: your own Den, from PostgreSQL after a real sign-in on `just dev`; then alice's on the dev profile (bob's too, unless the user switch is cut). You see exactly your own tree, and nothing else.

  *Two additions, by your rulings of 2026-10-09.* *Time limits:* every call from the web server to the API has a limit, and a signed-in page whose backend doesn't answer says "Zurfur isn't answering right now", never signing you out (D-35). Loading skeletons wait. *Report a bug:* a button on every signed-in page opens a short form; Zurfur's API stores the report, and you read it with `zurfur bug-reports list` (D-34). If it isn't on `main` by unit 9, it moves to 0.2.

  *What it costs.* 50 work packages, 75 units (the Delivery Plan). Joining the halves is 13 packages and 17 units; bug reports 8 packages and 11 units, built by people the plan leaves free and reviewed by a second security reviewer; the time limits two small packages. *The critical path stays at 17 units:* neither addition is on it.

  *Every decision is answered.* You ruled all 24 here and the Technical Design Document's 13 on 2026-10-09. One differs from Claude's answer: D-27 is (b), so Vulpes keeps building beside 0.1, its people not lent to it, while Wolven pauses; and D-36 adds your own exception. You approved den.proto as a whole (D-6 to D-10, D-14), where archived items are hidden (D-15) and the joined component diagram, and settled the three technical questions in `v2-carryover.md`: Atta builds on your draft, and no code path serves a payload. *Still yours, on their PRs:* den.proto's text (D-19), the one-time session delete (D-32) and the bug-report contract (`bug_report.proto`). A migration isn't a door in pre-alpha unless it deletes your real data (D-36).

  *Ready for your signature.* Signing remains a separate act: you say "signed" under "Sign-off", which approves the scope and starts the build. Two small asks outside the decisions ride on it, each with Claude's answer, listed there.
]

#v(4pt)
#note(title: "How to read this set.")[
  This *Product Design Document* says what 0.1 is and why. The *Technical Design Document* (`tdd.typ`) says how it works. The *Delivery Plan* (`plan.typ`) says who builds what, in which order, and what blocks it. QA's challenges and their resolutions are in `qa.md`. Each team's objectives, with its QA, are in `objectives/`; the leads' sizing of the join is in `objectives/join-*.md`; the web lead's bug-report proposal, with its QA, is `proposals/bug-reports.md`. What the PDD sends on to the other two documents and the tech lead is in `v2-carryover.md`, which marks what each version resolved. Detail lives in the study documents under `.understand/`; these documents point to them rather than copying them.
]

= The problem, and who it's for

*The problem.* Independent artists run their commissions through a patchwork of generic tools, and the people commissioning them have no shared place at all (Project MVP, "Problem to solve"). Zurfur's answer is one place: the *Den*, the tree that holds everything a person has access to (The Den, decision 1).

Before any commission feature can live in the Den, three things have to be true. 0.1 makes them true:
+ A person can sign in with the identity they own, and sign-in can't be turned into a way to attack Zurfur or its users.
+ There is a frame people can use, on a phone and on a desktop, with a keyboard or a mouse.
+ The Den shows each person exactly what they may see and nothing more, through the one read path every later feature reuses.

*Who it's for.*

#table(
  columns: (auto, 1fr),
  table.header([Who], [What 0.1 gives them]),
  [You, the Product Owner], [The first product you can click, and the planning made visible: these documents, QA's challenges, and a report at the end of each phase.],
  [The teams], [One proven path from the browser to the engine, and the privacy rules pinned by tests that later slices build on.],
  [Artists and commissioners, the MVP's target market], [Nothing in their hands yet, because 0.1 isn't deployed. They are the reason for the frame's rules: calm, keyboard-friendly, phone-ready, in plain words.],
)

= Goals and non-goals

*Goals.*
+ *G1. Sign-in.* A real Bluesky identity signs in, with every identity lookup hardened.
+ *G2. The frame.* The product's first screen, usable at 320 px and on a desktop, with a keyboard, a mouse or a screen reader, in a neutral dark style. It never waits on the backend without a limit, and says when Zurfur isn't answering instead of signing you out (D-35).
+ *G3. My Den.* A signed-in user browses their own Den, read-only, served by the real API and Atta: from PostgreSQL after a real sign-in, and from the fixture world on the dev profile.
+ *G4. Privacy on the read path.* The Den's rules hold and are pinned by tests: one not-found for every miss, only your own root, no counts, and a node's own visibility shown only to those who can open it. And what is yours (your root, an Account you belong to) never opens what is mounted into it; each mounted item is checked on its own.
+ *G5. A visible process.* The release runs with written plans, QA before you, security reviews where they apply, and phase reports.
+ *G6. Report a bug, from the UI* (can be cut to 0.2). A signed-in user reports a bug from the page they're on, seeing exactly what the report carries, and you read every report from the terminal (D-34).

*Non-goals: what 0.1 leaves out.*
- *Uploads and editing.* 0.1 adds no new way to change anything, with one exception: filing a bug report (D-34), which changes nothing in anyone's Den and touches no domain entity. Today's Account pages (found an Account, delete one) keep working, inside the new frame. Your own commissions reach your Den through a script that calls today's commission routes as you. One read goes: the owner's commission list, which your Den's `commissions` folder replaces (D-28).
- *Commissions beyond browsing:* no status, brief, deadline, files' contents or changelog entries on screen.
- *Moderation, posts and the Gallery.* Your `posts` folder appears, marked Public because `/post`, where posts live, is always public (The Den, decisions 5 and 20). It says "Content not shown yet" and lists nothing. Zurfur reads nothing from your PDS to fill it, and 0.1 neither accepts nor serves post or public-blob bytes. 0.1 needs no moderation, because it isn't deployed: the only people in it are you and the fixture users. Today's API, with no web screen, still takes two kinds of input. One is a commission file upload, served without a scan. The other is free text that other Participants read: notes, markup and the linked channel. That is harmless only while no one else can reach the build. A bug report adds free text that only you read, through the terminal (and the Orchestrator, when it runs the same command), never another user (D-34).
- *The Claude Design visuals,* until you sync them. 0.1 uses neutral placeholder colours under the design language's token names, so the sync changes values, not names. Your sona's colours (design language, section 2) are still blank; 0.1 doesn't need them.
- *Deployment.* 0.1 runs on a developer machine. Sign-in uses atproto's local-development client, which only works there; a real domain needs the production client. The production client and the deployment checklist (the hardened-resolver design, "Deployment checklist") wait for the first build that anyone but you can reach, whatever its stage. Moderation's floor joins that checklist (D-26): the scan gate in front of every upload route, a report path, and your answers on database roles and on user text and file names in changelog entries. The bug-report form is not that report path (D-34).
- *Loading states.* No skeleton rows, "Still loading…" line or progress bar yet: they wait by your ruling (D-35), their look goes to the Claude Design brief, and in 0.1 the time limits bound every wait instead (planned ahead in `objectives/web.md`, "Planned ahead: loading states").
- *Also not in 0.1,* because the approved first slice leaves them out: view grants, bringing a commission into an Account, opening links, file contents, a real card (only hand-made fixtures show one), dotfiles, a maturity gate on your own Den, the `participants` entry, the public User Page, rate limits on sign-in (they come with the User Page; the one limit 0.1 adds is the bug report's, 10 an hour), and a light-theme switch.
- *Not in 0.1, under your ruling on D-1:* character storage in PostgreSQL and any way to create a character. Nor a way to create a Workflow (D-30), or to add a second Participant to a real commission (invitations can be issued and revoked, never accepted, yet).
- *Parked by your note on D-3:* a Finder-style column view, where each opened folder becomes a new column, as a future theme or view. 0.1’s tree behaves like a file explorer.

= What 0.1 delivers on the Release Board

The alpha column of the Release Board (Project MVP, "Core capabilities") is the scope source of truth. 0.1 completes none of its items. It lays the read path and the sign-in that every alpha item uses.

#table(
  columns: (1.3fr, 2.6fr, auto),
  table.header([Alpha item], [What 0.1 gives it], [How much]),
  [Login / logout, and DID operations], [Login hardened: one resolver, guarded fetches, the DID bound at start and checked at the end, the sign-in bound to the browser, and identity only (D-17). Sign-out ends your Zurfur session, clears this browser's session even when the backend can't confirm it, and says so. Your PDS tokens stay stored, sealed, until PDS writes need them deleted (parked). No new DID operations: founding an Account still mints its DID, kept local; nothing is sent to the public directory.], [Part],
  [Security-first: the items that cannot be escaped], [*Covers what the identity design reads as the https-only PDS redirect:* the authorization endpoint must be https on port 443 at a named host. The board names the item without defining it, so whether this closes it is your call: a small ask in "Sign-off", where Claude answers yes. General hardening, advanced for identity fetches. The other eleven items are untouched. Two of them, CSAM scanning and a report path, are moderation's; the bug-report form is not that report path (D-34). Project MVP sequences them ahead of feature work; 0.1 builds My Den first because it isn't deployed. Both are due before the first build that anyone but you can reach (D-26).], [One of thirteen, unless you say otherwise at signing],
  [User pages & Accounts], [Your user root and your Accounts as folders in My Den: your real ones after sign-in, the fixture users' on the dev profile. Today's Account pages inside the frame. Not the public User Page.], [Part],
  [View grants & visibility], [The read side of the visibility rules for you, Account members, character keepers (Zurfur's own record: 0.1 writes, reads and checks no ownership claim) and commission Participants. No grants.], [Part],
  [Closed-door privacy], [Private by default, enforced where the Den reads: a node with no stored level is Private, and a miss looks like any other miss.], [Part],
  [Frontend for exactly what the backend supports], [The frame around today's Accounts pages, and the My Den screens, each backed by the Den read, on every stack. The bug-report form, backed by its own route (D-34).], [Part],
  [Characters; Workflows; Commission core; Slots; the Changelog], [Listed in My Den with names and visibility only. A commission opens to its four entries: `attachments` lists file names, `slots` lists slot titles, `products` is empty. A Workflow, each file, the changelog and each slot (D-21) open to "Content not shown yet." No lifecycle, status, brief or deadline. On `just dev` your `characters` and `workflows` folders are empty, because nothing in 0.1 creates either (D-30, D-33).], [Placeholder],
  [Deployment], [Nothing; out of 0.1.], [None],
  [All other alpha items], [Nothing.], [None],
)

Under your ruling on D-1, the My Den side of every "Part" and "Placeholder" row ships on every stack. On `just dev` it shows your real data: the Accounts you belong to, and the commissions you take part in.

= What each team commits to

Each team lead wrote their objectives for 0.1, and their QA challenged them (`objectives/`). The leads revised them for the join (`objectives/join-*.md`). Each "Done when" is something you can check by hand; the exact commands are on the team's page.

#table(
  columns: (auto, 1.25fr, 2fr),
  table.header([], [Objective], [Done when]),
  table.cell(colspan: 3, fill: colors.faint)[*Identity* · lead Fox, QA Magpie · sign-in, and My Den's users, Accounts and characters],
  [1], [*You sign in with your real Bluesky handle,* on the identity branch first, then on `main`.], [You see your handle in the top bar. It appears in no log line at `info` or above. Two refused sign-ins print the same `422` body.],
  [2], [*A sign-in finishes only as the account you named, in the browser you started in,* and says why when it doesn't.], [A sign-in started outside your browser lands on the refusal message (D-18). Approving as a different account shows the account message (if your PDS lets you switch accounts; otherwise its named test). Unless cut, a stray callback link in a second tab no longer cancels your sign-in.],
  [3], [*Sign-in asks your PDS for your identity only* (D-17), and the old full-write sessions go.], [The address sign-in sends you to carries no `transition` scope, and sign-in still works. The slice deletes the stored sign-in sessions once, and nothing else (D-32): after the first start, `just db-shell` counts no stored sign-in session, and every Account you founded is still there.],
  [4], [*The corpus says what sign-in now is,* before it reaches `main`.], [The corpus PR you review has a decision page for the hardened resolver and the browser-bound cookie on the Authenticators page, and the stale note on the approved-crates page is gone.],
  [5], [*Your user root, Accounts and characters show in My Den exactly as you may see them,* on both stacks.], [On the dev profile, alice and bob each see their own Accounts and characters, and alice's Supreme Arts, opened as bob, is "Nothing here". After your real sign-in on `just dev`: `/den` opens your root; `accounts` lists the same Accounts as the Accounts page; `characters` says "This folder is empty." (D-33). A deactivated Account shows only to its Owner, and only with the toggle on, on `just dev-mock` ("Former Studio") and in identity's PostgreSQL test (on `just dev` a deleted Account is gone: one with no facts is hard-deleted, and today every delete is). Each identity PR names one test command, covering both adapters, and it passes. It includes the guard that no Den answer names a User other than you, and a root name that falls back to your DID, within a short wait, when your PDS never answers.],
  [6], [*The reads that bypass the Den go through it.*], [The Accounts list and `/me` call den's access check, with a test per read in both adapters, and their answers for alice and bob are byte-identical to today's.],
  [7], [*The session check answers within its limit, whatever your PDS does* (D-35; the tech lead's call).], [On the identity branch, a test whose profile source never answers gets `/me`'s bare DID within its short wait, inside the web's 5 s. On `main`, the top bar shows your handle, or your DID while your PDS is slow, never the "isn't answering" page while the API is up.],
  table.cell(colspan: 3, fill: colors.faint)[*Web* · lead Fennec, QA Crow · the frame, the dark style, and My Den's screens],
  [1], [*The frame and My Den on mock data are merged and clickable* on `main`.], [On `just dev-mock`: the tree works by keyboard, the toggle marks items Archived and Deactivated, "More" loads the next page, a new Account appears in the tree, and the drawer opens at 320 px. On `just dev`, until My Den reaches `main`: the Accounts pages sit in the frame with no tree, a typed `/den` is "Nothing here", and with the API stopped, "Sign out" still leaves no `zurfur.sid` cookie. Once My Den reaches `main`, objective 3 replaces this half.],
  [2], [*den.proto reaches the contract as you approved it,* with den.], [The den.proto decisions carry your answers, CI's contract job is green, and the generated TypeScript is in the web app.],
  [3], [*My Den shows the real backend through the same screens,* on both backends.], [On the dev profile, alice sees Supreme Arts, Ember, Kael-sona and "Untitled" with its four entries (bob his, if the switch survives). On `just dev`, signed in with your real handle: `~` lists `accounts`, `characters` (empty), `commissions` and `posts`; an Account you found appears under `accounts`, and once deleted it leaves `accounts` with the toggle off and on (an Account with no facts is hard-deleted; today every delete is); `/den/nope` is "Nothing here"; with the API stopped, "Sign out" leaves no `zurfur.sid` cookie. The gate lists the web's golden-response tests passing.],
  [4], [*Nothing leaks from the web,* on mock, the dev profile and `just dev` alike.], [In the browser's developer tools: every request goes to Zurfur (your own avatar the one exception), nothing in browser storage, `zurfur.sid` the only cookie, the three privacy headers, the generic page title. The web's log shows no DID, handle, Den path or page token, your own included after a real sign-in. The end-of-slice review has no open must-fix for the web.],
  [5], [*The frame's accessibility is checked, and stays checked.*], [Crow's check file records a keyboard-only walk and the 320 px run. A contrast test turns the gate red below WCAG 2.2 AA (D-25).],
  [6], [*Zurfur says when it isn't answering, and never signs you out for it* (D-35).], [On `main` after the time-limit slice, on `just dev`, signed in: stop the API and reload a signed-in page. It shows "Zurfur isn't answering right now." with "Try again", not the sign-in page; DevTools shows `zurfur.sid` still there and the page's three privacy headers. Start the API and press "Try again": the page comes back, still signed in. The gate lists the slice's tests passing: a backend that never answers gets that page within about 5 s; a page whose reads hang ends within 15 s; no form action runs behind the page; a `401` still goes to `/login`; a sign-in start that runs out says "Zurfur isn't answering right now. Please try again." on the login form.],
  table.cell(colspan: 3, fill: colors.faint)[*Den* · lead Badger, QA Owl · Atta, the den module and the den API routes],
  [1], [*Atta answers the Den's reads by the Den's rules, on its own.*], [Atta's tests pass. A guard in the gate fails if Atta gains a workspace dependency. A table maps every Atta rule in the security table to a named test.],
  [2], [*Zurfur answers Atta through one host and one exhaustive match,* and no arm can break a read.], [The match has no catch-all arm, and no arm can panic. A test shows every valid name converts. The `/post` stub lands as its own slice, with the posts lead's review.],
  [3], [*Every miss looks the same,* on both stores, and the viewer comes only from the session.], [As alice, Abco's address and a made-up one show the same "Nothing here". Named tests, run in memory and on PostgreSQL, show every not-found cause gives the same status, body and headers (all but the date), bob's DID as a path piece included. On PostgreSQL, both causes in each pair run the same database statements. The API never imports Atta.],
  [4], [*den.proto lives in the contract as you reviewed it,* and the web tests against real answers.], [The den.proto PR differs from the reviewed draft only by your edits. CI's contract job passes. The route list lists the two den routes like any other, with no dev-profile marking. Regenerating the golden responses changes nothing.],
  [5], [*My Den reaches `main` reviewed,* and the corpus says what the code does.], [`feature/my-den` merges. The coherence check is clean. The end-of-slice security review lists no open must-fix. The My Den rulings spell the query parameter as you ruled (D-7), and their name rule allows the two joiners (D-29) and covers Account names (D-31).],
  [6], [*One rule decides who opens what.*], [The Accounts list, `/me`, the changelog read and the file download each call den's access check. A test for each shows that a node the Den refuses gets that read's usual answer: dropped from the Accounts list, `/me`'s usual `401`, the usual `404` for the changelog and a file. It lands on `feature/my-den` before the end-of-slice security review starts.],
  table.cell(colspan: 3, fill: colors.faint)[*Commission* · lead Wolf, QA Jay · commissions, slots, files and Workflows in My Den],
  [1], [*A commission you take part in shows in My Den,* with its parts.], [As alice, "Untitled" (Private, mounted) opens to `attachments` (two files), an empty `products`, `slots` (four) and `changelog`. Files, the changelog and slots say "Content not shown yet." (D-21).],
  [2], [*A Workflow appears, opens, and never looks empty.*], [Supreme Arts › `workflows` lists "Queue", Private, which says "Content not shown yet." and lists nothing; so does Supreme Arts' `commissions`.],
  [3], [*Archived commissions stay out of listings unless asked for,* open when typed, and carry the backend's word.], [The archived-check fix is on `main` before `feature/my-den` is cut. "Old sketch" (D-22) shows only with the toggle on, marked "Archived", and opens when typed. The hiding change's tests pass. Rests on den.proto as you approved it (D-6), with the soft-deleted word's field name (D-14) and the hiding change (D-15), both ruled.],
  [4], [*The commission side leaks nothing,* and the commission reads outside the Den follow its rules.], [The security reviews show no open must-fix. Name tests refuse a right-to-left override in each of the four names. The golden responses hold no status, brief, deadline, owner, maturity or linked-channel value. As bob, "Old sketch" is "Nothing here". The owner's commission list is gone (D-28); the changelog read and the file download ask the Den first, and their route tests match the Den's answers.],
  [5], [*After a real Bluesky sign-in on `just dev`, My Den shows the commissions you take part in,* from PostgreSQL.], [Sign in and run the fill script: `commissions` lists the live commission, which opens to its four entries with its real files and slots; the archived one appears only with the toggle on, marked "Archived". `cargo test -p adapter-pg` runs the five read tests, and the PostgreSQL end-to-end test passes (its command is named in its PR).],
  table.cell(colspan: 3, fill: colors.faint)[*Bug reports* (D-34; can be cut to 0.2) · shared ground under the tech lead · backend: moderation's programmers (lead Meerkat, QA Lapwing) · web: Finch (lead Fennec, QA Crow) · security review: Mantis, not Heron],
  [1], [*You can report a bug from any signed-in page, and read it back.*], [On `main`, on `just dev`, signed in with your real handle: "Report a bug" in the top bar (on a phone, an icon in the top bar until My Den brings the drawer) opens the form, which lists every item it would send. Send one: "Thanks. Your report is saved." and a reference. `just bug-reports` prints it between delimiter lines, with your DID, and the reference finds it.],
  [2], [*A report carries only what its form shows, and can't harm the terminal that reads it.*], [Named tests pass: the request and the stored row hold no cookie, token, page token, query or error message; the reporter is always the session's DID; U+202E and U+009B are refused in every text field, and the command prints a stored one escaped; an 11th report within an hour gets the calm limit message, and the text stays.],
  [3], [*It reaches `main` reviewed by unit 9, or moves to 0.2.*], [`feature/bug-reports` merges to `main` after the identity branch, with its coherence check; Mantis's review lists no open must-fix; the corpus PR offering you a decision page is in your queue. If it isn't on `main` by unit 9, it waits until My Den reaches `main` and is 0.2's.],
)

*Teams with no 0.1 work of their own.* Each names what 0.1 must not close off.

#table(
  columns: (auto, 1fr, 2.2fr),
  table.header([Team], [0.1 work], [What 0.1 needs from them]),
  [*Vulpes* \ Reynard · QA Rook], [None in 0.1. By your ruling on D-27, Vulpes keeps building its library in parallel, and its people aren't lent to 0.1. Zurfur doesn't depend on it.], [No ownership claim enters 0.1, fixtures included: every fixture character is Private and the in-memory record store stays empty (the tech lead's call, 2026-10-09), a line in the security reviews. Under your ruling on D-1, 0.1 mints no Character either. Nothing Vulpes builds meanwhile enters Zurfur: the end-of-slice review checks for no vulpes dependency. The hardened resolver's decision page keeps room for the vulpes ports.],
  [*Wolven* \ Lobo · QA Shrike], [None; idle since 2026-08-22, and paused through 0.1 (D-27), its tree walk to be committed to a branch (`v2-carryover.md`, row 95).], [Nothing of Wolven's enters Zurfur: a search of `main` at acceptance finds none. The corpus PR for sign-in settles neither part you deferred: who attests for Wolven, and who may run the lookup from an outside account to a DID.],
  [*Posts* \ Bowerbird · QA Kea], [Builds nothing; reviews the empty `/post` stub that den builds.], [Your `posts` folder is an honest placeholder: Public, "Content not shown yet", and "Nothing here" below it. No post and no public blob byte moves in 0.1; Kea's checks print nothing at acceptance. Posts owns the post record type; the public-records port stays shared ground (the tech lead's call, 2026-10-09).],
  [*Moderation* \ Meerkat · QA Lapwing], [None of its own. Its programmers build the bug-report backend, which is shared ground under the tech lead, not a moderation feature (D-34).], [den.proto and Atta's one host check leave room for take-down and hide, with no new field; Lapwing reviews the host check's inputs on Atta's read side. No build that anyone but you can reach goes out before moderation's floor stands (D-26). A bug report is not a content report, and doesn't meet the floor's report path.],
  [*Plugin* \ Octopus · QA Mantis], [None of its own; the plugin surface is beta work. Mantis is the bug reports' security reviewer (the tech lead's call).], [den.proto stays open for plugins: a node of an unknown type and an unknown content case show safely (tolerant-reader tests in the gate), and requests carry only the path and its two options.],
)

= Features

Each feature is a set of user stories. Every acceptance criterion is written so it can be checked, and each maps to a test, or, where a test can't see it (look and feel), to a recorded check in the sign-off report. "D-n" marks a criterion that follows a decision in "Decisions: product" below or in the Technical Design Document.

== Sign in with Bluesky, hardened

#story(
  id: "US-1.1",
  title: [Sign in with my handle],
  who: [a visitor with a Bluesky account],
  want: [to sign in by typing my handle],
  why: [Zurfur knows me by the identity I own],
  criteria-items: (
    [On `/login`, typing a valid handle takes me to my PDS to approve, then back to Zurfur signed in, with my handle in the top corner.],
    [Zurfur resolves the handle itself: DNS first, then the handle's HTTPS file. It never asks Bluesky's AppView: no request carries `public.api.bsky.app`.],
    [The handle is confirmed both ways: the DID document must name the same handle, or sign-in stops.],
    [In the browser, the login form refuses a web address, a DID or a handle with a leading `@` with a field message, before it calls the API.],
    [At the API, a direct `POST /signin` with any of those, or any failed lookup, gets one `422` with a byte-identical body, whatever the cause.],
    [Sign-in asks my PDS only for my identity (the `atproto` scope), never for write access to my repo. A test checks the scope Zurfur sends (D-17).],
  ),
)

#story(
  id: "US-1.2",
  title: [Zurfur can't be aimed inward],
  who: [the operator],
  want: [sign-in lookups that can't reach internal addresses],
  why: [a visitor can't use Zurfur to probe our network or the cloud's metadata service],
  criteria-items: (
    [Every identity fetch is https on port 443 to a named host. IP-literal hosts, and names whose addresses are private or internal, are refused.],
    [No redirect is followed on any identity fetch.],
    [Each fetch stops at 512 KiB and 10 seconds; starting a sign-in gives up after 30 seconds at the API. The web stops waiting for it after 10 seconds, and the login form then says "Zurfur isn't answering right now. Please try again." (D-35)],
    [The PDS's authorization endpoint passes the same rule before Zurfur sends the browser there.],
    [A PDS address with a path, a query or a fragment is refused (D-11). _Can be cut: it then moves to 0.2._],
  ),
)

#story(
  id: "US-1.3",
  title: [Finish as the person who started],
  who: [a visitor],
  want: [a sign-in that completes only as the account I named, and only in the browser I started in],
  why: [nobody can sign me in to their account, or finish my sign-in as someone else],
  criteria-items: (
    [Approving a different account at my PDS than the handle I typed refuses the sign-in, deletes the session Zurfur saved, asks my PDS to revoke its tokens (best effort, within 10 seconds), and returns me to `/login` with "You signed in to a different account than the handle you typed."],
    [A sign-in callback opened in a browser that didn't start the sign-in is refused before any code exchange.],
    [That case, and a sign-in that expired, shows its own message: "This sign-in expired or was started in a different browser. Please sign in again here." (D-5, D-18)],
    [The browser binding lasts 10 minutes, and a newer sign-in replaces it.],
    [After the identity merge, the binding is cleared only when it matched this sign-in, so a stray link from another site no longer cancels a sign-in in progress (D-12). _Can be cut: it then moves to 0.2._],
  ),
)

#story(
  id: "US-1.4",
  title: [Handles stay out of logs],
  who: [a visitor],
  want: [my handle never written into the operator's logs],
  why: [a leaked log can't tie me to Zurfur],
  criteria-items: (
    [No log line at `info` or above carries a visitor's handle, a host they chose, the sign-in `state` or the browser token.],
  ),
)

== The Explorer frame, in a neutral dark style

#story(
  id: "US-2.1",
  title: [One frame on a desktop],
  who: [a signed-in user on a desktop],
  want: [one frame, with my Den's tree on the left, the open item on the right and its path along the top],
  why: [I always know where I am],
  criteria-items: (
    [Every signed-in page uses the frame. The Accounts pages fill the right pane (D-2).],
    [The top bar shows the path by names, the "My Den" and "Accounts" links, my avatar with my handle, and "Sign out". On a phone, the avatar, the handle and "Sign out" sit in the drawer.],
    [The tree opens down to the open item and highlights it, but leaves the item itself closed: its contents are in the right pane.],
    [An error on a signed-in page shows inside the frame and never shows the error's own message.],
    [The frame is the same on every stack, the everyday stack (`just dev`) included: the sidebar, the tree and the "My Den" and "Open My Den" links show wherever I'm signed in, and a typed `/den` reads the Den (D-1).],
    ["Sign out" clears this browser's session even when the backend can't confirm it. The sign-in page then says the server's session ends when it expires.],
  ),
)

#story(
  id: "US-2.2",
  title: [The frame on a phone],
  who: [a signed-in user on a phone],
  want: [the tree to slide in from the left and the item to fill the screen],
  why: [I can browse on a small screen],
  criteria-items: (
    [From 320 px up to 48rem, the tree is a drawer opened with ☰. Nothing scrolls sideways.],
    [Choosing an item opens it and closes the drawer. Escape closes the drawer, and focus returns to ☰.],
    [The path bar shows only the parent, as a back link; the item's name is the pane's title.],
  ),
)

#story(
  id: "US-2.3",
  title: [The tree with a keyboard and a mouse],
  who: [a keyboard or mouse user],
  want: [the tree to behave like a file explorer],
  why: [I can move fast without hunting for buttons],
  criteria-items: (
    [Tab enters the tree once. Up and Down move; Right opens a folder, or goes to its first child; Left closes it, or goes to the parent; Home and End jump; a letter jumps to the next name starting with it; Enter or Space opens the item in the pane (D-3).],
    [After opening an item from the tree, focus stays on that tree item.],
    [After opening an item from the pane or the path bar, focus moves to the pane's heading. After "More", focus moves to the first new row.],
    [Clicking a folder's name opens it in the pane; only the arrow beside it expands or collapses it (D-3).],
    [Before the page's script runs, the tree is plain links that Tab reaches one by one.],
  ),
)

#story(
  id: "US-2.4",
  title: [A screen-reader user],
  who: [a screen-reader user],
  want: [the frame announced plainly],
  why: [I can use Zurfur without seeing it],
  criteria-items: (
    [A "Skip to content" link comes first, then the header, a navigation region named "My Den", and the main region.],
    [Every Den page's title is "My Den · Zurfur", so no Private name reaches the browser's history. A polite announcement names the item that opened.],
    [In a folder that isn't fully loaded, every item carries its level and position with the set size marked unknown, so the tree reveals no count.],
    [Visibility is an icon and a word, never colour alone. Names are isolated, so a right-to-left name can't reorder its row.],
  ),
)

#story(
  id: "US-2.5",
  title: [The neutral dark style],
  who: [anyone using Zurfur],
  want: [a calm, neutral dark look],
  why: [the art, later, carries the colour],
  criteria-items: (
    [Dark is the default, and the only theme in 0.1. The light theme is defined but can't be switched on yet.],
    [Every colour, size, radius and duration comes from `tokens.css`: the design language's names, plus four layout tokens the app owns. The phone breakpoint is one literal, because CSS can't use a variable in a media query. So the Claude Design sync changes values, not names.],
    [Text, icons, control edges and focus rings meet WCAG 2.2 AA contrast in the dark theme (D-25).],
    [With "reduce motion" set, nothing animates. With "more contrast" set, borders and text get stronger. In forced colours, borders and the focus ring stay visible.],
    [Fonts and icons are served by Zurfur itself. The one request to another host is your own avatar, loaded from your own PDS, as today.],
  ),
)

#story(
  id: "US-2.6",
  title: [Zurfur says when it isn't answering],
  who: [a signed-in user],
  want: [a plain page when Zurfur's backend doesn't answer, instead of an endless wait or being signed out],
  why: [I know the problem is Zurfur's, and I'm still signed in when it comes back],
  criteria-items: (
    [Every call from the web server to the API stops waiting after its limit: 5 seconds for the session check, 10 seconds for every other call, 15 seconds for the whole request. Sign-out keeps its own 5 seconds, and still runs when the backend is down (D-35).],
    [When the session check runs out, or the API can't be reached at all, opening a signed-in page shows the gate's own fixed page: "Zurfur isn't answering right now.", with "Try again". Never the sign-in page, "Nothing here", an empty folder, or the frame. A navigation made by the page's script ends there too, after SvelteKit's own retries (about three session checks).],
    [That page keeps my session cookie and carries the signed-in headers (`private, no-store`, `Referrer-Policy: same-origin`, `frame-ancestors 'none'`). No page load and no form action runs behind it. A form sent from a page already open runs no action either, and says that Zurfur isn't answering, never "signed out"; the bug-report form also keeps my text.],
    [A slow PDS never shows that page: the session check waits only a short time for my profile, and otherwise answers with my DID, which the top bar then shows.],
    [A read inside a page that runs out shows what an unreachable backend shows today: for the open item, the frame's error view; for a folder above it, a tree that opens less far. Never "Nothing here".],
    [A session that has really ended (the API's `401`) still goes to `/login`.],
  ),
)

== My Den, read-only, on the real backend

#story(
  id: "US-3.1",
  title: [Open My Den],
  who: [a signed-in user],
  want: [to open My Den and see my own root],
  why: [everything I have access to is in one place],
  criteria-items: (
    ["My Den" in the header opens `/den`. The pane's title reads "My Den", and the tree's root shows as `~`.],
    [The root lists `accounts`, `characters`, `commissions` and `posts`. `posts` carries the mounted mark and opens as Public with "Content not shown yet". Any path below it shows "Nothing here".],
    [On the dev profile, alice sees her Account "Supreme Arts", her characters Ember and Kael-sona, and the commission "Untitled". bob sees Abco and "Untitled", and no Account.],
    [After a real sign-in on `just dev`, the root is my own, read from PostgreSQL: `accounts` lists the Accounts I belong to, the same as the Accounts page; `characters` says "This folder is empty." (D-33); `commissions` lists the commissions I take part in.],
    [My Den opens even when my PDS doesn't answer: my root's name falls back to my DID after a short wait. Zurfur looks up only my own name, never another user's.],
    [A signed-out visit to `/den` goes to `/login`.],
  ),
)

#story(
  id: "US-3.2",
  title: [Browse a folder],
  who: [a signed-in user],
  want: [to open any folder and see what's in it],
  why: [I can find my things],
  criteria-items: (
    [Each entry shows its name and, for what I can open, whether it's a folder, a file or a link, and its own visibility: Private, Listed or Public. A User's name is the display name; else the handle, shown only when it checks out both ways; else the DID. A commission's is its title; a slot's is its title; a Workflow's is its name; a file's is its file name. A stored name that fails the Den's rules shows "Unnamed", never its key (D-20); an Account's shows its handle (D-31). Under your ruling on D-29, a name may hold the zero-width joiner and non-joiner.],
    [Folders come first among the entries loaded so far, then files, each group in the order the server sends (today, by name). When "More" loads a later page, a folder on it moves up, above the files already shown.],
    [A mount shows its target's name, type and kind and, when I can open it, its target's visibility, plus the mounted mark. Under your ruling on what a card carries (D-8), a mount I can see only as a card shows its name and type, plus the mounted mark.],
    [An empty folder says "This folder is empty." A folder whose content 0.1 doesn't show (a Workflow, `posts`, an Account's `commissions`) says "Content not shown yet." and never looks empty. On the dev profile, Supreme Arts' `workflows` lists one Workflow, "Queue", which says "Content not shown yet." On `just dev`, my Accounts' `workflows` folders say "This folder is empty." (D-30).],
    [When a folder holds more than one page, "More" loads the next page. Nothing anywhere shows a count.],
  ),
)

#story(
  id: "US-3.3",
  title: [Look inside a commission I take part in],
  who: [a commission Participant],
  want: [to open the commission and see its parts],
  why: [I can find its files and slots],
  criteria-items: (
    [The commission shows four entries: the folders `attachments`, `products` and `slots`, and the file `changelog`. Your scope ruling for this slice gave four (question P2); the fifth, `participants`, which the corpus and the approved fixture diagram show, arrives with its own slice.],
    [`attachments` lists its files by name; `products` says "This folder is empty."; `slots` lists its four slots by title, each opening to "Content not shown yet." (D-21)],
    [Opening a file, or the changelog, shows its header and "Content not shown yet."],
    [Someone who isn't a Participant has no entry for it, and its address shows "Nothing here".],
  ),
)

#story(
  id: "US-3.4",
  title: [Nothing leaks],
  who: [any signed-in user],
  want: [Zurfur to show me only what I may see],
  why: [my private things stay private],
  criteria-items: (
    [Every Den address starts at my own root, and no address or field can name another user's Den. Answers carry Den paths, never a real path. The only name of mine an answer carries is my own root's, and only I am served it.],
    [Something that doesn't exist, something hidden from me, and something inside a thing I can't open all show the same "Nothing here" page. So do a deactivated Account I'm not the Owner of, an archived item I could see only as a card (your ruling on D-10), a path that runs on past a file or a card, and an empty or malformed address. The API answers each with the same `404`: the same status, body and headers, all but `date`.],
    [A card, something I may know exists but can't open, shows only its name and type, the type as a word when this build knows it (D-23): no visibility and no link. 0.1’s real data produces no card; hand-made fixtures prove the screen.],
    [Signed-in pages are never cached (`Cache-Control: private, no-store`), never send their address to another site (`Referrer-Policy: same-origin`), and no other site can frame any Zurfur page (`frame-ancestors 'none'`).],
    [Den addresses, path pieces and page tokens never appear in logs; only route templates do.],
    [The API answers an address option it doesn't know with one fixed `400`, and a bad value with one fixed `422`, both before it reads the path, so neither reveals what exists.],
    [Nothing about the tree is stored in the browser. Its memory belongs to the signed-in user, and every change of user is a full page load.],
    [The reads outside My Den follow its rules: the Accounts list, `/me`, a commission's changelog and its file downloads show only what the Den would open. An Account the Den refuses drops out of the list; a refused changelog or file gets that read's usual not-found. The owner's commission list is gone (D-28).],
    [All of this holds in memory and on PostgreSQL alike.],
  ),
)

#story(
  id: "US-3.5",
  title: [Show archived and deactivated],
  who: [a user who has archived something],
  want: [a toggle that also lists archived commissions and my deactivated Accounts],
  why: [hidden isn't gone],
  criteria-items: (
    ["Show archived and deactivated" adds archived commissions, and deactivated Accounts I own, to listings, each with its word: "Archived" or "Deactivated" (D-4).],
    [It never shows anything I couldn't otherwise open. A deactivated Account shows only to its Owner.],
    [The toggle lives only in the address (`?includeDeleted=true`, spelled as you ruled, D-7), never in a cookie or in browser storage.],
    [Typing the path of an archived commission I take part in opens it, marked "Archived", with or without the toggle. Typing the path of a deactivated Account opens it, marked "Deactivated", for its Owner only; for anyone else it is "Nothing here". On the dev profile, alice's archived "Old sketch" shows only with the toggle on, and bob never sees it (D-22).],
    [A soft-deleted item whose word this build doesn't know shows "Set aside", never as live.],
  ),
)

#story(
  id: "US-3.6",
  title: [Switch fixture users on the dev profile (first to cut)],
  who: [the Engineer],
  want: [to switch between alice and bob on the dev profile],
  why: [I can see both Dens without a test],
  criteria-items: (
    [A dev-only control signs me in as alice or bob. Every switch is a full page load, so no tree from one user shows in the other's session.],
    [The dev profile refuses to start unless the environment is `dev`, and a normal build doesn't contain it. On every other stack the switch's page doesn't exist: it is the ordinary not-found.],
  ),
)

== Report a bug, a side feature (can be cut to 0.2)

By your ruling on D-34, from the web lead's proposal (`proposals/bug-reports.md`) with its QA resolutions. If it isn't on `main` by unit 9, the whole feature moves to 0.2. Its wireframe is the last figure under "Wireframes".

#story(
  id: "US-4.1",
  title: [Report a bug from where I am],
  who: [a signed-in user],
  want: [to report a bug from the page I'm on],
  why: [it gets fixed without my having to explain where I was],
  criteria-items: (
    [On every signed-in page, "Report a bug" sits in the top bar, after "Accounts". On a phone it sits in the drawer, under "Accounts"; where a phone has no drawer (on `main` until My Den arrives), it is an icon in the top bar, named "Report a bug". The frame's error view offers "Report this problem", which opens the same form. Signed out, there is no button in 0.1 (D-34).],
    [Without the page's script, the button is a link to the same form on its own page, `/report-a-bug`, inside the frame. There the Page item comes from the browser's `Referer` only when it names Zurfur itself, without its query, shown and removable; with none, an optional "Which page?" field takes its place. A report sent from that page keeps its line breaks.],
    [The form opens with focus in "What happened?". Escape, ✕, "Cancel" or a click outside closes it, and focus returns to the button that opened it (to ☰ from the drawer). What I typed stays until I send, sign out or reload, and is never kept in browser storage.],
    [Text that is empty or only spaces is refused with "Tell us what happened." The text holds up to 4,000 characters, counted as Unicode code points by the web, the API and the database alike.],
    ["Send report" saves it and shows "Thanks. Your report is saved." with a reference, the last 8 characters of its id, and a polite "Report sent." announcement.],
    [An 11th report within an hour gets "You've sent several reports in the last hour. Please try again a little later.", and my text stays. When Zurfur doesn't answer (the send's 10 seconds, or the gate's 5-second session check), the form says "Couldn't send your report. Your text is still here; please try again." (D-35)],
    [At 320 px the form fills the screen, "Send report" stays visible, and nothing scrolls sideways.],
  ),
)

#story(
  id: "US-4.2",
  title: [Only what I see is sent],
  who: [a reporter],
  want: [to see, and choose, exactly what my report carries],
  why: [nothing about me, or anyone else, leaves without my knowing],
  criteria-items: (
    [Under "Sent with your report", each item the report would carry shows with its exact value: the page address (without its query or fragment, "(showing archived)" when the toggle is on, and a note that it can hold the ids of my Accounts, characters and commissions), the page type, Zurfur's version, the browser, the window size and, from the error view, the status code. Each starts ticked, except the three display settings, which start unticked (D-37).],
    [An unticked item greys its value and adds "Not sent", and is stored empty.],
    [A fixed line says who it is sent as: "Sent as \@handle. Zurfur also records when it arrives." Then: "Never sent: your cookies, sign-in tokens, other pages you visited, or what's on your screen."],
    [The request the web sends, and the stored row, hold no cookie, token, page token, query or error message. The reporter is always the session's DID: no form field and no contract field names one.],
    [Every text field refuses control and bidirectional characters (named tests send U+202E and U+009B). A browser text over 512 bytes is cut, and shown as cut, never refused.],
    [The test stacks say so plainly: the dev profile, "This test stack forgets reports when it stops."; mock mode, "Mock mode doesn't keep reports."],
  ),
)

#story(
  id: "US-4.3",
  title: [Read reports safely],
  who: [the operator],
  want: [to read every report from the terminal],
  why: [I can fix the bugs, and a report can neither take over my terminal nor pass for instructions],
  criteria-items: (
    [`zurfur bug-reports list` (or `just bug-reports`) prints each report between fixed delimiter lines, its text indented, and every control and bidirectional character escaped, C1 controls such as U+009B included.],
    [The reference shown after sending finds the report in that list.],
    [No route reads reports: the API's only bug-report route files one, and its answer carries only the caller's own reference.],
    [Inside its fence, each report's text sits under a fixed line saying it is the reporter's text, to be read as data and never as instructions; a test checks the line.],
  ),
)

#text(size: 8.5pt, fill: colors.muted)[*Who reads a report.* You, through the terminal, and the Orchestrator when it runs the same command; what the Orchestrator reads reaches its model provider, as your chat with it does. Nothing else leaves your machine. On `just dev`, the form action refuses a post that didn't come from Zurfur's own pages, so another local page can't file a report as you (the Technical Design Document, Feature 4).]

= Wireframes

From the Explorer shell study (`.understand/20261008-web-explorer-shell.md`, "Wireframes"), with your rulings on D-2 to D-4 and D-23 drawn in. alice has opened the commission "Untitled" through her own `commissions` folder. Every stack shows the same frame (D-1), so version 2’s fourth figure, the everyday stack without a tree, is gone. The last two figures are new in version 4: the page that says Zurfur isn't answering (D-35), and the bug-report form (D-34), from the web lead's proposal.

#let wf(body, width: 100%) = block(width: width, stroke: 0.8pt + colors.ink, radius: 3pt, clip: true, inset: 0pt, {
  set text(font: fonts.mono, size: 7pt)
  set par(leading: 0.5em, spacing: 0.5em)
  body
})
#let wf-bar(body) = block(
  width: 100%,
  fill: colors.faint,
  stroke: (bottom: 0.6pt + colors.ink),
  inset: (x: 6pt, y: 5pt),
  below: 0pt,
  body,
)
#let ind(n) = h(n * 1.2em)
#let cur(body) = box(fill: colors.accent-soft, stroke: (left: 1.5pt + colors.accent), outset: (y: 1.5pt), inset: (x: 2pt), body)
#let muted(body) = text(fill: colors.muted, body)
#let rows(..cells) = grid(columns: (1fr, auto), row-gutter: 4pt, ..cells.pos())

#figure(
  caption: [Desktop, wider than 48rem.],
  wf({
    wf-bar(grid(
      columns: (auto, 1fr, auto, auto, auto, auto),
      column-gutter: 9pt,
      text(weight: "bold")[zurfur], [\~ / commissions / Untitled], [My Den], [Accounts], [(◕) alice], [Sign out],
    ))
    grid(
      columns: (34%, 1fr),
      block(width: 100%, height: 47mm, stroke: (right: 0.6pt + colors.ink), inset: 7pt)[
        ▾ \~ \
        #ind(1)▸ accounts \
        #ind(1)▸ characters \
        #ind(1)▾ commissions \
        #ind(2)#cur[▸ Untitled ↪] \
        #ind(1)▸ posts ↪ \
        #ind(2)#muted[not shown yet] \
        #v(10pt)
        #muted[── View ──────────] \
        (○ ) Show archived \
        #ind(2)and deactivated
      ],
      block(width: 100%, height: 47mm, inset: 8pt)[
        #text(weight: "bold")[▣ Untitled ↪] \
        #muted[Folder · ◆ Private · Mounted]
        #line(length: 100%, stroke: 0.4pt + colors.rule)
        #rows(
          [▸ attachments], [◆ Private],
          [▸ products], [◆ Private],
          [▸ slots], [◆ Private],
          [· changelog], [◆ Private],
        )
      ],
    )
  }),
)

#text(size: 8.5pt, fill: colors.muted)[The shaded row is the open item. ▸ closed folder · ▾ open folder · "·" file · ↪ mounted · ◆ the visibility marker, an icon and a word, never colour alone. The visibility shows only in the right pane; the mounted mark shows in both.]

#figure(
  caption: [Phone, 48rem or narrower, down to 320 px: the drawer closed (left) and open (right).],
  grid(
    columns: (1fr, 1fr),
    column-gutter: 16pt,
    wf({
      wf-bar[☰ #h(8pt) ‹ commissions]
      block(width: 100%, height: 44mm, inset: 8pt)[
        #text(weight: "bold")[▣ Untitled ↪] \
        #muted[Folder · ◆ Private · Mounted]
        #line(length: 100%, stroke: 0.4pt + colors.rule)
        #rows(
          [▸ attachments], [◆ Private],
          [▸ products], [◆ Private],
          [▸ slots], [◆ Private],
          [· changelog], [◆ Private],
        )
      ]
    }),
    wf(grid(
      columns: (80%, 1fr),
      block(width: 100%, height: 54mm, inset: 7pt, stroke: (right: 0.6pt + colors.ink))[
        #grid(columns: (1fr, auto), text(weight: "bold")[My Den], [✕])
        ▾ \~ \
        #ind(1)▸ accounts \
        #ind(1)▸ characters \
        #ind(1)▾ commissions \
        #ind(2)#cur[▸ Untitled ↪] \
        #ind(1)▸ posts ↪ #muted[not shown yet] \
        #v(6pt)
        #muted[── View ──────────] \
        (○ ) Show archived and \
        #ind(2)deactivated \
        #v(6pt)
        Accounts \
        (◕) alice #h(1fr) Sign out
      ],
      block(width: 100%, height: 54mm, fill: luma(222))[],
    )),
  ),
)

#figure(
  caption: [The pane's other states (the phone shows the same pane at full width).],
  {
    let pane-state(title, sub, msg, label) = stack(
      spacing: 4pt,
      wf(block(width: 100%, height: 27mm, inset: 7pt)[
        #text(weight: "bold", title) \
        #muted(sub)
        #line(length: 100%, stroke: 0.4pt + colors.rule)
        #msg
      ]),
      align(center, text(size: 7.5pt, fill: colors.muted, label)),
    )
    grid(
      columns: (1fr, 1fr, 1fr),
      column-gutter: 8pt,
      row-gutter: 10pt,
      pane-state([▣ posts ↪], [Folder · ◆ Public · Mounted], [ⓘ Content not shown yet.], [Content not shown yet]),
      pane-state([▤ changelog], [File · ◆ Private], [ⓘ Content not shown yet.], [A file]),
      pane-state([▢ Some commission], [Commission · Can't open], [ⓘ You can see that this exists, but you can't open it.], [A card (hand-made fixtures only)]),
      pane-state([▣ products], [Folder · ◆ Private], [This folder is empty.], [An empty folder]),
      pane-state([Nothing here], [], [This doesn't exist, or you can't open it. \ \[ Back to My Den \]], [Not found: every cause, one copy]),
    )
  },
)

*A card shows no visibility,* because a card viewer never learns the level set for others (My Den rulings, decision 5). It shows its type as a word (D-23). *The not-found view has no breadcrumbs,* because the not-found answer carries none.

*Where 0.1 differs from your mockup* (the shell study, "In one screen"):
+ No "Status: in progress" line. A commission's status is part of its content, and 0.1 serves names and visibility only.
+ No `profile` folder. Profiles live in their own public directory, `/profile`, outside anyone's Den (My Den rulings, decision 16).
+ No `participants` file yet. It arrives with its own slice.
+ No dotfiles toggle yet. No dotfile exists, and Project MVP's rule allows no screen without a backend capability behind it.
+ The path bar shows names ("Untitled"); the browser's address bar shows keys (the commission's id).

#figure(
  caption: [When Zurfur isn't answering (D-35): the gate's own page, for a signed-in page whose session check ran out or whose backend is down. No frame, no tree and no form; the session cookie stays.],
  wf(width: 62%, block(width: 100%, height: 30mm, inset: 9pt)[
    #muted[zurfur]
    #v(6pt)
    #text(weight: "bold", size: 8pt)[Zurfur isn't answering right now.]
    #v(5pt)
    \[ Try again \]
  ]),
)

#let tick(on, label, value) = grid(
  columns: (10pt, 1fr),
  column-gutter: 3pt,
  if on [☑] else [☐],
  [#label \ #muted(value)],
)

#figure(
  kind: image,
  caption: [Report a bug (D-34): the button in the top bar (top), the form over the frame (left) and after sending (right). On a phone the form fills the screen, and the button sits in the drawer under "Accounts", or is an icon in the top bar where there's no drawer. ✲ stands for the bug icon.],
  {
    wf(wf-bar(grid(
      columns: (auto, 1fr, auto, auto, auto, auto, auto),
      column-gutter: 9pt,
      text(weight: "bold")[zurfur], [\~ / commissions / Untitled], [My Den], [Accounts], [✲ Report a bug], [(◕) alice], [Sign out],
    )))
    v(8pt)
    grid(
      columns: (1.45fr, 1fr),
      column-gutter: 12pt,
      wf({
        wf-bar(grid(columns: (1fr, auto), text(weight: "bold")[Report a bug], [✕]))
        block(width: 100%, inset: 8pt)[
          #set align(left)
          *What happened?*
          #block(width: 100%, height: 11mm, stroke: 0.6pt + colors.ink, inset: 4pt, above: 3pt)[#muted[What did you do, what did you expect, and what happened instead?]]
          #v(3pt)
          *Sent with your report*
          #v(2pt)
          #tick(true, [Page], [`/den/commissions/0192f0c4-…/slots` \ This address can contain the ids of your Accounts, characters and commissions.])
          #tick(true, [Page type], [My Den])
          #tick(true, [Zurfur version], [9a01808 (local changes)])
          #tick(true, [Browser], [Mozilla/5.0 (X11; Linux x86_64) …])
          #tick(true, [Window size], [1440 × 900])
          #tick(false, [Display settings], [#text(fill: luma(175))[reduce motion on · more contrast off · forced colours off] · Not sent])
          #v(3pt)
          #muted[Sent as \@alice.example. Zurfur also records when it arrives. \ Never sent: your cookies, sign-in tokens, other pages you visited, or what's on your screen.]
          #v(2pt)
          ⓘ This test stack forgets reports when it stops.
          #v(4pt)
          \[ Send report \] #h(8pt) Cancel
        ]
      }),
      wf({
        wf-bar(grid(columns: (1fr, auto), text(weight: "bold")[Report a bug], [✕]))
        block(width: 100%, inset: 8pt)[
          #set align(left)
          ✓ Thanks. Your report is saved.
          #v(3pt)
          #muted[Reference: …3e7a91c2]
        ]
      }),
    )
  },
)

#text(size: 8.5pt, fill: colors.muted)[☑ sent · ☐ not sent: the display settings (reduce motion, more contrast, forced colours) start unticked (D-37), their value greyed. alice is on the dev profile, so the form says the stack forgets reports; on `just dev` that line is absent. The handle and the values are examples. The same form, without its dialog, is the page `/report-a-bug`, which works without the page's script.]

= Success criteria

0.1 is done when all of these hold. Each says how it is checked.

#table(
  columns: (auto, 2.2fr, 1.5fr),
  table.header([\#], [Criterion], [How it is checked]),
  [SC-1], [You complete the 0.1 click-through on your own machine, as two walks, on `main`. First, your own Den on `just dev`: sign in with your real Bluesky handle, run the fill script, and browse your Accounts and commissions. It is sparse: no characters and no Workflows (D-30, D-33). Second, alice's Den on the dev profile, and bob's if the user switch survives: the commission "Untitled", the Workflow "Queue" and the archived "Old sketch" (D-22). What neither holds (a folder long enough for "More", a card) you see on `just dev-mock`, whose hand-made extras show it, and in the tests. If bug reports made 0.1, you also file one during the first walk and find it with `just bug-reports` (D-34).], [Your two walks at acceptance, at the end of the release, recorded in the final status report.],
  [SC-2], [Every acceptance criterion maps to a green test or, where a test can't see it, to a recorded check.], [The Definition of Done's mapping, attached to each feature branch's PR to `main`.],
  [SC-3], [`just gate` and CI are green on `main` after the shell, the time-limit slice, and the identity, bug-report (unless cut) and My Den branches merge.], [CI on each merge.],
  [SC-4], [No security must-fix is open: the identity final verdict ("ship", 2026-10-08), the follow-up slices' reviews, the joining packages' reviews, the time-limit slice's review, Mantis's review of bug reports (unless cut), and the end-of-slice My Den review.], [The review files under `.understand/`.],
  [SC-5], [The privacy rules hold, in memory and on PostgreSQL: the same not-found for every cause, and on PostgreSQL both causes in each pair run the same database statements; no address or field can name another user's Den, no answer names a User other than you, and no answer carries a real path, a count or a hash; what is yours never opens what is mounted into it; hidden entries are dropped before a page is cut, so no page is shortened by what's hidden and no page token hints at it; the reads outside the Den open only what it would open; no handle reaches a log line at `info` or above; the web app's only request to another host is your own avatar; a bug report carries only what its form shows, its reporter comes only from the session, and its text can't rewrite the terminal that prints it.], [Named tests, listed in the Technical Design Document's security sections.],
  [SC-6], [A keyboard-only walk of My Den works on a desktop; the phone layout works at 320 px with no sideways scroll; text, icon, control-edge and focus contrast meet WCAG 2.2 AA in the dark theme.], [Crow records the keyboard walk and the 320 px run once. Contrast is checked on every gate by a test over a declared list of colour pairs from `tokens.css` (D-25).],
  [SC-7], [`main` shows no screen without a backend capability behind it, outside mock mode (`just dev-mock`), which refuses to start in a production build (Project MVP's rule; D-24).], [Mock mode's boot-guard test, and the `just dev` walk, where every My Den screen reads the real backend. Until My Den reaches `main`, the shell's frame without a tree keeps `main`'s `just dev` free of a Den, pinned by its own test.],
  [SC-8], [Every document and every slice had its QA pass before reaching you, with the challenges and resolutions attached.], [The QA sections of each document and PR.],
  [SC-9], [No request waits on the API for more than 15 s, and a backend that doesn't answer never signs you out: a signed-in page shows "Zurfur isn't answering right now", with the signed-in headers, and no form action runs behind it (D-35).], [The time-limit slice's named tests (a backend that never answers, a page whose reads hang, a form action that records whether it ran), and the `just dev` walk with the API stopped.],
)

= Choices already made for you (veto any)

These are choices made inside your rulings: in the shell study, in the build, and one the corpus answers. Work proceeds on them; say so if you want one changed.

#table(
  columns: (1.3fr, 2.4fr),
  table.header([Choice], [What it means]),
  [Folders first, then files], [Each group keeps the server's order (by name), as your mockup's right pane shows; a folder from a later page moves above the files already shown. The tech lead allowed grouping; the den.proto draft words its promise as "clients keep it within each kind".],
  [Your root shows as `~`], [As in your mockup. The pane's title there reads "My Den".],
  [The tree opens down to the open item], [It highlights the item and leaves it closed; its contents are in the right pane.],
  [Dark only in 0.1], [The design language makes dark the default. Switching to light needs a remembered choice, which belongs in your preferences file, and that isn't built yet. The device's own setting can't trigger it, because browsers report "light" when someone has expressed no preference.],
  [The landing page links to My Den], ["Your commissions will appear here." becomes a link, "Open My Den", on every stack.],
  [Nothing about the tree is kept in browser storage], [Den addresses hold DIDs and commission ids, which a shared computer would keep.],
  [Den pages have a generic title], ["My Den · Zurfur". The frame announces the item's name instead, because names in titles reach the browser's history (the security review of the shell's second step).],
  [An unknown soft-deleted word shows "Set aside"], [A build that meets a word it doesn't know never shows the item as live.],
  [After you found an Account, `~` stays open and reloads], [So the new Account is one click away.],
  [The phone's top bar], [The avatar, the handle and "Sign out" are in the drawer.],
  [Placeholders until the Claude Design sync], [The fonts Inter and JetBrains Mono, Lucide icons and a lavender accent.],
  [A commission's own level is its stored visibility], [The Commission page's Den banner: "Its visibility follows the Den's three levels." Nothing widens it in 0.1.],
  table.cell(colspan: 2, fill: colors.faint)[*New in version 4: the time limits (D-35)*],
  [A backend that isn't running gets the same page], [The gate doesn't tell "down" from "slow", and needn't: both read "Zurfur isn't answering right now", where today a stopped backend sends you to sign in.],
  [Public pages keep today's stance], [On the landing and sign-in pages, which hold nothing private and clear no cookie, a backend that doesn't answer still shows you signed out, as today. Your ruling is about the gate, in front of every signed-in page.],
  ["Try again" on that page], [A link back to the same address. The page has no frame, so no form can be sent from it.],
  [A folder read that runs out keeps today's behaviour], [The web lead also planned a "Couldn't load. Retry." row in the tree. It waits with the loading states, since your ruling names the limits and the gate's page for this slice; until then a folder above the open item that runs out is left out, and the tree opens less far.],
  table.cell(colspan: 2, fill: colors.faint)[*New in version 4: bug reports (D-34), from the proposal's own veto list*],
  [The names], [`BugReport` (the domain element), the table `bug_report`, the contract's `BugReportService`, and "reference" for the last 8 characters of a report's id.],
  [A bug report isn't a Den node], [While nobody can read their own reports back. A "Your reports" page would move them into the reporter's Den (parked).],
  [The operator read runs on the machine with the database, in 0.1], [Like `migrate`. Your ruling on the command-line tool keeps user commands on HTTP because an in-process tool holds database admin power, so from the alpha the read moves to HTTP behind an operator's sign-in (parked).],
  [The limits], [10 reports an hour per reporter; 4,000 characters of text; a browser text over 512 bytes cut and shown as cut; 10 seconds to send (D-35).],
  [The form's manners], [No keyboard shortcut (the tree already uses letter keys); the draft kept in the frame's memory, never in browser storage, with no "discard?" prompt; the form fills the screen on phones; calm copy, with red only for a system error; "Report this problem" on the error view; test stacks say they forget reports.],
  [No screenshots], [An image is an upload, which waits for moderation's scan gate (D-26), and a screenshot can show other people's private names.],
)

= Decisions: product

D-1 to D-5 are version 1’s. D-17 to D-27 are the leads' questions in version 2, D-28 to D-33 their questions on joining the halves, and D-34 and D-35 the two additions of 2026-10-09, each marked with the team that raised it. D-6 to D-16, D-36 and D-37 are in the Technical Design Document. *You ruled every one on 2026-10-09:* D-1, D-17, D-34 and D-35 first, then the rest by phone the same day, as the tech lead recorded them (`rulings-2026-10-09.md`). Each block keeps Claude's proposal, labeled, above your ruling, and below it any note of yours and what the ruling changes. One ruling differs from Claude's answer: D-27. D-4’s contract half is in den.proto, which you approved as a whole (D-6).

#decided(
  id: "D-1",
  title: [Your own Den after a real sign-in: in 0.1, or next?],
  date: "2026-10-09",
  question: [The release name promises "sign in with Bluesky and browse My Den". As version 2 planned it, the halves ran on different backends: real sign-in on the everyday stack, which keeps its data in PostgreSQL, and My Den on the in-memory dev profile, signed in as the fixture users. That follows your ruling that My Den runs on seeded in-memory data as well as the web's mock (question P7). Joining them means My Den reads PostgreSQL after a real sign-in.],
  options: (
    [*Side by side in 0.1, joined in 0.2.* Version 2’s answer.],
    [*Joined in 0.1, without character storage.* My Den also runs on the everyday stack after a real sign-in. Joining adds 13 work packages, 17 units (the Delivery Plan): the den routes on every stack, and the web's frame on every stack; den's access check, with the reads that bypass the Den moved behind it; your real root, named from your own PDS; your real Accounts and commissions from PostgreSQL; a PostgreSQL test setup and the route tests on it; the not-found timing re-test; a script that fills your Den; and the walk on `just dev`. Joining adds one unit to the critical path, and the larger security review another, to 17. Your `characters` folder says "This folder is empty." (D-33), because PostgreSQL can't store characters yet.],
    [*Joined in 0.1, fully.* As (b), plus character storage (M) and a way to make a character (M to L): a `character` table keyed by the Character's DID, with its keeper, its name, Private only and a tombstone time; and a route, a contract message, and a command or a form. 4 to 5 more units, sized by the identity lead, and two new forks: a write in a read-only release, and the keys. A Character minted today gets the old custodial key list (a cold key, then the operational key), where Characters on ATProto rules the owner-first list, with a key you hold, so every such Character would need re-minting before the alpha.],
  ),
  why: [(b). Under (c), 4 to 5 units buy an empty folder you can't fill, two forks, and Characters minted with the wrong key list. Under (b), the empty folder is honest, and character storage arrives with the Character slice, which must mint with the owner-first list.],
  ruling: [(b) Joined in 0.1, without character storage. First you ruled that the halves join in 0.1 ("I don't understand why ship the two halves and then join them in 0.2. Each team can have more than one total PRs and joining can be done on 0.1."); then you confirmed (b) over (c). Ruled by the Engineer, 2026-10-09.],
  after: [*What your ruling changes* (the Technical Design Document, Feature 3; the Delivery Plan): the den routes are served on every stack from their first slice, and the switch that hid My Den goes. The follow-up for the reads that bypass the Den, and the not-found timing re-test, move into 0.1, before the end-of-slice security review. Acceptance at the end becomes two walks (SC-1). The everyday-stack criteria that assumed no Den (the frame, the phone, the screen reader, SC-7) now describe one frame on every stack. Character storage and a way to make a character wait for the Character slice (the Technical Design Document, "Parked").],
  source: [the Engineer's rulings, 2026-10-09; the four joining pages, `objectives/join-den.md`, `join-identity.md` (section 2 sizes (c)), `join-web.md` and `join-commission.md`; `adapter-pg/src/character.rs` (`unimplemented!`); `adapter-atproto/src/did_minter.rs` (cold, then operational); Characters on ATProto (decisions/54427650), amendment of 2026-09-12 (`[owner, pds, zurfur]`, senior first).],
)

#decided(
  id: "D-2",
  date: "2026-10-09",
  title: [Do today's Accounts pages sit inside the Explorer frame?],
  question: [Today `/accounts` (your Account list and "Found Account") and `/accounts/<id>` (one Account, with its delete form) are plain pages with a header.],
  options: (
    [*Yes.* Every signed-in page uses the frame. The Accounts pages fill the right pane, and the top bar keeps both "My Den" and "Accounts". On an Accounts page the path bar reads "Accounts" and nothing in the tree is highlighted. Accounts then appear twice in one frame, through My Den and through the older read, and the two can disagree, for example on a deactivated Account.],
    [*No.* Only My Den gets the frame; the Accounts pages keep today's plain layout.],
  ),
  why: [(a). One frame for the whole signed-in app matches your ruling that the Den is the web app, and there's one layout to test instead of two. The duplicate disappears when Accounts move into the Den; folding them in later only removes a link.],
  ruling: [(a) Yes, in the frame: every signed-in page uses the frame, and the Accounts pages fill the right pane. Ruled by the Engineer, 2026-10-09.],
  source: [the shell study, "Questions for you", Q1.],
)

#decided(
  id: "D-3",
  date: "2026-10-09",
  title: [How does the tree behave with the keyboard and the mouse?],
  question: [You asked for a tree like VS Code's or Obsidian's.],
  options: (
    [*Keyboard like a file explorer.* Tab enters the tree once, the arrow keys move, Right opens and Left closes, and a letter jumps to a name (the W3C tree pattern). Before the script runs, the tree is plain links.],
    [*Keyboard like a web menu.* Every item is its own Tab stop, and each folder has its own open/close button.],
    [*Mouse: clicking a folder's name opens it in the pane* and highlights it; only the arrow beside it expands or collapses it.],
    [*Mouse: clicking a folder's name opens it and also expands it* in the tree.],
  ),
  why: [(a) with (c). The W3C guide's caution is about site menus, and its own navigation-tree example is a tree of links like this one. Under (d) the tree repeats the folder's contents right beside the pane; your mockup shows the open item collapsed in the tree, with its contents on the right.],
  ruling: [Keyboard: (a), like a file explorer. Mouse: (c), a click on a folder's name opens it only; the arrow beside it expands it. Ruled by the Engineer, 2026-10-09.],
  after: [*Your note:* "I enjoy the way MacOS does it most with the columns as you go opening. This last thing may be deferred as a theme in the future." *Parked:* a Finder-style column view, where each opened folder becomes a new column, as a future theme or view (the Technical Design Document, "Parked"). 0.1’s tree keeps the behaviour you ruled.],
  source: [the shell study, Q2.],
)

#decided(
  id: "D-4",
  date: "2026-10-09",
  title: [Which words do archived commissions and deactivated Accounts get?],
  question: ["Show archived" lists archived commissions, and a deactivated Account to its Owner. The corpus uses two words: archiving is a commission's soft delete (your ruling P8), and Account Deletion renders a soft-deleted Account as "deactivated".],
  options: (
    [*Each its own word, sent by the backend.* The toggle reads "Show archived and deactivated", and items show "Archived" or "Deactivated". den.proto's soft-deleted mark becomes a word, `archived` or `deactivated`, instead of a yes/no flag.],
    [*Each its own word, picked by the web* from the item's type name, with no contract change. But type names are provisional and meant only to pick an icon.],
    [*One word for both,* which you pick.],
  ),
  why: [(a). The backend knows which it is, so the web never guesses, and the command-line tool gets the same word. It changes one field in the den.proto draft while that draft is still unratified, the cheapest time.],
  ruling: [(a) Each its own word, sent by the backend: "Archived" for commissions, "Deactivated" for Accounts. Ruled by the Engineer, 2026-10-09.],
  after: [The web builds it on mock data. The contract half, the word field, is in revision 2.1 of the den.proto draft (WP-25), which you approved as a whole (D-6); the field is `soft_delete` (D-14).],
  source: [the shell study, Q3; Account Deletion (decisions/23003138), decision 6.],
)

#decided(
  id: "D-5",
  date: "2026-10-09",
  title: [Does a sign-in finished in a different browser get its own message?],
  question: [Since the browser binding, a sign-in started in one browser and finished in another (some phones' in-app browsers do this) is refused. Today it shows the generic failure.],
  options: (
    [*Yes.* A new callback code, `browser_mismatch`, with its own message (its wording is D-18).],
    [*No.* Keep the generic failure message.],
  ),
  why: [(a). The visitor learns what to do, and the message tells them nothing they don't already know.],
  ruling: [(a) Yes, its own message. Ruled by the Engineer, 2026-10-09.],
  after: [The callback code `browser_mismatch`, with its own message (its wording is D-18). The sign-in port gains a `BrowserMismatch` refusal beside `AccountMismatch` for it, covering all three browser refusals; D-12 reuses it (WP-2).],
  source: [the end of `.understand/20261008-hardened-resolver-design.md`; QA's slice 7 question.],
)

#decided(
  id: "D-17",
  title: [Should sign-in ask your PDS only for your identity? #raised[identity]],
  date: "2026-10-09",
  question: [
    - *What sign-in asks for today:* `atproto transition:generic`, the constant `OAUTH_SCOPES` in `adapter-atproto`, on `main` and on the identity branch (checked 2026-10-09).
    - *What that grants.* The atproto OAuth spec (atproto.com/specs/oauth, read 2026-10-09) calls `transition:generic` "broad PDS account permissions, equivalent to the previous 'App Password' authorization level". It may "write (create/update/delete) any repository record type". It excludes account management: handle, email, deletion and migration. The same spec: "A client may include only the `atproto` scope if they only need account authentication."
    - *What the corpus says.* Publish Consent & OAuth Scopes, decision 1: sign-in "requests the minimal atproto scope. No repo access at the door." Write access is asked for at first use (decision 2), and `transition:generic` is only a fallback for a PDS without granular scopes, offered as its own explicit consent moment (decision 4).
    - *What uses the grant in 0.1:* nothing. The profile read is unauthenticated, and the record writer holds its own token (the identity lead, checked by its QA).
    - *Why it matters now.* The account you sign in with is your real Bluesky account. Each sign-in stores a refreshable session carrying that grant, sealed, and sign-out doesn't delete it (parked until PDS writes use the sessions). The spec says a public client's sessions "should" last at most 2 weeks.
    - *Why the earlier precondition doesn't bind 0.1.* The unified-identity investigation said the production client and the first-use scope request must exist first. But 0.1’s local-development client carries its scope in its own `client_id` (the spec's default there is `atproto`), so no production client is needed; and nothing in 0.1 writes to your repo, so the first-use request waits for the first feature that does (the character `owner` record).
  ],
  options: (
    [*Yes, in 0.1.* One small slice sets the scope to `atproto`, with QA and a security review, before sign-in reaches `main`. The old full-write sessions on your machine are deleted once: by the slice, which deletes only the stored sessions, or by `just db-reset`, which wipes all local data, the Accounts you founded included.],
    [*Not in 0.1, but before the alpha.* Sessions with write access to your real repo stay stored after sign-out through 0.1, and whether sign-out should revoke them becomes a question.],
    [*Amend Publish Consent* to allow full access at sign-in.],
  ),
  why: [(a). The decision already says so; the change is one constant and its tests; and the real sign-in check proves it still works. *Unverified:* that a token with `atproto` alone can't write. The spec says `atproto` alone is for account authentication, but doesn't state the write prohibition outright.],
  ruling: ["Fix the sign-in scope in 0.1": sign-in asks only for `atproto`, and it is in 0.1’s scope.],
  after: [*The work* (the Delivery Plan, WP-26): a small identity slice on the identity branch, after the close-out slice, reviewed by Heron for security, before sign-in reaches `main`. *How it's checked:* a test on the scope Zurfur sends ("Sign in with my handle", last criterion), and the sign-in address carrying no `transition`. How the old sessions are cleared is D-32.],
  source: [identity's question 1 and correction 3 (Fox; QA Magpie); moderation's QA confirmed the constant; Publish Consent & OAuth Scopes (decisions/29687820), decisions 1, 2 and 4; atproto.com/specs/oauth, read 2026-10-09; the Engineer's ruling, 2026-10-09.],
)

#decided(
  id: "D-18",
  date: "2026-10-09",
  title: [What does a sign-in refused for its browser say? #raised[identity]],
  question: [D-5 gives the refusal its own message. But the refusal covers three cases: no binding cookie, a cookie with no live sign-in, and the wrong cookie. Cookie and sign-in both expire after 10 minutes, so someone who takes 11 minutes at their PDS, in the same browser, would read that the sign-in "was started in a different browser". Zurfur can't tell that case from a real second browser: both arrive with no cookie.],
  options: (
    ["This sign-in expired or was started in a different browser. Please sign in again here."],
    [Version 1’s wording: "This sign-in was started in a different browser. Please sign in again here."],
  ),
  why: [(a). It is true in every case the code sends there, and still tells the visitor what to do.],
  ruling: [(a) "This sign-in expired or was started in a different browser. Please sign in again here." Ruled by the Engineer, 2026-10-09.],
  after: [The different-browser message (WP-2) builds this text.],
  source: [identity's question 3 and correction 8 (Fox; QA Magpie).],
)

#decided(
  id: "D-19",
  date: "2026-10-09",
  title: [Does the pre-alpha tag close den.proto, and does your review still come first? #raised[web and den]],
  question: [Your stages ruling keeps the proto tagged `v1` through breaking changes in pre-alpha. Two rules in the contract bear on that. First, its lifecycle file says `v1` is pre-release "until the first release tag is cut". The versioning contract's example of such a tag is a contract tag (`api/v1.4.0`), not a product tag like `pre-alpha-v0.1.0`, and the planned check against the last released tag isn't built yet: CI checks each PR only against its base. Second, until the first published client, which you ruled is the alpha, a breaking change goes in as its own declared "baseline reset" PR (The API Contract, amendment of 2026-08-30). That line was ruled but never written into the versioning contract. If the pre-alpha tag counted as a release tag, den.proto would close at 0.1.],
  options: (
    [*The pre-alpha tag doesn't end the pre-release period, and your review still comes first.* After an edit you review, the lifecycle file and CI's comment say pre-release ends at the first alpha tag. The den.proto package waits for your review, as planned. A later breaking edit before the alpha is its own declared baseline reset.],
    [*As (a), but if your review runs late, den.proto goes in without it.* If the review hasn't landed one unit before the den module finishes, den.proto and the web's wiring go ahead on Claude's strict answers (a card carries name and type only; an archived card is the not-found), and your review is due before the alpha tag. The web lead's proposal, for the plan's risk that your review lands late.],
    [*The pre-alpha tag ends the pre-release period.* den.proto closes at 0.1, and your review blocks, as in (a).],
  ),
  why: [(a). Your release rule holds a one-way door for your explicit review before it's built, and the den lead keeps the review first. (b) only helps if the review is late, and the review is already at the top of your queue. (a) also keeps your stages ruling literal: the proto stays `v1`, and nothing closes before the alpha.],
  ruling: [(a) The pre-alpha tag doesn't end the pre-release period, and your review comes first. Ruled by the Engineer, 2026-10-09.],
  after: [*What it changes:* the edit to the contract's lifecycle file and CI's comment (pre-release ends at the first alpha tag) is a contract change, and comes to you with den.proto (WP-13). You approved den.proto as a whole the same day (D-6); its text still comes to you on that PR. A later breaking edit before the alpha is its own declared baseline reset, as the retired owner's list is (D-28).],
  source: [web's question 2 (Fennec; QA Crow); den's question 3 (Badger; QA Owl); `contract/lifecycle.toml`, `contract/VERSIONING.md` (its two breaking-change gates) and CI's contract job, read 2026-10-09; The API Contract (decisions/40992770), amendment of 2026-08-30; the My Den question sheet, C13 (the alpha is the first published client).],
)

#decided(
  id: "D-20",
  date: "2026-10-09",
  title: [How long may a name be, and what shows when a stored name fails? #raised[den and commission]],
  question: [Every name the Den shows passes one name rule, and an arm that fails to convert a stored name would break the whole listing. Some names have caps today: a Workflow name, 196 characters, and a file name, 255 bytes. A display name's 640 bytes is the profile lexicon's limit, which Zurfur doesn't check. Commission titles, slot titles and character names have no cap.],
  options: (
    [*Cap every Den name at 2048 bytes,* atproto's hard limit for a DID. A DID ends a User's name chain, and the atproto DID spec makes it ASCII without `/`. Zurfur's DID type checks only its syntax today, so identity's DID type, or the name arm, refuses a DID over 2048 bytes; then that last step can't fail. Today's capped names fit. Commission titles, slot titles and character names take a rule at least as strict as the Den's, checked when they're written and pinned by a test. A stored name that still fails shows the fixed word "Unnamed", never its key, and the read goes on.],
    [*No new caps at write.* Any name that fails the Den's rule shows "Unnamed".],
  ),
  why: [(a). The cap comes from the protocol, every name 0.1 shows fits under it, and checking at write means a title stored after the rule can't fail a read. Under (b), titles people typed and saved show as "Unnamed", with no warning when they were saved. Under (a), creating a commission, uploading a file or declaring a slot starts refusing names the rule refuses. A name stored before the rule is never re-checked when it is loaded; it shows "Unnamed".],
  ruling: [(a) 2048 bytes, with "Unnamed" as the fallback. Ruled by the Engineer, 2026-10-09.],
  after: [Titles, slot titles and character names are checked at write; "Unnamed" is the last fallback ("Browse a folder", first criterion), and an Account shows its handle in its place (D-31). Joining makes the fallback live on your real data. Names stored before the rule need no `just db-reset`: the rule runs only when a name is written, a stored name loads as plain text, and one that fails shows "Unnamed" while the read goes on (the tech lead, 2026-10-09).],
  source: [den's question 5 (Badger; QA Owl); commission's question 3 (Wolf; QA Jay); atproto.com/specs/did (2048 characters) and the profile lexicon (display name, 640 bytes), both read 2026-10-09; Zurfur's DID and profile types on `main` (no length cap).],
)

#decided(
  id: "D-21",
  date: "2026-10-09",
  title: [What does an opened slot say? #raised[commission]],
  question: [A slot opens as a folder. 0.1 doesn't show its notes, or later its occupant. Your ruling fixes the message per type, and the approved fixture picture shows slots that open empty. This changes that picture and version 1’s "each opening empty".],
  options: (
    [*"Content not shown yet."* So a slot with notes never looks empty.],
    [*"This folder is empty."*, as the approved picture shows.],
  ),
  why: [(a). Under (b), a slot whose notes 0.1 doesn't show would say it's empty. The den and web leads agree; the web's mock slots change to match in a small follow-up. Reversible in pre-alpha.],
  ruling: [(a) "Content not shown yet." Ruled by the Engineer, 2026-10-09.],
  after: [*Your question, answered:* you asked whether a slot is a symlink, a mount or a container of symlinks. The tech lead answered from The Den (decision 17 and ruling 23): a slot is a real directory holding its own files, with its character referenced by DID in its data and shown as a card. Commissions are the ones mounted. *What it changes:* the approved fixture picture's empty slots and version 1’s "each opening empty"; the web's mock slots change to match when the shell goes to `main` (WP-27).],
  source: [commission's question 2 (Wolf; QA Jay); den's page, "Not this release"; web's page, "Cited, not re-asked"; the My Den architecture, diagram 4a.],
)

#decided(
  id: "D-22",
  date: "2026-10-09",
  title: [Seed one archived commission on the dev profile? #raised[commission]],
  question: [The approved fixture picture says the seed holds no archived commission, so "Show archived and deactivated" and "only Participants see it" can be checked by hand only on `just dev-mock`, not on the real backend.],
  options: (
    [*Yes: "Old sketch",* archived, with alice its only Participant. One seed row and one golden response. alice sees it only with the toggle on, and opens it by typing its path; bob never sees it.],
    [*No.* You see the archived state on `just dev-mock` and in tests only, as version 1 says.],
  ),
  why: [(a). You can check the hiding rules by hand on the real backend. The den lead agrees. It changes the approved fixture picture, and the hiding change's note that the dev world holds nothing archived.],
  ruling: [(a) Yes, "Old sketch". Ruled by the Engineer, 2026-10-09.],
  after: [Archived, with alice its only Participant: one seed row and one golden response (WP-17, WP-20). It changes the approved fixture picture, and the hiding change's note that the dev world holds nothing archived. bob's half is checked with the user switch, or by a named test if the switch is cut.],
  source: [commission's question 1 and corrections 4 and 5 (Wolf; QA Jay); den's page, "Not this release"; the My Den architecture, diagram 4b.],
)

#decided(
  id: "D-23",
  date: "2026-10-09",
  title: [How does a card show its type? #raised[web]],
  question: [Your ruling (My Den rulings, decision 12) says an actor's card shows its name, its type and the viewer's resolved level, and D-8 keeps a card to its name and type. The card the web built shows the name and a "can't open" mark, but no type.],
  options: (
    [*Follow the ruling.* Show the type as a word for the types the web knows (Account, Character, Commission), and no word for a type it doesn't know.],
    [*No type in 0.1.* Only hand-made fixtures make a card, and type names are provisional; the type comes with the type catalog. This needs a corpus change to your ruling.],
  ),
  why: [(a). It keeps your ruling with no corpus change, costs one small change in the web's wiring, and an unknown type shows nothing rather than a guess.],
  ruling: [(a) The type as a word for the types this build knows; none for an unknown type. Ruled by the Engineer, 2026-10-09.],
  after: [In the web's wiring to den.proto (WP-10).],
  source: [web's question 3 (Fennec; QA Crow); My Den rulings (decisions/2026100701), decision 12; the Technical Design Document, D-8.],
)

#decided(
  id: "D-24",
  date: "2026-10-09",
  title: [May `main` carry a My Den that only the mock shows? #raised[web]],
  question: [The web lead proposes that the frame and My Den on mock data reach `main` once their five slices merge, before the backend. That is the tech lead's call first: it replaces "one integration branch for My Den" and costs one web restack. If the tech lead agrees, `just dev-mock` on `main` shows a My Den with only the mock behind it. Your rule (Project MVP): a frontend for exactly what the backend supports.],
  options: (
    [*Yes, in mock mode only.* Mock mode (`just dev-mock`) refuses to start in a production build, so no production screen lacks its backend. The success criterion on screens without a backend (SC-7) reads "outside mock mode". Version 2 wrote "outside a dev build", but `just dev` runs a dev build too, so that wording would have exempted it (the web lead, on joining).],
    [*No.* The shell waits on its own branch until My Den reaches `main` with the backend.],
  ),
  why: [(a). The rule protects the people using Zurfur from screens with nothing behind them, and only you run mock mode. It lets the web's wiring start from `main`. Between the shell's merge and My Den's, the shell's frame without a tree keeps `main`'s `just dev` free of a Den; the branch that serves the real Den deletes it.],
  ruling: [(a) Yes, in mock mode only. Ruled by the Engineer, 2026-10-09.],
  after: [The tech lead confirmed on 2026-10-09 that the shell goes to `main` before the identity branch. SC-7 reads "outside mock mode".],
  source: [web's question 1 (Fennec; QA Crow), and web's joining page, question 1; Project MVP, "Core capabilities"; the tech lead's call on the merge order, 2026-10-09.],
)

#decided(
  id: "D-25",
  date: "2026-10-09",
  title: [How is contrast checked? #raised[web]],
  question: [Version 1 left the contrast check's tool open (the accessibility success criterion, SC-6). WCAG 2.2 AA sets 4.5:1 for text, and 3:1 for large text and for non-text, such as icons, control edges and focus rings (success criteria 1.4.3 and 1.4.11). The colours are placeholders until the Claude Design sync.],
  options: (
    [*A test on every gate,* over a declared list of colour pairs from `tokens.css`: each text, icon, control-edge and focus colour on each background it sits on. Crow records the keyboard walk and the 320 px run once.],
    [*By hand once,* with a contrast tool, recorded at sign-off.],
  ),
  why: [(a). The sync will change every value; a gate test re-checks each change, where a hand check would be redone each time.],
  ruling: [(a) A test on every gate. Ruled by the Engineer, 2026-10-09.],
  after: [Over a declared list of colour pairs from `tokens.css`, built in the live run (WP-11); Crow records the keyboard walk and the 320 px run once.],
  source: [web's question 4 and objective 5 (Fennec; QA Crow); W3C WCAG 2.2, read 2026-10-09 by web's QA.],
)

#decided(
  id: "D-26",
  date: "2026-10-09",
  title: [When is moderation's first deadline? #raised[moderation]],
  question: [Project MVP sequences the "Security-first" items ahead of feature work; two are moderation's, CSAM scanning and a report path. The moderation study set two more decisions for before any public deploy: database roles (today's dev database user is a superuser), and whether user text and file names stay in changelog entries for good. 0.1 is safe without them only because it isn't deployed. Today's API already takes a commission file upload, served without a scan, and free text that other Participants read. Pre-alpha has no sign-in allowlist, so a pre-alpha build put on a domain would open that upload to any Bluesky user. And the floor may need lead time: one scanning service (PhotoDNA) vets applicants through a third party, and whether that needs a legal entity, which Zurfur doesn't have, is unverified.],
  options: (
    [*Whichever comes first:* the first build that anyone but you can reach, whatever its stage; and, for the scan gate, the first slice that adds or changes an upload path.],
    [*The alpha.* Simple, but a pre-alpha build put on a domain would be reachable without the floor.],
    [*Now, in 0.1,* ahead of My Den, as Project MVP's order reads.],
  ),
  why: [(a). Who can reach a build sets the risk, not its stage's name. The upload-path half matters if you take the study's proposal that an upload becomes a node only once it's scanned clear: an upload slice built before the gate would be rebuilt. 0.1 is neither, so (a) adds no 0.1 work. (c) delays My Den for a risk 0.1 doesn't carry.],
  ruling: [(a) Whichever comes first: the first build that anyone but you can reach, or the first slice that changes an upload path for scanning. Ruled by the Engineer, 2026-10-09.],
  after: [The tech lead adds moderation's floor to the deployment checklist: the scan gate in front of every upload route, a report path, and your answers on database roles and on changelog text. 0.1 is neither, so it adds no 0.1 work. The commission arms (WP-18) change which file names an upload accepts (D-20, D-29), not where its bytes go or when it becomes a node, so they aren't the scan gate's trigger (Kestrel's reading, for the moderation lead to confirm).],
  source: [moderation's question 1 and objective 2 (Meerkat; QA Lapwing); Project MVP, "Core capabilities", the Security-first bullet; `.understand/20261008-moderation-study.md`; Microsoft's PhotoDNA page, read by the moderation lead and its QA, 2026-10-09.],
)

#decided(
  id: "D-27",
  date: "2026-10-09",
  title: [Do Vulpes and Wolven pause through 0.1? #raised[vulpes and wolven]],
  question: [Neither has 0.1 work, and Zurfur depends on neither. Vulpes: the three code cuts from your library-only ruling are committed on one disk and pushed nowhere; and a vulpes PR rewrites `characters-atproto.md`, the file the Characters on ATProto decision names as its source of record, which still describes the co-owner claims your 2026-09-12 rulings removed. Wolven: idle since 2026-08-22; its spec and two PRs need rework against today's Vulpes before they can merge; its tree walk is uncommitted.],
  options: (
    [*Pause both until release acceptance, with nothing left on one disk.* The three vulpes cut branches are pushed (no PR), and Wolven's tree walk is committed to a new branch. The vulpes PR on `characters-atproto.md`, rebased, joins your queue below every 0.1 item. Wolven resumes only after sign-off, once you and the Vulpes team settle what it waits on: who attests for Wolven, who may run the lookup from an outside account to a DID, and how Wolven mints its node DIDs.],
    [*Keep Vulpes building* in parallel with 0.1.],
  ),
  why: [(a). 0.1 needs nothing from either, your queue stays on 0.1, and the vulpes document fix is short and backs the "no ownership claim in 0.1" line.],
  ruling: [(b) Vulpes keeps building, in parallel with 0.1. Ruled by the Engineer, 2026-10-09. It differs from Claude's (a).],
  after: [*What it means:* option (b) names only Vulpes, so Wolven still pauses through 0.1, as option (a) has it, with its tree walk committed to a branch. That commit isn't made yet; the tech lead makes it (`v2-carryover.md`, row 95). Vulpes's people are not lent to 0.1. *What it changes:* the Vulpes and Wolven rows of "What each team commits to"; the Delivery Plan's risk R11, whose fallback offered Vulpes's two programmers and now offers the plugin team alone; and a new risk there, R21: Vulpes's PRs share your queue with 0.1’s. No package was staffed from Vulpes, so no package, unit or slack moves, and the critical path stays at 17 units.],
  source: [vulpes's questions (Reynard; QA Rook); wolven's questions (Lobo; QA Shrike).],
)

#decided(
  id: "D-28",
  date: "2026-10-09",
  title: [Retire the owner's commission list? #raised[commission]],
  question: [Three commission reads skip the Den today: the owner's commission list, the changelog read and the file download. Joining moves the last two behind den's access check, because they serve content the Den doesn't show in 0.1. The owner's list (`GET /commissions`) repeats what your Den's `commissions` folder lists, and nothing calls it: neither the web nor the command-line tool (checked by the commission lead; the web lead agrees). Removing it is a contract break.],
  options: (
    [*Retire it.* Remove the route and its contract entry; the create response keeps its shape. As a breaking change before the alpha, it goes in as its own declared baseline-reset PR (D-19; The API Contract, amendment of 2026-08-30), and it goes on the end-of-slice security review's list.],
    [*Keep it, rewired* as a list of your own commissions, each passed through den's check. The package grows from S to M, and a second listing has to stay in step with the Den.],
  ),
  why: [(a). Nothing calls it, the Den's folder replaces it, and a listing that doesn't exist can't drift from the Den. Your stages ruling allows the break at `v1` in pre-alpha. It is reversible: a list can come back later as an addition.],
  ruling: [(a) Retire it, as its own declared baseline-reset PR. Ruled by the Engineer, 2026-10-09.],
  after: [WP-36: the route and its contract entry go, and the create response keeps its shape; its first slice writes the baseline-reset line into `contract/VERSIONING.md`; it is on the end-of-slice security review's list.],
  source: [commission's joining page, question 2 (Wolf); `api/src/routes/commissions/list.rs`; `contract/VERSIONING.md`, whose baseline-reset line is still to be written (`v2-carryover.md`, row 14).],
)

#decided(
  id: "D-29",
  date: "2026-10-09",
  title: [May names hold the zero-width joiner and non-joiner? #raised[commission]],
  question: [Your name rule (My Den rulings, decision 19) refuses every control and format character (Unicode categories Cc and Cf) in node names, file names and titles. Two format characters do visible work. The zero-width joiner (U+200D) builds emoji such as "woman artist" and the rainbow flag. The zero-width non-joiner (U+200C) is part of everyday Persian spelling. Both are Cf (checked with Python's Unicode data, 2026-10-09). Joining makes this real: the rule now meets titles you typed and stored.],
  options: (
    [*Allow those two, and only those two.* The rule keeps refusing control characters, the bidirectional controls and every other format character, for every name the Den checks, Account names included (D-31). A name needs at least one character other than the two joiners, so a name of joiners alone counts as empty.],
    [*Keep refusing every format character.* Such emoji and Persian names are refused at write, and a stored one shows "Unnamed".],
  ),
  why: [(a). Names are display text, not addresses: a Den path is built from keys, so a joiner can't redirect a path, and names are isolated on screen, so it can't reorder a row. The bidirectional controls, the real risk, stay refused. What it costs: two names can look alike while differing by an invisible joiner. Look-alike letters from other scripts already allow that, and the rule never tried to stop them. Under (a), the rule still refuses the flags of England, Scotland and Wales (built from tag characters, also Cf) and the soft hyphen (U+00AD, Cf) that pasted text can carry; a stored one shows "Unnamed".],
  ruling: [(a) Allow U+200D and U+200C, and nothing else from Cf. Ruled by the Engineer, 2026-10-09.],
  after: [*What it changes:* the shared name step in the identity and commission host arms (WP-16, WP-18), and the My Den rulings' decision 19, by a design-sync in WP-13, beside D-7’s spelling and D-31’s Account names. A name needs at least one character other than the two joiners, so a name of joiners alone is refused as empty, with a test. The tech lead also offers you one design-sync for every corpus follow-up your rulings create (`v2-carryover.md`, row 94).],
  source: [commission's joining page, question 4 (Wolf); My Den rulings (decisions/2026100701), decision 19; Python's `unicodedata`, run 2026-10-09 (U+200C, U+200D, the tag characters U+E0067 and U+E007F, and U+00AD: category Cf).],
)

#decided(
  id: "D-30",
  date: "2026-10-09",
  title: [Do your real Accounts' Workflows stay empty in 0.1? #raised[commission]],
  question: [No route creates a Workflow, so on `just dev` every Account's `workflows` folder is empty. On the dev profile, Supreme Arts holds the Workflow "Queue".],
  options: (
    [*Accept it.* Your real `workflows` folders say "This folder is empty."; the Workflow checks stay on the dev profile.],
    [*Add a route that creates a Workflow* in 0.1: a write, in a read-only release.],
  ),
  why: [(a). A create route is a write, which 0.1 leaves out, and the dev-profile walk still shows a Workflow.],
  ruling: [(a) Accept it. Ruled by the Engineer, 2026-10-09.],
  after: [Your real `workflows` folders say "This folder is empty."; the Workflow checks stay on the dev profile.],
  source: [commission's joining page, question 5 (Wolf).],
)

#decided(
  id: "D-31",
  date: "2026-10-09",
  title: [What does an Account show when its stored name breaks the Den's name rule? #raised[identity]],
  question: [Today's Account name type checks only that a name isn't empty and isn't too long (`domain/src/elements/account/value.rs`, checked 2026-10-09). So a real Account's stored name can hold control or format characters, bidirectional overrides included. D-20 shows "Unnamed" for every stored name that fails.],
  options: (
    [*Its handle,* never a cleaned-up name, and the read goes on. From now on, new Account names refuse the same characters as every other name.],
    [*"Unnamed",* as D-20 says for every name.],
  ),
  why: [(a). An Account's handle always passes the rule and tells you which Account it is; "Unnamed" doesn't. A cleaned-up name would show text the Owner never typed.],
  ruling: [(a) Show its handle; new Account names refuse the same characters. Ruled by the Engineer, 2026-10-09.],
  after: [In identity's PostgreSQL Accounts package (WP-33). The My Den rulings' decision 19 gains Account names in WP-13's design-sync, beside D-29.],
  source: [identity's joining page, question 1 (Fox); den's question 5 (D-20).],
)

#decided(
  id: "D-32",
  date: "2026-10-09",
  title: [How are the old full-write sign-in sessions cleared? #raised[identity]],
  question: [Under D-17, sign-in asks only for `atproto`. The sessions already stored on your machine carry the old full-write grant, and D-17 named two ways to clear them once. Joining changes the cost: your Den now shows your real Accounts, which `just db-reset` would wipe.],
  options: (
    [*The scope slice deletes only the stored sign-in sessions.* Nothing reads them back: there is no restore outside tests (identity's QA checked). Their tokens can't be used without the session's own key, which is stored in the same row, though they aren't revoked at your PDS. The deleted rows stay on your disk, sealed, until PostgreSQL vacuums them, and lapse within your PDS's refresh lifetime (at most 2 weeks, by the spec's "should").],
    [*`just db-reset`,* which also wipes your Accounts and the local PDS and DID directory.],
  ),
  why: [(a). It removes the grant and keeps your data. The commission lead had proposed one `just db-reset` before the first joined run, for names stored before the stricter name rule; the tech lead ruled that out on 2026-10-09, because D-20’s "Unnamed" keeps those reads alive.],
  ruling: [(a) The scope slice deletes only the stored sign-in sessions. The delete is a one-way door, so you review that PR before it merges. Ruled by the Engineer, 2026-10-09.],
  after: [The scope slice is WP-26; Heron reviews it too.],
  source: [identity's joining page, question 2 (Fox; Magpie's check that nothing restores a session); D-17; the key binding: atproto.com/specs/oauth ("Tokens are always bound to a unique session DPoP key") and `adapter-atproto/src/auth_store.rs` on the identity branch, as den QA read them for version 2 (`v2-carryover.md`, row 2); the PostgreSQL documentation, "Routine Vacuuming", as identity QA read it; commission's joining page, question 3, and the tech lead's answer on the board, 2026-10-09.],
)

#decided(
  id: "D-33",
  date: "2026-10-09",
  title: [What does your `characters` folder say on PostgreSQL? #raised[web]],
  question: [Under your ruling on D-1, PostgreSQL stores no characters in 0.1, so on `just dev` your `characters` folder lists nothing. The identity arm sets which message it carries, and the web shows either.],
  options: (
    [*"This folder is empty."* True: nothing can be stored there yet.],
    [*"Content not shown yet."* It would hint that something is hidden.],
  ),
  why: [(a). It's true and hints at nothing. It turns false the day character storage lands, and the Character slice replaces it then: the empty PostgreSQL answer is a named exemption from "no stubs", with a pinning test.],
  ruling: [(a) "This folder is empty." Ruled by the Engineer, 2026-10-09.],
  after: [Set by the identity host arms (WP-16). It turns false the day character storage lands, and the Character slice replaces it: the empty PostgreSQL answer is a named exemption from "no stubs", with a pinning test.],
  source: [web's joining page, question 2 (Fennec); the Technical Design Document, "Calls the tech lead makes", the keeper-read row.],
)

#decided(
  id: "D-34",
  title: [A way to report bugs, from the UI #raised[you; proposed by web]],
  date: "2026-10-09",
  question: [You asked: "I need a way to report bugs. Preferably using the UI so that it becomes normalized. Maybe a button that opens a form modal?" The web lead proposed one, and Crow's QA (19 challenges, all closed) shaped it: `proposals/bug-reports.md`. It is a write, in a release whose non-goal says 0.1 adds no new way to change anything, and D-30 turned down a Workflow route on that ground. The difference: a bug report changes nothing in anyone's Den and touches no domain entity.],
  options: (
    [*In 0.1, as a side feature that can't hold the release:* built by people the plan leaves free, reviewed by a second security reviewer, on `main` after the identity branch. If it isn't there by unit 9, it waits for My Den and is 0.2's.],
    [*0.2's first item.* In 0.1 you keep telling the Orchestrator about bugs.],
    [*In 0.1, as an item that holds the release.*],
  ),
  why: [(a). You get it for the walks and your acceptance, which is when you'll find bugs, and it can't delay My Den: no package on the critical path depends on it, and nobody on the path builds it.],
  ruling: [(a): in 0.1, and it can be cut; if it isn't on `main` by unit 9, it moves to 0.2. Follow the proposal, with its QA resolutions. Its other questions keep Claude's answers: reports are stored by Zurfur's API and read with `zurfur bug-reports list`; only a signed-in user can report, and the reporter is always recorded; the full page address, without its query, is attached by default, shown and removable; the button reads "Report a bug". It stays separate from moderation's content reports, and doesn't meet moderation's report-path item (D-26). Ruled by the Engineer, 2026-10-09.],
  after: [*What it changes:* a new side feature, "Report a bug" (US-4.1 to US-4.3), and its wireframe; goal G6; the non-goals "Uploads and editing" (the one new write) and "Deployment" (not the report path); the rate-limit line (the report's own 10 an hour); bug reports in "What each team commits to", as shared ground under the tech lead, built by the people the proposal names (the tech lead's call, confirmed): the backend by moderation's programmers, the web by Finch, the security review by Mantis; success criteria SC-1, SC-3, SC-4 and SC-5. *The stored row's shape* is no longer a one-way door (D-36); it proceeds on Claude's answer in the Technical Design Document (D-37). *Cost:* 8 packages, 11 units, all off the critical path (the Delivery Plan, WP-42 to WP-48, and WP-50, which brings them to the dev profile).],
  source: [your request, quoted in the proposal; `proposals/bug-reports.md`, sections 1 to 6, Crow's QA and Fennec's resolutions; the Engineer's ruling, 2026-10-09; the tech lead's calls on who builds and reviews it, 2026-10-09.],
)

#decided(
  id: "D-35",
  title: [Time limits on the web server's calls, and a page that says Zurfur isn't answering #raised[Heron's review; planned by web]],
  date: "2026-10-09",
  question: [The sign-in gate's call to the API, which every signed-in page waits on, has no time limit, and no other call from the web server has one except sign-out's 5 seconds, so a hung backend makes signed-in pages wait (Heron's review of the shell, "Copilot round 2", the note under row 8). The gate also reads an unreachable backend as "signed out", and sends you to the sign-in page. The web lead planned the limits beside the loading states (`objectives/web.md`, "Planned ahead: loading states"), as small work that "can ride the wiring if the tech lead wants it in 0.1", and asked you nothing; Crow's QA added that the limits must ship with the gate's own page, or a slow session check would send you to sign in. The options below are recorded after your ruling, to show what it settled.],
  options: (
    [*The limits and the gate's own page in 0.1, in one slice;* the loading states wait.],
    [*The limits, the gate's page and the loading states in 0.1:* skeleton rows, "Still loading…", and a retry row in the tree.],
    [*Neither in 0.1.*],
  ),
  ruling: [(a). Every call from the web server to the API gets a limit: the session check 5 s, other calls 10 s, the whole request 15 s. The gate shows its own fixed "Zurfur isn't answering right now" page, and never treats a timeout as signed out, in the same slice. Skeletons and the other loading states wait: they are planned ahead in `objectives/web.md`, and the design-language items listed there go to the Claude Design brief. Ruled by the Engineer, 2026-10-09.],
  after: [*What it changes:* a new story, "Zurfur says when it isn't answering" (US-2.6), and its wireframe; goal G2; web objective 6; success criterion SC-9; a non-goal for the loading states; two small packages off the critical path: the web's limits and page (WP-41), and identity's bound on the session check's wait for your profile (WP-49, the tech lead's call). *Three effects to know:* a backend that isn't running gets the same page, where today the gate sends you to sign in; a slow PDS shows your DID in the top bar rather than that page; and starting a sign-in now gives up after 10 s at the web, inside the API's own 30 s, with "Zurfur isn't answering right now. Please try again." on the login form (US-1.2; the tech lead's call).],
  source: [`objectives/web.md`, "Planned ahead: loading states", with Crow's QA and Fennec's resolution; `.understand/20261008-explorer-shell-security-steps-1b-2.md`, "Copilot round 2", row 8's note; `session-gate.ts` and `session.ts` on the shell's branch (`sessionOrAnonymous` reads a network failure as anonymous); the Engineer's ruling, 2026-10-09; the tech lead's calls on the session check and the login form, 2026-10-09.],
)


#pagebreak(weak: true)
= Sign-off

*Every decision is answered, and this version is ready for your signature.* Signing remains a separate act: you say "signed".

*Your signature approves 0.1’s scope and starts the build.* The packages the tech lead holds until then wait for it: the backend's first packages, den.proto into the contract, and bug reports' contract and web (the Delivery Plan, "What blocks the start", row 1).

*Two small asks ride on your signature,* outside the numbered decisions. Signing accepts Claude's answer to each unless you say otherwise:
+ *Does 0.1 close the Release Board's "https-only PDS redirect" item?* The board names the item without defining it ("What 0.1 delivers on the Release Board", the Security-first row). *Claude's answer:* yes, as the identity design reads it: the authorization endpoint must be https on port 443 at a named host, so 0.1 covers one of the thirteen Security-first items.
+ *Does 0.1 go ahead of the other eleven Security-first items,* against Project MVP's order (`v2-carryover.md`, row 47)? D-26 sets when moderation's two are due; this is about the rest. *Claude's answer:* yes, because 0.1 isn't deployed; their order is set when the alpha is planned.

A third ask comes with the session delete's PR (the Delivery Plan, "What blocks the start", row 10).

*Acceptance comes at the end, and is separate:* two walks on `main` (SC-1), your own Den on `just dev`, after a real sign-in and the fill script, then alice's Den on the dev profile, and bob's if the user switch survives. Your own Den is sparse, with no characters or Workflows; alice's still holds the commission, the Workflow and "Old sketch".

By signing, the Engineer approves 0.1’s scope as written in this document, with the rulings on D-1 to D-5 and D-17 to D-35, and bug reports as a side feature that moves to 0.2 if it isn't on `main` by unit 9. The Technical Design Document records the Engineer's other rulings of 2026-10-09, given separately: the den.proto contract, approved as a whole (D-6 to D-10, D-14), whose text still comes to the Engineer on its PR, with D-19’s edit to the contract's lifecycle file; where archived items are hidden (D-15), with the joined component diagram; and D-11 to D-13, D-16, D-36 and D-37.

#signoff(
  ("The Engineer (Zuri)", "Product Owner: approves the scope"),
  ("claude", "Tech lead: commits the teams to the plan"),
)

= What changed in version 5

Version 5 folds in your rulings of 2026-10-09 on every decision (`rulings-2026-10-09.md`), and is ready for your signature. No decision, story or criterion is renumbered, and no package or unit changes (50 packages, 75 units; the critical path stays at 17). In the Delivery Plan, your review of den.proto's text moves inside WP-13 and is due by unit 5 (was unit 4, before WP-13), and one slack figure is corrected (R4).

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [Every decision your rulings file covers (D-2 to D-5, D-18 to D-33) is a decided block, "Ruled by the Engineer, 2026-10-09", with Claude's proposal kept above your ruling, and your notes and what each ruling changes below it.], [your rulings],
  [D-27 is (b), against Claude's (a): Vulpes keeps building beside 0.1, its people not lent; Wolven pauses, its tree walk on a branch. The Vulpes and Wolven rows say so; the Delivery Plan's R11 lends the plugin team only, and its new R21 covers Vulpes's PRs in your queue.], [your ruling],
  [Summary and "Sign-off": the "not for signature" lines are gone. Every decision is answered; signing remains a separate act; what stays yours is three reviews on PRs (den.proto's text, the session delete, `bug_report.proto`). The held packages are named as the plan holds them: den.proto into the contract joins, and the time limits leave, since they start on your merges.], [your rulings; the tech lead],
  [D-3: your note, and a Finder-style column view parked as a future theme or view (a new non-goal; the Technical Design Document, "Parked").], [your note],
  [D-21: your question on what a slot is, with the tech lead's answer from The Den.], [your note; the tech lead],
  [D-29’s design-sync of the My Den rulings joins D-7’s in WP-13, and den objective 5 checks both; D-29 and the Technical Design Document's D-13 point to the tech lead's offer of one design-sync for every corpus follow-up (`v2-carryover.md`, row 94).], [the rulings file],
  [Where the text read "Claude's answer", it reads "your ruling": stories "Browse a folder" (D-8, D-29), "Nothing leaks" (D-10), "Show archived and deactivated" (D-7), the wireframes' opening, and commission objective 3 (den.proto, D-14, D-15). "Decisions needed: product" becomes "Decisions: product".], [your rulings],
  [*After Owl's QA* (`qa.md`, "QA challenges, version 5"): "Sign-off" lists the two small asks that ride on your signature, with Claude's answers (the Release Board's https-only item; carry-over row 47), and the summary points there; the summary says D-36 adds your own exception; Wolven's tree walk is "to be committed", by the tech lead (carry-over row 95); D-29 and D-31 put Account names into the same design-sync of decision 19, and D-29 refuses a name of joiners alone; D-26 says why the commission arms aren't the scan gate's trigger.], [den QA],
)

= What changed in version 4

Version 4 records your four rulings of 2026-10-09: bug reports are in 0.1 and can be cut (D-34); the time limits and the gate's "isn't answering" page are in, and the loading states wait (D-35); a migration isn't a one-way door in pre-alpha unless it deletes your real data (D-36, in the Technical Design Document); and D-1 is (b). It follows the web lead's bug-report proposal and its QA resolutions (`proposals/bug-reports.md`), and the tech lead's calls of the same day. No decision, story or criterion is renumbered; the new ones take the next numbers.

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [D-1 is ruled: (b), joined in 0.1 without character storage. Its block now shows your ruling instead of Claude's pre-filled answer, and every "Claude's answer to D-1" reads "your ruling on D-1".], [your ruling],
  [New decisions, both ruled: D-34 (bug reports, in 0.1 and cuttable at unit 9, with the proposal's other answers) and D-35 (the time limits and the gate's page; the loading states wait). The sign-off covers D-1 to D-5 and D-17 to D-35.], [your rulings; web],
  [Summary: the two additions, what they cost (50 packages, 75 units; the critical path stays 17), D-36 among what waits for you, three technical questions left in the carry-over, 24 decisions.], [your rulings; the tech lead],
  [Goals: G2 never waits without a limit; new G6, report a bug. Non-goals: filing a bug report is the one new write; the form isn't moderation's report path; the loading states wait, with why; the one rate limit 0.1 adds.], [your rulings; the proposal],
  ["What each team commits to": web objective 6 and identity objective 7 (the time limits); a new block for bug reports, shared ground under the tech lead (backend by moderation's programmers, web by Finch, review by Mantis); the moderation and plugin rows say what their people do; the heading reads "no 0.1 work of their own".], [the tech lead's calls; the proposal],
  [Stories: "Zurfur says when it isn't answering" (US-2.6); the side feature "Report a bug" (US-4.1 to US-4.3), from the proposal's nine criteria, split three ways; "Zurfur can't be aimed inward" says the web stops waiting for a sign-in start after 10 s.], [your rulings; web; the tech lead],
  [Wireframes: the gate's "isn't answering" page, and the bug-report form with its button and its sent state.], [web],
  [Success criteria: SC-1 (a bug report during the first walk), SC-3 (the time-limit slice and the bug-report branch), SC-4 (their reviews), SC-5 (what a report carries), new SC-9 (no wait over 15 s, never signed out by a timeout).], [your rulings; the proposal],
  [Choices: four for the time limits (a stopped backend gets the same page; "Try again"; the tree's retry row waits; public pages keep today's stance) and six from the proposal's veto list.], [web; the proposal; den QA],
  [*After den QA's pass* (`qa.md`, "QA challenges, version 4"): the summary and "Sign-off" say this version isn't for signature until your other answers of 2026-10-09 are folded in; "Zurfur says when it isn't answering" says what the installed SvelteKit does with a form post and a script navigation, and SC-9 bounds each request; "Report a bug" gains the phone's top-bar icon where there's no drawer, the page without script (its `Referer` rule and line breaks) and the gate's case; an unticked item is greyed; "Read reports safely" has a checkable last criterion and a note on who reads; the wireframe shows the greyed value and the test-stack line; D-34 and D-35 name who raised them, and D-35's options are marked as recorded after your ruling; bug reports are 8 packages and 11 units (the dev profile's package, WP-50).], [den QA],
)

*Where the rulings and the proposal weren't applied as written.*
- *The session check's wait for your profile.* On the identity branch `/me` reads your profile through a cache, then your PDS, through the resolver, which allows 10 s per fetch and 20 s per lookup. A cold cache and a slow PDS could outrun the 5 s session check, so the gate would say Zurfur isn't answering while it is. The tech lead's call: `/me` waits only a short time for your profile, inside the 5 s, and otherwise answers with your bare DID. It is a small identity package (WP-49), beyond the eight the rulings named.
- *Bug reports on the dev profile.* The dev profile is built on My Den's branch, which takes bug reports only in its restack onto `main`, so the proposal's "wired into the dev profile" had no package. Owl's QA found it; a small web package after that restack (WP-50) wires it and shows the form's "This test stack forgets reports" line only there. Bug reports are 8 packages and 11 units, not the proposal's 7 and 10.
- *A sign-in start runs out at the web first.* Your 10 s for other calls ends a sign-in start inside the API's own 30 s. The tech lead's call: the login form then says "Zurfur isn't answering right now. Please try again." It rides the web's time-limit slice.
- *The tree's retry row* was in the web lead's time-limit design. Your ruling names the limits and the gate's page for the slice, so it waits with the loading states; it is a veto row under "Choices".
- *The proposal's question 5* (the stored row's shape) was a one-way door; under D-36 it is an ordinary decision, D-37 in the Technical Design Document, and it proceeds on Claude's answer.

= What changed in version 3

Version 3 follows your ruling of 2026-10-09 that 0.1 joins the halves (D-1), on Claude's answer to the fork it leaves: without character storage. The den, identity, web and commission leads sized the join and revised their objectives (`objectives/join-*.md`); the tech lead confirmed nine calls the same day (the Technical Design Document, "Calls the tech lead makes"). Decision numbers D-1 to D-27 are unchanged.

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [D-1 is ruled in part: joined in 0.1. The fork left, (b) or (c), carries Claude's answer (b), with (c) sized at 4 to 5 more units and its key-list problem stated.], [your ruling; identity],
  [Summary: what joining costs (13 packages, 17 units; the critical path grows to 17 units, one unit from joining and one because the tech lead plans the end-of-slice review as L), the two walks, the scope call ruled in part, your signature starting the build, and what awaits your approval, the joined component diagram among it.], [all four leads; the tech lead; den QA],
  [Goal G3 and the Release Board: My Den on every stack, from PostgreSQL after a real sign-in. The line that kept the My Den rows to the dev profile is replaced.], [den, web],
  [Non-goals: the fill script and the retired owner's list; character storage, Workflow creation and a second real Participant left out.], [commission, identity],
  ["What each team commits to": identity 3 and 5 revised, 6 new; web 1, 3 and 4 revised; den 3 and 4 revised, 6 new; commission 4 grown and 5 replaced; the tech lead's calls on fixture characters and the post record type in the Vulpes and Posts rows.], [all four leads; the tech lead],
  [Frame stories: every "where My Den isn't served" clause removed (one frame, phone, screen reader); the everyday-stack criterion of "One frame on a desktop" now says the frame is the same on every stack.], [web],
  [My Den stories: your real root after sign-in, and its name falling back to your DID; the Account handle fallback (D-31); joiners in names (D-29); empty real Workflows (D-30); the reads outside the Den following its rules (D-28); everything on PostgreSQL too.], [identity, den, commission, web],
  [Wireframes: the fourth figure, the everyday stack without a tree, is gone.], [web],
  [Success criteria: SC-1 as two walks; SC-3 names three branches; SC-4 adds the joining reviews; SC-5 adds PostgreSQL, the statement-equality gate and the reads outside the Den; SC-7 reads "outside mock mode", checked by mock mode's boot guard and the `just dev` walk.], [web, den, identity, commission],
  [Choices: the landing-page link and the phone's top bar lose their "where My Den isn't served" halves.], [web],
  [D-17 points to D-32; D-20 adds the Account handle and "no `just db-reset` for old names"; D-24’s answer reads "in mock mode only", now that the shell goes to `main` first.], [identity, web, the tech lead],
  [New decisions D-28 to D-33. The sign-off covers them, D-29 among those awaiting approval.], [commission, identity, web],
  [Sign-off: your signature approves the scope and starts the build; acceptance at the end is two walks. "Sign-off" at the end of a release now reads "acceptance" (also in the Wolven and Posts rows, SC-1 and D-27's option (a)).], [web; den QA],
)

*Where the leads' pages weren't applied as written.*
- *Slack and the web's slice.* The leads' slack figures hold once the tech lead dropped your real root's wait for sign-in on `main` (identity QA): den's check and commission's move keep 2 units, and sign-in 5. Identity's two joining chains run 1 unit behind the critical path. The web lead sized "My Den on every stack" S inside the wiring; the tech lead made it its own package, which puts one unit on the path.
- *Commission's `just db-reset` before the first joined run* (its question 3) collided with identity's question 2 (D-32) and with D-20. The tech lead ruled on the board (2026-10-09): no `just db-reset`; old names show "Unnamed".
- *Not decisions here:* the den lead's question on gating equal timing is the tech lead's technical gate (the Technical Design Document), and commission's question 1, where your real commissions come from, is a work package: the fill script.

= What changed in version 2

Version 1 was written by Kestrel alone. Version 2 applies every correction the nine leads made that survived their QA (`objectives/`). What goes to the Technical Design Document, the Delivery Plan or the tech lead is in `v2-carryover.md`.

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [The header and summary name the stage, `pre-alpha-v0.1.0`, and what pre-alpha means.], [plugin; the stages ruling],
  [One one-way door, den.proto, carrying six decisions, plus D-15 for approval; version 1 said "five one-way doors".], [plugin, den],
  [New section "What each team commits to": every lead's objectives in plain words, and what 0.1 needs from the five teams without build work.], [all nine],
  [Summary, sign-in: "never asks Bluesky's AppView", not "without asking Bluesky's servers"; nobody has yet signed in with a real account; three slices remain, not two.], [identity; vulpes endorsed],
  [Goal G4: what is yours never opens what is mounted into it.], [den],
  [Non-goal "Moderation, posts and the Gallery": why `posts` is Public, that nothing fills it, and why 0.1 needs no moderation (it isn't deployed).], [posts, moderation],
  [Non-goal "Deployment": the local-development client; the checklist waits for the first build others can reach; moderation's floor joins it.], [identity, moderation],
  [Release Board, login row: sign-out's behaviour, tokens left stored, no new DID operations. Security-first row: moderation's two items and when they're due. View-grants row: keepers are Zurfur's own record. Frontend row: the frame on every stack. Placeholder row: what a commission opens to.], [identity, web, moderation, vulpes, commission],
  [Sign-in stories: identity-only scope; one field message; revocation is best effort; the expired-or-other-browser message.], [identity],
  [Frame stories: the phone's top bar; errors on signed-in pages; a typed `/den` on the everyday stack; sign-out; the drawer and the "My Den" region only where My Den is served; focus after the pane, the path bar and "More"; generic page titles.], [web],
  [My Den stories: a card shows no kind; slot and Workflow names; what a mount shows; folders across pages; the Workflow "Queue"; `posts` opens as Public with nothing below it; a commission's parts; a non-Participant sees nothing.], [den, commission, posts, web],
  ["Nothing leaks": one wording on naming another user's Den; every not-found cause; `400` and `422` before the path; the framing refusal; nothing kept in the browser.], [den, identity, web],
  ["Show archived and deactivated": typed paths for Participants and Owners; "Old sketch"; "Set aside" for an unknown word.], [den, commission, web],
  [Wireframes: a fourth figure for the everyday stack; the card shows its type.], [web],
  [Success criteria: SC-1 (the seed holds an archived commission), SC-5 (the joint wording, mounts and pages), SC-6 (the contrast test), SC-7 (the not-found on `/den`; "outside a dev build").], [commission, den, identity, web],
  [Choices: generic titles; folders across pages; four new rows from the build and one from the Commission page.], [web, commission],
  [D-1: `/den` answers the ordinary not-found, rather than "no route". D-4: revision 2.1 of the draft. D-5: its wording moves to D-18.], [web, den, identity],
  [New decisions D-17 to D-27; D-17 as you ruled it on 2026-10-09. The sign-off sentence covers them.], [identity, den, commission, web, moderation, vulpes, wolven],
)

*Where corrections conflicted, or weren't applied as written.*
- *The door count.* The den lead wrote "six one-way doors"; the plugin lead and its QA, "one door carrying six decisions". Version 2 uses one door, as the Delivery Plan does; the facts are the same.
- *When den.proto closes.* The den and plugin leads state "at the alpha" as a fact. The contract's lifecycle file ends the pre-release period at "the first release tag", and the web lead asks whether the pre-alpha tag is one. Version 2 writes "at the alpha" as Claude's answer to D-19, not as a fact.
- *A late den.proto review.* The web lead proposes going ahead on Claude's strict answers if your review is late; the den lead keeps your review first. Both are options in D-19; Claude recommends the den lead's.
- *Two teams on one passage,* merged so each keeps its facts: identity and web on the login row; den and commission on typing an archived path; posts and moderation on the moderation non-goal; identity and moderation on the deployment non-goal; den and identity on naming another user's Den.
- *Identity's fallback non-goal* (sign-in still asks for full access) isn't added: you ruled D-17 (a).
- *Corrections made conditional on a lead's question* (the slot message, the archived seed, "outside a dev build") are written under Claude's answers to D-21, D-22 and D-24, and marked with them.
- *Not in this document:* the plugin lead's question on how a plugin's type shows on the wire is a type-catalog question, not a 0.1 one; it goes to the Technical Design Document's parked row. The technical questions (identity's on the door list, den's on the Atta draft, the payload path and starting Atta before D-15) and every plan correction are in `v2-carryover.md`.
