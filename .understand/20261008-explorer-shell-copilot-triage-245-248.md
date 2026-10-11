# Copilot triage: Explorer shell PRs #245 to #248 (2026-10-08)

**Short:** 4 comments, all on #247 and #248. #245 and #246 have none: Copilot recommends approval on #246, and asks for a closer human look at #245 because it touches authentication; QA and security have both passed it. Three comments are real and get fixed in the slice that introduced them, each proved first by a test that fails. One is a wording fix: the behaviour is right, but the claim overstated it.

| # | PR | Where | Comment, in short | Tech lead's triage | Action |
| --- | --- | --- | --- | --- | --- |
| 1 | #247 | `lib/api/den.ts:125`, `isDenLink` | It accepts `/den/../accounts` and `/den/%2e%2e/accounts`, which URL resolution turns into `/accounts`; `isDenPageData` then trusts preload data with such links | **Real** (security-adjacent: a Den link escaping the Den) | Require the parsed path and query to round-trip unchanged before branding. Test with both strings. Heron verifies |
| 2 | #247 | `den/[...path]/+page.server.ts:20` | The "ordinary 404 with no backend call" runs after the session gate, which calls `/me` and redirects anonymous visitors to `/login` | **Wording.** What matters is that the everyday stack never calls the Den backend, and that still holds. A signed-in visitor makes the one `/me` call every signed-in page makes, then gets the 404; an anonymous visitor is redirected like every other signed-in page. Moving `/den` out of the session group would make it the one signed-in page that behaves differently | Reword the claim ("no Den call") in the code comment, the test name and the PR text. Add a request-level test through the hook: signed-in → 404, with no Den port call; anonymous → `/login` |
| 3 | #247 | `den/[...path]/+page.svelte:91` | An in-flight "More" isn't tied to its page: after a navigation or a toggle flip, the old folder's entries are appended to the new pane and tree memory | **Real** (a correctness bug) | Drop the result when the page identity changes, as the tree loader already does with its generation counter. Test it with a deferred response |
| 4 | #248 | `(public)/login/+page.server.ts:29` | `?signout=unconfirmed` is trusted from the address. During an outage the layout maps a network failure to "no session", so a visitor whose `zurfur.sid` is still live can be told the cookie was cleared | **Real** (low: it needs a crafted link during an outage, but the false claim is exactly the shared-computer harm 2b exists to prevent) | Show the notice only when the request carries no `zurfur.sid` (Copilot's second option; no new server state). Test with the cookie present and with it absent. Heron verifies |

Replies are posted on each comment, prefixed "Claude:", once the fixes are pushed.

## Round 2: Copilot's re-review of #247 and #248, and its review of #249 (2026-10-08)

The four round-1 fixes stand; Copilot re-posted no new comment on them. Five new comments. All are real and get the same treatment: a failing test first, then the fix, in the slice that introduced it.

| # | PR | Where | Comment, in short | Tech lead's triage | Action |
| --- | --- | --- | --- | --- | --- |
| 5 | #247 | `lib/server/api/den-read.ts:29`, `den.ts` | An unknown access value falls through to the open-node path and gets a link: it fails open, not closed as a card | **Real, must-fix** (the tolerant-reader rule says an unknown access shows as a card) | Normalise every access value except exactly `open` to a card. Test with an unknown value. Heron verifies |
| 6 | #247 | `TreeBranch.svelte:147` | Before the script runs, a "More" node renders as a `<span>`, so a no-JavaScript visitor can't reach the next page | **Real** (the plain-links fallback is a stated criterion) | Render `node.href` as a link until the tree is enhanced. Test the server render |
| 7 | #247 | `tree-model.ts:167` | A card page is never matched in its loaded listing, so after "More…" the same card is appended twice | **Real** (only the mock's card page can hit it in 0.1) | Match cards by a stable internal identity, or don't append card entries as strays. Test it |
| 8 | #248 | `(session)/logout/+page.server.ts:30` | The backend sign-out call has no timeout; if the backend accepts the connection and never answers, the action never returns and `zurfur.sid` survives | **Real, must-fix** (it breaks 2b's "always clears" guarantee) | Bound the call and map a timeout or abort to the unconfirmed outcome. Test with a backend that never answers. Heron verifies |
| 9 | #249 | `lib/server/session-gate.ts:20` | The exception still lets any non-data POST to `/logout` through, including a named action (`?/other`); actions run before layout loads, so the layout can't backstop it, and a named action added later would run anonymously | **Real, must-fix** | Let only the default action through: reject SvelteKit's `?/name` form. Test a named action anonymously. Heron verifies |

## Round 3: Copilot's third pass (2026-10-08)

#248 and #249: "Approval recommended", no new comment. #247: one new comment; the round-2 fixes stand.

| # | PR | Where | Comment, in short | Tech lead's triage | Action |
| --- | --- | --- | --- | --- | --- |
| 10 | #247 | `den/[...path]/+page.svelte:91` | The pane pages a folder on its own state while the tree pages the same folder in the shared tree memory; using the tree's "More…" and then the pane's stale "More" fetches the same page twice, and both append, so the tree shows rows twice | **Real** (a correctness bug in the click-through) | One pagination state per folder, shared by the pane and the tree, or appends that apply only when the expected next-page link still matches. Test both orders |
