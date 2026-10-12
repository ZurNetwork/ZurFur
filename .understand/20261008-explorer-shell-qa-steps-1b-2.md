# QA: Explorer shell, steps 1b "No sidebar where no Den is served" and 2 "My Den on the mock"

2026-10-08 · Crow (web QA) for Fennec (web lead) and the tech lead · reviewed step 1b at commit 7fac6afe (change vprutnrt) and step 2 at commit ca936fa6 (change vrzmmnwu), in the jj workspace `~/code/zurfur-workspaces/crow` (change srylnrqp on top, never pushed). Checked against the shell design (`.understand/20261008-web-explorer-shell.md`: steps 1b and 2, Q1 to Q3, and "Questions from the build" Q4 to Q8) and the Product Design Document's stories for the frame and My Den.

## In one screen

- **What I checked.**
  - The full gate, as CI runs it.
  - Each criterion against a test.
  - 31 planted bugs.
  - The rulebook, accessibility basics and the `NODE.json` edits.
  - The mock app in a real browser at 1280 px and 320 px: keyboard, mouse, drawer, paging, the switch, founding an Account, and going Back into a deep page.
  - The everyday stack against a stub API.
- **The gate.** Green: every step exits 0, and the web suite passes 436 of 436. No port clash this time.
- **What holds.**
  - **The everyday stack (1b).** No sidebar, no tree, no drawer and no "My Den" link. "Accounts" and "Sign out" stay in the top bar on a phone, and nothing scrolls sideways at 320 px.
  - **The path builder.** It refuses every malformed address I tried and drops every unknown query parameter.
  - **Not-found.** Every miss shows the one "Nothing here" page.
  - **Cards.** A card shows no visibility and no link.
  - **The switch.** It lives only in the address and keeps focus.
  - **The tree.** It follows the W3C keyboard pattern, keeps focus on the item you open, and opens down to the open item after you come Back into the frame.
  - **The pane.** Opening from the pane or the path bar moves focus to its heading.
  - **Founding an Account.** It shows up in `accounts`.
  - **Tests.** 27 of the 31 planted bugs turned the suite red.
- **Must-fix: none.**
- **Should-fix (7).**
  - Focus drops to the page body after the last "More", in the pane and in the tree.
  - A malformed address leaves the tree's `~` loading forever.
  - The open tree item is marked only by a faint tint (1.33:1).
  - The tree's "More…" row has no level or position, though the release asks it of every item.
  - A relative import gets past the rule that only the path builder may mint a Den link.
  - Four behaviours have no test.
  - On the everyday stack, a typed `/den` still answers with a "not connected yet" page. The release's answer to its first decision says there is no route there, and no one owns closing that before `main`.
- **Notes (11),** including challenges to Finch's own choices. I'd keep three of them: the fixed `/den`, the plain link before the script, and the 300 ms skeleton. I'd change one: clearing the tree after any `goto`. The step-1 note already covers the fifth (the error log).
- **Heads-up.** Finch restacked during my review. The new step 2 (89e5accc) has conflicts in 9 files, including the signed-in error page and the frame. My findings are against ca936fa6; re-check once the conflicts are resolved.

**Verdict: ready for the lead.** No must-fix. The should-fixes are small, and the four test gaps are one spec each.

## Findings

| # | Severity | Where (file:line) | What's wrong | Proof | Fix |
| --- | --- | --- | --- | --- | --- |
| 1 | should-fix | `src/lib/components/explorer/EntryList.svelte:59-79`; `src/routes/(session)/den/[...path]/+page.svelte:55-72`; `src/lib/components/explorer/TreeBranch.svelte:144-170` | **Focus falls to the page body after the last "More".** Pressing "More" on the last page removes the link (or the tree row), so focus lands on `<body>`. A keyboard user has to Tab from the top of the page to find the new rows. | Browser run on the mock, "Big ref batch" › attachments (40 files). In the pane, Enter on "More" took the rows from 25 to 40, and focus went to `BODY`. In the tree, Enter on "More…" took the rows to 40, and focus went to `BODY`. | After a page is appended, move focus to the first new row: the pane's first new link, or the tree's first new item. Add a test for each. |
| 2 | should-fix | `src/lib/components/explorer/tree-memory.svelte.ts:68`, `:101-106`; `src/lib/server/den.ts:320-334` | **A malformed address leaves the tree loading forever.** A refused path reads nothing, not even the ancestors, yet `~` is marked open. The missing-folder check returns nothing on a not-found, so the root's listing is never fetched. The tree shows a busy group under `~` for good. An address that's merely missing (like `/den/nope`) gets the root listing, so the two "Nothing here" pages also look different. | Browser run, full load of `/den/a//b`: after 2 s the tree holds 1 group with `aria-busy="true"` and 0 rows under `~`. `/den/nope`: the root's four folders. | On a not-found, fetch the root's listing if the tree lacks it, or leave `~` closed. Add a test. |
| 3 | should-fix | `src/lib/components/explorer/TreeBranch.svelte:248-250` | **The open tree item is marked by colour alone, and faintly.** Its background is 1.33:1 against the sidebar, and 1.12:1 against a hovered row. The wireframe draws a bar beside the open item (┃). WCAG's guidance puts a background change inside a control outside the 3:1 rule, but says it may fail "Use of Color". `aria-current` covers screen readers, not sighted users. | Contrast from the token values: `--color-selected` over the surface gives #312e42 (1.33:1 against #1a191e). W3C, Understanding "Non-text Contrast", Figure 15. The step-1 QA, note on colour-only current marks. | Add a non-colour cue: the wireframe's bar (an inset border), or bold text. |
| 4 | should-fix | `src/lib/components/explorer/tree-model.ts:152-169`; `src/lib/components/explorer/TreeBranch.svelte:147-151` | **In a partly loaded folder, the "More…" row and an open item listed after it have no level or position.** The release's screen-reader story asks that "every item carries its level and position with the set size marked unknown". The rows around them do. | Browser run: the attachments' "More…" is `role="treeitem"` with `aria-level`, `aria-posinset` and `aria-setsize` all null, while "ref-01.png" has 5, 1 and -1. The model gives the stray open item `position: undefined`. | Give both rows the folder's level, the next position and `aria-setsize="-1"`. Extend the partly-loaded test to every row. |
| 5 | should-fix | `eslint.config.js:86-92` | **The rule that only the path builder mints a Den link can be bypassed with a relative import.** `no-restricted-imports` matches the specifier `$lib/types/brand` exactly, so `import { denHref } from '../types/brand'` passes lint. The design relies on this rule as the builder's guard. | Planted in `lib/server/den.ts`: `import { denHref as mintHref } from '../types/brand'` plus a minted `/den/../accounts`. `eslint` exits 0. The same import through `$lib/types/brand` is caught. Reverted. | Add a `patterns` entry covering every spelling (for example `**/types/brand`, with `importNames: ['denHref']`), or move the mint into a module only `den-path.ts` may import, and test the rule with a lint fixture. Heron may want this too. |
| 6 | should-fix | `src/routes/(session)/+layout.svelte:40`; `src/routes/(session)/den/[...path]/+page.svelte:38-40`, `:51`; `src/routes/(session)/+error.svelte:16` | **Four designed behaviours have no test.** Each planted bug left the suite green (436/436): (a) the tree no longer fetching folders it lacks, which is how it opens down to the item after you come Back into the frame; (b) no focus to the pane's heading after opening from the pane or the path bar; (c) the pane never showing busy; (d) the signed-in error page offering "Back to My Den" on the everyday stack. My browser run shows (a) and (b) work today. Finch's new layout spec in the restack covers the drawer and the skip link, not (a). | Harness results for "missing ancestors never fetched", "no focus to the pane heading", "pane never shows busy" and "error page offers My Den on the everyday stack". All reverted; every planted file compared byte-for-byte with ca936fa6. | (a) A layout spec: an in-app navigation into a deep page with empty memory fetches each missing crumb. (b) A den-page spec: after a non-tree navigation, focus is on the `h1`. (c) A fake-timer test for the delay. (d) An error-page test with `denServed` false. |
| 7 | should-fix | `src/routes/(session)/den/[...path]/+page.server.ts:16-20`; `src/lib/server/api/zurfur-api.ts:392`, `:408` | **On the everyday stack, `/den` still answers.** The page has a path bar, the heading "My Den" and "My Den isn't connected to the backend yet." The release's answer to its first decision says the everyday stack has "no route, no 'My Den' link, and a frame with no sidebar or tree", and its success criterion 7 is checked by "no `/den` route". Step 1b hides the links but leaves the route. The design accepts the notice on the feature branch until step 3. But after step 3, the everyday stack still serves no Den, and no step or work package says what `/den` does there. | Stub API, no mock, signed-in `GET /den` and `/den/commissions`: 200 with that notice (screenshot at 320 px). The den page spec asserts the notice ("answers not connected yet"). | Make the den load answer the in-frame not-found (or a 404) whenever `denServed()` is false, test it as success criterion 7's check, and name an owner before the branch reaches `main`. *Shouldn't the tech lead say which step owns it?* |
| N1 | note | `src/lib/api/den-route.ts:4-9`; `src/lib/server/den/den-path.ts:76-81` | **Challenge: Den links on a fixed `/den`, not `resolve()`.** I'd keep it. In SvelteKit 2.70.1, the server's `resolve()` returns a path relative to the request (`./…`, `../…`) whenever a request is in flight, loads included. For a data request it also counts the `__data.json` segment, so the path builder's absolute parse check can't use it. The path review asked for absolute hrefs. That also fixed a latent step-1 bug: the account detail's path link used `resolve()` in a load. The fixed path assumes the app sets no base path, and nothing checks that. | `@sveltejs/kit/src/runtime/app/paths/server.js` (`resolve`: `prefix + resolved` when `relative` and a request store exist); `src/runtime/server/page/render.js:121-131`. Path review, "Small additions": "Hrefs are absolute and encoded." | Add a test that fails if `paths.base` is ever set, or prefix the builder with the configured base. |
| N2 | note | `src/lib/components/explorer/ViewOptions.svelte:40-47` | **Challenge: the switch as a plain link before the script runs.** I'd keep it. The design said "a small form with a submit button", but a GET form makes the browser assemble the Den address from its fields, which breaks "the browser never puts a Den address together itself". The link carries the server-built address and is honest navigation. | `explorer-ssr.spec.ts` (link before the script); browser run (link `href="/den/commissions?includeDeleted=true"`). | Update the design study's section on the switch to say "a link". |
| N3 | note | `src/lib/components/explorer/tree-memory.svelte.ts:95`; `routes/(session)/NODE.json` (the frame's role) | **Challenge: the tree starting over after any `goto`.** Today it does the right thing for both callers: the switch (which also clears through its own flag check) and superforms' redirect after an Accounts action. But it ties the clear to a mechanism, not a cause, so any future `goto` (a stale-token reload, a dev control) silently empties the tree. The node's role says "cleared after a form action or a flag flip", which is no longer what the code does. Two related edges: a folder fetch already in flight when the tree clears still writes its listing back afterwards (`tree-loader.ts:52-69`); and a tree "More…" whose token went stale gets the den load's reload redirect, which `fetchListing` follows as a full page load into that folder (`tree-loader.ts:38-41`). | Code reading; planted "no reset after an action redirect" was caught (`tree-memory.svelte.spec.ts:112`). | Clear on the cause (the action's result) rather than on `goto`, revisiting it together with the known "keep `~` open" change; fix the node's role. Ignore a fetch that finishes after a clear. |
| N4 | note | `src/routes/(session)/den/[...path]/+page.svelte:21`; `src/lib/styles/tokens.css:106` | **Challenge: the pane's skeleton after 300 ms.** I'd keep it: the design asks for "a short delay", and keeping the old pane until then avoids a flash. But the delay lives twice: `--delay-busy: 300ms` in the tokens (which no code reads) and `BUSY_DELAY_MS = 300` in the page. It's also untested (row 6c). | Grep: the token's only use is its definition. | Keep one source and add the timer test. |
| N5 | note | `src/lib/server/error-log.ts` | **Challenge: `handleError` logging the route and the status.** It holds: the release asks that only route templates appear in logs, and a status identifies nothing. The 404 noise is the step-1 note, already in the fix round. | Step-1 QA, note on 404s logged as unexpected errors. | None here. |
| N6 | note | `src/lib/components/explorer/DenPane.svelte:67`; `EntryList.svelte:26`; `tree-model.ts:142` | **Folders first, applied over several pages, reorders rows you already see.** The server pages by name, and the web puts folders first across everything loaded. A folder arriving on page 2 jumps above page 1's files when you press "More". The mock's paged folder holds only files, so nobody has seen it yet. | Reading; the release's "folders first, then files, each group in the server's order" and the den.proto draft's "clients keep it within each kind". | A design question for the lead and the den lead: group per page, or have the server send folders first. |
| N7 | note | `src/lib/components/explorer/DenPane.svelte:69` | A later page with no entries (a token past the end; a no-script "More" after items vanished) says "This folder is empty." about a folder that isn't. | Mock: `…/attachments?pageToken=p1.999` shows "This folder is empty." for 40 files. | On a later page, say "No more items here" and link to the first page. |
| N8 | note | `src/lib/components/ui/TreeItem.svelte:43-44`, `:71-73`; `src/lib/components/explorer/EntryList.svelte:51-52`; `NodeHeader.svelte:60` | **Cards.** The release says a card "shows only its name and type", but the type isn't shown anywhere (the marker is a lock). In the pane's list, a screen reader hears both the hidden "Can't be opened" and the visible "Can't open". | Browser text of `/den/commissions`: "Some commission Mounted Can't be opened Can't open". | Ask the lead whether "only" allows leaving the type out. Drop one of the two phrases. |
| N9 | note | `src/lib/components/explorer/TopBar.svelte` (1b, no drawer); `Avatar.svelte` | On the everyday stack at 320 px, the handle shrinks to "a…" and "Sign out" wraps onto two lines. It works, but you can't tell who is signed in. | Screenshot at 320 px with a 61-character handle. | Let the avatar stand alone on a phone, or give the handle its own line. |
| N10 | note | `EntryList.svelte:93`; `TreeBranch.svelte:266-267`; `Toggle.svelte:49-62`; `Skeleton.svelte:22`; `ViewOptions.svelte:63` | New hard-coded sizes, the same pattern as step 1's "every value from tokens.css" finding. | Grep of the new styles. | Fold them into that fix. |
| N11 | note | `eslint.config.js:65` | A new JSDoc comment carries a design-decision number ("Effect is server-only (DD 39944194)"). The repo's rule keeps those pointers in `NODE.json`. The lint messages already quoted the number before this change. | CLAUDE.md, "The node tree". | Drop the number from the JSDoc. |

## Each criterion and its test

"Planted" means I broke the code on purpose, ran the whole suite, then reverted. "Caught" means the suite went red. Known deltas in Finch's fix round aren't counted: the unknown soft-delete word becomes "Inactive", and `~` stays open after a form action.

| Criterion | Test that proves it | Planted bug and result |
| --- | --- | --- |
| Everyday stack: no sidebar, tree, drawer or "My Den" link (1b) | `AppShell.svelte.spec.ts`; `den-served.spec.ts`; `layout.server.spec.ts`; `SessionHeader.svelte.spec.ts`; landing spec | Den served everywhere, sidebar regardless, top-bar link regardless, public-header link regardless: all caught. Error page's way back on the everyday stack: **not caught** (row 6d). `/den` itself still answers (row 7) |
| "My Den" opens `/den`; the pane reads "My Den"; the root shows as `~` | `DenPane.svelte.spec.ts`; `Tree.svelte.spec.ts`; `den.spec.ts` | Holds in the browser run |
| The root lists four folders; `posts` mounted and "Content not shown yet" | `den-mock.spec.ts`; `DenPane.svelte.spec.ts` | Holds |
| Signed-out `/den` goes to sign in | `den.spec.ts` ("sends an ended session to sign in"); the session gate | Holds |
| Each entry: name, kind, own visibility for openers; a mount shows its target's kind and level plus the mark | `DenPane.svelte.spec.ts`; `den.spec.ts` | Unknown level shown as Public: caught |
| Folders first, then files, in the server's order | `tree-model.spec.ts`; `Tree.svelte.spec.ts`; `DenPane.svelte.spec.ts` | Folders-first removed: caught. Across pages, see note N6 |
| Empty folder versus "content not shown yet" | `DenPane.svelte.spec.ts`; `tree-model.spec.ts` | Holds |
| "More" loads the next page; no count anywhere | `DenPane.svelte.spec.ts`; `Tree.svelte.spec.ts`; `den-mock.spec.ts` | Holds, apart from focus after the last page (row 1) |
| A commission's four entries; slots open empty; files and the changelog show "Content not shown yet" | `den-mock.spec.ts`; `DenPane.svelte.spec.ts` | Holds |
| Every miss and a malformed address show the same "Nothing here" | `den.spec.ts` (refused addresses); `den-path.spec.ts`; `den-mock.spec.ts` | Dot segments allowed, parse check dropped, decoded twice: all caught. The tree differs on a malformed address (row 2) |
| A card: name only, no visibility, no link | `den.spec.ts`; `DenPane.svelte.spec.ts`; `Tree.svelte.spec.ts` | Card treated as open: caught. "Can't be opened" removed: caught |
| Only allowlisted parameters; the flag only in the address | `den-path.spec.ts`; `ViewOptions.svelte.spec.ts` | Repeated flag accepted: caught |
| "Show archived and deactivated": the words; the typed path opens it, marked, either way | `den-mock.spec.ts`; `DenPane.svelte.spec.ts` | Deleted always listed, typed path refused, "Deactivated" read as "Archived": all caught |
| A stale page token reloads from page one, once | `den.spec.ts`; `page.server.spec.ts`; `den-mock.spec.ts` | Stale token shown as not-found: caught. Browser: `p1.abc` gives a 303 to the first page |
| Tree keys (Tab once, arrows, Home/End, letters, Enter/Space) | `Tree.svelte.spec.ts` | Every row a Tab stop, typeahead off: caught. Browser run: Tab reaches the tree once, after "Sign out" |
| Focus stays on the tree item after opening from the tree | `Tree.svelte.spec.ts` (keepfocus) | keepfocus removed: caught. Browser: focus on "Untitled" after Enter |
| A click on a name opens it; only the arrow expands | `Tree.svelte.spec.ts` | Holds |
| Before the script, plain links | `explorer-ssr.spec.ts` | Switch rendered before the script: caught. The tree's "More…" isn't a link before the script; the pane's is |
| Partly loaded folder: level, position, set size -1 on every item | `tree-model.spec.ts`; `Tree.svelte.spec.ts` | Positions removed: caught. "More…" row and the stray open item lack them (row 4) |
| Title "<name> — My Den — Zurfur" | `den.spec.ts` | Name dropped from the title: caught |
| The tree opens down to the open item, highlighted, item closed | `tree-memory.svelte.spec.ts`; `tree-model.spec.ts` | Holds. Highlight is colour-only (row 3) |
| Back into a deep page from outside the frame fills the tree | `tree-memory.svelte.spec.ts` (names the missing crumbs) | Layout no longer fetching them: **not caught** (row 6a). Works in the browser |
| Opening from the pane or the path bar focuses the pane's heading | None | **Not caught** (row 6b). Works in the browser |
| The pane shows busy after a short delay | `DenPane.svelte.spec.ts` (the busy view only) | Never busy: **not caught** (row 6c) |
| Tree memory: per visitor, cleared on a flag flip and after an action | `tree-memory.svelte.spec.ts` | Owner never changes, flag flip keeps old listings, no reset after an action redirect: all caught |
| Founding an Account shows its mount | `den-mock.spec.ts` | Browser: "QA Studio" listed in `accounts` after founding |
| Account delete goes through the path builder | `zurfur-api.spec.ts` | Builder skipped: caught |
| Phone: drawer, choose closes it, no sideways scroll | step 1's specs; Finch's new layout spec in the restack | Browser at 320 px: no overflow (long name too); choosing "posts" closed the drawer, focus on ☰ |

## Also checked, and holds

- **The gate, as CI runs it**, at ca936fa6: `cargo fmt`, `clippy -D warnings`, `cargo test` (95 suites, none failing), `cargo deny`, `typos`, the design-index check, `nodes check` (0 errors, the same 2 old warnings), the hooks tests, `buf lint`, `svelte-check` (0 errors, 0 warnings), lint (0 errors; 6 warnings in generated files, as on `main`), web tests 436/436, build. Judged by exit code and output.
- **Rulebook, new production code:** no `null`, no `throw`, no assertion outside `src/lib/types/**` except `as const`, no enum. Every switch over a union is exhaustive. Data shapes are `readonly` unions, and Den strings are branded.
- **No component reads the Den.** Only the den page's load calls `denRoot` and `denNode`, and every link in its answer is built by the path builder.
- **`NODE.json`:** the signed-in group now cites The Den and My Den rulings under `refs`, and doc comments carry no pointers (apart from note N11).
- **Accessibility basics:** a navigation region named "My Den" holds the tree; Tab order is skip link, header, tree (one stop), switch, pane; visibility words are 9.3 to 10.7:1; the arrow, skeleton and switch animations all follow reduced motion; the browser console showed no errors or warnings during the run.
- **Finch's coverage gap, the drawer's own focus handling:** agreed, as in the step-1 note. Chromium restores focus to ☰ by itself, so a spec in Chromium alone can't catch a regression. In my run, focus did return to ☰ after choosing an item in the drawer.

## Choices to ask about

1. **Which step owns `/den` on the everyday stack** (row 7): the tech lead.
2. **Folders first across pages** (note N6): the lead and the den lead.
3. **Whether a card must show its type** (note N8): the lead.
4. **What clears the tree** (note N3): the lead, together with the known "keep `~` open" change.

## Verification (Crow, 2026-10-08)

Re-checked at the fixed tips: 1b at 99c6072b, step 2 at 3aa6a1e4. For each finding I re-ran its original proof: the planted bug, the browser run on the mock, or the stub-API run on the everyday stack. All fixed tips are free of conflicts.

**The gate.**
- At 3aa6a1e4: the full `just gate` exits 0, with 531/531 web tests.
- At 99c6072b: the web job exits 0, with 280/280 tests.
- No port clash. Two browser-test runs at once now both pass.

| # | Finding | Status | Proof |
| --- | --- | --- | --- |
| 1 | Focus falls to the body after the last "More" | **Closed** | Browser run, "Big ref batch" › attachments: after the last "More", focus is on "ref-26.png" in the pane, and on the "ref-26.png" tree item in the tree. Planted (focus move removed, pane and tree): both went red. |
| 2 | A malformed address leaves `~` loading forever | **Closed** | Browser run, full load of `/den/a//b`: no busy group, and `~` lists its 4 folders. Planted (root fetch removed): 3 tests went red. |
| 3 | The open tree item marked by colour alone | **Closed** | A 4 px accent bar now marks it (`rgb(172,157,255) 4px 0 0 0 inset`), on top of the tint. Planted (bar removed): a test went red. |
| 4 | "More…" and the stray open item without level or position | **Closed** | Browser run: "More…" has level 5, position 26 and set size -1; the stray "ref-38.png" has 5, 27 and -1. Planted (both positions removed): red. |
| 5 | A relative import gets past the DenHref ban | **Partly closed: still open** | A relative path, a namespace import and a dynamic `import()` are now caught. **But `import { denHref } from '$lib/types/brand.js'` still passes lint, passes `svelte-check` and runs.** I planted it in `lib/server/den.ts`, lint exited 0, and a probe spec read the minted value back; reverted. The `.ts` spelling passes lint too. Fix: let the pattern cover any extension (`**/types/brand*` and `$lib/types/brand*`), and add the `.js` case to a lint fixture. |
| 6 | Four behaviours with no test | **Closed** | Planted again, all red: the layout not fetching folders it lacks, no focus to the pane heading, the pane never busy, and the error page offering My Den on the everyday stack. |
| 7 | A typed `/den` answers on the everyday stack | **Closed** | Stub API, no mock: signed-in `GET /den` and `/den/commissions` return 404 inside the frame ("Nothing here", "Back to the start"). The load reads no Den; only the gate's whoami reaches the backend. Planted (`denServed` check removed): red. |
| N1 | Fixed `/den`, no guard on the base path | **Closed** | New `den-route.spec.ts` fails if a base path is ever set. |
| N2 | The study still describes the switch as a form | Open, doc only | §6 of the shell study still says "a small form with a submit button". |
| N3 | Clearing the tree after any `goto` | **Closed** | The tree now restarts on a form action's result, keeping `~` open. A late fetch can't write back after a clear, and a redirect back into the Den isn't followed. Planted, both red: late fetches written back, the Den redirect followed. The node's role matches. **New small gap:** no test covers the Accounts page calling the restart (planted, removed: suite green, 531/531). The browser run shows it works: after founding "Verify Studio", `~` stays open and `accounts` lists it. One assertion in the Accounts page spec would close it. |
| N4 | The busy delay in two places | **Closed** | The token is gone; the delay lives only in the den page, and a timer test covers it. |
| N5 | `handleError` logging route and status | Holds | As the step-1 file says. |
| N6 | Folders first across pages | Skipped by Finch as a design call | Not visible in the click-through: the only paged folder (mock and seed) holds files only. Worth raising in the den.proto review, where the ordering promise ("clients keep it within each kind") is written. No question for the Engineer now. |
| N7 | An empty later page says "This folder is empty." | Skipped by Finch as a design call | Reachable only by a hand-edited token, or a no-script "More" after items vanish. Not in the click-through. No question. |
| N8 | Card: type not shown; double announcement | Double announcement **closed** (the pane's card row no longer repeats "Can't be opened"). Type: open, a question for the lead | |
| N9 | A long handle at 320 px | Skipped by Finch as a design call. **It shows in the click-through**, so it's Q1 below | |
| N10 | New raw sizes | **Closed** | The tree, the switch, the skeleton and the entry list read tokens. |
| N11 | A design-decision number in a JSDoc | **Closed** | The comment now reads "Effect is server-only, below the runes seam." |

**Also checked in the fix round:**
- **Den pages now use a generic title**, "My Den · Zurfur", and a polite live region names the opened item ("Untitled" in the browser run). The release's screen-reader story allows the security review to replace the title, so this holds. Planted (no announcement): red.
- **The unknown soft-delete word is now "Set aside".** That matches Claude's revised answer in the shell study and den.proto's generic fallback; it supersedes the "Inactive" known delta.
- **Founding an Account** keeps `~` open and shows the new mount, as Claude's answer to the "after a form action" question (Q5) says. The browser console stayed clean throughout.

### Questions (each with my recommended answer)

**Q1. On a phone, the everyday stack's top bar cuts your handle down to almost nothing.** With no drawer, the brand, "Accounts", the avatar with handle and "Sign out" share one row. At 320 px, `zuri.bsky.social` shows as "z…", at 375 px as "zuri.bsk…", and "Sign out" wraps onto two lines (screenshots taken at both widths). You'll see this if you sign in on a phone.
- **Recommendation:** below 48rem, show the avatar alone, with the handle as its accessible name and tooltip, and keep "Sign out" on one line. The full handle stays on desktop and in the drawer. The alternative is to drop the "zurfur" brand on phones to make room.
- **Who:** the Engineer, through the lead; it's a look-and-feel call.

**Q2. Should sign-out say so when it can't reach the backend?** Today, with the backend unreachable, "Sign out" lands on "Sign in" and leaves the session cookie in place. The server-side session was never ended, so the same browser is signed in again once the backend returns. On a shared computer that's the case the step-1 must-fix was about. It isn't new (the old step 1 did the same), and the design flags "a stopped backend looks like being signed out" as a separate call.
- **Recommendation:** yes. Delete the browser's session cookie on every sign-out attempt, and on failure show "You're still signed in on the server. Try again."
- **Who:** the tech lead and Heron first, since it's session handling; then the Engineer.

### Still open

- **Should-fix (2):**
  - The `.js`-suffixed import that gets past the DenHref ban (row 5).
  - The missing assertion for the Accounts page's tree restart (row N3).
- **Notes and questions:** the card's type (N8), the switch's wording in the study (N2), and Q1 and Q2 above.

**Verdict: ready for the Engineer.** No must-fix. The two open should-fixes are a lint pattern and one test assertion. Neither changes anything you'll see in the click-through, and both can land in the next round.

## Slice 2c (Crow, 2026-10-08)

Slice 2c "Review follow-ups" at 58282348 (change omlpyrms), on 2b (eb112078) and step 2 (7970eae4). Checked in the crow workspace; never pushed. I abandoned my old divergent probe change, as asked.

**The gate.**
- At 58282348: the full `just gate` exits 0. Its output is clean: 95 Rust suites, none failing; `cargo deny`, typos, the design index and `nodes check` all pass (0 errors, the same 2 old warnings); `svelte-check` has 0 errors and 0 warnings; lint has 0 errors (the 6 generated-file warnings, as on `main`); web tests pass 553/553; the build passes.
- The web job also exits 0 at 7970eae4 (540/540) and at eb112078 (548/548), so every commit in the chain is green.

| Item | Status | Proof |
| --- | --- | --- |
| The brand-module lint ban covers any extension, relative path and dynamic `import()` | **Closed** | Planted in `lib/server/den.ts`; eslint exits 1 for each: `$lib/types/brand.js`, `../types/brand.ts`, `../types/brand`, `import * as b from '../types/brand.js'`, `import('$lib/types/brand.js')`, `import('../types/brand')`, and an absolute path. All reverted. |
| A spec proves the Accounts page restarts the tree after founding an Account | **Closed** | New `accounts/tree-restart.svelte.spec.ts`. Planted, both red: the restart removed; the restart run on a failure too. |
| Q10: below 48rem the top bar shows the avatar alone, the handle its accessible name, "Sign out" on one line | **Closed** | Two new layout specs. Planted, all red: the handle shown on a phone, "Sign out" allowed to wrap, the tooltip removed, the top bar not compact, the handle hidden on desktop too. Browser run on the everyday stack with `zuri.bsky.social`: at 320 px and 375 px the row shows "zurfur · Accounts · avatar · Sign out" on one line, with no sideways scroll; the handle's text stays in the DOM, clipped, for screen readers. |
| Copilot's "More" race on #247 (step 2) | **Closed** | `page.svelte.spec.ts` holds the response, moves to another folder, then releases it. Planted, both red: the page-generation check removed; navigation no longer bumping the generation. |
| Also in 2c: a data request posted to `/logout` is gated again | **Closed** | Planted (`!event.isDataRequest` removed): `session-gate.spec.ts` went red. |
| Also in step 2: `isDenLink` parses a link back before trusting it | **Closed** | Planted (parse check dropped): 6 tests in `den.spec.ts` went red. |

**Notes (not blocking):**
- **Nothing in the gate pins the widened lint ban.** I put the old pattern back (no extensions) and `yarn lint` still exits 0, because no code uses the bypass. A spec that runs ESLint over the seven spellings above (ESLint's `RuleTester`, or `ESLint#lintText`) would keep it from quietly regressing.
- **The pane's second "More" guard has no test of its own.** That guard, "append to the tree only if its memory wasn't cleared", left the suite green when I planted it out (553/553). Within a den page, every way the memory clears is also a navigation, so the page-generation check above already covers it. It's belt and braces; a test is optional.
- **A computed `import(variable)` can't be checked by lint at all**, a static-analysis limit. The no-assertion rule and review stay the guard there.

**Verdict: ready for the Engineer.** No must-fix. Every 2c item and the "More" race fix is proven by a test that goes red when the code is broken, and the gate is green at every commit in the chain.

## Copilot round 2 (Crow, 2026-10-08)

Rows 6 and 7 of the triage's "Round 2" (`.understand/20261008-explorer-shell-copilot-triage-245-248.md`), fixed in step 2 (f263e144). Checked at the 2c tip, f6f15cca. Rows 5, 8 and 9 are Heron's.

**The gate.** The full `just gate` at f6f15cca exits 0, and its output is clean:
- 95 Rust suites, none failing;
- `cargo deny`, typos, the design index and `nodes check` pass (0 errors, the same 2 old warnings);
- `svelte-check` reports 0 errors and 0 warnings;
- lint reports 0 errors (the 6 generated-file warnings, as on `main`);
- the web tests pass 558/558, and the build passes.

| Row | Status | Proof |
| --- | --- | --- |
| 6. Before the script runs, "More…" is a plain link to the next page | **Closed** | `explorer-ssr.spec.ts` ("is a plain link to the next page"). Planted, both red: the old `<span>` back; the link pointing at its folder instead of the next page. On the mock, the server-rendered tree for a file beyond the first page of "Big ref batch" › attachments holds `<a … href="…/attachments?pageToken=p1.25" data-tree-kind="more">More…`, and that link's page answers with files 26 to 40. |
| 7. A card page is no longer listed twice after "More…" | **Closed** | `tree-model.spec.ts` ("never adds a card page as a stray row"). Planted, both red: cards placed as strays again; open items no longer placed. The mock can't show the double listing (its card sits in a one-page folder), so the unit test is the proof. An open item beyond its folder's loaded page is still placed once: the tree lists "ref-38.png" once, after "More…". |

**Nothing else in the click-through regressed.** I re-ran all my earlier browser scripts on the mock, and the everyday-stack checks:
- **Tab and keys.** Tab order is the skip link, the header, the tree (one stop), the switch, then the pane. The tree keys and typeahead work, and Enter keeps focus on the tree item.
- **Focus.** Opening from the pane or the path bar focuses the heading. After the last "More", focus lands on the first new row, in the pane and in the tree.
- **The switch.** It keeps focus and lists "Winter YCH" as Archived.
- **Edge pages.** A malformed address lists `~`'s four folders. Back from the landing page reopens the tree down to the item.
- **Founding an Account.** It keeps `~` open and shows the new Account.
- **Phone, 320 px.** No sideways scroll; choosing an item in the drawer closes it, and focus returns to ☰.
- **Titles.** The title stays "My Den · Zurfur", and the live region says "Untitled".
- **The browser console** stayed clean throughout.
- **The everyday stack.**
  - `/den` is a 404, and the Accounts page renders in the frame.
  - When the backend ends the session, sign-out clears `zurfur.sid` and lands on `/`.
  - When the backend refuses, sign-out still clears the cookie and lands on `/login?signout=unconfirmed`, which reads "You're signed out on this device. Zurfur couldn't confirm it, so your session on the server ends when it expires."

**Verdict: ready for the Engineer.** No must-fix.
