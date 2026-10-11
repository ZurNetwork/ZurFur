#import "template.typ": *
#import "@preview/fletcher:0.5.8" as fletcher: diagram, node, edge

#show: doc.with(
  title: "Delivery Plan",
  short: "Delivery Plan",
  subtitle: [Release 0.1, `pre-alpha-v0.1.0`: "Sign in with Bluesky and browse My Den"],
  version: "4",
  date: "2026-10-10",
  status: "In review",
  author: "Kestrel, the tech lead's release planner",
  approver: "The Engineer (Zuri)",
)

#summary[
  *The work.* 50 work packages, 75 units, across four features, five teams, the tech lead and two security reviewers. Version 2 had 40 packages and 62 units. Your rulings of 2026-10-09 add 10 packages and 13 units: the time limits (WP-41, the web's limits and the gate's page, and WP-49, identity's bound on the session check's wait, both S) and bug reports (WP-42 to WP-48, and WP-50 for the dev profile: 8 packages, 11 units). D-1 is ruled (b), as version 2 planned. Version 4 folds in your rulings of 2026-10-09 on every decision, and adds no package. Sizes are relative: S = 1, M = 2, L = 3.

  *Four phases, each ending in something you can click.*
  + *Sign-in lands* (identity, from now): sign in with your real handle on `main`. The shell reaches `main` first.
  + *The shell on `main`, the side work, and the engine's groundwork:* My Den clickable on mock data; a signed-in page that says when Zurfur isn't answering; "Report a bug" on `main` by unit 6.
  + *My Den on the real backend:* the fixture world on the dev profile, and your own Den from PostgreSQL after a real sign-in on `just dev`.
  + *Release:* the end-of-slice security review, My Den to `main`, your two walks and your acceptance.

  *Critical path: 17 units, unchanged from version 2, and unchanged by your rulings:* D-27 (b) keeps Vulpes's people on Vulpes, and no package was staffed from them; your approval of den.proto moves your review onto WP-13's PR, which keeps its 4 units of slack. Neither addition is on it: every new package has slack (the time limits feed sign-in's merge with 7 units to spare; bug reports reach the path only through their dev-profile package, with 5), and their builders are free when they build (Finch until unit 8, Gecko until unit 3, Stoat between his packages, and moderation's programmers, who have no 0.1 work of their own). The path: Atta's read side, the den module, the arms and the den API routes, the golden responses, the web's wiring, "My Den on every stack", both live runs, the security review, My Den to `main` and your acceptance. Identity's two joining chains run beside it with 1 unit of slack. Sign-in keeps 5 units of slack for the release, 3 for bug reports.

  *Bug reports' cut line: on `main` by unit 9, or they are 0.2's* (your ruling on D-34). Their chain ends at unit 6, 3 units early. It needs the shell on `main` by unit 4 (their web starts there), sign-in on `main` by unit 8, Mantis's review (the tech lead's call), and one more restack from you.

  *What blocks work, all from you.* At unit 0, your signature on the Product Design Document, which starts the build: every decision is answered, and you approved D-15, the joined component diagram and the Atta draft on 2026-10-09. Then your merges (PRs \#236, \#237, \#244 into `main`; \#238, \#239 into the identity branch; the shell's \#245 to \#249); three reviews on PRs: den.proto's text (WP-13) by unit 5, `bug_report.proto` (WP-42) by unit 4, and the one-time session delete (WP-26) by unit 9, or 7 for bug reports; your three restacks (the identity branch by unit 9, or 7 for bug reports; bug reports by 8; My Den by 10; slices never move between branches); and your real sign-in for the `just dev` walk, at unit 11.

  *First to cut if time runs short:* the dev profile's "sign in as" switch, then the cookie follow-up. Bug reports leave by their own rule at unit 9; cutting them sooner saves the release nothing. Nothing with a security effect is cut, and no joining package is.
]

= What's already done

#table(
  columns: (1.3fr, 2.4fr, 1.3fr),
  table.header([Item], [State], [Where]),
  [The hardened sign-in], [Seven slices built. QA's final pass: "the stack is complete". Security's final verdict: "Nothing must close before `feature/identity-resolver` merges to `main`. Ship." (Heron, 2026-10-08). Copilot's comments are triaged, and the fix rounds are done.], [\#238 and \#239 into `feature/identity-resolver`; \#240 to \#243 stacked above slice 3 (they are slices 4, 5, 7 and 6). Slice 3 has its branch; its PR opens once \#238 and \#239 merge.],
  [The Explorer shell: the dark style, the frame and My Den on mock data (WP-7, WP-8, WP-9)], [Built and reviewed, as five stacked PRs; Heron reviewed steps 1b to 2. Slice 1b hides the tree where no Den is served. Slice 2 (\#247) stays a draft until Crow's last check.], [\#245 into `feature/web-explorer-shell`; \#246 to \#249 stacked above it (slices 1b, 2, 2b, 2c), each after its base],
  [The den.proto revision (WP-25), approved], [Written and QA'd: D-4’s word field and D-10’s sentence are in the draft. You approved revision 2.1 as a whole on 2026-10-09 (D-6); its text still comes to you on WP-13's PR.], [`.understand/20261007-den-proto-draft.md`],
  [Every decision, ruled], [Your rulings of 2026-10-09 answer all 37 decisions in the Product and Technical Design Documents, approve D-15 and the joined component diagram, and settle the carry-over's questions: WP-12 builds on your Atta draft, no code path serves a payload, and `bug_report.proto` comes to you on its PR. One ruling differs from Claude's answer: D-27 (b), Vulpes keeps building beside 0.1; and D-36 adds your own exception.], [`rulings-2026-10-09.md`; the decision blocks],
  [Already on `main`], [`Cache-Control: private, no-store` on signed-in pages (\#233); the API proxy forwards only `zurfur.sid` (\#234); the My Den rulings indexed (\#230); the posts team (\#231); the moderation and plugin teams (\#235).], [`main`],
  [Ready for `main`], [The `cid` workspace dependency (\#236); three team rules: QA before the Engineer, Claude pre-fills answers, ask at the slice (\#237); the Rust toolchain pinned to 1.99.0, so the local gate matches CI (\#244).], [Open PRs into `main`],
  [My Den's design], [Architecture diagrams approved (2026-10-07). The question sheet answered (questions P1 to P42 and C1 to C14). Corpus decision 2026100701 decided. The path-handling security review done before any code (2026-10-07). den.proto drafted, QA'd (Crow) and confirmed by the den lead (Badger). The join sized by the four leads (2026-10-09).], [`.understand/` and the corpus],
  [The bug-report proposal, and the time limits' design], [Written and QA'd: the proposal by Fennec, with Crow's 19 challenges closed; the time limits in web's objectives, "Planned ahead: loading states", with Crow's QA. Both ruled in by you on 2026-10-09 (D-34, D-35).], [`proposals/bug-reports.md`; `objectives/web.md`],
  [Not started], [Bug reports and the time limits. All My Den backend code. On `main`, `atta` is a six-line stub. An 83-line draft (with the `Key(Uuid)` the rulings replace) sits uncommitted in the primary checkout's working copy; the question sheet calls it your draft, and you ruled that WP-12 builds on it. No den module, den routes, den.proto file or `/den` page backed by the API exists.], [—],
)

= What starts now, and what waits for you

You ruled every decision on 2026-10-09, so no work waits on an answer. What waits for you is your signature, your merges and restacks, and three reviews on PRs: den.proto's text (WP-13), the one-time session delete (WP-26) and `bug_report.proto` (WP-42).

#table(
  columns: (1.1fr, 2.6fr),
  table.header([State], [Work packages]),
  [*Built, in review*], [WP-7, WP-8, WP-9 (the shell, \#245 to \#249). WP-25 (the den.proto revision) is approved (D-6).],
  [*Starts on your merges*], [Merging the sign-in stack (WP-1); after it, the different-browser message (WP-2), the bare-origin refusal (WP-3), the identity-only scope (WP-26), the session check's bounded wait (WP-49, D-35) and the corpus updates (WP-4). The shell to `main` (WP-27), once \#245 to \#249 are in, then the time limits (WP-41, D-35). These follow up work already in flight (the identity branch and the shell), so they start on your merges, not on the signature; the scope slice and the two time-limit packages also carry rulings you gave before the rest (D-17, D-35).],
  [*Starts when you sign the PDD*], [WP-12 (Atta's read side, built on your Atta draft, with D-15 as you ruled it), WP-13 (den.proto into the contract; you review its text on its PR), WP-15 (identity ports and fixtures), WP-17 (commission ports and fixtures, with D-21 and D-22 as ruled), WP-29 (den's PostgreSQL test setup), WP-36 (retiring the owner's list, D-28), and bug reports' contract (WP-42) and web (WP-45, once the shell is on `main`). The tech lead holds these until sign-off, so 0.1’s build starts on the scope you signed (your release rule: plan, review, then async).],
  [*Doesn't wait for you, but follows another package*], [The den module (WP-14) after Atta's read side; the dev profile (WP-21) after the two ports packages; then the arms (WP-16, WP-18) and den's access check (WP-28); then your Accounts on PostgreSQL, commission's move behind the Den and its seeder (WP-33, WP-35, WP-37); then den's route tests on PostgreSQL (WP-30) and the timing re-test (WP-31). Bug reports: the backend (WP-43) after the contract; the operator command and the corpus step (WP-44, WP-46) after the backend; Mantis's review (WP-47) after all five.],
  [*Waits for den.proto's PR* (WP-13, whose text you review)], [WP-19 (the den API routes), WP-20 (golden responses), WP-10 (the web's wiring), WP-40 (My Den on every stack), WP-11 (the dev-profile run), WP-30 and WP-39 (the `just dev` walk).],
  [*Follows the identity merge*], [WP-6 (the cookie cleared only on a match, D-12); WP-38 (the fill script); bug reports to `main` (WP-48), after your restack of their branch; bug reports on the dev profile (WP-50), after `feature/my-den` restacks onto `main`; and, once `feature/my-den` restacks onto `main`, the `just dev` walk (WP-39), where your real root is first checked live.],
)

#page(flipped: true)[
= Work breakdown

Teams and owners come from the board roster; each new package's owner is its lead, with the builder the lead named. Gates: *Q* is the team's QA before the lead and before you; *S* is a security review by Heron. Every PR also gets Copilot, and every slice your review. "Lands in" is the branch its slices merge into.

#show table: set text(size: 7.9pt)
#table(
  columns: (auto, 3.6fr, auto, 1.1fr, auto, 1fr, auto, 0.95fr, 0.85fr),
  align: (left, left, left, left, center, left, center, left, left),
  table.header([ID], [Work package], [Team], [Owner], [Size], [Depends on], [Phase], [Gates], [Lands in]),
  table.cell(colspan: 9, fill: colors.faint)[*Feature 1. Sign in with Bluesky, hardened* (identity: lead Fox, QA Magpie)],
  [WP-1], [Merge the sign-in stack into `feature/identity-resolver`, in order: \#238 and \#239; then slice 3’s PR, opened once both are in; then \#240, \#241, \#242, \#243 (slices 4, 5, 7, 6), each after its base. Check that the migration `20261009002000_…` came from `just migrate-add`.], [identity], [Fox; you merge], size("S"), [your merges], [0], [—], [identity branch],
  [WP-2], [The different-browser message: the `browser_mismatch` callback code, the `BrowserMismatch` refusal in the sign-in port, and the login page's copy, D-18’s text (D-5).], [identity], [Lynx], size("S"), [WP-1], [0], [Q, S], [identity branch],
  [WP-3], [Refuse a PDS address that isn't a bare origin (D-11). Can be cut to 0.2.], [identity], [Otter], size("S"), [WP-1], [0], [Q, S], [identity branch],
  [WP-26], [*New.* Sign-in asks only for `atproto` (D-17): one constant; a test that the sign-in address carries no `transition`; the stored sign-in sessions deleted once, and nothing else (D-32). After the close-out slice. The delete is a one-way door, so you review it on the PR before it merges.], [identity], [Fox (Lynx builds)], size("S"), [WP-1], [0], [Q, S, your review], [identity branch],
  [WP-49], [*New in version 3.* The session check answers within its limit (D-35; the tech lead's call): `/me` waits only a short, fixed time for your profile, inside the web's 5 s, and otherwise answers with the bare DID, as its contract allows. Identity names the wait's length; a test whose profile source never answers pins it. The fetch finishes in the background, one lookup per DID, so the cache fills, and a test shows a slow PDS's profile on a later load. Your real root (WP-32) may reuse the same bound.], [identity], [Fox (Stoat builds)], size("S"), [WP-1], [0], [Q, S], [identity branch],
  [WP-4], [Corpus updates (D-13): design-sync the Authenticators decision for the browser-bound cookie; write the hardened resolver's decision page; fix the stale note on the approved-crates page; repoint `adapter-atproto/NODE.json`.], [tech lead], [claude, written by a designer Spawn], size("M"), [WP-1, WP-2, WP-3], [0], [Q (Magpie); your corpus review], [the corpus],
  [WP-5], [`feature/identity-resolver` to `main`, as one squash, after the shell (WP-27); the branch restacks onto `main` once first. Before the merge, on the branch's dev stack: sign in with a real Bluesky handle, check your handle is in no log line at `info` or above, and run `scripts/web-smoke.sh` (it proves the proxy's routing only).], [identity], [Fox; you merge], size("S"), [WP-2, WP-3, WP-4, WP-26, WP-27, WP-41, WP-49], [0], [CI], [`main`],
  [WP-6], [Clear the binding cookie only when it matched this sign-in (D-12), reusing WP-2’s `BrowserMismatch`. Can be cut to 0.2.], [identity], [Lynx], size("S"), [WP-5], [3], [Q, S], [`main`],
  table.cell(colspan: 9, fill: colors.faint)[*Feature 2. The Explorer frame, in a neutral dark style* (web: lead Fennec, QA Crow)],
  [WP-7], [*Built.* The neutral dark style: `tokens.css` in three tiers, `base.css`, self-hosted fonts and icons, the motion and contrast rules.], [web], [Finch], size("M"), [—], [1], [Q], [shell branch],
  [WP-8], [*Built.* The frame (shell step 1): the two route groups; AppShell, TopBar, Sidebar and the phone drawer; errors inside the frame; the error hook that logs only the route; `Referrer-Policy`; the Accounts pages in the right pane (D-2); slice 1b, no tree where no Den is served, which WP-10 deletes.], [web], [Gecko], size("M"), [—], [1], [Q, S], [shell branch],
  [WP-9], [*Built.* My Den on mock data (shell step 2): the plain Den view; `denRoot` and `denNode`; the mock Den world; the path builder, also hardening the account-delete call; the `/den` route; the Tree (D-3); every pane state; the archived-and-deactivated toggle (D-4).], [web], [Gecko; Finch on the path builder], size("L"), [WP-7, WP-8], [1], [Q, S], [shell branch],
  [WP-27], [*New.* The shell to `main`: \#245 merges into `feature/web-explorer-shell`, and \#246 to \#249, stacked above it, each after its base (slice 2 out of draft after Crow's last check); then the branch goes to `main` as one squash, with its coherence check and corpus sync. Before the identity branch (the tech lead's call). The mock's slots say "Content not shown yet." (D-21, ruled; `v2-carryover.md`, row 32).], [web], [Fennec; you merge], size("S"), [WP-7, WP-8, WP-9; your merges], [1], [CI], [`main`],
  [WP-41], [*New in version 3.* The time limits and the gate's own page (D-35), one slice: every call through the API client's one fetch helper gets a limit (the session check 5 s, other calls 10 s; sign-out keeps its 5 s) and the whole request 15 s; the gate reads a network failure, a timeout included, as its own fixed page, "Zurfur isn't answering right now." with "Try again", returned rather than thrown so the signed-in headers stamp it, with no load or action behind it; a `401` still goes to `/login`; a sign-in start that runs out says so on the login form; how an enhanced form post meets the gate, worded as "isn't answering", never "signed out"; "Try again" never writes the request's address into the page unescaped; a timed-out write says it may have gone through where that matters; the Skeleton row takes `--row-height`, kept ready for the loading states. Tests: a backend that never answers, a page whose reads hang, an action that records whether it ran, a script navigation, an enhanced form post, a path carrying `'`, `"`, `<` and `>`. The tree's retry row waits with the loading states.], [web], [Fennec (Gecko builds)], size("S"), [WP-27], [1], [Q, S], [`main`, before the identity branch's restack],
  [WP-10], [Wire to den.proto (shell step 3): generated TypeScript; the live port decodes and maps; tolerant-reader tests; golden-response tests; the card's type as a word (D-23); at most 4 Den reads in flight per page; tests that a Den read failing as a timeout does (a network failure) shows the error view, and that a failing folder read above the item is dropped (D-35's criteria; no wait on the time limits themselves).], [web], [Finch], size("M"), [WP-9, WP-13, WP-20], [2], [Q, S], [`feature/my-den`],
  [WP-40], [*New.* My Den on every stack: deletes slice 1b's switch and its no-Den frame, so the sidebar, the tree, the drawer and the "My Den" links show on every signed-in stack and a typed `/den` reads the Den; the "Den not connected" notice goes. The web lead sized it S inside the wiring; the tech lead made it its own package after the wiring.], [web], [Fennec (Finch builds)], size("S"), [WP-10, WP-19], [2], [Q, S (with the wiring)], [`feature/my-den`],
  [WP-11], [The live run on the dev profile (shell step 4): the web as alice and bob; the contrast test (D-25); fix what the run finds.], [web], [Fennec], size("S"), [WP-40, WP-16, WP-18, WP-19, WP-21], [2], [Q], [`feature/my-den`],
  [WP-39], [*New.* The live run on `just dev` after your real sign-in: `~` lists `accounts`, `characters` (empty), `commissions` and `posts`; found an Account and see it under `accounts`; delete it and see it leave `accounts`, toggle off and on (an Account with no facts is hard-deleted; today every delete is); run the fill script and check commission's objective 5; `/den/nope` is "Nothing here"; the "sign in as" page and its route are the ordinary not-found; with the API stopped, a signed-in page says "Zurfur isn't answering right now", not the sign-in page, and "Sign out" leaves no `zurfur.sid`; the leak checks against your real DID and handle, reading the web's own log (`just dev-web` in its own terminal). Fixes land as slices; the walk goes in Crow's check file.], [web], [Fennec (Gecko drives the browser you sign in)], size("S"), [WP-5, WP-40, WP-32, WP-33, WP-34, WP-35, WP-38, WP-41], [2], [Q; S on a fix touching sessions or data], [`feature/my-den`],
  table.cell(colspan: 9, fill: colors.faint)[*Feature 3. My Den, read-only, on the real backend* (den: lead Badger, QA Owl; identity; commission: lead Wolf, QA Jay)],
  [WP-25], [*Done.* Revise den.proto for your review: D-4’s word field and D-10’s sentence written into the draft; `buf lint` and `buf build` re-run; the counts updated.], [web, with den], [Fennec, with Badger], size("S"), [—], [1], [Q (Crow)], [the draft],
  [WP-12], [Atta's read side, built on your Atta draft, committed as its first slice (`v2-carryover.md`, row 9): `Segment`, `NodeName`, `Level`, `Metadata`; the host trait (D-15 as you ruled it; the host learns which relationship opened a node, not only standing, for D-15's table), with no payload method, since no code path serves a payload (row 12, which replaces the question sheet's checklist item 19 and its section 7 answer 2, the in-memory verifier: neither is built); resolve and list (traversal, the level formula, standing along the real path, mounts and the mount guard); filtering, ordering and page tokens; one not-found; the caps; the guard that Atta depends on no workspace crate. The path segment replaces the draft's `Key(Uuid)`; the payload hash, `Payload` and attributes wait for the first payload.], [den], [Mole, with Wren], size("L"), [—], [1], [Q, S; Lapwing on the host check's inputs], [`feature/my-den`],
  [WP-13], [den.proto into the contract: the draft you approved (revision 2.1, D-6); the error-registry entries; the id rule amended (D-9) with `session.proto`'s comment; the lifecycle file's and CI's line that pre-release ends at the first alpha tag (D-19); the route-test pin moved, listing the two den routes like any other; generated Rust and TypeScript; the My Den rulings corrected by one design-sync: the parameter's spelling (D-7), and decision 19’s name rule with the two joiners (D-29) and Account names (D-31). Its PR brings you den.proto's text (D-19).], [den, with web], [Badger, with Fennec], size("S"), [WP-25 (approved, D-6); your signature], [1], [Q; your review of its text], [`feature/my-den`],
  [WP-14], [The den application module: the den Query (raw pieces into `Segment`s; one `NotFound`); Zurfur's type enum and the one host implementation (an exhaustive `match`, no panics on arms, mounts matched against the viewer's own set); `includeDeleted` passed to the host only; den's own types: the root and the empty `/post/<did>` stub, as its own slice.], [den], [Wren], size("M"), [WP-12], [1–2], [Q, S; posts (Bowerbird, QA Kea) on the stub's slice], [`feature/my-den`],
  [WP-15], [Identity read ports and fixture actors: the keeper read (empty on PostgreSQL, a named exemption) and the deactivated-Accounts-for-their-Owner read, in both adapters; alice, bob, Supreme Arts, Abco, Ember, Kael-sona, all characters Private; names with their fallback chain.], [identity], [Stoat], size("M"), [—], [1], [Q, S], [`feature/my-den`],
  [WP-16], [Identity host arms: user, account and character nodes; the user's system folders and an Account's `characters` (empty); mounts for memberships and kept characters; deactivated Accounts for their Owner only; tombstoned characters hidden; the shared name step (D-20, D-29), with a test that a name of joiners alone is refused as empty; `characters` on PostgreSQL says "This folder is empty." (D-33).], [identity], [Otter], size("M"), [WP-14, WP-15], [2], [Q, S], [`feature/my-den`],
  [WP-17], [Commission read ports and fixtures: the participant listing (archived rows included), the file list, the name-only file read, the slots and the Workflow listing, in both adapters; the by-user index on the participant table (`just migrate-add`); "Untitled", "Old sketch" (D-22) and the Workflow "Queue". Loaded names are a separate stored-text type; only write paths build `CommissionTitle` or `FileName` (the tech lead's call). The `Commission::is_archived` fix goes to `main` first, on its own bug branch.], [commission], [Raven], size("L"), [—], [1], [Q, S], [`feature/my-den`],
  [WP-18], [Commission host arms: the commission node (its title as its name), its four entries, slots and files; Workflows (open, empty, "content not shown yet"); an Account's `commissions` (flagged); archived commissions out of listings unless asked for, a typed path opening marked; file-name and title validation at write (D-20, D-29; a name of joiners alone refused as empty, with a test), with stored names converted through the Den's rule and "Unnamed" as the fallback. Before it starts, den and commission write down how a commission looked up by a real path runs the same statements whether absent or not yours (for den's check, WP-28).], [commission], [Egret], size("M"), [WP-14, WP-17], [2], [Q, S], [`feature/my-den`],
  [WP-19], [The den API routes: the root, the path and the explicit empty-path route; inside the cookie surface; session, then query (`400`, `422`), then path; split, then decode once; one `404` problem; `Cache-Control`; route-template logs; mounted on every stack from the first slice (the tech lead's call); the guard that the API never depends on Atta.], [den], [Vole], size("M"), [WP-13, WP-14], [2], [Q, S], [`feature/my-den`],
  [WP-20], [Golden responses: generated from the fixture world through the proto types and committed; hand-written card fixtures, marked.], [den], [Vole], size("S"), [WP-16, WP-18, WP-19], [2], [Q], [`feature/my-den`],
  [WP-21], [The dev profile: `adapter-mem` behind a Cargo feature, refusing to start unless the environment is `dev`; the seeded stores and the fixed sign-in wired; a `just` recipe; the gate and CI build the feature for its guard test; the in-memory session store, so the two stacks never accept each other's sessions, tested both ways (the tech lead's call); the Cargo feature alone chooses the profile, never a `.env` variable, and the refusal outside `ZURFUR_ENV=dev` stays as a second lock against a deployed or misbuilt binary (the tech lead's call); the "sign in as" switch (first to cut). The web builds the switch's page inside this package (not sized by the web lead): it shows only when a route that only the dev profile serves answers, and is the ordinary not-found anywhere else, with a test that `just dev` shows no switch (den QA's proposal, for the web lead).], [tech lead, with web for the page], [claude, built by a builder Spawn; the page by Fennec's team], size("M"), [WP-15, WP-17], [1–2], [Q (Owl; Crow for the page), S], [`feature/my-den`],
  table.cell(colspan: 9, fill: colors.faint)[*Feature 3, joining the halves* (your ruling on D-1; sized in `objectives/join-*.md`)],
  [WP-28], [*New.* Den's access check: "may this viewer open the node at this real path?" Atta's own resolve; "opens" or the one not-found; a card doesn't count; real paths stay on the server; it checks the node each read serves. Starts from the note den and commission write before WP-18 on equal statements for a real path.], [den], [Badger (Wren builds)], size("M"), [WP-12, WP-14], [2], [Q, S], [`feature/my-den`, its own slice, before the rewiring],
  [WP-29], [*New.* The PostgreSQL test setup: serves the app on the PostgreSQL stores, signed in through the in-process sign-in, driving only the sign-in port.], [den], [Badger (Mole builds)], size("S"), [—], [1], [Q (test code only)], [`feature/my-den`, its own slice],
  [WP-30], [*New.* The den route tests on PostgreSQL: the den route suite on both setups: your root, a membership mount, a typed archived path, the one `404` for every cause, bob's DID as a path piece, `%2F` and `%252F`.], [den], [Badger], size("S"), [WP-19, WP-29, WP-33, WP-37], [2], [Q, S], [`feature/my-den`, its own slice],
  [WP-31], [*New, moved in from 0.2.* The not-found timing re-test: for each pair of causes at the same depth (absent; present but not yours), a test that both run the same database statements, with the profile cache's state fixed first. Clock times are measured for Heron.], [den], [Badger], [#size("S") \ M if a pair differs], [WP-30, WP-32], [2], [Q, S], [`feature/my-den`, its own slice, before WP-22],
  [WP-32], [*New.* Your real root: the user arm names your root from your own PDS through the hardened resolver (display name, else a handle that checks out both ways, else your DID); cache first, one lookup per DID in flight, your DID after a short wait, whose length identity names in the package; a test whose PDS never answers pins it, and a deep page against it finishes within about one wait; a recording profile port shows only your DID is fetched. It builds against the profile ports, which `main` and the identity branch share; it is checked live only after `feature/my-den` restacks onto sign-in (the tech lead's call).], [identity], [Fox], size("M"), [WP-16], [2], [Q, S], [`feature/my-den`],
  [WP-33], [*New (changes WP-15, WP-16).* Your real Accounts on PostgreSQL: identity's seeder (alice, bob, one Account; DIDs agreed with commission) and its cases on den's setup; a deactivated Account only to its Owner; a stranger's and a missing Account make the same store calls; no Den answer names another User; a refused stored name shows the handle, and new Account names refuse the same (D-31).], [identity], [Fox], size("M"), [WP-15, WP-16, WP-29], [2], [Q, S], [`feature/my-den`],
  [WP-34], [*New, moved in from 0.2.* The reads that bypass the Den, identity's half: the Accounts list keeps only what den's check opens; `/me` asks it about your own root; wire shapes unchanged; a test per read in both adapters.], [identity], [Fox], size("S"), [WP-28, WP-32], [2], [Q, S], [`feature/my-den`, before WP-22 starts],
  [WP-35], [*New, moved in from 0.2.* Commission's reads that bypass the Den move behind it: the changelog read and the file download call den's check on the node each serves. Added by QA: each pair of not-found causes gets the same answer and runs the same statements: as a stranger, an absent commission against one that isn't yours; as a Participant, an absent file key against another commission's key. The download asks about the commission's node first, then the file's, keeping today's two answers. Commission confirms the size.], [commission], [Wolf], size("M"), [WP-18, WP-28], [2], [Q, S (a joint sitting with WP-37 at unit 9)], [`feature/my-den`, before WP-22 starts],
  [WP-36], [*New.* Retire the owner's commission list (D-28): the route and its contract entry, as their own declared baseline-reset PR. Its first slice writes the baseline-reset line into `contract/VERSIONING.md`.], [commission], [Wolf], size("S"), [your signature (D-28 ruled)], [1], [Q; on WP-22’s list], [`feature/my-den`],
  [WP-37], [*New.* Commission's PostgreSQL seeder and end-to-end test: alice and bob signed in through den's setup, which creates their users; one commission and one archived commission through today's use cases; bob added by a test-only insert; read back through the arms as a Participant and as a stranger; a legacy title shows "Unnamed" on the listing, on its typed path, on the changelog read and on a download with a legacy file name.], [commission], [Wolf], size("S"), [WP-18, WP-29], [2], [Q, S (a joint sitting with WP-35 at unit 9)], [`feature/my-den`],
  [WP-38], [*New.* A script to fill your Den: calls today's commission routes as you, after a real sign-in: two commissions with files and slots, one archived. No server code. It asks for your session cookie with echo off and keeps it unexported, hands it to `curl` on standard input, never as an argument or a file, and never runs verbose; the worksheet ends with "Sign out", which deletes that session (proposed by QA).], [commission], [Wolf], size("S"), [WP-5], [2], [Q, S (light: never logs or stores your session cookie)], [`feature/my-den`],
  table.cell(colspan: 9, fill: colors.faint)[*Side feature: bug reports* (your ruling on D-34; on `main` by unit 9, or 0.2's; shared ground under the tech lead; from `proposals/bug-reports.md`)],
  [WP-42], [*New.* The contract: `bug_report.proto`, `BugReportService` with `FileBugReport` (`post: "/api/v1/bug-reports"`, `body: "*"`), with no reporter field; the route-to-contract test gains the route. Additive. You review the file on its PR before it merges, as a contract fixed at the alpha (your ruling on `v2-carryover.md`, row 84).], [tech lead, with web], [claude; Mastiff builds, with Finch], size("S"), [your signature], [1], [Q (Lapwing); your review], [`feature/bug-reports`],
  [WP-43], [*New.* The backend: the `BugReport` element and its four newtypes (`TryFrom<String>`, refusing control and bidirectional characters); `FileBugReport` (count, then insert; 10 an hour) and `ListBugReports`; the `BugReportStore` port; the `bug_report` table (`just migrate-add`; D-37) and its in-memory twin, in the in-memory runtime the tests use (the dev profile takes it in WP-50); `POST /bug-reports` in the cookie surface; `ZURFUR_BUILD_ID` in the API's config.], [tech lead], [claude; Mastiff builds], size("M"), [WP-42], [1], [Q (Lapwing); S in WP-47], [`feature/bug-reports`],
  [WP-44], [*New.* The operator command: `zurfur bug-reports list [--since <time>]`, in-process and read-only, each report fenced, indented and escaped (C1 controls included); `just bug-reports`.], [tech lead], [claude; Goose builds], size("S"), [WP-43], [1], [Q (Lapwing); S in WP-47], [`feature/bug-reports`],
  [WP-45], [*New.* The web: TextInput (multi-line), Checkbox and InlineMessage; the Dialog's initial focus, `aria-labelledby` and full-screen variant; the button in the top bar, the drawer, a phone's top bar where there's no drawer, and the error view; the dialog and the `/report-a-bug` page with one form action, which turns CR LF into LF, takes the page from `Referer` only from Zurfur's own origin, and refuses a post from anywhere but Zurfur's own pages, in development too; a send that meets the gate keeps the text and says it couldn't send; `ZurfurApi.fileBugReport`, live and mock; `ZURFUR_BUILD_ID` from the Justfile; tests for the keyboard, focus, 320 px, no script and a foreign origin. Starts on `feature/bug-reports` once the shell is on `main`, so no slice moves between branches.], [web], [Fennec (Finch builds)], size("L"), [your signature; WP-27], [1], [Q (Crow); S in WP-47], [`feature/bug-reports`],
  [WP-46], [*New.* The corpus: a decision page offered to you (the entity, the table, the route, the operator read), then a design-sync.], [web, with moderation], [Fennec, with Meerkat], size("S"), [WP-43], [1], [Q (Crow); your corpus review], [the corpus],
  [WP-47], [*New.* The security review of WP-42 to WP-46 in one sitting, not by Heron; the two gaps that predate the feature go to Heron's list.], [security], [Mantis, at xhigh (the tech lead's call)], size("S"), [WP-42, WP-43, WP-44, WP-45, WP-46], [1], [—], [—],
  [WP-48], [*New.* `feature/bug-reports` to `main`, after the identity branch: your one restack of it onto `main`, its coherence check, doc comments, one squash. Not on `main` by unit 9: it waits for My Den, and is 0.2's.], [web], [Fennec; you merge], size("S"), [WP-47, WP-5, WP-41], [1], [CI], [`main`],
  [WP-50], [*New, from QA.* Bug reports on the dev profile, after `feature/my-den` restacks onto `main` with them: the dev profile serves the in-memory bug-report store, and the form says "This test stack forgets reports when it stops." only there, detected by the same dev-only route as the "sign in as" page; a test that `just dev` never shows the line. Goes to 0.2 with bug reports if they're cut.], [web, with the tech lead], [Fennec (Gecko builds)], size("S"), [WP-48, WP-21, and the restack], [2], [Q (Crow; Owl for the dev profile)], [`feature/my-den`],
  table.cell(colspan: 9, fill: colors.faint)[*Release*],
  [WP-22], [The end-of-slice security review, at the highest effort, planned as L because it covers the joined path (the tech lead's call; Heron may size it down), starting once WP-34 and WP-35 are merged (the tech lead's call): the question sheet's 19 findings as the checklist (finding 19’s payload default now reads "no code path serves a payload", `v2-carryover.md`, row 12), plus the PostgreSQL path as served; den's check and each caller; your real root's lookup; the timing results; the retired list; the account-delete path; page titles; the dev profile's guard; the upload's file-key log line at `warn`, and two that carry an Account's DID (founding at `info`, a failed tombstone at `warn`); the gate's "isn't answering" page in front of the Den routes; no vulpes dependency, ACP record name or ownership half. Fix rounds.], [security], [Heron], size("L"), [WP-11, WP-39, WP-31, WP-36, WP-50 (if bug reports made 0.1); WP-34 and WP-35 merged], [3], [—], [—],
  [WP-23], [`feature/my-den` to `main`: the coherence check (`/close-gaps --post`), doc comments on changed signatures, corpus sync, one squash.], [den], [Badger, with the tech lead; you merge], size("S"), [WP-22], [3], [CI], [`main`],
  [WP-24], [Release acceptance: your two walks (success criterion SC-1), the final status report with posts' checks (Kea), and your acceptance of the release. (Your signature on the Product Design Document comes at the start: it approves the scope.)], [release], [Kestrel and the tech lead; you accept], size("S"), [WP-5, WP-23], [3], [—], [—],
)
]

#page(flipped: true)[
= Dependency graph and critical path

#let wpn(pos, id, title, team: colors.shared, crit: false, done: false) = node(
  pos,
  align(center, text(size: 6.4pt)[#strong(id) \ #title]),
  width: 18mm,
  inset: 3pt,
  fill: if crit { colors.critical-soft } else if done { colors.faint } else { white },
  stroke: if crit { 1.6pt + colors.critical } else if done { (paint: team, thickness: 0.8pt, dash: "dashed") } else { 0.9pt + team },
  corner-radius: 3pt,
)
#let dep(..args, crit: false) = edge(
  ..args,
  marks: "-|>",
  stroke: if crit { 1.6pt + colors.critical } else { 0.6pt + colors.muted },
)

#figure(
  caption: [Work packages by dependency, roughly by start time. Outlines show the team; the critical path is thick and orange; the dashed box is built; the hexagon is den.proto's approval (D-6, given 2026-10-09), and you review its text on WP-13's PR. Left out: edges implied by others; two that would cross the chart (WP-19 to WP-30, WP-5 to WP-39); three packages with 5 or more units of slack: the dev profile (WP-21, feeds WP-11), the fill script (WP-38, feeds WP-39) and retiring the owner's list (WP-36, feeds WP-22); and the time limits and bug reports, drawn in their own figure under "Beside the path".],
  diagram(
    spacing: (3.4mm, 1.6mm),
    {
      // Sign-in (identity) and the shell (web)
      wpn((0, 0), "WP-1", [Merge sign-in stack], team: colors.identity)
      wpn((1, 0), "WP-2", [Different-browser message], team: colors.identity)
      wpn((1, 1), "WP-3", [Bare-origin PDS], team: colors.identity)
      wpn((1, 2), "WP-26", [Identity-only scope], team: colors.identity)
      wpn((2, 0), "WP-4", [Corpus updates], team: colors.shared)
      wpn((3, 0), "WP-5", [Sign-in to `main`], team: colors.identity)
      wpn((4, 0), "WP-6", [Cookie cleared on match], team: colors.identity)
      wpn((2, 2), "WP-27", [Shell to `main`], team: colors.web)
      wpn((1, 3), "WP-7 to 9", [The shell, built], team: colors.web, done: true)
      // den.proto
      node(
        (1, 4),
        align(center, text(size: 6.2pt, fill: colors.door)[*Approved* \ den.proto (D-6)]),
        shape: fletcher.shapes.hexagon,
        width: 18mm,
        inset: 3pt,
        fill: colors.door-soft,
        stroke: 1pt + colors.door,
      )
      wpn((2, 4), "WP-13", [den.proto in the contract], team: colors.den)
      // The critical path
      wpn((0, 5), "WP-12", [Atta's read side], team: colors.den, crit: true)
      wpn((1, 5), "WP-14", [den module], team: colors.den, crit: true)
      wpn((3, 5), "WP-19", [den API routes], team: colors.den, crit: true)
      wpn((4, 5), "WP-20", [Golden responses], team: colors.den, crit: true)
      wpn((5, 5), "WP-10", [Wire to den.proto], team: colors.web, crit: true)
      wpn((6, 5), "WP-40", [My Den on every stack], team: colors.web, crit: true)
      wpn((7, 5), "WP-11", [Dev-profile run], team: colors.web, crit: true)
      wpn((7, 6), "WP-39", [`just dev` walk], team: colors.web, crit: true)
      wpn((8, 6), "WP-22", [Security review], team: colors.shared, crit: true)
      wpn((9, 6), "WP-23", [My Den to `main`], team: colors.den, crit: true)
      wpn((10, 6), "WP-24", [Acceptance], team: colors.shared, crit: true)
      // Identity's joining chains
      wpn((0, 6), "WP-15", [Identity ports, actors], team: colors.identity)
      wpn((3, 6), "WP-16", [Identity arms], team: colors.identity, crit: true)
      wpn((4, 6), "WP-32", [Your real root], team: colors.identity)
      wpn((5, 6), "WP-34", [Identity reads behind the Den], team: colors.identity)
      wpn((2, 7), "WP-29", [PostgreSQL test setup], team: colors.den)
      wpn((4, 7), "WP-33", [Your Accounts on PostgreSQL], team: colors.identity)
      wpn((5, 7), "WP-30", [Den route tests on PostgreSQL], team: colors.den)
      wpn((6, 7), "WP-31", [Timing re-test], team: colors.den)
      // Commission, and den's access check
      wpn((0, 8), "WP-17", [Commission ports, fixtures], team: colors.commission)
      wpn((3, 8), "WP-18", [Commission arms], team: colors.commission, crit: true)
      wpn((4, 8), "WP-28", [Den's access check], team: colors.den)
      wpn((5, 8), "WP-35", [Commission reads behind the Den], team: colors.commission)
      wpn((4, 9), "WP-37", [Commission seeder, end-to-end], team: colors.commission)

      // Sign-in
      dep((0, 0), (1, 0))
      dep((0, 0), (1, 1))
      dep((0, 0), (1, 2))
      dep((1, 0), (2, 0))
      dep((1, 1), (2, 0))
      dep((1, 2), (3, 0))
      dep((2, 0), (3, 0))
      dep((2, 2), (3, 0))
      dep((3, 0), (4, 0))
      dep((3, 0), (3, -0.7), (10, -0.7), (10, 6))
      // The shell
      dep((1, 3), (2, 2))
      dep((1, 3), (5, 3), (5, 5))
      // den.proto
      dep((1, 4), (2, 4))
      dep((2, 4), (3, 5))
      // The critical path and the arms
      dep((0, 5), (1, 5), crit: true)
      dep((1, 5), (3, 5), crit: true)
      dep((1, 5), (3, 6), crit: true)
      dep((1, 5), (1, 7.5), (3, 8), crit: true)
      dep((1, 5), (4, 8))
      dep((0, 6), (3, 6))
      dep((0, 8), (3, 8))
      dep((3, 5), (4, 5), crit: true)
      dep((3, 6), (4, 5), crit: true)
      dep((3, 8), (3.5, 7.5), (3.5, 5.5), (4, 5), crit: true)
      dep((4, 5), (5, 5), crit: true)
      dep((5, 5), (6, 5), crit: true)
      dep((6, 5), (7, 5), crit: true)
      dep((6, 5), (7, 6), crit: true)
      dep((7, 5), (8, 6), crit: true)
      dep((7, 6), (8, 6), crit: true)
      // Identity's joining chains
      dep((3, 6), (4, 6))
      dep((4, 6), (5, 6))
      dep((5, 6), (7, 6))
      dep((4, 6), (6, 7))
      dep((3, 6), (4, 7))
      dep((2, 7), (4, 7))
      dep((2, 7), (2.5, 7.5), (2.5, 9), (4, 9))
      dep((4, 7), (5, 7))
      dep((4, 7), (4.5, 6.5), (6.5, 6.5), (7, 6))
      dep((5, 7), (6, 7))
      dep((6, 7), (7.5, 7), (8, 6))
      // Den's check and commission's joining work
      dep((4, 8), (5, 6))
      dep((4, 8), (5, 8))
      dep((3, 8), (4, 9))
      dep((3, 8), (3.5, 8.5), (4.5, 8.5), (5, 8))
      dep((4, 9), (5, 7))
      dep((5, 8), (6.5, 7.5), (7, 6))
      // Release
      dep((8, 6), (9, 6), crit: true)
      dep((9, 6), (10, 6), crit: true)
    },
  ),
)

#grid(
  columns: (1fr, 1fr, 1fr, 1fr, 1.4fr, 1.3fr),
  column-gutter: 8pt,
  text(size: 8pt)[#box(width: 8pt, height: 8pt, stroke: 1pt + colors.web) web],
  text(size: 8pt)[#box(width: 8pt, height: 8pt, stroke: 1pt + colors.den) den],
  text(size: 8pt)[#box(width: 8pt, height: 8pt, stroke: 1pt + colors.identity) identity],
  text(size: 8pt)[#box(width: 8pt, height: 8pt, stroke: 1pt + colors.commission) commission],
  text(size: 8pt)[#box(width: 8pt, height: 8pt, stroke: 1pt + colors.shared) tech lead, security, release],
  text(size: 8pt)[#box(width: 8pt, height: 8pt, fill: colors.critical-soft, stroke: 1.6pt + colors.critical) critical path],
)
]

== The critical path, listed

*WP-12* Atta's read side → *WP-14* the den application module → *WP-16, WP-18 and WP-19*, the identity arms, the commission arms and the den API routes, side by side → *WP-20* the golden responses → *WP-10* the web's wiring to den.proto → *WP-40* My Den on every stack → *WP-11* the dev-profile run and *WP-39* the `just dev` walk, side by side → *WP-22* the end-of-slice security review → *WP-23* My Den to `main` → *WP-24* your acceptance.

*One unit behind it,* two joining chains from the identity arms: *WP-32* your real root → *WP-34* identity's half of the reads that bypass the Den → the `just dev` walk; and *WP-33* your Accounts on PostgreSQL → *WP-30* den's route tests on PostgreSQL → *WP-31* the timing re-test → the security review.

== Beside the path: the time limits and bug reports

#let side-node(pos, id, title, team: colors.shared, ref: false, start: none) = node(
  pos,
  align(center, text(size: 6.6pt)[#strong(id) \ #title #if start != none [\ #text(fill: colors.muted)[#start]]]),
  width: 25mm,
  inset: 3pt,
  fill: if ref { colors.faint } else { white },
  stroke: if ref { (paint: team, thickness: 0.8pt, dash: "dashed") } else { 0.9pt + team },
  corner-radius: 3pt,
)
#let side-edge(..args) = edge(..args, marks: "-|>", stroke: 0.6pt + colors.muted)

#figure(
  caption: [The time limits and bug reports, beside the critical path. Each box shows its units (earliest start to earliest finish). Dashed boxes are drawn in the main graph above. Bug reports must be on `main` by unit 9, or they are 0.2's; their chain to `main` ends at 6, and WP-50 follows My Den's restack.],
  diagram(
    spacing: (6mm, 3mm),
    {
      side-node((0, 0), "WP-27", [Shell to `main`], team: colors.web, ref: true, start: [0 to 1])
      side-node((1, 0), "WP-41", [Time limits, gate's page], team: colors.web, start: [1 to 2])
      side-node((4, 0), "WP-39", [`just dev` walk], team: colors.web, ref: true, start: [11 to 12])
      side-node((0, 1), "WP-1", [Merge sign-in stack], team: colors.identity, ref: true, start: [0 to 1])
      side-node((1, 1), "WP-49", [Session check's wait], team: colors.identity, start: [1 to 2])
      side-node((3, 1), "WP-5", [Sign-in to `main`], team: colors.identity, ref: true, start: [4 to 5])
      side-node((0, 2), "WP-42", [Contract (your review)], start: [0 to 1])
      side-node((1, 2), "WP-43", [Backend], start: [1 to 3])
      side-node((2, 2), "WP-44", [Operator command], start: [3 to 4])
      side-node((2, 3), "WP-46", [Corpus step], team: colors.web, start: [3 to 4])
      side-node((1, 4), "WP-45", [Web: button, form, page], team: colors.web, start: [1 to 4])
      side-node((3, 3), "WP-47", [Review (Mantis)], start: [4 to 5])
      side-node((4, 2), "WP-48", [Bug reports to `main`], team: colors.web, start: [5 to 6])
      node((5, 2), align(center, text(size: 6.4pt, fill: colors.critical)[*Cut line* \ on `main` by unit 9, \ else 0.2]), shape: fletcher.shapes.hexagon, width: 22mm, inset: 3pt, fill: colors.critical-soft, stroke: 1pt + colors.critical)
      side-node((4, 4), "WP-50", [Bug reports on the dev profile], team: colors.web, start: [after My Den's restack])
      side-node((5, 4), "WP-22", [End-of-slice review], ref: true, start: [12 to 15])

      side-edge((0, 0), (1, 0))
      side-edge((1, 0), (4, 0))
      side-edge((1, 0), (1.5, 0.5), (2.6, 0.5), (3, 1))
      side-edge((1, 0), (1.5, 0.5), (3.5, 0.5), (4, 2))
      side-edge((0, 0), (0.5, 0.5), (0.5, 3.5), (1, 4))
      side-edge((0, 1), (1, 1))
      side-edge((1, 1), (3, 1))
      side-edge((3, 1), (4, 2))
      side-edge((0, 2), (1, 2))
      side-edge((1, 2), (2, 2))
      side-edge((1, 2), (2, 3))
      side-edge((2, 2), (3, 3))
      side-edge((2, 3), (3, 3))
      side-edge((1, 4), (3, 4), (3, 3))
      side-edge((0, 2), (0.6, 2.5), (2.5, 2.5), (3, 3))
      side-edge((3, 3), (4, 2))
      side-edge((4, 2), (4, 4))
      side-edge((4, 4), (5, 4))
      edge((4, 2), (5, 2), stroke: (paint: colors.critical, thickness: 0.8pt, dash: "dashed"))
    },
  ),
)

#text(size: 8pt)[#box(width: 8pt, height: 8pt, stroke: 1pt + colors.web) web #h(10pt) #box(width: 8pt, height: 8pt, stroke: 1pt + colors.identity) identity #h(10pt) #box(width: 8pt, height: 8pt, stroke: 1pt + colors.shared) tech lead (built by moderation's programmers), security]

== How it was computed

Each size is a relative unit: *S = 1, M = 2, L = 3.* Units model order and weight, not hours. Each package starts when everything it depends on is finished, and every team's people work in parallel; capacity is checked by hand below. Unit 0 is your signature on the Product Design Document; you approved D-15, the joined component diagram and the Atta draft on 2026-10-09, and den.proto as a whole (D-6). The model counts the built shell (WP-7 to WP-9) and the approved den.proto revision (WP-25) as done at unit 0, assumes your merges arrive then too, and your reviews on PRs (den.proto's text on WP-13's, `bug_report.proto` on WP-42's, the session delete on WP-26's) as soon as each is asked for. A script recomputed the forward and backward pass from the "Depends on" column.

#table(
  columns: (auto, 2.6fr, auto, auto, auto, auto),
  align: (left, left, center, center, center, center),
  table.header([ID], [Package], [Units], [Earliest start], [Earliest finish], [Slack]),
  table.cell(colspan: 6, fill: colors.faint)[*On the critical path*],
  [WP-12], [Atta's read side], [3], [0], [3], [*0*],
  [WP-14], [den module], [2], [3], [5], [*0*],
  [WP-16, WP-18, WP-19], [Identity arms, commission arms, den API routes], [2 each], [5], [7], [*0*],
  [WP-20], [Golden responses], [1], [7], [8], [*0*],
  [WP-10], [Wire to den.proto], [2], [8], [10], [*0*],
  [WP-40], [My Den on every stack], [1], [10], [11], [*0*],
  [WP-11, WP-39], [Dev-profile run; `just dev` walk], [1 each], [11], [12], [*0*],
  [WP-22], [Security review], [3], [12], [15], [*0*],
  [WP-23], [My Den to `main`], [1], [15], [16], [*0*],
  [WP-24], [Acceptance], [1], [16], [17], [*0*],
  table.cell(colspan: 6, fill: colors.faint)[*Off it*],
  [WP-32, WP-33], [Your real root; your Accounts on PostgreSQL], [2 each], [7], [9], [1],
  [WP-34], [Identity's reads behind the Den], [1], [9], [10], [1],
  [WP-30], [Den route tests on PostgreSQL], [1], [9], [10], [1],
  [WP-31], [Timing re-test], [1 (2 if a pair differs)], [10], [11], [1 (0 at M)],
  [WP-28], [Den's access check], [2], [5], [7], [2],
  [WP-35], [Commission reads behind the Den], [2], [7], [9], [2],
  [WP-37], [Commission seeder, end-to-end], [1], [7], [8], [2],
  [WP-17], [Commission ports and fixtures], [3], [0], [3], [2],
  [WP-15], [Identity ports and fixtures], [2], [0], [2], [3],
  [WP-13], [den.proto in the contract], [1], [0], [1], [4],
  [—], [Your review of den.proto's text, on WP-13's PR], [—], [1], [—], [needed by 5],
  [WP-1 to WP-5], [The sign-in chain to `main`], [1, 1, 1, 2, 1], [0 to 4], [1 to 5], [5 (3 for bug reports)],
  [WP-38], [Fill script], [1], [5], [6], [5],
  [WP-21], [Dev profile], [2], [3], [5], [6],
  [WP-26], [Identity-only scope], [1], [1], [2], [7 (5 for bug reports)],
  [WP-49], [Session check's wait], [1], [1], [2], [7 (5 for bug reports)],
  [WP-29], [PostgreSQL test setup], [1], [0], [1], [7],
  [WP-27], [Shell to `main`], [1], [0], [1], [7 (3 for bug reports)],
  [WP-41], [Time limits, gate's page], [1], [1], [2], [7 (5 for bug reports)],
  [WP-6], [Cookie cleared on match], [1], [5], [6], [11],
  [WP-36], [Retire owner's list], [1], [0], [1], [11],
  [WP-7 to WP-9, WP-25], [The shell; the den.proto revision], [built], [—], [0], [—],
  table.cell(colspan: 6, fill: colors.faint)[*Bug reports: slack against their cut line* (on `main` by unit 9). They can be cut, so the release figures above leave them out],
  [WP-42], [Contract], [1], [0], [1], [3],
  [WP-43], [Backend], [2], [1], [3], [3],
  [WP-44, WP-46], [Operator command; corpus step], [1 each], [3], [4], [3],
  [WP-45], [Web], [3], [1], [4], [3],
  [WP-47], [Security review (Mantis)], [1], [4], [5], [3],
  [WP-48], [Bug reports to `main`], [1], [5], [6], [3],
  [WP-50], [Bug reports on the dev profile], [1], [6, in practice after My Den's restack], [7], [5, before the end-of-slice review],
)

*What this says.*
- *The release is 17 units, as in version 2, two more than version 1.* Joining added one, the web's "My Den on every stack" as its own package on the path; the end-of-slice review, planned as L, added the other. *Version 3's ten packages (13 units) don't move it:* the forward and backward pass gives every one of them slack. The time limits (WP-41) and the session check's wait (WP-49) feed sign-in's merge, which keeps its 5, with 7 units each; bug reports reach the path only through WP-50, which the end-of-slice review waits for with 5 units to spare, and leave with it if they're cut.
- *Bug reports have their own deadline, the cut line at unit 9* (your ruling on D-34). Their chain (WP-42 to WP-48) ends at unit 6, 3 units early; their web (WP-45) starts on their own branch once the shell is on `main`, so the shell must be there by unit 4. They wait on the identity branch: sign-in must be on `main` by unit 8 (3 units of slack for this, 5 for the release), so your restack of the identity branch is due by unit 7 for bug reports, by 9 for the release. The time limits now come before sign-in's merge in the model too (an edge from WP-41 to WP-5), as the merge order has them, so the identity restack absorbs them: their slack is 7, not 9. Past the cut line they wait for My Den and are 0.2's; the release doesn't move.
- *Identity's joining chains run 1 unit behind.* Your real root and your Accounts on PostgreSQL feed the `just dev` walk and the timing re-test with 1 unit of slack each. So the timing re-test can grow from S to M without moving the release; a second unit late in either chain moves it.
- *den.proto must be in the contract by unit 5,* when the den module (WP-14) finishes and the API routes start. You approved the draft (D-6), so your review now sits inside WP-13: its PR reaches you at unit 1, and your review of its text is needed by unit 5 (in version 3, the review came before WP-13 and was needed by unit 4). After that, every unit of delay moves the end of the release by one.
- *Sign-in keeps 5 units of slack,* as the identity lead estimated, now that your real root builds against the profile ports without waiting for sign-in on `main` (the tech lead's call). Sign-in on `main` and `feature/my-den`'s restack onto it are due before the fill script (unit 10) and the `just dev` walk (unit 11). The shell's merge (WP-27) sits before sign-in's with 7 units to spare (3 for bug reports, whose web starts after it). The session check's bounded wait (WP-49) joins the identity branch's follow-ups with 7.
- *Den's access check and commission's move behind it keep 2 units each,* as their leads estimated.
- *Version 1's lever helps again, by one unit.* Moving the golden-response tests out of the wiring (WP-10) into the dev-profile run lets the wiring start at unit 1; the release then ends at 16, where identity's joining chains bind. The web lead holds it in reserve.
- *A smaller lever, the identity lead's call:* land identity's PostgreSQL seeder as WP-33's first slice. Den's route tests on PostgreSQL could then start at unit 8, giving the timing re-test a second unit of slack.
- *Review time isn't modelled.* QA, Copilot, your reviews and Heron's security reviews take time the units don't show. Worst spots: unit 7, when three critical packages (WP-16, WP-18, WP-19) and den's access check (WP-28) finish together, all for Heron; then units 9 to 11, when the joining packages and My Den on every stack (WP-40) finish (risk R13). Version 3 adds two small reviews to Heron's unit 2 and 3 (the time limits, WP-41, and the session check's wait, WP-49) and gives bug reports to Mantis (WP-47, unit 4 to 5), so Heron's units 4 to 5 don't grow. Lapwing QAs bug reports' backend (units 0 to 4) while also reviewing the host check's inputs on Atta's read side (units 0 to 3).
- *Capacity, checked by hand.* Only Wren (den's check) and Mole (the PostgreSQL setup) were named by a lead; the other joining builders are Kestrel's picks, for each lead to confirm. Den: Mole and Wren on Atta (0 to 3); then Wren on the den module (3 to 5) and den's check (5 to 7); Mole on the PostgreSQL setup (3 to 4), den's route tests (9 to 10) and the timing re-test (10 to 11); Vole's routes (5 to 7) and golden responses (7 to 8). Identity: Lynx on the different-browser message (1 to 2), the scope slice (2 to 3, inside its slack) and the cookie follow-up (5 to 6); Otter on the bare-origin slice (1 to 2), the arms (5 to 7), your real root (7 to 9) and identity's reads behind the Den (9 to 10); Stoat on the ports (0 to 2), the session check's wait (2 to 3, inside its slack) and your Accounts on PostgreSQL (7 to 9). Commission: Raven on the ports (0 to 3), the retired list (3 to 4), the fill script (5 to 6) and the seeder (7 to 8); Egret on the arms (5 to 7) and the reads behind the Den (7 to 9). Web: Finch on bug reports' web (1 to 4), the wiring (8 to 10) and My Den on every stack (10 to 11); Gecko on the time limits (1 to 2), the "sign in as" page inside the dev profile (3 to 5) and bug reports on the dev profile (after My Den's restack, by unit 11); Fennec merges the shell (0 to 1), takes bug reports' corpus step (3 to 4) and their merge (5 to 6), and runs the dev-profile run while Gecko drives the `just dev` walk in the browser you sign in, both 11 to 12. If one person ran both runs, the release would grow by a unit. Bug reports' backend: Mastiff on the contract (0 to 1) and the backend (1 to 3), Goose on the operator command (3 to 4); Kestrel's picks, confirmed by the tech lead, for Meerkat to adjust. No builder holds two packages at once. No package is staffed from Vulpes or Wolven: under your ruling on D-27, Vulpes keeps building and its people aren't lent, and Wolven pauses. So D-27 (b) moves no figure here.
- *Re-check at each phase report.* The sizes are estimates; when a package runs long, look at its slack first.

= Phases and milestones

Phases overlap across teams. Each ends in something you can see or click, and with a status report (last section).

#table(
  columns: (auto, 1.7fr, 1.3fr, 2.3fr),
  table.header([Phase], [What runs], [Starts when], [Milestone: what you can click]),
  [*0. Sign-in lands*], [Merging the sign-in stack (WP-1), its four follow-up slices (WP-2, WP-3, the identity-only scope WP-26, the session check's wait WP-49), the corpus updates (WP-4) and the merge to `main` (WP-5), after the shell's (WP-27).], [Now, on your merges], [*M0.* Before the merge to `main`, on the identity branch's dev stack, with a workspace's own database (once a database has run the branch's migrations, `main` won't start on it until the branch lands): sign in with a real Bluesky handle; your handle is in no log line at `info` or above; `scripts/web-smoke.sh` passes (it proves the proxy's routing only). Then the same on `main`: `just dev`, sign in with your real handle; a web address or a DID is refused; a sign-in finished in another browser shows its own message; the address sign-in sends you to carries no `transition`; your handle, or your DID if your PDS is slow, is in the top bar within 5 s.],
  [*1. The shell on `main`, the side work, and the engine's groundwork*], [The shell to `main` (WP-27), then the time limits (WP-41). Bug reports (WP-42 to WP-48), on `main` after sign-in. den.proto into the contract (WP-13), its text reviewed by you on its PR. Backend: Atta's read side (WP-12), the den module begins (WP-14), the identity and commission ports and fixtures (WP-15, WP-17), the dev profile (WP-21), den's PostgreSQL test setup (WP-29), the retired owner's list (WP-36).], [Now, on your rulings; the backend and den.proto on your signature], [*M1a* (after WP-27 and WP-41): on `main`, today's Accounts pages inside the new dark frame; `just dev` shows no tree yet; stop the API and a signed-in page says "Zurfur isn't answering right now", not the sign-in page. *M1c* (after WP-48, by unit 9 or never in 0.1): on `main`, "Report a bug" on every signed-in page; send one, and `just bug-reports` shows it. *M1b:* `just dev-mock` on `main`, sign in, open My Den, browse alice's mock Den, and open the drawer on a phone; found an Account and watch its mount appear in the tree.],
  [*2. My Den on the real backend*], [The den module finished (WP-14), the arms (WP-16, WP-18), the den API routes (WP-19), the golden responses (WP-20), the web's wiring (WP-10), My Den on every stack (WP-40), the dev-profile run (WP-11). Joining: den's access check (WP-28), your real root (WP-32), your Accounts on PostgreSQL (WP-33), the reads behind the Den (WP-34, WP-35), the commission seeder and fill script (WP-37, WP-38), den's route tests on PostgreSQL (WP-30), the timing re-test (WP-31), the `just dev` walk (WP-39). Bug reports on the dev profile (WP-50), after My Den's restack, unless cut.], [After den.proto is in the contract (WP-13, its text reviewed by you) and the den module; your real root after sign-in reaches `main`], [*M2a.* Run the dev-profile recipe, sign in as alice, and browse My Den served by the real API and Atta; switch to bob, if the switch survives the cuts. *M2b.* On `feature/my-den`'s `just dev`: sign in with your real handle, run the fill script, and browse your own Den from PostgreSQL.],
  [*3. Release*], [The end-of-slice security review (WP-22), once the reads behind the Den are merged and both runs pass; My Den to `main` (WP-23); your acceptance (WP-24). The cookie follow-up (WP-6) any time after the sign-in merge.], [When both runs pass], [*M3.* On `main`, your two walks: your own Den on `just dev`, and alice's on the dev profile (success criterion SC-1), and you accept the release.],
)

= Review gates

Every piece passes these in order: its team's QA, its lead, the security review where it applies, Copilot, then you. Nothing reaches you without its QA pass.

#table(
  columns: (1.2fr, 2.4fr, 1.5fr),
  table.header([Gate], [Applies to], [Rule]),
  [Team QA before the lead and before you], [Every work package and document. QA: Magpie (identity, and the corpus updates, which record identity's design), Crow (web, and the den.proto revision), Owl (den, and the dev profile, which serves the Den), Jay (commission), Lapwing (bug reports' contract, backend and operator command), Crow also for the time limits and bug reports' web, corpus step and dev-profile line (with Owl), Magpie also for the session check's wait (WP-49). Kea (posts) joins for the `/post/<did>` stub's slice, and Lapwing (moderation) for the host check's inputs on Atta's read side. The challenges and the resolutions travel with the piece.], [Your ruling of 2026-10-08 (PR \#237)],
  [Security review by Heron], [Everything touching authentication, the private↔public boundary, DID or handle correlation, or session and token handling: WP-2, WP-3, WP-26, WP-49, WP-6 (sign-in); WP-41 (the gate's page and the time limits); WP-8 (headers and error logging); WP-9 (the path builder); WP-10 and WP-40 (the decoder now reads real users' data, and the switch's removal), reviewed together; WP-12 (Atta's resolver, the one read path); WP-14 (mount matching, the one not-found); WP-15 (the read that pairs a user with their characters, and an Owner-only read); WP-16 and WP-18 (standing and levels); WP-17 (who counts as a Participant); WP-19 (path handling); WP-21 (a fixed-identity sign-in); and the joining packages: WP-28 (den's access check), WP-30 and WP-31 (the one not-found on PostgreSQL), WP-32 (your root's name from your PDS), WP-33 (Owner-only reads and the cross-persona guard on PostgreSQL), WP-34 and WP-35 (the reads behind the Den), WP-37 (Participants on PostgreSQL), WP-38 (light: the session cookie never logged or stored), and any fix from the `just dev` walk that touches sessions or data. Den's check is reviewed as it lands with the arms (unit 7); commission's move behind it and the commission seeder get a joint sitting at unit 9, where den's check is read again as the code they call. Then the full end-of-slice review, WP-22, which starts only once WP-34 and WP-35 are merged.], [Definition of Done; the tech lead's review plan on the question sheet; the tech lead's call of 2026-10-09],
  [Security review by Mantis], [Bug reports (WP-42 to WP-46) in one sitting, WP-47: the reporter from the session only, what a report carries, the text rules and the terminal's escaping, the rate limit, the route in the cookie surface. Not Heron, so his queue doesn't grow. At xhigh, the designer lane's effort for every security review.], [The proposal; the tech lead's calls of 2026-10-09 and, on the effort, 2026-10-10],
  [Copilot on every PR], [Every PR, stacked or not, triaged with `/address-comments`.], [Your directive of 2026-07-05],
  [The local gate before a PR], [`just gate` green on the pinned toolchain (format, lint, the full test suite), with the dev-profile feature built for its guard test.], [Definition of Done; PR \#244],
  [Your review], [Each slice, by its jj change id; every corpus PR; every contract break, as its own declared baseline-reset PR (WP-36); and every one-way door *before* it is built or merged: den.proto, whose draft you approved (D-6) and whose text you review on WP-13's PR (D-19); the one-time session delete on its PR (D-32); and `bug_report.proto` on its PR (your ruling on `v2-carryover.md`, row 84). The other candidates are listed in the Technical Design Document.], [CLAUDE.md, "Roles"; your rulings of 2026-10-08 and 2026-10-09],
  [Definition of Done before `main`], [Each feature branch: every acceptance criterion maps to a green test; gates green; doc comments updated; the corpus in sync; `/close-gaps --post` clean; security-reviewed; no domain decision was Claude's.], [CLAUDE.md, "Definition of Done"],
)

= Merge plan

*Merge each piece as soon as it passes review* (your rule after the resolver's stack of seven unmerged slices). No slice waits on another unmerged slice longer than it must. *The order to `main`:* the shell, the time limits, the identity branch, bug reports (unless cut), then My Den (the tech lead's calls of 2026-10-09; bug reports' place follows your cut line, D-34).

- *`feature/web-explorer-shell`* first: \#245 merges into it, and \#246 to \#249, stacked above it, each after its base (slice 2 out of draft after Crow's last check); then the branch reaches `main` as one squash (WP-27), with its coherence check and corpus sync. Slice 1b keeps `main`'s `just dev` free of a Den until My Den arrives.
- *`feature/identity-resolver`* already exists, cut at `main`'s tip. Slices merge in as each is approved: \#238, \#239, then slice 3’s PR (opened once both are in), then \#240 → \#241 → \#242 → \#243. WP-2, WP-3, WP-26 and WP-49 land as slices 8, 9, 10 and 11. Once WP-4’s corpus PR is in and the real-Bluesky check passes, the branch reaches `main` as one squash (WP-5).
- *`feature/my-den`* is new, cut at `main`'s tip at the start of Phase 1, after \#236, \#237 and \#244 merge, so it starts on the pinned toolchain and the shared `cid` dependency. Proposed by the web lead: cut it after the shell (WP-27) too, so the wiring starts from the shell. Every My Den slice, the joining ones included, is cut from its tip and merged back as soon as it passes review. It reaches `main` only after both runs, the reads behind the Den (WP-34, WP-35) and the end-of-slice review pass (WP-23).
- *`feature/bug-reports`* is new, cut at `main`'s tip once the shell is there (WP-27), after \#236, \#237 and \#244. The web's first slices (WP-45) stack on the shell's branch until then and move across with it; the contract's slice (WP-42) merges in once the branch exists. It reaches `main` after the identity branch and before `feature/my-den`'s restack (WP-48), or not in 0.1: past unit 9 it waits until My Den reaches `main`, and is 0.2's.
- *Straight to `main`:* the `is_archived` fix, on a bug branch, before `feature/my-den` is cut; the time limits (WP-41), on their own branch after the shell, before the identity branch's restack, so that restack absorbs them; and WP-6, after WP-5.
- *Restacks, which you drive with git-town:*
  - `feature/identity-resolver` onto `main`, once, after \#236, \#244 and the shell are in: slice 2 changed the root `Cargo.toml` and `Cargo.lock`, and the branch must pass on the pinned toolchain. The shell shares ten web files with it: the API client, its mock and its spec, `session.ts` and its spec, two `NODE.json` files, and three login files the shell moves into `(public)/login/` (the login page's server code, its spec, the callback errors), which the branch edits in place. The restack carries those edits across the move, and absorbs the time limits (WP-41), which also edit the API client. Proposed: the different-browser message (WP-2) is cut after the restack. Due by unit 9 for the release, by unit 7 for bug reports to make their cut line;
  - `feature/bug-reports` onto `main`, once, after the identity branch lands, by unit 8: it shares the API client and its mock with the identity branch;
  - `feature/my-den` onto `main`, once, after the identity branch and bug reports land (or at unit 9 without bug reports, which then wait for My Den), and before the fill script (unit 10) and the `just dev` walk (unit 11). It absorbs the time limits too. Bug reports share these files with it: the cookie surface in `routes/mod.rs`, the test that matches routes to the contract, the generated contract code, the API client and its mock, and the top bar, drawer and error view. Your real root is checked live only after it; den's PostgreSQL test setup may need a small fix at it.
  Conflicts are expected in the root `Cargo.toml` and `Cargo.lock`, and in those ten web files.
- *Names:* tickets are stopped, so slices follow the identity branch's pattern: commits and PRs `[my-den][n] …`, on branches `feature/my-den-<n>-<slug>`, so CI runs on their PRs into `feature/**`; bug reports likewise, `[bug-reports][n] …` on `feature/bug-reports-<n>-<slug>`.
- *Every commit on an integration branch is gate-green;* a slice branch may hold work in progress.

= What blocks the start

Rows 1 to 11 block work; the rest are requests. *Rows 2, 3 and 12 are done:* you ruled them on 2026-10-09, with every decision, so row 1, your signature, is all that unit 0 waits for: it gates Atta's read side (WP-12), which has no slack. Rows 7 and 10 now ask only for reviews on PRs. The rows keep their numbers, because the other documents cite them.

#table(
  columns: (auto, 2fr, 2fr, 1.1fr, 1.3fr),
  table.header([\#], [What you do], [What it unblocks], [Requested by], [Latest harmless]),
  [1], [Sign the Product Design Document: say "signed". Every decision is answered.], [Starts the build: the packages the tech lead holds until then: Atta's read side (WP-12), den.proto into the contract (WP-13), identity's and commission's ports and fixtures (WP-15, WP-17), den's PostgreSQL test setup (WP-29), retiring the owner's list (WP-36), and bug reports' contract and web (WP-42, WP-45)], [Now], [Unit 0; later moves the release unit for unit],
  [2], [*Done, 2026-10-09.* You approved where archived items are hidden (D-15), with its inner question answered and the link question deferred to the first slice with a symlink, and the joined component diagram (the Technical Design Document, Feature 3).], [Atta's read side (WP-12), which builds the host trait; den's access check (WP-28)], [—], [—],
  [3], [*Done, 2026-10-09.* WP-12 builds on your uncommitted Atta draft (`v2-carryover.md`, row 9): it commits the draft as its starting point, replaces its `Key(Uuid)` with the path segment (My Den rulings, decision 2), and leaves the payload hash, `Payload` and attributes for the first payload.], [WP-12's first slice; nobody discards your work by accident], [—], [—],
  [4], [Merge \#236, \#237 and \#244 into `main`], [Cutting `feature/my-den` on the pinned toolchain and the shared `cid` dependency; the identity branch's restack], [2026-10-09], [Before the first My Den slice merges],
  [5], [Merge \#238 and \#239 into `feature/identity-resolver`], [Phase 0 (WP-1), then slice 3’s PR], [2026-10-09], [Unit 5 (unit 3 for bug reports): the sign-in chain has 5 units of slack (3 for bug reports)],
  [6], [Merge the shell: \#245 into its branch, \#246 to \#249 each after its base, then the branch to `main` (WP-27)], [The time limits (WP-41); bug reports' web (WP-45); the identity branch's merge to `main` (WP-5), which follows it], [2026-10-09], [Unit 7, so the shell is on `main` by unit 8 (unit 3, on `main` by 4, for bug reports)],
  [7], [*Review den.proto's text on its PR* (WP-13). You approved revision 2.1 as a whole on 2026-10-09 (D-6 to D-10 and D-14, the one-way door); per D-19 the file still comes to you as text, with the lifecycle file's edit.], [WP-13's merge, then WP-19, WP-20, WP-10, WP-11, and through them WP-30 and WP-39: all of Phase 2], [When WP-13's PR opens (unit 1)], [Unit 5, when the den module (WP-14) finishes and the API routes start; later moves the release unit for unit],
  [8], [Restack `feature/identity-resolver` onto `main` once, after \#236, \#244 and the shell are in (git-town)], [The identity branch's merge to `main` (WP-5)], [When the shell and the time limits land], [Unit 9 (unit 7 for bug reports)],
  [9], [Restack `feature/bug-reports` onto `main` once, after the identity branch lands; then `feature/my-den` once, after both land, or at unit 9 without bug reports (git-town)], [Bug reports' merge (WP-48); the fill script and the `just dev` walk on My Den's branch], [When the identity branch lands], [Bug reports': unit 8 (past it they are 0.2's). My Den's: unit 10],
  [10], [Review two PRs: the one-time session delete on the scope slice's (WP-26, D-32), and `bug_report.proto` on the contract's (WP-42; `v2-carryover.md`, row 84). With the first, one small ask your rulings didn't cover (`v2-carryover.md`, row 70): should the delete first revoke each stored session at your PDS? *Claude's answer (adjust if wrong):* no, D-32 (a) as ruled: the delete is a migration, which can't call your PDS, and each session's tokens can't be used without its own key, deleted with it. The approvals this row also asked for (D-21, D-22, D-29) you gave on 2026-10-09.], [The scope slice's merge; bug reports' backend (WP-43)], [When each PR opens], [The session delete before the scope slice merges, by unit 9 (7 for bug reports); `bug_report.proto` by unit 4],
  [11], [Sign in with your real handle for the `just dev` walk (WP-39); Gecko then drives that browser, or you run the walk. An account whose password an agent holds would put your credential in an agent's hands, so Claude's answer is: you sign in yourself.], [The `just dev` walk, on the critical path], [When the walk starts], [Unit 11; later moves the release unit for unit],
  [12], [*Done, 2026-10-09.* You ruled every decision this row asked you to skim; one, D-27, differs from Claude's answer.], [—], [—], [—],
  [13], [Install Typst yourself, if you want to rebuild the PDFs (`sudo pacman -S --needed typst`)], [The set compiles with Typst 0.15.1 and fletcher 0.5.8 (a copy in Claude's scratch folder, 2026-10-09)], [—], [—],
  [14], [Merge slice PRs as they pass review], [Keeps branches short and restacks rare], [As each comes], [—],
  [15], [Say yes or no to one design-sync for the corpus follow-ups your rulings create: D-29 and D-31 (the My Den rulings, decision 19: the two joiners, and Account names), D-7 (their parameter's spelling), D-13 (the Authenticators page, the hardened resolver's decision page, the approved-crates note) and the Character page's stale "one or more Keepers" (`v2-carryover.md`, row 94)], [The corpus matching your rulings. Until you answer, WP-13 and WP-4 carry their parts as planned, and the Character page waits], [When the tech lead offers it], [—],
)

*Requested-by dates are requests,* chosen so you are never the bottleneck. When the den module actually finishes isn't known until Phase 1 shows the pace.

= Risk register

#risk-table(
  ([R1], [Your review of den.proto's text on WP-13's PR lands after unit 5.], "L", "H", [You approved the draft as a whole (D-6), and every answer is already written in, so the PR's den.proto is revision 2.1 unchanged; around it the PR adds D-19’s line, the registry entries, D-9’s id-rule text, the route-test pin and the generated code, and any other difference is a finding. It reaches you at unit 1, four units early. The web keeps building on mock data and the backend on Atta, so nothing idles first.], [Phase 2 slides unit for unit; M1b (mock data) still lands on time.]),
  ([R2], [Atta's read side, the first package on the critical path, is bigger than its L.], "M", "H", [The smallest slices first (`Segment` and `NodeName`, traversal, mounts, listing), QA on each; Mole and Wren together.], [Fall back to "each node answers alone" if standing along the real path stalls (your recorded backup). No rule of Atta's is cut. Lever: move the golden-response tests out of the wiring, which saves one unit.]),
  ([R3], [The host seam doesn't fit across three teams.], "M", "M", [The host trait lands first; teams build against it; QA reviews after the interfaces settle.], [The den lead arbitrates; the tech lead escalates to you only for a domain fork.]),
  ([R4], [The shell's merge to `main` (WP-27) runs late and holds up the identity branch's (the tech lead's merge order).], "L", "M", [The shell is built and reviewed; only slice 2’s last check and your merges remain. WP-27 has 7 units of slack (3 for bug reports).], [The tech lead revisits the merge order; the later branch restacks once either way.]),
  ([R5], [The end-of-slice security review finds must-fixes late, or runs past its L.], "M", "M", [Heron reviews each security piece as it lands; the checklist (19 findings, plus the joining items) is known now; the tech lead planned it as L, not M, so it isn't under-sized.], [Fix rounds before the merge; the release waits. Security over speed. If Heron sizes it M, the release gains back a unit.]),
  ([R6], [Real Bluesky sign-in fails on the local stack after the resolver and the scope change; nobody has run it end to end since.], "L", "H", [The check runs on the identity branch before WP-5, with `scripts/web-smoke.sh`, and again on `main` (M0); the scope slice tests the address it sends.], [Hold WP-5 (identity to `main`) until it passes. Your real root (WP-32) waits with it.]),
  ([R7], [Branches drift; the root `Cargo.toml`, `Cargo.lock` and ten shared web files conflict, three of them moved by the shell.], "M", "L", [Merge \#236 first, then the shell, then the identity branch; small slices; you drive the three planned restacks; the identity restack carries its login edits across the move.], [—]),
  ([R8], [D-1 flips to joining, unsized.], "L", "L", [*Resolved 2026-10-09:* you ruled joined before any My Den build started; the four leads sized it, and this version places it.], [—]),
  ([R9], [The dev profile, which signs everyone in, reaches a deployed build.], "L", "H", [A Cargo feature, a refusal to start unless the environment is `dev`, and a test for both, built in the gate. 0.1 deploys nothing.], [—]),
  ([R10], [The identity migration `20261009002000_…` was typed by hand and collides with another branch.], "L", "M", [Checked in WP-1; regenerated with `just migrate-add` if so.], [—]),
  ([R11], [Identity's three programmers carry Phase 0 (now with the session check's wait, WP-49), the arms and five units of joining: the arms on the critical path, and three joining packages one unit behind it (WP-32, WP-33, WP-34).], "M", "M", [Phase 0’s slices are small and end by unit 5; the joining packages start at unit 7, after the arms.], [Cut: WP-6 (the cookie follow-up) moves to 0.2. The tech lead may lend the plugin team; Vulpes's people keep building Vulpes and aren't lent (your ruling on D-27).]),
  ([R12], [The demo feels empty: most folders say "content not shown yet", and your own Den has no characters or Workflows.], "M", "L", [The Product Design Document says so plainly; the fill script gives your Den commissions; the dev-profile walk and the mock world show every pane state.], [—]),
  ([R13], [Security reviews pile up: four packages finish at unit 7 (three of them critical), then the joining packages at units 9 to 11 and My Den on every stack at 11, all for Heron.], "H", "M", [Heron reviews slices as they land rather than whole packages; commission's move and seeder share one sitting at unit 9. Version 3 gives bug reports to a second reviewer, Mantis (the tech lead's call), and adds only two small reviews to Heron's units 2 and 3 (WP-41, WP-49).], [The release waits; no review is skipped.]),
  ([R14], [Another Claude answer is overturned late.], "L", "L", [*Resolved 2026-10-09:* you ruled every decision before any build started. One ruling differs from Claude's answer, D-27 (b), and it moves no package; D-36 adds your own exception, already planned.], [—]),
  ([R15], [The timing re-test finds a pair of causes that runs different statements: WP-31 grows from S to M, or more.], "M", "M", [Its 1 unit of slack absorbs an M; the cache's state is fixed in the tests; landing identity's seeder first gives it a second unit.], [Beyond M, the release moves. Equal statements are never cut.]),
  ([R16], [The `just dev` walk waits on four teams' joining work, sign-in on `main`, and your real sign-in at unit 11.], "M", "M", [It is S; its joining inputs have 1 or 2 units of slack; Gecko drives the browser you sign in, beside the dev-profile run; your sign-in is row 11 of "What blocks the start".], [A late walk delays the end-of-slice review, not the dev-profile run.]),
  ([R17], [One of your three restacks lands late: the identity branch's after unit 9, bug reports' after unit 8, or `feature/my-den`'s after unit 10.], "M", "M", [All three are in "What blocks the start" with their units; the merge order (shell, time limits, identity, bug reports, My Den) keeps each branch to one restack; conflicts are expected in `Cargo.toml`, `Cargo.lock` and ten web files, three of them moved.], [Past unit 10, the `just dev` walk slips, and the release with it. A late bug-reports restack only moves bug reports to 0.2.]),
  ([R18], [Bug reports miss their cut line (unit 9): sign-in reaches `main` after unit 8, Mantis's review runs long, or their restack is late.], "M", "L", [Built from unit 0 by people with no other work then; their chain ends at unit 6, 3 units early; Mantis is named now; the shell's merge (by unit 4 for them) and their restack are in "What blocks the start".], [They move to 0.2 by your ruling and wait for My Den; nothing else moves.]),
  ([R19], [Bug reports' branch adds conflicts to My Den's restack: the cookie surface in `routes/mod.rs`, the route-to-contract test, the generated contract code, the API client and its mock, and the top bar, drawer and error view.], "M", "L", [They land before My Den's one planned restack, which absorbs them; small slices; the cut line keeps them out of the way after unit 9.], [Past unit 9 they wait for My Den, and restack onto it in 0.2.]),
  ([R20], [The 5 s session check runs out while the API is up, because `/me` waits on your PDS for your profile (10 s per fetch, 20 s per lookup in the resolver), so the gate says Zurfur isn't answering when it is.], "L", "M", [WP-49 bounds `/me`'s wait for your profile inside the 5 s and falls back to your DID (the tech lead's call), with a test whose PDS never answers.], [Until WP-49 lands with the identity branch, "Try again", or the profile cache; the limits are your ruling and stay.]),
  ([R21], [Vulpes keeps building beside 0.1 (your ruling on D-27), so its PRs share your review queue with 0.1’s PRs, merges and restacks.], "M", "L", [The tech lead keeps your one queue most blocking first, and each 0.1 item that blocks work carries its latest unit in "What blocks the start". Vulpes's people aren't lent, and 0.1 takes no Vulpes dependency (WP-22 checks).], [A 0.1 item past its latest unit moves the release as R1 and R17 say.]),
)

== What to cut first, in order

+ The dev profile's "sign in as" switch (US-3.6). bob's Den is then shown by tests and golden responses only.
+ Clearing the cookie only on a match (WP-6, D-12): it moves to 0.2, and its criterion in the Product Design Document is marked as cuttable.
+ The mock world's extras beyond one card and one archived commission; the rest stay as unit tests.
+ Refusing a PDS address that isn't a bare origin (WP-3, D-11): it moves to 0.2, criterion marked likewise. It isn't on the never-cut list because it has no security effect.

*Bug reports* aren't on this list: they leave by their own rule if they aren't on `main` by unit 9, and cutting them sooner saves the release nothing. *The time limits* are your ruling, small and off the path.

*Never cut:* anything with a security effect, the one not-found, page tokens, the path builder, the reviews or QA, and the joining packages: the reads behind the Den are a gate on My Den's merge, and the `just dev` walk is half of your acceptance.

= Status reports

One report at the end of each phase, filled in by the tech lead and Kestrel, short, with the top of your queue at the end. The final report carries posts' checks (no post and no public-blob byte moved; the public-records port wired nowhere), run by Kea before your acceptance.

#let report(n, name) = block(width: 100%, stroke: 0.6pt + colors.rule, radius: 3pt, inset: 9pt, above: 10pt, {
  text(weight: "bold", fill: colors.accent)[Phase #n: #name]
  v(4pt)
  let line-field(field) = grid(
    columns: (32mm, 1fr),
    column-gutter: 6pt,
    text(size: 8.5pt, fill: colors.muted, field),
    box(width: 100%, height: 1.1em, stroke: (bottom: 0.5pt + colors.rule)),
  )
  stack(
    spacing: 7pt,
    line-field[Date],
    line-field[Milestone reached?],
    line-field[Done],
    line-field[Next],
    line-field[Blocked on],
    line-field[Decisions taken],
    line-field[Risks changed],
    line-field[Critical path changed?],
  )
})

#report(0, [Sign-in lands])
#report(1, [The shell on `main`, and the engine's groundwork])
#report(2, [My Den on the real backend])
#report(3, [Release])

= What changed in version 4

Version 4 goes with the Product Design Document's version 5 and the Technical Design Document's version 4. It folds in your rulings of 2026-10-09 on every decision and on the carry-over's questions (`rulings-2026-10-09.md`), and the tech lead's answer of 2026-10-10 on Mantis's effort. *No package is added and none changes size: 50 packages, 75 units, and the critical path stays at 17 units,* recomputed by script from the "Depends on" column.

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [D-27 is (b): Vulpes keeps building beside 0.1 and its people aren't lent; Wolven pauses. R11's fallback lends the plugin team only; a new risk, R21, for Vulpes's PRs in your queue; capacity says no package was staffed from Vulpes, so no figure moves.], [your ruling],
  [den.proto is approved (D-6): your review moves inside WP-13, whose PR brings you the text with D-19’s line in the lifecycle file; WP-13 starts at your signature (phase 1, slack 4) and must merge by unit 5 (was: the review by unit 4). The summary, "What starts now", the graph's hexagon, "How it was computed", the phases, the review gates, "What blocks the start", row 7, and R1 (now unlikely) say so.], [your ruling],
  [Unit 0 waits only for your signature: rows 2 and 3 of "What blocks the start" (D-15, the joined diagram, the Atta draft) and row 12 (skim the answers) are done; row 10 asks only for the session delete's and `bug_report.proto`'s PR reviews. Rows keep their numbers. A new row 15: the tech lead's offer of one design-sync for the rulings' corpus follow-ups.], [your rulings],
  [WP-12 builds on your Atta draft, with no payload method, and leaves the payload hash, `Payload` and attributes for the first payload; WP-22 reads the question sheet's finding 19 as "no code path serves a payload".], [your rulings on carry-over rows 9 and 12],
  [WP-13 also carries D-29’s design-sync of the My Den rulings beside D-7’s; WP-27 makes the mock's slots say "Content not shown yet." (D-21); WP-42 and the "Your review" gate: you review `bug_report.proto` on its PR (row 84); WP-47 and the Mantis gate: at xhigh.], [your rulings; the tech lead],
  ["What's already done": the den.proto revision approved, and a new row for your rulings. R14 is resolved. R4's slack for the shell corrected to 7 (3 for bug reports), which version 3 left at 8 after adding the time limits' edge.], [your rulings; Kestrel],
  [*After Owl's QA* (`qa.md`, "QA challenges, version 5"): R1 says the PR's den.proto is revision 2.1 unchanged and lists what the PR adds; row 10 carries one small ask with the session delete's PR, whether to revoke at your PDS first (carry-over row 70, Claude's answer: no); WP-13, row 15 and carry-over row 94 add D-31’s Account names to decision 19’s design-sync; WP-16 and WP-18 test that a name of joiners alone is refused; WP-12 names the question sheet's section 7 answer 2 as replaced too; row 2 says D-15’s link question is deferred; row 5 gives bug reports' unit 3; "What starts now" gives the real reason the follow-up slices start on your merges; WP-36 waits for your signature; D-36’s exception is named beside D-27.], [den QA],
)

= What changed in version 3

Version 3 goes with the Product Design Document's version 4 and the Technical Design Document's version 3. It follows your four rulings of 2026-10-09 (D-1 is (b); bug reports in 0.1 and cuttable, D-34; the time limits and the gate's page, D-35; migrations in pre-alpha, D-36), the web lead's bug-report proposal, and the tech lead's calls of the same day.

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [Ten new packages, 13 units: the time limits (WP-41, web, S), the session check's bounded wait for your profile (WP-49, identity, S), and bug reports (WP-42 to WP-48: the contract S, the backend M, the operator command S, the web L, the corpus step S, Mantis's review S, the merge S; and WP-50, S, bug reports on the dev profile, added after Owl's QA). 50 packages, 75 units.], [your rulings; the proposal; the tech lead; den QA],
  [Critical path recomputed by script, and by den QA independently: 17 units, unchanged; every new package has slack. Bug reports get their own slack against the cut line at unit 9 (3 units), and the release figures leave them out since they can be cut; sign-in's slack is 5 for the release, 3 for bug reports. After QA: an edge from the time limits to sign-in's merge (as the merge order has it), and bug reports' web starting on its own branch after the shell, so no slice moves between branches. A new figure, "Beside the path", draws them.], [Kestrel; den QA],
  [WP-5 depends on WP-49; WP-39 on WP-41 and checks the gate's page with the API stopped; WP-22's list adds the gate's page; M0, M1a and a new M1c check the additions.], [Kestrel; web],
  [Capacity: Finch on bug reports' web (0 to 3), Gecko on the time limits (1 to 2) and the "sign in as" page (3 to 5), Fennec on the corpus step and the merge, Mastiff and Goose on bug reports' backend, Stoat on WP-49 (2 to 3). Review time: Heron gains two small reviews at units 2 and 3; Mantis takes bug reports at 4 to 5.], [the tech lead; the proposal],
  [Review gates: Lapwing, Crow and Magpie's new QA; Heron on WP-41 and WP-49; a new row for Mantis; your review of `bug_report.proto` and the session delete on their PRs.], [the tech lead; the Technical Design Document],
  [Merge plan: the order is the shell, the time limits, the identity branch, bug reports, then My Den; `feature/bug-reports` is new; your restacks are three, with the files bug reports share with the identity branch and My Den.], [the tech lead; the proposal],
  [What blocks the start: the signature also starts WP-41, WP-42 and WP-45; bug reports' latest units beside the release's; the new restack; `bug_report.proto` in row 10; D-1's fork leaves the skim row, D-37 joins it.], [Kestrel],
  [Risks: R7, R13 and R17 updated; R14's D-1 fork resolved; R18 to R20 new (bug reports' cut line, their shared files with My Den, the session check against your PDS). The cut list says why bug reports and the time limits aren't on it.], [Kestrel; the proposal; the tech lead],
  [*After den QA's pass* (`qa.md`, "QA challenges, version 4"): WP-50, bug reports on the dev profile; WP-41 before sign-in's merge in the model, and on your merges like the scope slice; WP-45 on its own branch after the shell; WP-41, WP-45, WP-49 and WP-10 gain QA's points (escaping, a form post meeting the gate, the origin check, CR LF, the phone icon, the profile fetch finishing, the Den reads' failure tests); WP-22 waits for WP-50; the shell is due on `main` by unit 4 for bug reports; R7 counts three restacks; plan row 1 names its packages in words.], [den QA],
)

= What changed in version 2

Version 2 goes with the Product Design Document's version 3. It follows your ruling that 0.1 joins the halves (D-1, on Claude's answer: without character storage), the four leads' joining pages (`objectives/join-*.md`), the tech lead's calls of 2026-10-09, and the corrections in `v2-carryover.md` that this version applies (marked there).

#table(
  columns: (3fr, 1fr),
  table.header([Change], [Driven by]),
  [Fifteen new packages: the scope slice (WP-26), the shell to `main` (WP-27), and thirteen for joining (WP-28 to WP-40). The web lead sized "My Den on every stack" S inside the wiring; the tech lead made it its own package (WP-40), and the wiring stays M.], [all four leads; the tech lead],
  [WP-17 is L, not M: five reads, not three, and the by-user index. WP-22 is L, not M (the tech lead). WP-7 to WP-9 and WP-25 are built.], [commission; web; the tech lead],
  [Changed packages, after den QA and the team QA passes: WP-5 follows the shell and checks the log; WP-10 gets Heron, the read cap and the card's type; WP-13’s pin lists the den routes like any other; WP-17 and WP-18 load stored names as text and check names only at write; WP-19 is mounted on every stack and carries the Atta guard; WP-21 keeps both locks and has its page built by web; WP-22 is L, waits for the reads behind the Den and widens its list; WP-24 is acceptance, two walks; WP-32 no longer waits for sign-in on `main`; WP-35 and WP-37 name their pairs and share a sitting; WP-38 asks for the cookie with echo off; WP-39 checks the hard delete, the switch page and sign-out.], [den, web, identity, commission; their QA; the tech lead],
  [Graph, critical path and slack recomputed by script: 17 units, two more than version 1 (My Den on every stack on the path, and the L review); identity's two joining chains 1 unit behind; sign-in's slack 5; den's check and commission's move 2 each; the timing re-test can grow to M; version 1's lever saves a unit again.], [Kestrel; the tech lead],
  [Phases: the shell reaches `main` in Phase 1; M2 splits into the dev-profile walk and the `just dev` walk; M3 is two walks.], [web, the tech lead],
  [Review gates: Heron on every new security-reviewed package; Kea and Lapwing join for their pieces; contract breaks go to you as declared baseline resets.], [the joining pages; earlier carry-over],
  [Merge plan: the shell, then identity, then My Den; the two restacks and the ten shared web files, three moved.], [the tech lead; web QA],
  [What blocks the start: your signature, D-15 with the joined diagram, and the Atta draft at unit 0; the shell's merges; your two restacks; the other approvals; Typst now compiles.], [Kestrel; den QA],
  [Risks: R4 rewritten for the shell's merge; R5 for the L review; R8 resolved; R11 and R13 grown; R14 to R17 new.], [Kestrel; the leads' risks; den QA],
  [Acceptance at the end is separate from your signature at the start, which starts the build (WP-24, M3).], [den QA],
)
