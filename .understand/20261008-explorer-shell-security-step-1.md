# Explorer shell, step 1 "The frame": security review

2026-10-08 · Heron (security reviewer) for the tech lead (`claude`) · commit 9dba2fcf (jj change tmmtumrs, bookmark `feature/web-explorer-shell-1-frame`), one commit on `main` 9a01808a · QA is Crow's, separately.

## In one screen

**How I checked.** I read every changed file. Then I built the app for production and ran it against a small stand-in backend that I could make misbehave on purpose. I also ran a type probe and a lint probe, and read the installed SvelteKit 2.70.1 source wherever its behaviour mattered. The probes ran in an empty jj change on top of the commit (xnkqxnzl), and nothing was kept.

**The builder's three security claims all hold.**
1. **Error views show the status only.** Both error pages pass nothing but the status to the error view. An unexpected error that carried a secret path rendered "Something went wrong", with no trace of the secret in the page or its data.
2. **The error hook logs the route only.** It logs the route template and the status. Across six probe errors, the server log held no path, id, token or error text.
3. **Signed-in responses carry both headers.** Every signed-in response I tried left with `Cache-Control: private, no-store` and `Referrer-Policy: same-origin`: pages, page data, redirects, 404s, error pages and a failed form action. Anonymous responses carry neither.

**What else I attacked:**
- **The route-group move.** No signed-in page moved. Only the landing page and `/login` went into the new public group, and every signed-in page is still under the sign-in gate. But the gate doesn't protect page loads either. An anonymous visitor's page load still runs, and one crafted request skips the gate entirely. Nothing leaks today, because the backend checks the session on every call. The gate's docs say otherwise, and step 2 must not lean on them (first should-fix).
- **The signed-in frame.** It holds nothing that could cross between users: no module-level state, no browser storage and no raw HTML. Nothing is keyed by the session's DID yet, because the tree that needs that arrives in step 2.
- **The mock sign-in.** It can't be reached outside dev. A production build answers 404, and a production server started with the mock flag refuses to boot.
- **Fonts and icons.** Both are self-hosted, and the build makes no third-party request for them. The one off-site request in the frame is the signed-in user's own avatar, loaded from their own data server. It was already there before this commit.
- **Names.** Names in the path bar are wrapped in `<bdi>`, and handles and DIDs are ASCII. The Accounts pages, which now sit inside the frame, print an Account's name without `<bdi>`. That was there before this commit.
- **The path builder.** It's absent, as the plan places it in step 2. Two of the findings below are things step 2 needs before it builds Den links.

**Counts:** 0 must-fix · 5 should-fix · 6 notes · 2 questions for the Engineer.

**Verdict: clear for a PR.** Three should-fixes are small enough to ride this slice: the gate's docs, the sign-out failure copy, and `<bdi>` on account names. The other two should land before step 2's Den links: the lint rule on error messages, and the brand on the frame's links.

## Findings

| # | Severity | Finding | Where | When |
| --- | --- | --- | --- | --- |
| 1 | should-fix (low) | The sign-in gate's docs say it guards page loads; it doesn't | `routes/(session)/+layout.server.ts:10`, `routes/(session)/NODE.json:12` | This slice |
| 2 | should-fix (low) | A failed sign-out now reads as a generic "reading" error | `routes/(session)/logout/+page.server.ts:26`, `lib/components/explorer/ErrorView.svelte:25-31` | This slice |
| 3 | should-fix (low) | An expected error's message still reaches the browser, just not the screen | `routes/(session)/logout/+page.server.ts:26` today; any future `error(status, message)` | Before step 2's Den page |
| 4 | should-fix (low) | The frame's link type accepts off-site links (`//host/`) | `lib/api/trail.ts:11-13` | Before step 2's Den links |
| 5 | should-fix (low, older than this commit) | Account names render without `<bdi>` inside the frame | `routes/(session)/accounts/+page.svelte:55`, `routes/(session)/accounts/[id]/+page.svelte:24` | This slice |

Paths are under `frontend/web/src/`.

### 1. The sign-in gate's docs say it guards page loads; it doesn't

- **Severity:** should-fix (low). This isn't exploitable today; it's a false statement in the file step 2 will read.
- **Where:**
  - `routes/(session)/+layout.server.ts:10`, which says "this gates page LOADS".
  - `routes/(session)/NODE.json:12`, which says "The gate is UX ONLY and covers page LOADS". This commit edited both files.
- **The exploit:**
  - **A normal request.** The page's own load starts at the same time as the gate. An anonymous `GET /accounts` calls the backend's account listing with no session before the gate's redirect wins.
  - **A crafted data request.** The client chooses which loads run. An anonymous `GET /accounts/__data.json?x-sveltekit-invalidated=001` skips both layout loads, the gate included, and returns the page load's data.
  - **Why nothing leaks.** Each load in `(session)` today reads only through the backend with the caller's own cookie, and axum refuses a call without a session: `CallingUser` (`backend/crates/api/src/extract.rs:8-24`) on every account route (`routes/accounts.rs:177-179`).
  - **The risk is later.** A future load could read server-side state that isn't keyed to the cookie, such as a cache, a process-wide store, or the mock's single shared store. It would serve that state to anyone, behind a gate the docs call a guard.
- **Proof** (production build, stand-in backend that answers the account listing *without* checking the cookie, probes P1 to P3 in the appendix):
  - Anonymous `GET /accounts` → `303 /login`. The stand-in still logged `GET /accounts` with no cookie.
  - Anonymous `GET /accounts/__data.json?x-sveltekit-invalidated=001` → `200` with `{"type":"skip"},{"type":"skip"},{"type":"data",… "UNGUARDED-DATA" …}`.
- **Sources:**
  - SvelteKit 2.70.1 source:
    - `src/runtime/server/respond.js:121-126` reads the client's `x-sveltekit-invalidated` list.
    - `src/runtime/server/data/index.js:89-97` answers `skip` for a node it doesn't invalidate.
    - `src/runtime/server/page/index.js:179-221` starts every node's load at once.
  - svelte.dev, Loading data, "Implications for authentication":
    - "Layout and page `load` functions run concurrently unless `await parent()` is called."
    - "If a layout `load` throws, the page `load` function runs, but the client will not receive the returned data."
    - A layout guard "requires all child pages to call `await parent()` before protected code".
- **The fix:**
  - Reword both places to: "UX only, for loads and actions alike: a page load runs alongside the gate, and a data request can skip it entirely. The backend's own session check is the only guard."
  - Add a step-2 rule to the node's conventions: "A `(session)` load reads only through the backend with the caller's cookie, or awaits `parent()` before anything else."

### 2. A failed sign-out now reads as a generic "reading" error

- **Severity:** should-fix (low). This is a security-relevant message that got lost.
- **Where:**
  - `routes/(session)/logout/+page.server.ts:26` throws `error(502, 'Sign-out did not complete. Try again.')`.
  - The new error view picks its copy by status alone (`ErrorView.svelte:25-31`).
- **The exploit:**
  - **Before this commit,** SvelteKit's default error page showed "502 / Sign-out did not complete. Try again." (`@sveltejs/kit` `src/runtime/components/svelte-5/error.svelte` renders `page.error.message`).
  - **Now** the page says "Something went wrong reading this. Try again in a moment." On a shared computer, the person can walk away believing they signed out. The frame still shows their handle and a Sign out button, which is the only hint.
  - **Nothing is logged.** An expected error bypasses `handleError` (finding 3), so a failing sign-out leaves no line in the server log.
- **Proof (P7):** a browser-style form post to `/logout`, with the stand-in failing sign-out, returned `502`. The page held `<h1>Something went wrong</h1>` and "Something went wrong reading this…", and the frame still showed `alice.test` and "Sign out".
- **The fix:**
  - Let the error view choose its copy from the status *and the route id*, through a closed map, still never reading the message. For example, a 502 on `/(session)/logout` reads "You're still signed in: sign-out didn't complete. Try again."
  - Wording is the Engineer's to veto.
  - Optionally, log the failed sign-out by route and status.

### 3. An expected error's message still reaches the browser, just not the screen

- **Severity:** should-fix (low). This is hardening for step 2.
- **Where:** any `error(status, message)`. Today the only one with a message is the sign-out failure above, a fixed string.
- **The exploit:**
  - In 2.70.1, `error()` "will cause SvelteKit to return an error response without invoking `handleError`".
  - Its message travels in the page's hydration script and in `__data.json` and action JSON. The error views don't show it, but anyone with the page source sees it.
  - If step 2 ever writes ``error(404, `no node at ${path}`)``, a Den path lands in the page source and in a proxy's view of the JSON.
- **Proof (P7):**
  - The sign-out failure's HTML holds `error: {message:"Sign-out did not complete. Try again."}`.
  - The JSON action response holds `{"type":"error","error":{"message":"Sign-out did not complete. Try again."}}`.
- **Sources:**
  - `@sveltejs/kit` 2.70.1, `src/exports/index.js:64-81`, the doc comment quoted above.
  - `src/runtime/server/utils.js:102-106`: an `HttpError` returns its body before `handleError` is called.
  - `src/runtime/server/page/render.js:480-498`: `error` is serialized into the page.
  - The live svelte.dev Errors page describes a newer SvelteKit, where app errors *do* pass through `handleError` with `kind: 'app'`. On that upgrade, this hook would replace every such message with "Internal Error" and log it as unexpected.
- **What doesn't work:** narrowing `App.Error`'s `message` to a fixed list. SvelteKit declares it `message: string`, and the conflicting merge is hidden by `skipLibCheck`. My probe compiled `error(404, `…${path}`)` with no error.
- **The fix:** a lint rule beside the repo's existing `no-restricted-syntax` bans.
  - Selector, verified on a probe file:
    `CallExpression[callee.name='error'][arguments.length>1]:not([arguments.1.type='Literal'])`
  - It flags a template, a variable or an object message, and allows a string literal or a bare `error(404)`.
  - It keys on the name `error`, so ban renaming that import too.

### 4. The frame's link type accepts off-site links

- **Severity:** should-fix (low). Every link the path bar shows today is a `resolve()` constant; step 2 starts feeding it server-built Den links.
- **Where:** `lib/api/trail.ts:11-13`: `href: ResolvedPathname | undefined`.
- **The exploit:**
  - SvelteKit generates that type as `` `${"" | `/${string}`}${Pathname}` ``, where the first part is the base path.
  - `//evil.example/` fits it, and a browser reads that as a link to another site.
  - Any code that hands the frame a trail can put an off-site link in the path bar.
- **Proof:**
  - `svelte-check` passed a probe assigning `'//evil.example/'` to a `TrailStep`'s `href` with 0 errors.
  - The control, `'https://evil.example/'`, failed as expected.
  - The generated type is in `.svelte-kit/non-ambient.d.ts:48`.
- **The fix:** type `TrailStep.href` with a brand that only two places can mint:
  - the path builder (the design's `DenHref`), for Den steps;
  - a `resolve()` wrapper, for the fixed section links.

### 5. Account names render without `<bdi>` inside the frame

- **Severity:** should-fix (low). This is older than this commit, but those pages now sit inside the frame, whose rule is `<bdi>` wherever a name shows.
- **Where:** `routes/(session)/accounts/+page.svelte:55` and `routes/(session)/accounts/[id]/+page.svelte:24` print `{account.name}` bare.
- **The exploit:**
  - An Account's name accepts any characters, including bidi controls (`backend/crates/domain/src/elements/account/value.rs:28-50`).
  - An Owner can name the Account so that a right-to-left override reverses the rest of the line, " as member" included, for every member who opens the page.
  - The validation side is already on the question sheet (section 5, item 9).
- **The fix:** `<bdi>{account.name}</bdi>` in both places.

## Notes

1. **Keep `same-origin`; never tighten it to `no-referrer`.**
   - Under `no-referrer`, a form post sends `Origin: null`. Fetch Standard, "append a request `Origin` header": for a non-CORS request whose method is neither GET nor HEAD, `"no-referrer"` sets the serialized origin to `null`.
   - SvelteKit's origin check then refuses every form action: sign-out, found Account, delete (`src/runtime/server/respond.js:81-92`). So does the backend's own Origin check.
   - `same-origin` keeps the real Origin on same-origin posts.
   - Worth one line in `session-headers.ts`'s doc comment.
2. **What the header adds, and what it doesn't.**
   - **What it adds.** Browsers already default to `strict-origin-when-cross-origin`, which sends other sites the origin but never the path (MDN, Referrer-Policy). `same-origin` drops the origin too, and overrides any looser policy a page sets.
   - **What it doesn't stop.** Same-origin requests still carry the full address as `Referer`, including requests to `/api/v1/*` through Caddy. Today:
     - the browser makes no direct API calls;
     - the dev Caddyfile has no `log` directive, and Caddy writes access logs only when one is configured;
     - axum traces no request headers.
   - **For production.** Caddy redacts only Cookie, Set-Cookie, Authorization and Proxy-Authorization when access logs are on (Caddy docs, `log`). A production Caddy that turns them on should drop `Referer` too.
3. **The avatar is the frame's one off-site request.**
   - It's the user's own data server's `getBlob` URL (`backend/crates/adapter-atproto/src/profile.rs:116-127`), and it was already in the old header.
   - It now goes without a `Referer`.
   - Question 2 covers other people's avatars.
4. **Mock mode is dev only.** I verified this.
   - A production build answers `/mock/signin` with 404, with or without a handle.
   - A production server with `ZURFUR_WEB_MOCK=1` exits at boot with "mock mode must never reach production".
   - In `just dev-mock` no session cookie exists. So neither header is ever sent there, and every visitor is alice. QA can't observe the headers in the mock click-through.
5. **Every 404 for an unknown address is logged at error level** as "unexpected server error", with no path. Scanners will fill the log. Consider logging 404s at a lower level. The current svelte.dev Errors page says 404s are routine.
6. **Fonts, icons and supply chain.**
   - Every `@font-face` in both font packages points at the package's own files, bundled under `/_app/immutable/assets/`. The production build holds no third-party URL.
   - Icons are inline SVG path data.
   - Both font packages have no install scripts and no dependencies, and the lockfile pins them with integrity hashes.

## Questions for the Engineer

**Q1. Should signed-in pages refuse to be framed by other sites?**
- **What's missing.** No response sets `Content-Security-Policy: frame-ancestors 'none'` or `X-Frame-Options`. Any site can frame the signed-in app and trick a click on Sign out, or later on a Den action.
- **What `'none'` does.** It means "this resource may not be embedded", like `X-Frame-Options: deny`, and it can't be set from a `<meta>` tag (MDN, frame-ancestors).
- **The trade-off.** It closes the door on other sites embedding Zurfur's pages. Nothing in the corpus wants that: Zurfur frames plugin pages, never the reverse (Plugin Security Model).
- **Recommendation:** yes. Add both headers in the same hook that sends the two headers above, on every response, not only signed-in ones.

**Q2. When the app first shows *other people's* avatars, are they loaded from their data servers or through Zurfur?**
- **The trade-off.** Loading from their servers costs nothing to build. But each image tells that person's data-server operator that this viewer looked, with the viewer's IP address and the time.
- **Recommendation:** route other people's avatars through Zurfur's own origin when the first such screen is built. Keep the signed-in user's own avatar as it is.

<details>
<summary><b>Appendix: the probes</b></summary>

Setup:
- `yarn build`, then `node build` on 127.0.0.1:5199.
- `ZURFUR_API_UPSTREAM` pointed at a stand-in on :5198. It answers `/me` (signed in only with a `zurfur.sid` cookie), `/accounts` (without checking the cookie, on purpose) and `/logout`. It logs every hit, with or without a cookie.
- Cookie values switch failure modes: `broken` gives an unparsable listing that names `did:plc:SECRETBODY`, `brokenme` an unparsable `/me`, and `logoutfail` a 500 on sign-out.

| Probe | Request | Result |
| --- | --- | --- |
| P1 | anonymous `GET /accounts` | `303 → /login`; stand-in logged `GET /accounts` with no cookie (page load ran) |
| P2 | anonymous `GET /accounts/__data.json` | `{"type":"redirect","location":"/login"}` |
| P3 | anonymous `GET /accounts/__data.json?x-sveltekit-invalidated=001` | `200`, layouts `skip`, page data returned (`UNGUARDED-DATA`) |
| P4 | signed in: `/`, `/login`, `/accounts`, `/accounts/acct-1`, `/accounts/__data.json`, `/logout`, an unknown address | each `private, no-store` + `same-origin`; anonymous `/`, `/login`, 404: neither; static font: `public, max-age=31536000, immutable` (no hook, no data) |
| P5 | signed in, `broken`, `GET /accounts/did:plc:SECRETURL?pageToken=SECRETTOKEN` | `500`; page "Something went wrong…", payload `error: {message:"Internal Error"}`; no `SECRET` in the page; data request answers `{"type":"error","error":{"message":"Internal Error"}}` |
| P6 | signed in, `GET /den/did:plc:SECRET404` (no such route yet) | 404 "Nothing here", payload `Internal Error`, no `SECRET` |
| P7 | signed in, `logoutfail`, browser-style `POST /logout` | `502`; "Something went wrong reading this…"; payload `error: {message:"Sign-out did not complete. Try again."}`; frame still shows `alice.test` + Sign out |
| P8 | signed in, `brokenme` (root layout fails) | SvelteKit's fallback page, title "Internal Error", both headers present |
| P9 | `POST /logout` with `Origin: https://evil.example` | `403`; never reached the stand-in |
| P10 | production `/mock/signin`, with and without `?handle=` | `404`; production boot with `ZURFUR_WEB_MOCK=1` exits 1 with the guard's message |
| Log | the whole run's server output | only `[web] unexpected server error { route: …, status: … }` lines; 0 hits for `SECRET`, 0 for "unparsable" or "contract" |

Type and lint probes:
- **The frame's link type.** `const offsite: ResolvedPathname = '//evil.example/'` passed `svelte-check` with 0 errors. The control, `'https://evil.example/'`, gave 1 error.
- **A closed `App.Error`.** Narrowing `message` to two literals still compiled `error(404, `…${path}`)`, so that mechanism doesn't work.
- **The lint selector in finding 3.** It flagged the template, variable and object forms (3 errors), and passed the literal and bare forms.

Existing specs on the commit:
- `error-log`, `session-headers`, `hooks.server` and the gate's spec: 27 tests passed.
- `(session)/error.svelte.spec.ts` in Chromium: 2 tests passed, exit 0.

</details>

<details>
<summary><b>Sources checked</b></summary>

- `@sveltejs/kit` 2.70.1, as installed:
  - `src/runtime/server/respond.js` (the invalidation list, the origin check);
  - `src/runtime/server/data/index.js`;
  - `src/runtime/server/page/index.js`;
  - `src/runtime/server/page/render.js`;
  - `src/runtime/server/utils.js`;
  - `src/runtime/server/index.js` (the default `handleError` logs the path);
  - `src/exports/index.js`;
  - `types/index.d.ts` (`error` overloads, `App.Error`);
  - `src/runtime/components/svelte-5/error.svelte`.
- `@sveltejs/adapter-node` 5.5.7, `files/index.js`: it logs nothing per request.
- svelte.dev, [Loading data](https://svelte.dev/docs/kit/load), "Implications for authentication".
- svelte.dev, [Errors](https://svelte.dev/docs/kit/errors). The live page describes a newer SvelteKit than 2.70.1.
- MDN, [Referrer-Policy](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Referrer-Policy) and [CSP frame-ancestors](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/frame-ancestors).
- WHATWG [Fetch Standard](https://fetch.spec.whatwg.org/), "append a request `Origin` header".
- [atproto handle spec](https://atproto.com/specs/handle): "The overall handle must contain only ASCII characters".
- [Caddy `log` directive](https://caddyserver.com/docs/caddyfile/directives/log).
- Corpus: Plugin Security Model (`entities/41222145-plugin-security-model.md`), the opaque iframe Zurfur puts plugins in.
- Study documents: `.understand/20261008-web-explorer-shell.md`, its security calls 11 to 20 and Crow's row 3; `.understand/20261006-my-den-question-sheet.md`, sections 5 and 12.

</details>

## Verification (Heron, 2026-10-08)

Re-checked on step 1 at 9b4543c8, with steps 1b (99c6072b) and 2 (3aa6a1e4) on top. I re-ran every proof against a fresh production build of the tip, with the stand-in backend now refusing any cookie it didn't issue. The probes lived in an empty jj change, and nothing was kept.

**Verdict: clear for a PR.**
- All five findings and both questions are closed, apart from one gap in the sign-out fix.
- The new sign-in gate holds against every attack I tried.
- One should-fix remains, of medium severity and older than this branch: signing out while the backend is unreachable looks like it worked, but leaves the session live (G1 below). It should land before the feature branch reaches `main`.
- No must-fix.

### The findings

| # | Finding | Status | Proof at the tip |
| --- | --- | --- | --- |
| 1 | The gate's docs claimed it guarded loads | **Closed, superseded.** The gate is now the server `handle` hook, and the docs say so | The crafted `__data.json?x-sveltekit-invalidated=001` answers `{"type":"redirect","location":"/login"}`, and the stand-in saw only `/me`, no page load (compare P3 above) |
| 2 | A failed sign-out read as a generic error | **Closed when the backend answers with a failure; open when it can't be reached** (G1) | `logoutfail` → 502, "You're still signed in" / "Sign-out did not complete. Try again.", page data `error: {message:"Error: 502"}` (no app message) |
| 3 | Expected-error messages reached the page source | **Closed** | The lint rule flags `error(404, `…${path}`)` and an aliased `error` import; a literal message passes |
| 4 | The path bar's link type accepted `//evil.example/` | **Closed** | `TrailHref` is now `'/accounts' \| DenHref`; `svelte-check` rejects `href: '//evil.example/'` |
| 5 | Account names without `<bdi>` | **Closed** | Both Accounts pages wrap the name |
| Q1 | Refuse framing? | **Done** | Every response through the hooks carries `frame-ancestors 'none'` and `X-Frame-Options: DENY`, anonymous ones included |
| Q2 | Proxy other people's avatars? | **Parked** to the User Page (your ruling) | n/a |
| Note 1 | Never tighten to `no-referrer` | **Done** | The comment sits in `session-headers.ts` |

The three original claims still hold:
- **Error pages.** An unexpected error with secrets in its address renders "Something went wrong", with page data `Internal Error` and no secret anywhere.
- **The log.** It held only route templates and statuses, and 0 lines with `SECRET`.
- **Headers.** Every signed-in page, data request and 404 carries `private, no-store`, `same-origin`, `frame-ancestors 'none'` and `DENY`.
- **The mock.** It is still unreachable in production: boot with the flag exits 1, `/mock/signin` answers 404, and the compiled `mockModeEnabled()` is `return false`.

### The sign-in gate (`lib/server/session-gate.ts`)

The gate runs inside `handle`, after SvelteKit has matched the route. For any route id under `/(session)`, it asks the backend `/me` before anything else runs, and redirects to `/login` if nobody is signed in. Otherwise it puts the session in `locals`, which the root layout then reuses.

**What I attacked, and what held** (production build):

| Attack | Result |
| --- | --- |
| Anonymous `GET` on every `(session)` page: `/accounts`, `/accounts/acct-1`, `/logout`, `/den`, `/den/commissions` | 303 → `/login`, and no page load ran |
| Anonymous data requests asking to skip the layouts: `/accounts/__data.json?x-sveltekit-invalidated=001`, `/den/x/__data.json?…=001` | JSON redirect to `/login`; the stand-in saw only `/me` |
| Anonymous form actions: a plain `POST /accounts` and an enhanced `POST /accounts/acct-1?/delete` | 303 → `/login`, and `{"type":"redirect","status":303,"location":"/login"}`; no action ran |
| A forged cookie (`zurfur.sid=forged`) on a page, on a skipping data request and on `POST /logout` | Each refused; the backend's `/me` decides, so a forged or stale cookie gets exactly as far as the backend lets it |
| Route ids it might misclassify | Route ids come from SvelteKit's own route table, matched before `handle` runs (`respond.js:333-343`). Every `(session)` route, step 2's `den/[...path]` included, starts with `/(session)/`, so new routes are covered without a list. Public routes and unmatched addresses (`route.id` null) go through ungated. The spec covers a look-alike group, `(sessionx)` |
| The backend's `/me` breaks its contract | 500, SvelteKit's fallback page "Internal Error"; nothing private; logged by route |
| The backend is down | Pages and actions redirect to `/login`: they fail closed for access. But see G1 for sign-out |
| One sign-in check per request | 1 `/me` call per signed-in page or data request (it was 1 before, from the root layout, plus the page's own calls) |

**G1. Signing out while the backend is unreachable looks like it worked, but leaves the session live**
- **Severity:** should-fix (medium). It's older than this branch: `main` (9a01808a) does the same.
- **Where:** `lib/server/session-gate.ts:23-24` treats "can't reach the backend" as signed out, and `session-gate.spec.ts:52-57` pins that.
- **The exploit:**
  - With the backend down, `POST /logout` gets a 303 to `/login` before the sign-out action runs.
  - No cookie is cleared, and `/login` then shows "Sign in", so the person believes they signed out.
  - The `zurfur.sid` cookie stays in the browser, and the backend's session stays live. When the backend is back, the next person at a shared computer is signed in as them.
  - **Before the gate** the result was the same. The action failed, and rendering its error ran the group layout's redirect to `/login` (`page/index.js` loads layouts before the leaf).
  - This is the case my step-1 finding 2 was about. The "You're still signed in" copy never shows here.
- **Proof:**
  - Tip, stand-in stopped, `POST /logout` with `zurfur.sid=ok`: `303 location: /login`, no `set-cookie`. Then `/login` shows `signin-link`.
  - `main`, built and probed the same way: `303 location: /login`.
- **The fix:**
  1. Let the gate pass the sign-out action through. It reads nothing, so it needs no session.
  2. Have the action clear the browser's `zurfur.sid` whatever the backend answers, before or after calling it.
  3. If the backend didn't confirm, say so: "Signed out on this device. Zurfur didn't confirm; your session ends when it expires."
  - **What changes.** On an outage, the server-side session then outlives the cookie until it expires. Nobody holds that cookie any more, so the shared-computer risk goes.
  - **The alternative.** Show "Can't reach Zurfur" instead of redirecting. That's honest, but it still leaves the cookie in the browser, so I recommend the cookie clearing.

**G2. The gate's own redirects skip the header stamping: acceptable**
- **What Finch reports, confirmed.** A redirect thrown in `handle` escapes the outer hooks and is built by SvelteKit (`respond.js:412-426`), so it carries none of our headers. The same goes for the 500 fallback page when `/me` breaks.
- **Why that's acceptable:**
  - **The page redirect** is a bodiless 303. A 303 isn't heuristically cacheable: only 200, 203, 204, 206, 300, 301, 308, 404, 405, 410, 414 and 501 are (RFC 9110 §15.1). So without explicit freshness, no cache stores it.
  - **The data-request redirect** already carries SvelteKit's own `cache-control: private, no-store` (`data/index.js:168-176`).
  - **Neither holds anything private.**
  - **Framing a redirect is moot.** A frame follows it to `/login`, which carries `frame-ancestors 'none'`.
  - **Referrer.** The follow-up request is same-origin, so no address leaves the site.
  - **The fallback page** says only "Internal Error".
- **Don't "fix" it by returning a 303 `Response` instead of throwing.** SvelteKit turns a returned redirect on a data request into a fresh JSON response (`respond.js:533-540`), which drops the headers anyway, and an enhanced form needs the action-JSON form that only the thrown path builds.

**G3. Note: remote functions would get past a route-id gate.**
- A SvelteKit remote-function request takes its pathname, and so its route, from the client's `x-sveltekit-pathname` header (`respond.js:127-129`).
- The app has no remote functions today.
- If any are added, each must check the session itself.

**G4. Note: `handleChain` doesn't merge resolve options the way `sequence` does.**
- `sequence` composes `transformPageChunk` and `filterSerializedResponseHeaders` from every level (`exports/hooks/sequence.js:106-120`). `handleChain` lets an inner hook's options replace an outer one's.
- No hook passes options today. Worth a line in `handle-chain.ts` before one does.

**Also holding:**
- **Order.** The chain is framing outermost, then session headers, then the gate.
- **No preset session.** `locals.session` is set only by the gate; nothing from the client can preset it.
- **The layout guard.** The group layout's guard now only narrows the type: on gated routes `locals.session` is always set.
- **The mock.** In mock mode the gate passes for anyone, because the mock has one shared session. That's dev only, unreachable in production.
- **Checks.** The server test project passes (36 files, 398 tests, exit 0), and `eslint .` reports 0 errors.

## Slice 2b (Heron, 2026-10-08)

Slice 2b, "Sign-out works offline": change uomooyms, commit 7ff7bbb0, on step 2 (3aa6a1e4). I reviewed it against G1 above. I re-ran the outage proof and attacked the gate's new exception, all on a fresh production build against the stand-in backend. The probes lived in an empty jj change, and nothing was kept.

**Verdict: clear for a PR. G1 is closed.**
- No must-fix.
- One low should-fix: the gate's exception is a little wider than its docs say. It's harmless today and a one-line tightening.
- Two notes.

### G1, re-proved with the stand-in backend stopped

- **Plain form post.** `POST /logout` with a live cookie answers `303 location: /login?signout=unconfirmed`, with `set-cookie: zurfur.sid=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax`.
- **Enhanced form post.** The same request gets `{"type":"redirect","status":303,"location":"/login?signout=unconfirmed"}` and the same `set-cookie`.
- **The landing page.** `/login?signout=unconfirmed` shows the fixed notice ("You're signed out on this device. Zurfur couldn't confirm it, so your session on the server ends when it expires.") above the sign-in link.
- **The result.** The browser forgets the session, and on an outage the server's session simply expires. Nobody holds the cookie any more, so the shared-computer risk is gone.

### What I attacked

| Attack | Result |
| --- | --- |
| **The exception's method** | Only `POST` passes. Anonymous `GET` and `HEAD /logout` get a 303 to `/login`, and `PUT /logout` is still gated |
| **The exception's path** | It matches by route id, which SvelteKit sets before `handle` runs. `/logout/` gets SvelteKit's 308 → `/logout` before any hook. `/LOGOUT` and `/logout%2F` are 404s. Every other `(session)` action is still gated (anonymous `POST /accounts` → `/login`) |
| **What else rides through: `POST /logout/__data.json`** | Ungated. It runs the root layout (the caller's own whoami), the group layout and the logout page's load, which only redirects home: `{"type":"redirect","location":"/"}`. With `x-sveltekit-invalidated=001`, only that load runs. Nothing private comes back. This is the should-fix below |
| **What else rides through: `POST /logout?/other`** | No such action; the group layout's own redirect sends it to `/login`. That redirect "never fires" per its comment, and here it's the backstop |
| **Cross-site sign-out** | SvelteKit's origin check refuses a form post from `https://evil.example`, from `Origin: null` and with no Origin header: 403 each, before the gate (`respond.js:81-100`; production builds only). A cross-site `application/json` post needs a preflight first, and on its own it gets SvelteKit's 415; no action runs. The cookie being `SameSite=Lax` also keeps it off any cross-site post |
| **The `signout` parameter** | The page reads it as a yes/no: the first value must be exactly `unconfirmed`. The notice is a fixed constant, and nothing from the address is echoed. `UNCONFIRMED`, `confirmed`, a script tag, and `x` before `unconfirmed` show nothing. A signed-in visitor to `/login?signout=unconfirmed` is sent home, so only a signed-out visitor ever sees "You're signed out on this device" |
| **The cleared cookie's attributes** | The backend sets `zurfur.sid` with no `__Host-` prefix in any environment, `HttpOnly`, `SameSite=Lax`, `Secure` only in production and staging (`backend/crates/api/src/main.rs:38-43`). Tower-sessions 0.15.0 defaults it to `Path=/` and no `Domain`, so it's host-only (`service.rs:137-138`). The deletion uses the same name, `Path=/` and no `Domain`, so it replaces that cookie. Its `Secure` flag is right for production's HTTPS |
| **A confirmed sign-out clears what it did before** | Stand-in clearing `zurfur.sid` alone: one deletion of `zurfur.sid`, then home. Stand-in clearing `zurfur.sid` and `zurfur.extra`: one deletion of each, then home. That's the backend's own list, plus `zurfur.sid` always, never twice |
| **An anonymous sign-out** | It reaches the backend's `/logout`, which flushes an empty session and confirms (`routes/session.rs:168-174`). The visitor lands home. It costs one backend call, as the gate's `/me` did before |

**Checks.** The server test project passes (36 files, 403 tests, exit 0), and `eslint .` reports 0 errors.

### Should-fix (low): the exception is wider than its docs

- **Where.** `session-gate.ts:16-18` lets through *any* `POST` on the sign-out route, so `POST /logout/__data.json` runs that route's loads ungated.
- **Why it's harmless now.** The loads read nothing private: the root layout's whoami uses the caller's own cookie, and the logout load only redirects.
- **The docs are now wrong in two places:**
  - The group layout's comment says its redirect "never fires" (`(session)/+layout.server.ts:9`), but it does fire for `POST /logout?/other`.
  - The node's convention says nothing but the sign-out action runs ungated.
- **The fix:** require `!event.isDataRequest` in `isSignOutAction`, so the exception is the action alone. Then reword the layout comment to "the backstop for the sign-out route, which the gate lets through".

### Notes

1. **Dev may not drop the cookie** [unverified].
   - Outside the address `localhost`, SvelteKit marks every cookie it writes `Secure` (`@sveltejs/kit` `src/runtime/server/cookie.js:65`). The dev stack runs on `http://127.0.0.1:8080`, so the deletion arrives `Secure` over plain HTTP.
   - MDN says insecure sites can't set `Secure` cookies, with an exception "set by localhost". It doesn't say whether `127.0.0.1` counts.
   - If a browser ignores the deletion there, an offline sign-out in dev keeps the cookie, though production (HTTPS) is unaffected. This was already true of the confirmed path's mirror-deletes before this slice.
   - Worth checking in the dev click-through, in the browser's cookie panel after "Sign out".
2. **In dev, cross-site sign-out isn't blocked.** SvelteKit skips its origin check in the dev server (`respond.js:73`).
