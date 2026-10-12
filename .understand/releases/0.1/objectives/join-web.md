# Joining the halves in 0.1: web

Lead: Fennec · 2026-10-09 · Under the Engineer's ruling that 0.1 joins sign-in and My Den, with the pre-filled option of joining without Character storage.

**In short:** every stack now serves My Den, so the web loses more than it gains. The switch that hides My Den is deleted, the web's planned dev-profile flag isn't needed, and the live run gains a second walk on `just dev` after a real Bluesky sign-in. That adds one S package; the wiring stays M.

## 1. New or changed work packages

| Package | What it is | Size | Depends on | Lands in | Security review |
| --- | --- | --- | --- | --- | --- |
| **My Den on every stack** (new slice inside the wiring to den.proto) | Delete slice 1b's switch and its no-Den frame. The sidebar, tree, phone drawer, "My Den" and "Open My Den" links show on every signed-in stack, and a typed `/den` reads the Den instead of answering the ordinary not-found. The "Den not connected" notice goes too. Most of the ~20 files lose a branch and its tests. It replaces the dev-profile flag, so the wiring's size holds. | S | the wiring's live decode; the den API routes mounted on every profile | a `feature/my-den` slice, right after the live decode | **Yes**: real sessions and real data reach the Den page. Reviewed together with the wiring |
| **The wiring to den.proto**, changed | The web's dev-profile flag is dropped (above). The planned cap on parallel reads now also guards the PostgreSQL pool: a deep page load makes up to 17 Den reads at once, and the pool has 5 connections with a 5 s wait (`adapter-pg/src/lib.rs`). A Den read borrows a connection per query, not for the whole read (den lead). The cap is 4 reads in flight per page. | M, unchanged | as planned | `feature/my-den` | **Yes**: Heron's review was a proposal; it now applies, because the decoder handles real users' data |
| **Live run on `just dev` after a real sign-in** (new, beside the dev-profile run) | Sign in with your handle and open My Den. Check that `~` lists `accounts`, `characters` (empty), `commissions` and `posts`. Found an Account and see it under `accounts`. Delete it and see it only with the toggle, marked "Deactivated". Check that `/den/nope` is "Nothing here", and run the leak checks against your real DID and handle. Fix what the run finds. | S | the slice above; identity's root for real users and the PostgreSQL arms; the follow-up for reads that bypass the Den; sign-in on `main` (WP-5), then `feature/my-den` restacked onto it | fixes as `feature/my-den` slices; the walk in Crow's check file | The walk, no. A fix that touches sessions or data, yes |
| **Live run on the dev profile** (WP-11), unchanged | alice and bob on the fixtures, plus the contrast test. | S | as planned | as planned | No |

**For Kestrel, PDD lines that go:** each "where My Den isn't served" clause (in the one-frame, phone and screen-reader stories, and in the phone top-bar and landing-page choices), the fourth wireframe, and the success-criterion check on screens without a backend (see question 1).

## 2. What existing packages already cover

- **The wiring as planned:** the live port, the decoder, the tolerant reader, the path builder and the golden-response tests. If den also commits a PostgreSQL set of golden responses, the same spec reads one more folder.
- **Built and the same on every stack:**
  - every leak guard: no caching, same-origin referrers, the framing refusal, route-only logs, nothing in browser storage, generic page titles, one cookie, and your own avatar as the one outside request;
  - the Accounts pages in the frame, and sign-out with the API down;
  - the dark style, the keyboard and the phone layout.
- **A real user's root name** (from their PDS) needs no web work: the web shows `~` and "My Den", never the root's name.
- **The commission reads that bypass the Den** have no web side, because the web calls no commission endpoint.
- **The account list and the session read** (`GET /accounts`, `GET /me`): identity rewires them through the Den's check, and their wire shapes don't change (den lead's plan). The Accounts pages and the top bar stay as built.

## 3. Risks and questions

1. **Does "a mock-only My Den on `main`, in a dev build only" (D-24) still stand?** *Proposed:* yes, with one wording fix. Slice 1b keeps `main`'s `just dev` free of a Den between the shell's merge and My Den's, and the branch that serves the real Den deletes it. But "outside a dev build" would exempt `just dev` itself, which runs `yarn dev`, a dev build. *Proposed wording:* "`main` shows no screen without a backend capability behind it, outside mock mode (`just dev-mock`), which refuses to start in a production build." *Checked by:* mock mode's boot-guard test, plus the `just dev` walk.
2. **What does your `characters` folder say on PostgreSQL?** *Proposed:* "This folder is empty." It is true, since nothing can be stored there yet. "Content not shown yet." would hint that something is hidden. Identity's arm sets the flag, and the web shows either one.
3. **What does the sign-off click-through become?** *Proposed:* two walks. Your own Den on `just dev` is sparse, with no commissions or characters unless made through the API. alice's Den on the dev profile still holds the commission, the Workflow and "Old sketch".
4. *Risk:* the `just dev` walk waits on other teams' joining work (sized in their joining files). A late walk delays the end-of-slice security review, not the dev-profile run.
5. *Risk, low:* whether 17 parallel reads would exhaust the pool is unverified. Each read holds a connection only per query, and the cap keeps a page to 4. The `just dev` walk opens a deep address to check.
6. *Risk, low:* the cost you accepted with the Accounts pages in the frame (D-2) now shows on real data: the Accounts page and the tree could disagree about an Account. Both hide a deleted one by default today, and identity's rewiring of the account list through the Den's check should align the rest.
7. *For the tech lead:* the den lead proposes that the den routes mount on every profile from their first slice, with the follow-up for reads that bypass the Den merged before the end-of-slice security review. If the tech lead instead keeps the den routes dev-only until that follow-up lands, the web keeps its dev-profile flag (S, as planned) until then. *Proposed:* the den lead's reading.

## 4. Revised objectives (changes only)

- **1. The frame and My Den on mock data:** its `just dev` half (no tree, a typed `/den` is "Nothing here") holds only until My Den reaches `main`. After that, objective 3 replaces it.
- **3. My Den shows the real backend through the same screens, on both backends.** Done when the dev-profile walk passes as written, and the `just dev` walk passes, signed in with your real handle (the new live-run package above). "`just dev` still hides the Den" is deleted.
- **4. Nothing leaks from the web:** its checks also run on `just dev` after a real sign-in, and the web's log shows neither your DID nor your handle.
- **2 and 5:** unchanged.
