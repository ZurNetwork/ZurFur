# Explorer shell, steps 1b and 2: security review

2026-10-08 · Heron (security reviewer) for the tech lead (`claude`) · step 1b "No sidebar where no Den is served" (change vprutnrt, commit 7fac6afe) and step 2 "My Den on the mock" (change vrzmmnwu, commit ca936fa6), stacked on step 1 (9dba2fcf) · QA is Crow's, separately.

## In one screen

**How I checked.** I read every security-relevant file in both changes. I built step 2 for production and ran it against a stand-in backend, then ran it as a dev server in mock mode. I added 28 attacks of my own on the path builder, and ran lint and type probes against the `DenHref` brand. I read the installed SvelteKit 2.70.1 source wherever its behaviour mattered. The probes lived in an empty jj change on top of step 2 (zwlplokv), and nothing was kept.

**The path builder holds.** All 28 extra attacks were refused or kept harmless. They include the review's own `/den/..%2Faccounts`, overlong UTF-8 dots, `..;`, look-alike dots and slashes, and page tokens that try to smuggle in a parameter. Every literal that looks dangerous (`%2e%2e`, `.%2e`, `%2F`) reaches the API path escaped, so `fetch` can't resolve it. The rebuilt test set covers each of §12's rules, but it lacks 18 groups of cases worth pinning (first should-fix).

**`denServed()` fails closed.**
- **Production.** The production build compiles the mock switch to a constant `return false`. A production server started with the mock flag still refuses to boot, and `/mock/signin` answers 404.
- **Live port.** It answers every Den read with "not connected" and never calls the backend. The production build showed no sidebar and no drawer.
- **The mock.** A production build can't reach it, though its fixture data still ships in the server bundle as dead code.

**The `DenHref` brand closes the Den side of my step-1 link finding, but not the whole of it.**
- **The import rule misses two routes.** It catches a named import, a namespace import, a re-export, a type assertion and an `any` value. It misses a relative import of the brand module and a dynamic `import()`. I widened the rule in a probe, and it caught both.
- **A third mint.** The client-side check `isDenPageData` vouches for every link in a preloaded page while checking only one field.
- **The path bar.** Its link type still accepts any `ResolvedPathname` beside `DenHref`, and `'//evil.example/'` still type-checks there.

**Den links on a fixed `/den`.** The app sets no base path, and SvelteKit's server-side `resolve()` really does return relative paths (the reason the builder doesn't use it). If someone ever sets a base path, reading fails closed: every Den address becomes the not-found. But the links would point outside the app. A one-line test should pin "no base path".

**Expected-error messages.** These are still not covered by a lint rule. My step-1 finding is in Finch's pending fix round. Step 2 adds no new `error()` with a message: the Den answers 200 with an outcome, and its contract errors carry only the route template.

**Counts:** 0 must-fix · 5 should-fix · 6 notes · 1 question for the Engineer.

**Verdict: clear for a PR.** None of the should-fixes is exploitable today, because the live port reads no Den. All five should land before step 3 wires the live Den.

## Findings

| # | Severity | Finding | Where | When |
| --- | --- | --- | --- | --- |
| 1 | should-fix (low) | The path builder's tests lack 18 groups of cases §12 and §5 imply; the builder passes all of them | `lib/server/den/den-path.spec.ts`, `lib/server/api/zurfur-api.spec.ts` | Before step 3 |
| 2 | should-fix (low) | The `DenHref` brand has three side doors | `eslint.config.js:86-91`, `lib/api/den.ts:111-113` | Before step 3 |
| 3 | should-fix (low) | The path bar's link type still accepts `//evil.example/` (my step-1 finding, half closed) | `lib/api/trail.ts:10` | With finding 2 |
| 4 | should-fix (low) | Den links assume no base path, and nothing pins that | `lib/api/den-route.ts:9` | Any time before a base path could be set |
| 5 | should-fix (low, carried) | Expected-error messages still have no lint rule | `eslint.config.js` (absent) | Finch's step-1 fix round |

Paths are under `frontend/web/src/` unless they say otherwise.

### 1. The path builder's tests lack 18 groups of cases; the builder passes all of them

- **Severity:** should-fix (low). Every case below already passes against the builder, in my probe spec. The gap is that nothing pins them, so a later edit could break one silently.
- **What's covered.** The rebuilt set has one or more tests for each of §12's rules:
  - empty pieces, `.` and `..`, raw and encoded (both cases, both half-encodings);
  - 16 pieces and not 17; 2048 bytes and not 2049;
  - the ASCII allowlist, with NUL, newline, space, `?`, `#`, `\` and five Unicode look-alikes;
  - malformed and truncated escapes;
  - decoding exactly once;
  - the parse-equality check;
  - the query allowlist: unknown names, other spellings, repeats, the token's characters and length;
  - the hardened account delete: 9 refusals, `%` escaping, a UUID;
  - the uniform not-found without a read;
  - contract errors naming only the template;
  - a stale token reloading once, and no loop on a 422 without a token.
- **What's missing** (each one passed in my probe, `heronprobe/builder.spec.ts`, 28 of 28):

| §12 / §5 rule | Missing case | Why it matters |
| --- | --- | --- |
| §5 item 1: the traversal the review reproduced | `/den/..%2Faccounts` verbatim | It's the one experiment the review actually ran against SvelteKit and Node; pin it by name |
| §12 item 1: refuse `..` | `/den/..%2f..%2fme` (lowercase escapes) | Only `%2F` uppercase is tested |
| §12 item 1 | `/den/%2E./x` (uppercase half-encoding) | Only the lowercase half-encodings are tested |
| §12 item 1 | `/den/%2e%2e%2f` (dot-dot-slash in one piece) | |
| §12 item 4: decode once, strictly | `/den/%C0%AE%C0%AE` and `/den/a%C0%AFb` (overlong UTF-8 dot and slash) | The classic overlong bypass; `decodeURIComponent` refuses it, and a test should say so |
| §12 item 4 | `/den/%` (a lone percent) | |
| §12 item 4: the allowlist | `/den/..;/accounts` and `/den/a;b` | The servlet-style `..;` trick |
| §12 item 4 | tab `%09`, carriage return `%0D`, DEL `%7F` | Only NUL and newline are tested |
| §12 item 4 | `+`, `@`, `'` | Characters outside the allowlist that aren't separators |
| §12 item 4: look-alikes | `%E2%88%95` (division slash), `%E2%80%A4%E2%80%A4` (one-dot leader pair), `%EF%B9%92%EF%B9%92` (small full stop pair) | Five look-alikes are tested; these are the remaining common ones |
| §12 item 4: `.` and `..` only | `...` accepted as a literal | Pins that only the two dot segments are special |
| §12 item 1: encode, then check the parse | the API path for the literals `%2e%2e`, `.%2e`, `%2F`, `%` stays escaped (`/api/v1/den/accounts/%252e%252e`) and parses back unchanged | This is where `fetch` would resolve an unescaped `%2e%2e`; only the browser link's round trip is tested today |
| §12 item 1: the query allowlist | a page token holding `& = # + %` round-trips encoded and never sets `includeDeleted` | The token accepts any visible ASCII, so a token can carry those characters |
| §12 item 1: the account delete | id `%2e%2e` is sent as `/accounts/%252e%252e` | The case that would climb if the escaping regressed; only `a%2Fb` is tested |
| §12 item 1: the account delete | a `did:plc:` id is sent as one plain segment | Account ids are DIDs now; only a UUID is tested |
| §12 item 2: uniform not-found | a refused address and a backend miss give the same title and path bar | The refused-address test checks only the outcome |
| §5 items 7 and 12: session principal only | another user's DID in the path answers the not-found, never their Den | The mock reads the root from the session, and no test says so |
| §12 item 1: the prefix | `/den%2Fx` and `/DEN/x` are refused | |

- **The fix:** add these cases to `den-path.spec.ts`, `den.spec.ts`, `den-mock.spec.ts` and `zurfur-api.spec.ts`.

### 2. The `DenHref` brand has three side doors

- **Severity:** should-fix (low). Using any of these doors takes a code change, but the first is a plausible editor auto-import.
- **Where:** the rule `DEN_HREF_IMPORT` (`eslint.config.js:86-91`) matches the exact import string `'$lib/types/brand'`. And `isDenPageData` (`lib/api/den.ts:111-113`) is a type predicate.
- **The doors:**
  1. **A relative import:** `import { denHref } from '../../types/brand'`. The rule doesn't see it.
  2. **A dynamic import:** `(await import('$lib/types/brand')).denHref(…)`. `no-restricted-imports` doesn't cover `import()`.
  3. **`isDenPageData`.** It checks `kind === 'denPage'`, then vouches for the whole shape, every `DenHref` inside included. The tree loader and the pane mint links through it from `preloadData` results. Those come from the app's own server, so this is a gap in the brand's promise, not a live hole.
- **What the rule does catch:** a named import, a namespace import, a re-export, a type assertion (`as DenHref`) and an `any` value.
- **Proof:**
  - I ran eslint with the repo's config on probe files. It flagged the named, namespace, re-export, assertion and `any` probes, 5 errors. It passed the relative and dynamic ones.
  - A widened rule caught both, and still allowed a type-only import and the brand module's other names. The rule:
    - `no-restricted-imports` `patterns: [{ group: ['**/types/brand', '$lib/types/brand'], importNames: ['denHref'] }]`;
    - `no-restricted-syntax` `ImportExpression[source.value=/types/brand$/]`.
- **The fix:**
  - Use the widened rule, adding the selector to the existing `no-restricted-syntax` arrays rather than replacing them.
  - Better still, make the mint itself refuse anything that isn't `/den` or `/den/…` and fails the parse check, so a side door can only mint a well-formed Den link.
  - Have `isDenPageData` check that each link starts with `/den`, or document it as trusting the app's own server.

### 3. The path bar's link type still accepts `//evil.example/`

- **Severity:** should-fix (low). This is my step-1 finding, half closed.
- **Where:** `lib/api/trail.ts:10`: `TrailHref = ResolvedPathname | DenHref`.
- **The exploit:**
  - The `DenHref` arm is now guarded.
  - The `ResolvedPathname` arm is still SvelteKit's generated type, `` `${"" | `/${string}`}${Pathname}` ``, which a protocol-relative URL fits.
  - At runtime, a server-side `resolve()` returns a *relative* path while claiming the type (`@sveltejs/kit` `src/runtime/app/paths/server.js:26-40`). So the type promises nothing about the string's shape in either direction.
  - `PathBar.svelte` turns off `svelte/no-navigation-without-resolve` on the grounds that producers resolve their links.
- **Proof:** `svelte-check` passed `{ step: 'named', label: 'x', href: '//evil.example/' }` as a `TrailStep` with 0 errors.
- **The fix:** `TrailHref = DenHref | SectionHref`, where `SectionHref` is a closed list of fixed section paths (`'/accounts'` today).

### 4. Den links assume no base path, and nothing pins that

- **Severity:** should-fix (low).
- **Where:** `lib/api/den-route.ts:9`: `DEN_ROOT = '/den'`, with the comment "The app sets no base path".
- **The exploit:**
  - **Reading fails closed.** The app does set no base path today: there's no `svelte.config.*` and no `paths` in `vite.config.ts`. If one is set later, the builder compares `url.pathname`, which would then start with the base, against `/den`. It would refuse every address, so every Den page becomes the not-found.
  - **Links don't.** `/den/…` would point outside the app, to whatever else the origin serves there. Same origin, so cookies go along.
  - **Why not `resolve()`.** The choice not to use it is sound: on the server, `resolve()` returns `..`-relative paths during a request (`src/runtime/app/paths/server.js:26-40`).
- **The fix:** a unit test beside `den-route.ts` asserting that `base` from `$app/paths` is `''`, so setting a base path fails the build instead of silently mis-linking.

### 5. Expected-error messages still have no lint rule

- **Severity:** should-fix (low), carried from step 1, finding 3.
- **Status.** Not covered yet. `eslint.config.js` has no rule on `error()`'s message. It's in Finch's pending step-1 fix round.
- **Step 2 adds no new exposure:**
  - The den load answers 200 with an outcome, never `error()`.
  - Its contract errors are unexpected, so the page gets "Internal Error", and they name only `/den/{path}`. The tests check that the real path is absent.
- **The fix:** as in the step-1 report. The selector is `CallExpression[callee.name='error'][arguments.length>1]:not([arguments.1.type='Literal'])`, verified there.

## Notes

1. **`denServed()` fails closed in every build I could make.**
   - **Production.** The production build compiles `mockModeEnabled()`, and with it `denServed()`, to `return false`. Its server bundle shows `function mockModeEnabled() { return false; }`.
   - **Dev.** Without the flag it's false, per the spec and the code. Only the exact value `ZURFUR_WEB_MOCK=1` turns it on, and only in `vite dev`.
   - **Nothing a client can flip.** It's computed in the root layout load and sent to the page as data. It only hides UI; the Den route's data source is chosen by the same predicate on the server.
2. **A production build can't reach the mock.**
   - Signed in on the production build:
     - `/den` and `/den/commissions` answered "isn't connected to the backend yet", with no sidebar and no drawer in the page;
     - the stand-in received no Den call;
     - refused addresses answered "Nothing here".
   - A production boot with `ZURFUR_WEB_MOCK=1` exits 1 with the guard's message, and `/mock/signin` answers 404.
   - The mock's fixture data ships in the server bundle as unreachable code.
3. **Mock mode is still open to anyone, in dev only.** An anonymous request to the mock dev server got alice's Den (`/den` and `/den/__data.json`), with neither header. Same cause as step 1's note: the mock has one shared session and sets no cookie.
4. **The gate isn't what protects the den load, and that holds for now.**
   - The den load doesn't await `parent()`.
   - On the live port it reads nothing. In step 3, a read without a session gets the backend's 401, and the load sends the visitor to `/login` (tested: "sends an ended session to sign in").
   - My step-1 rule applies: a `(session)` load reads only through the backend with the caller's cookie.
5. **Ancestor reads multiply backend load.**
   - A full page load at depth 16 makes 17 parallel backend reads: the node, the root and 15 prefixes (`lib/server/den.ts:209-231`, unbounded concurrency).
   - That's harmless on the mock. Before step 3 goes live, consider capping the concurrency or the depth that pre-reads ancestors.
6. **Checked and holding:**
   - **Stale page token.** It answers a 303 to the folder's first page, built by the builder. It reloads only when a token was present, so it can't loop.
   - **Uniform not-found.** A refused address and a miss answer the same status (200), title and path bar. Only the ancestor listings differ, and those show nothing beyond what the viewer can open directly.
   - **Account delete.** It goes through the builder and refuses before any `fetch`.
   - **Tree memory.** It belongs to each frame instance, is keyed by the session's DID, and starts over when the DID differs.
   - **The browser.** Nothing goes to browser storage, and there's no `{@html}`.
   - **Names.** They render in `<bdi>` in the tree, the listing, the node header and the path bar.
   - **Page tokens.** The mock's are a version and an offset, with no names, per §12 item 5.

## Question for the Engineer

**Q1. Should a Den page's title carry the open item's name?**
- **What the code does.** Step 2 titles a page `<name> — My Den — Zurfur` (`lib/server/den.ts:246-259`).
- **The cost.** That writes Private names, such as a Private character's name or a commission's title, into the browser's history. A browser may sync history to other devices.
- **Why it's there.** SvelteKit's route announcer reads `document.title` after each navigation (`src/core/sync/write_root.js:162`), so the title is how a screen reader hears where it landed.
- **The plan.** It left this to the end-of-slice review. I'm raising it now because it's live code.
- **Recommendation:** a generic title ("My Den — Zurfur"), with the frame announcing the item's name in its own live region. Screen-reader users still hear the name, and history keeps only the generic title. The address bar already records DIDs and ids, but names are the readable, sensitive part.

<details>
<summary><b>Appendix: the probes</b></summary>

Production build of step 2 (`node build` on :5199, stand-in backend on :5198), signed in unless stated:

| Probe | Result |
| --- | --- |
| `GET /den`, `/den/commissions` | 200, "isn't connected to the backend yet", title "My Den — Zurfur", no `sidebar`/`open-drawer` in the page, `Referrer-Policy: same-origin`; stand-in got no Den call |
| `GET /den/a%2Fb`, `/den/..%2Faccounts` | 200, "Nothing here" |
| `GET /den/%252e%252e` | 200, "not connected" (a valid literal segment) |
| anonymous `GET /den` | 303 → `/login` |
| `ZURFUR_WEB_MOCK=1 node build` | exit 1, "mock mode must never reach production" |
| `GET /mock/signin` | 404 |
| compiled server bundle | `function mockModeEnabled() { return false; }`; `denServed()` returns it |

Dev server in mock mode (`vite dev`, `ZURFUR_WEB_MOCK=1`, :5196), no cookie:

| Probe | Result |
| --- | --- |
| `GET /den`, `/den/__data.json` | alice's Den, no `cache-control` or `referrer-policy` |
| `GET /den/commissions`, `/den/accounts` | their pages |
| `GET /den/a%2Fb`, `/den/%252e%252e` | 200, "Nothing here" |
| `GET /den/` | 308 → `/den` |
| `GET /den%2Fcommissions` | 404 |

Lint probes, run with the repo's eslint config:

| Probe | Result |
| --- | --- |
| named, namespace, re-export, `as DenHref`, `any` value | flagged (5 errors) |
| relative import, dynamic import | not flagged |
| widened rule (finding 2) on relative and dynamic | both flagged; a type-only import and another name pass |

Type probe:
- `svelte-check` passed `href: '//evil.example/'` on a `TrailStep` (0 errors).

Builder spec:
- `heronprobe/builder.spec.ts`: 28 of 28 passed. That's 21 refusals, 5 escaped literals on the API path, the `%2e%2e` delete path, and the token-injection round trip.

Existing specs on step 2:
- den path, den program, den mock, `denServed`, the live API port and the den route: 134 tests passed, exit 0.
- `eslint .`: 0 errors, exit 0.

</details>

<details>
<summary><b>Sources checked</b></summary>

- `@sveltejs/kit` 2.70.1, as installed:
  - `src/runtime/app/paths/server.js` (server `resolve()` returns relative paths during a request);
  - `src/core/sync/write_root.js` (the announcer reads `document.title`);
  - `src/runtime/server/respond.js`, `data/index.js` and `page/index.js` (from the step-1 review).
- The production build's server bundle: `build/server/chunks/chunks/zurfur-api-mock.js-*.js` and `entries/pages/_layout.server.ts.js-*.js`.
- ESLint: `no-restricted-imports` behaviour checked empirically against the repo's config (probes above).
- `.understand/20261006-my-den-question-sheet.md`, section 5 (items 1, 2, 4, 7, 8, 11, 12) and section 12 (items 1 to 5, the small additions).
- `.understand/20261008-web-explorer-shell.md`, security calls 11 to 19.
- `.understand/20261008-explorer-shell-security-step-1.md`, findings 1, 3 and 4.

</details>

## Verification (Heron, 2026-10-08)

Re-checked on step 1b at 99c6072b and step 2 at 3aa6a1e4, with step 1 at 9b4543c8 underneath. I re-ran the proofs on a fresh production build, on a dev server in mock mode, and with lint, type and test probes. The probes lived in an empty jj change, and nothing was kept.

**Verdict: clear for a PR.**
- All five findings and the question are closed.
- The sign-in gate holds for every Den route.
- No must-fix.
- One should-fix lives in the gate review in the step-1 file: signing out while the backend is unreachable looks like it worked, but leaves the session live. It's older than this branch and should land before the feature branch reaches `main`.

### The findings

| # | Finding | Status | Proof at the tip |
| --- | --- | --- | --- |
| 1 | 18 groups of path cases unpinned | **Closed** | Each group is now in `den-path.spec.ts`, `den.spec.ts`, `den-mock.spec.ts` or `zurfur-api.spec.ts`. I found every one by its case string, including: `..%2Faccounts`, overlong UTF-8, `..;`, `%09`/`%0D`/`%7F`, the look-alikes, `...`, the escaped API-path literals, the token round trip, the `%2e%2e` and DID deletes, refused-vs-miss title and path bar, another user's DID, and `/den%2F`/`/DEN/` |
| 2 | Three side doors in the `DenHref` brand | **Closed** | The rule now flags a relative import, a relative namespace import and a dynamic `import()`. `isDenPageData` now checks every link it vouches for (`isDenLink`: `/den`, `/den/…` or `/den?…`; no `//`, no `\`). The mint itself still casts without checking: an optional hardening, not needed now that every route to it is linted |
| 3 | The path bar's type accepted `//evil.example/` | **Closed** | `TrailHref = '/accounts' \| DenHref` (Finch's closed list instead of my brand: equivalent, and simpler); `svelte-check` rejects the probe |
| 4 | Den links assume no base path | **Closed** | `den-route.spec.ts` asserts the den root resolves to `/den`. With `paths: { base: '/app' }` added to the config, it fails: `expected '/app/den' to be '/den'`. So setting a base path now breaks the build instead of mis-linking |
| 5 | Expected-error messages had no lint rule | **Closed** | See the step-1 file, finding 3 |
| Q1 | Den page titles carried names | **Resolved** | Every Den page's title is now "My Den · Zurfur", and a live region names the open item. The tests check the title on the root, a miss and a refused address |

### Notes, re-checked

1. **`denServed()` still fails closed.**
   - Production compiles `mockModeEnabled()` to `return false`.
   - Step 1b's fix makes `/den` on the everyday stack the ordinary 404, with no backend call. Signed in on the production build, `/den` answered 404 "Nothing here", with all four headers.
2. **A production build still can't reach the mock.** Boot with the flag exits 1, and `/mock/signin` answers 404.
3. **Mock mode is still open to anyone in dev.** The gate passes there, because the mock has one shared session: an anonymous request got alice's Den. That's dev only.
4. **Ancestor reads are unchanged.** A full page load at depth 16 still makes 17 parallel backend reads. Consider a cap before step 3 goes live.
5. **The rest still holds:**
   - **Stale token.** It reloads the folder once, by a 303 built by the builder.
   - **Uniform not-found.** It's now pinned, title and path bar included.
   - **Tree memory.** It's per frame, keyed by DID, and nothing goes to browser storage.
   - **Names.** They render in `<bdi>`.

### The sign-in gate, for the Den routes

The full review is in `.understand/20261008-explorer-shell-security-step-1.md`, under "Verification".

**For step 2's routes:**
- **Anonymous requests.** `GET /den` and `/den/commissions` get a 303 to `/login`, and `/den/x/__data.json?x-sveltekit-invalidated=001` gets a JSON redirect to `/login`. No den load ran.
- **New routes.** The gate matches by the `/(session)/` route prefix, so `den/[...path]` is covered with no list to maintain.
- **Order.** The gate runs before the den load's own `denServed()` check. So on the everyday stack an anonymous visitor gets the sign-in redirect, not the 404, and learns nothing about whether a Den is served.

**Checks.**
- The server test project passes (36 files, 398 tests, exit 0).
- `eslint .` reports 0 errors.
- `svelte-check` rejects the off-site path-bar probe.

## Copilot fixes and 2c (Heron, 2026-10-08)

Re-checked at the 2c tip, 58282348, with step 2 at 7970eae4 and 2b at eb112078. I re-ran the proofs on a fresh production build against the stand-in backend, then ran the server tests, lint and the type check. The probes lived in an empty jj change, and nothing was kept.

**Verdict: clear for a PR.**
- All three fixes hold.
- Every earlier proof still passes.
- No must-fix, and no new should-fix.

### 1. `isDenLink` (Copilot on #247): closed

- **The fix.** A link now passes only when the URL parser reads its path and query back unchanged, on top of the old shape checks (`lib/api/den.ts:121-135`).
- **Refusals.** I ran it against 30 hostile links, built from my path-builder corpus. All 30 are refused:
  - every raw, encoded, half-encoded and mixed-case dot segment, including `/den/../accounts` and `/den/%2e%2e/accounts`;
  - `/den/x/..` and deeper climbs;
  - `//evil.example/den` and `/den//evil.example`;
  - backslashes;
  - fragments, and a bare `?`;
  - spaces, tabs, newlines and trailing spaces, which the parser rewrites or strips;
  - non-ASCII, `"`, `<`, backtick, `{`;
  - an absolute `http:` URL, and `/dent`.
- **No false refusals.** Every link the Den path builder makes still passes. I tried 9 segment sets: the root, DIDs, `did:web` with an escaped `%`, and the dangerous-looking literals `%2e%2e`, `.%2e`, `%2F`, `%` and `...`. Each one passed both plain and with a page token holding `& = # + % ' ! * ( )`.
- **Result.** 39 of 39 probe tests pass.
- **Note.** The `denHref` mint itself is still an unchecked cast. Every route to it is linted, and the client-side check is now strict, so that stays an optional hardening.

### 2. The unconfirmed sign-out notice (Copilot on #248): closed

- **The fix.** The notice now shows only when the request carries no `zurfur.sid` (`(public)/login/+page.server.ts`).
- **Proof.** `/login?signout=unconfirmed`:
  - with no cookies: shows the notice;
  - with an unrelated cookie: shows it;
  - with `zurfur.sid=stale`: hides it;
  - with an empty `zurfur.sid=`: hides it.
- **Copilot's crafted-link case is now harmless.** During an outage a live cookie keeps the notice hidden, so the page can't claim a cookie was cleared when it wasn't.
- **My dev note from 2b.** If a browser ever refuses the `Secure` deletion over plain HTTP in dev, the cookie survives and the notice stays hidden. The page now reports that honestly instead of claiming a sign-out.

### 3. Only the sign-out action skips the gate (2c): closed

- **The fix.** The exception now also requires `!event.isDataRequest` (`lib/server/session-gate.ts:17-21`).
- **Proof:**
  - **Data requests are gated again.** Anonymous `POST /logout/__data.json`, with and without `x-sveltekit-invalidated=001`, now answers `{"type":"redirect","location":"/login"}`, and the stand-in got no `/logout` call.
  - **The action still runs.** Anonymous `POST /logout` reaches the action, which clears the cookie and goes home.
  - **The backstop.** `POST /logout?/other` still lands on `/login` through the group layout's redirect, which the layout's comment now names as the backstop for that route.
  - **Plain requests.** `GET /logout` is gated.

### Earlier proofs, re-run at the tip

- **The gate.** The crafted `__data.json?x-sveltekit-invalidated=001` on `/accounts` and on `/den/x`, an anonymous form action and a forged cookie all redirect to `/login`. No anonymous or forged request reached a page load (the one listing call came from the signed-in probe).
- **Headers.** A signed-in `/accounts`, and `/den` on the everyday stack (a 404), each carry `private, no-store`, `same-origin`, `frame-ancestors 'none'` and `DENY`.
- **Sign-out.** A refused sign-out goes to `/login?signout=unconfirmed` and clears `zurfur.sid`. A cross-site sign-out gets a 403.
- **Outage, stand-in stopped:**
  - `POST /logout` with a live cookie: 303 to `/login?signout=unconfirmed`, with `set-cookie: zurfur.sid=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax`;
  - the landing page without the cookie shows the notice;
  - a signed-in `GET /accounts` fails closed to `/login`.
- **The log.** It held no error lines.
- **Checks.** The server test project passes (37 files, 413 tests, exit 0), `eslint .` reports 0 errors, and `svelte-check` reports 0 errors.

## Copilot round 2 (Heron, 2026-10-08)

Re-checked at the 2c tip, f6f15cca, with step 2 at f263e144 and 2b at f72a4a12. This covers rows 5, 8 and 9 of the triage's "Round 2". I attacked each fix, then re-ran the earlier proofs on a fresh production build against the stand-in backend, now with a mode that accepts a sign-out call and never answers. The probes lived in an empty jj change, and nothing was kept.

**Verdict: clear for a PR.**
- All three fixes hold, every earlier proof passes, and the checks are green.
- No must-fix.
- Row 9 also corrects my 2b review. There I took the group layout's redirect as the backstop for `POST /logout?/other`, which it was only because no such action existed. Copilot is right that actions run before layout loads, so a named action added later would have run anonymously.

### Row 5. An access value other than exactly `open` now fails closed: closed

- **The fix.** All three places now test `access !== 'open'` (`lib/server/den.ts:87, 164, 221`):
  - a listing row;
  - the page itself;
  - the ancestor listings the tree pre-reads.
- **Proof.** A probe port answered with seven odd values: `superuser`, `OPEN`, `open ` (with a trailing space), the empty string, `listed`, `owner` and `card2`. 21 of 21 probe tests pass:
  - a row becomes a card, with no `/den/…` link in its data;
  - the page becomes a card page with no body;
  - an ancestor lends the tree no listing.
- **The client.** It never sees the raw access value. It reads the server's `view` (`open` or `card`), and `isDenPageData` rejects anything else.

### Row 8. Sign-out is bounded at 5 s: closed

- **The fix.** The sign-out call gets an abort signal, fired by a 5,000 ms timer (`SIGNOUT_TIMEOUT_MS`, `lib/server/api/zurfur-api.ts:266`). The abort is a `NetworkFailure`, which means "unconfirmed".
- **Proof.** I made the stand-in accept the sign-out call and never answer:
  - a plain `POST /logout` returned after **5.03 s** with `303 location: /login?signout=unconfirmed` and `set-cookie: zurfur.sid=; Max-Age=0; …`;
  - the enhanced post returned after **5.01 s** with `{"type":"redirect","status":303,"location":"/login?signout=unconfirmed"}` and the same cookie clearing.
- **The abort reaches the real call.** It survives the API proxy's request rewrite, which copies the signal into the upstream request (`new Request(upstreamUrl, request)`). The probe's timing proves it.
- **Note.** The gate's own `/me` call, and the other backend calls, still have no bound of their own. A hung backend makes signed-in pages wait, and they still fail closed. That's availability, not security, and sign-out is no longer affected, because the gate lets it through.

### Row 9. Only the default sign-out action skips the gate: closed

- **The fix.** The exception also refuses any query key that starts with `/` (`lib/server/session-gate.ts:13-29`). That is exactly how SvelteKit picks a named action: the first search key starting with `/` (`src/runtime/server/page/actions.js:235-243`). The gate reads `event.url` and SvelteKit reads `request.url`, but they're the same for a page action request.
- **Proof.** All anonymous, all now refused by the gate itself (a 303 to `/login` with none of the hooks' headers, so the action never ran):
  - `POST /logout?/other`;
  - `?/default`;
  - `?x=1&/other`;
  - `?%2Fother`;
  - `?/`.
- **The default action still passes,** with or without an unrelated query (`?x=1`): it clears the cookie and goes home.
- **Data requests stay gated:** `POST /logout/__data.json` answers the JSON redirect.
- **The backend's view.** The stand-in got a `/logout` call only from the default-action requests.
- **Note, unchanged.** A remote-function-style request claiming `x-sveltekit-pathname: /logout` still matches the exception's route and method. It answered 404 here, because the app has no remote functions. My earlier note stands: if any are added, each checks the session itself.

### Earlier proofs, re-run at the tip

- **The gate.** The crafted `__data.json?x-sveltekit-invalidated=001` on `/accounts` and `/den/x`, an anonymous form action and a forged cookie all redirect to `/login`. Only the signed-in probe reached a page load.
- **Headers.** A signed-in `/accounts`, and `/den` on the everyday stack (a 404), carry `private, no-store`, `same-origin`, `frame-ancestors 'none'` and `DENY`.
- **Sign-out.** A refused sign-out goes to `/login?signout=unconfirmed` and clears `zurfur.sid`. A cross-site sign-out gets a 403. The notice shows with no cookie and stays hidden with a stale `zurfur.sid`.
- **Outage, stand-in stopped:**
  - `POST /logout` with a live cookie: 303 to `/login?signout=unconfirmed`, with the cookie cleared;
  - a signed-in `GET /accounts` fails closed to `/login`.
- **The mock.** Still unreachable in production: boot with the flag exits 1, and `/mock/signin` answers 404.
- **The log.** It held no error lines.
- **Checks.** The server test project passes (37 files, 418 tests, exit 0), `eslint .` reports 0 errors, and `svelte-check` reports 0 errors.
