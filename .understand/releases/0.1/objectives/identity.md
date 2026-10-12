# 0.1 objectives: identity

Stage: pre-alpha-v0.1.0 · Lead: Fox · QA: Magpie · Revised after QA (resolutions at the end).

## Objectives

1. **You sign in with your real Bluesky handle, first on the identity branch, then on `main`.** Done when, on the branch's dev stack (the merge to `main` waits for this) and again on `main`: `just dev`, sign in with your handle, and see it in the top bar; your handle is in no INFO, WARN or ERROR line of the `just dev` output; and two direct posts print the same `422` body: `curl -s -X POST -d handle=https://bsky.app http://127.0.0.1:8080/api/v1/signin > a`, the same with `handle=nobody-xyz.example.com > b`, then `cmp a b` prints nothing. `scripts/web-smoke.sh` passes too; it proves the proxy's routing, not sign-in. Serves: merging the sign-in stack, and sign-in to `main` (WP-1, WP-5).
2. **A sign-in finishes only as the account you named, in the browser you started in, and says why when it doesn't.** Done when:
   - *Browser:* `curl -s -o /dev/null -w '%{redirect_url}\n' -X POST -d handle=<your handle> http://127.0.0.1:8080/api/v1/signin`; open the printed address in your browser and approve. You land on `/login` with the different-browser message (its wording is Q3), not signed in: curl got the binding cookie, your browser didn't. (Copying the approval page to a second browser proves nothing: Bluesky's server refuses a request opened on another device.)
   - *Account,* with a second Bluesky-hosted account you aren't signed in as at bsky.social: type its handle, approve as the account you are signed in as, and see "You signed in to a different account than the handle you typed." Whether bsky.social's page lets you switch is unverified; the named test covers it either way.
   - *Cookie follow-up,* unless cut: start a sign-in, open `http://127.0.0.1:8080/signin-callback?code=x&state=y` in a second tab, then approve in the first. The sign-in succeeds; today it fails.

   Serves: the different-browser message (WP-2); clearing the binding cookie only on a match (WP-6, cuttable).
3. **Sign-in asks your PDS for your identity only, not for write access to your repo** (if you answer Q1 "yes"). Done when: the address objective 2's `curl` prints contains no `transition` (`… | grep -c transition` prints `0`), and objective 1's sign-in still works. Serves: Sign in with Bluesky, hardened, through one new small slice before sign-in reaches `main`. It restores the Publish Consent decision's "identity-only sign-in".
4. **What sign-in now is lives in the design corpus before it reaches `main`.** Done when: the corpus PR you review has a decision page for the hardened resolver, the Authenticators page names the browser-bound cookie, and the approved-crates page loses its stale `with_resolver_options` note; `just nodes get backend/crates/adapter-atproto refs` lists the new page, and `… notes` no longer mentions `.understand`. Serves: the corpus updates (WP-4: the tech lead's package, written from identity's design, QA'd by Magpie).
5. **In My Den, your user root, your Accounts and your characters appear exactly as you may see them, and no further.** Done when: on the dev profile, alice's `accounts` shows Supreme Arts and her `characters` show Ember and Kael-sona; bob's `characters` shows Abco and his `accounts` is empty; alice's address for Supreme Arts, opened as bob, shows "Nothing here" (if the user switch is cut, its test). Each identity PR names one `cargo test` command, and it passes. It runs the arm tests (a deactivated Account shown only to its Owner, tombstoned characters hidden, the name chain: display name, else a handle that checks out both ways, else the DID) and both new reads in the in-memory and the PostgreSQL adapters, with the PostgreSQL character read pinned empty and its reason documented. Serves: identity read ports and fixture actors, and the identity host arms (WP-15, WP-16).

## Where the stages change our plan

- **Which one-way doors pre-alpha opens is your call:** Q2 asks it for identity's three candidates.
- **Refusing a PDS address that isn't a bare origin (WP-3)** has no security effect and is reversible. The plan already lets it move to 0.2; if it runs late, we'd move it rather than let it hold sign-in's merge to `main`.
- **Breaking is free until the alpha** (the proto stays `v1`). Removing `Account.id`, which the Actor Addressing decision dropped and `account.proto` still carries, needs no version bump before the alpha. The TDD parks it after 0.1; when it lands is your call.
- **Both stacks run on one machine,** and cookies don't isolate by port (RFC 6265, section 8.5). The everyday stack and the dev profile can share `zurfur.sid` on `127.0.0.1`. Sent to the dev profile's owner (the tech lead; QA Owl): test both directions, since the dev profile may keep the PostgreSQL session store.
- **Rigor that stays:** every check in the sign-in security table; Heron's reviews on the follow-up slices and on both identity packages; a handle shown only when it checks out both ways; no handle in `info` logs; and the sign-in scope (Q1), because the account granting write access in 0.1 is your real one.

## Not this release

- **Production sign-in.** 0.1 uses atproto's local-development client, whose redirects are loopback addresses only (atproto OAuth spec, the "localhost" exception). A real domain needs the production client: hosted metadata, a signing key, a public redirect. It is an alpha prerequisite.
- **Rate limits on sign-in:** they come with the User Page (your ruling of 2026-10-08).
- **Sign-out deleting the stored PDS tokens:** parked until PDS writes use them. Today sign-out ends only Zurfur's session; the OAuth sessions stay stored, sealed. Under Q1 (a), new sessions carry only `atproto`, which the spec reserves for account authentication (that such a token can't write is unverified), and the old full-write sessions are cleared once after the change (`just db-reset`). Under (b) or (c), full-write tokens stay stored after sign-out, and whether sign-out should revoke them in 0.1 becomes a question.
- **Identity's follow-up for the reads that bypass the Den** (`accounts().list`, `users().me`; the My Den question sheet, the tech lead's answers): due before the Den first reads PostgreSQL, so 0.2 under the side-by-side answer, or inside 0.1 if that answer flips.
- **Real users' Den roots:** 0.2 under the side-by-side answer, with names read through the resolver.
- **Moderation's identity questions** (moderator sign-in, a suspended state, a PDS takedown worker): moderation is a non-goal; identity answers them in the moderation study, outside 0.1.
- **Nothing else to cut:** WP-3 and WP-6 are already cuttable; the rest guards security.

## Questions for the Engineer (each pre-answered)

- **Q1. Should sign-in ask only for your identity, in 0.1?** Today it asks your PDS for `atproto transition:generic` (`adapter-atproto/src/authenticator.rs` on the identity branch). The atproto spec: `transition:generic` is "equivalent to the previous App Password authorization level" and lets a client "write (create/update/delete) any repository record type"; "a client may include only the `atproto` scope if they only need account authentication" (atproto.com/specs/oauth, read 2026-10-09). The Publish Consent decision says sign-in "requests the minimal atproto scope". Nothing uses the grant: the profile read is unauthenticated, and the record writer holds its own token.
  (a) Yes: one small slice sets the scope to `atproto`, with QA and Heron's review, before sign-in reaches `main`. (b) Not in 0.1, but before the alpha. (c) Amend Publish Consent to allow full access at sign-in.
  **Proposed answer: (a).** The decision already says so, the change is one constant and its tests, and objective 1's real sign-in proves it. *Why the earlier precondition doesn't bind 0.1:* the unified-identity investigation (2026-10-07, its Q8, never answered) said the production client and the first-use scope request must come first. The local client carries its scope in its own `client_id`, so no production client is needed to change it; and nothing in 0.1 writes to your repo, so the first-use request isn't needed until the first feature that does (the Character `owner` record). Under (a), this joins the PDD's product decisions and its sign-off sentence (correction 3).
- **Q2. Are identity's two migrations and the `browser_mismatch` code still one-way doors in pre-alpha?** The TDD lists all three under your rule of 2026-10-08. **Proposed answer: no, for all three.** Only your local databases hold the migrations; before the alpha a migration can be edited and every database rebuilt with `just db-reset`. The code isn't in the proto, and only you see it. What stays either way: the check that a migration's version came from `just migrate-add` (Magpie found no collision for the index's version on any branch today).
- **Q3. What does a sign-in refused for its browser binding say?** The different-browser decision sends all three refusals to one message: no cookie, a cookie with no live sign-in, and the wrong cookie. Cookie and sign-in both expire after 10 minutes, so someone who takes 11 minutes at their PDS, in the same browser, is told the sign-in "was started in a different browser". Zurfur can't tell that case from a real second browser: both arrive with no cookie. **Proposed answer:** "This sign-in expired or was started in a different browser. Please sign in again here."

## PDD corrections

1. **"In one page", first bullet.** Overstated: "without asking Bluesky's servers". For a Bluesky-hosted account, Zurfur reads the handle file from Bluesky's host and the DID document from `plc.directory` (`resolver/limits.rs`), and you approve at Bluesky's own server. What it never asks is Bluesky's AppView. Missing: nobody has yet signed in through the hardened stack with a real account. Wording: "You type your handle. Zurfur finds your account itself, through DNS, your handle's own web address and the DID directory, and never asks Bluesky's AppView to do it for it. It refuses anything that would let a visitor aim Zurfur at an internal address, and checks that you finished as the account you named, in the browser you started in. It is built and security-reviewed, but nobody has signed in through it with a real account yet. That check, two small slices (three if sign-in asks only for your identity) and a corpus update remain before it reaches `main`."
2. **"Goals and non-goals", non-goal "Deployment."** Missing. Append: "Sign-in uses atproto's local-development client, which only works on this machine; the production client a real domain needs comes with the release that deploys."
3. **"Features", "Sign in with Bluesky, hardened"; "Decisions needed: product"; "Sign-off".** Missing: the scope. Under Q1 (a): add to US-1.1 "Sign-in asks my PDS only for my identity (the `atproto` scope), never for write access to my repo", tested by asserting the scope in the pushed request; add Q1 as its own product decision; and the sign-off sentence reads "with the answers to D-1 to D-5 and the sign-in scope". Under (b) or (c), add a non-goal instead: "*Identity-only sign-in.* Sign-in still asks your PDS for full write access to your repo, against the Publish Consent decision. Zurfur uses none of it."
4. **"What 0.1 delivers on the Release Board", row "Login / logout, and DID operations".** Ambiguous: "No DID operations" (founding an Account, which 0.1 keeps, still mints its DID, `application/src/account.rs`) and "Logout as today". Wording for the row's last two sentences: "Logout as today: it ends your Zurfur session, but your PDS tokens stay stored, sealed, until PDS writes need them deleted (parked). No new DID operations: founding an Account still mints its DID, kept local; nothing is sent to the public directory."
5. **"Features", US-1.3, first criterion.** Overstated: "revokes the tokens". The code deletes the saved session, then asks the PDS to revoke, best effort, within 10 seconds (`authenticator.rs`, `complete` and `revoke`). Wording: "…refuses the sign-in, deletes the session Zurfur saved, asks my PDS to revoke its tokens (best effort, within 10 seconds), and returns me to `/login` with …".
6. **"Features", US-3.4, first criterion; and "Success criteria", SC-5.** *Joint with the den team: the wording is to agree with Badger and Owl, since the den.proto draft carries the same sentence.* Wrong as written: "no field identifies a viewer or a root: no DID". The root's name, which the breadcrumbs carry, is your own display name, else handle, else DID; and Den paths name Accounts and characters by their DID (den.proto draft, section 1). The draft's rule is that no field *names* a viewer or a root. Wording for US-3.4: "Every Den address starts at my own root, and no address or field can name another user's Den. Answers carry Den paths, never a real path. The only name of mine an answer carries is my own root's, and only I am served it." In SC-5, replace "no field identifies a viewer or a root" with "no address or field can name another user's Den".
7. **"Features", US-1.1, fourth criterion.** "With its own field message" reads as one message per case; the form shows one, "This handle is not valid" (`frontend/web/src/lib/server/forms/handle.ts`). Wording: "…a web address, a DID or a handle with a leading `@` with a field message, before it calls the API."
8. **"Features", US-1.3, third criterion; and "Decisions needed: product", D-5.** Missing: the message also shows for a sign-in that expired in the same browser (Q3). Wording, under Q3's proposed answer: "That case, and a sign-in that expired, shows its own message: 'This sign-in expired or was started in a different browser. Please sign in again here.'" D-5's option (a) and answer take the same text.

## QA challenges

2026-10-09 · Magpie (identity QA). Code read on `origin/feature/identity-resolver-6-close-out` (called "the branch" below); paths are under `backend/crates/` unless they say otherwise. Totals: 1 must-fix, 10 should-fix, 5 notes.

**QA1 · must-fix · Objective 2, "Done when": neither check can show its message as written.**
- *Different browser.* Bluesky's reference authorization server ties a pending sign-in to the first browser that opens its approval page. It refuses that sign-in in any other browser ("This request was initiated from another device") and deletes it (bluesky-social/atproto `main`, `packages/oauth/oauth-provider/src/request/request-manager.ts`, `get` and `setAuthorized`, fetched 2026-10-09). So the private window never brings a code back to Zurfur. If the server instead returns an `error`, Zurfur shows "Sign-in was cancelled at your PDS." before any browser check (`api/src/routes/session.rs:178-180`). Which of the two bsky.social does is unverified. Either way, the browser binding is never tested.
- *Different account.* The same server approves without showing a page when exactly one session signed in there matches the login hint and needs neither login nor consent (`oauth-provider.ts`, `authorize`, fetched). After objective 1 you are signed in there as the handle you would type, so the server approves as that account and nothing mismatches.
- *Change to:* (a) Run `curl -s -o /dev/null -w '%{redirect_url}\n' -X POST -d handle=<your handle> http://127.0.0.1:8080/api/v1/signin`, open the printed address in your browser, and approve. The browser never received the binding cookie, so you land on the different-browser message. This is the "no cookie" refusal, which the different-browser decision maps to the same code. (b) Type the handle of your second account, one you are *not* signed in as at that server, and on the approval page pick the account you are signed in as. Both accounts must be hosted by Bluesky, so one server handles both.

**QA2 · should-fix · Objective 1 skips the check before the merge, and nothing checks the logs.**
- The plan's first milestone ("Phases and milestones", phase 0) checks *before* the merge to `main`: "sign in with a real Bluesky handle on the identity branch's dev stack". Its risk register holds the merge until that passes. The "Done when" checks only on `main`, after the merge.
- "Handles stay out of logs" (the PDD's handle-logging story; success criterion 5) has no check you can do by hand on this page.
- `scripts/web-smoke.sh` proves only Caddy's routing (the `/api/v1` strip and the callback and well-known carve-outs), not sign-in. Say so, so a pass isn't read as more than that.
- *Change:* run the check on the branch's stack first (the merge to `main` waits for it), then again on `main`. Add: "after signing in, search the `just dev` output for your handle: no line at INFO or above contains it."

**QA3 · should-fix · Question 1 contradicts the earlier record without saying why, and the PDD change it implies is incomplete.**
- The unified-identity investigation's Q8 says that "two things have to exist first: the production client metadata, and the first-use scope request", and its QA19 repeats it. Q1 says "one constant", and the code bears that out:
  - the scope is the constant `OAUTH_SCOPES` (`adapter-atproto/src/authenticator.rs:43`);
  - the local client carries its scope in its `client_id`, and the spec's default there is `atproto` (atproto.com/specs/oauth, fetched 2026-10-09);
  - jacquard's callback only exchanges the code (jacquard-oauth 0.12.0, `client.rs` `callback_core`);
  - nothing restores a stored session (no `restore` call outside tests).
  So the precondition doesn't bind 0.1. Q1 has to say so, or the Engineer reads two recommendations that disagree.
- Q1 is a product decision that sign-off covers, but PDD correction 3 adds only a criterion. Under (a):
  - Q1 needs its own entry under "Decisions needed: product", and the "Sign-off" sentence ("D-1 to D-5") needs updating;
  - correction 1's "two small slices" becomes three;
  - the new criterion needs a test (success criterion 2), for example a mock that asserts the scope in the pushed request.

**QA4 · should-fix · "Not this release", sign-out: "what stays behind can't write" holds only partly, and covers only answer (a).**
- Sign-out ends only Zurfur's own session (`session.rs:277-283`). The OAuth sessions stay in `atproto_oauth.client_session`. Sessions saved before the scope change keep `transition:generic`. The spec says a public client's session "should" last at most 2 weeks (atproto.com/specs/oauth, fetched).
- The OAuth spec never says outright that an `atproto`-only token can't write. It says that scope is "only" for account authentication. Mark "can't write" unverified, or cite the permission spec.
- Under (b) or (c), tokens that can be refreshed stay stored after sign-out with full write access to your real repo. The stage can't relax that.
- *Change:* under (a), delete the earlier sessions once (`just db-reset`). Under (b) or (c), state the residual plainly and ask whether sign-out should revoke in 0.1.

**QA5 · should-fix · Objective 2 claims to serve "clear the cookie only on a match", but no check covers it.**
- *Change:* add a check for after the identity merge: start a sign-in, open `http://127.0.0.1:8080/signin-callback?code=x&state=y` in a second tab, then approve in the first tab. Today the stray link clears the cookie (`session.rs:162-167`) and the sign-in fails. After the cookie follow-up it succeeds. Or drop the "Serves" claim.

**QA6 · should-fix · Objective 4's command reads the wrong field.**
- On the branch, `adapter-atproto/NODE.json` holds only corpus pages under `refs`. The `.understand` pointer is in `notes` ("Design record: `.understand/20261008-hardened-resolver-design.md`"). The command can't show it, so the stale note would survive.
- *Change:* "`just nodes get backend/crates/adapter-atproto refs` lists the new decision page, and `… notes` no longer mentions `.understand`." Also check the approved-crates page fix, which the corpus package includes.

**QA7 · should-fix · Objective 5: the test half can't be checked by hand, and the PostgreSQL half is missing.**
- You can't see which tests "`just test` runs". The two new reads go into both adapters. The PostgreSQL character read is the named exemption that answers empty: its doc comment says why, and a test pins it (TDD "Data"; "Calls the tech lead makes").
- *Change:* name the tests, or give one filtered `cargo test` command once they exist. Include the PostgreSQL tests for both reads.

**QA8 · should-fix · "Where the stages change", bullets 1 and 2: taking things off the one-way-door list is the Engineer's call.**
- The kinds of one-way door are the Engineer's rule of 2026-10-08. The tech lead confirmed the TDD's list ("One-way doors in 0.1"), which includes both migrations and the callback code.
- "`just db-reset` undoes them" is loose. The API re-runs every migration at boot (`api/src/main.rs`). What pre-alpha buys is that you can edit a migration and rebuild every database that exists with `just db-reset`.
- The "came from `just migrate-add`" check has no command. I checked every branch ref in this checkout: `20261009002000` exists only in the identity stack, so nothing collides today.
- *Change:* turn bullets 1 and 2 into one question, with your proposed answer.

**QA9 · should-fix · Missing from "Not this release": identity's own follow-up for the reads that bypass the Den.**
- The question sheet, "Tech-lead answers", answer 3, says: "The follow-up is owned by identity (`accounts().list`, `users().me`) and commission". It is due before the Den first reads PostgreSQL. The Engineer answered "a" to keeping those reads for now (P15).
- In 0.1 the end-of-slice security review still checks these reads. The Accounts pages sit in the frame beside My Den, and the two can disagree (D-2).
- *Change:* list it: it waits for 0.2 under the side-by-side answer. If the Engineer picks (b) or (c) on the scope call (D-1), its deadline falls inside 0.1.

**QA10 · should-fix · PDD correction 6: right finding, wrong owner.**
- Verified: Den paths carry Account and character DIDs (the PDD's own row "Nothing about the tree is kept in browser storage"). The root breadcrumb carries your own name, which can fall back to your DID (the den.proto draft, the note under the JSON example).
- But "Nothing leaks" and success criterion 5 belong to the den team. The den.proto draft has the same sentence ("No field names a viewer or a root", section 1 and the proto header), in text the Engineer approves as a one-way door.
- *Change:* agree the sentence with Badger and Owl, so the PDD and den.proto say the same thing, and mark the correction as joint.

**QA11 · should-fix · Missed PDD error: the different-browser message also shows for a sign-in that simply expired.**
- The different-browser decision (D-5) maps "all three browser refusals" to `browser_mismatch`: no cookie, no live sign-in, and the wrong cookie (`authenticator.rs:268-275`). Both the cookie and the server-side request expire after 10 minutes (`authenticator.rs:47`; `session.rs:104-111`; `get_browser_binding.sql`). So someone who spends 11 minutes on the approval page in the *same* browser is told "This sign-in was started in a different browser". That is false.
- *Change:* the wording is the Engineer's product call, so put it as a question with a proposed answer. For example: "This sign-in expired or was started in a different browser. Please sign in again here." Then US-1.3's third criterion and D-5 follow it.

**QA12 · note · Objective 3.** The grep works: today it finds only `authenticator.rs:42-43` and a test mock at `authenticator/tests.rs:78`. But it checks the source, not the request. A direct check is `curl -s -o /dev/null -w '%{redirect_url}\n' -X POST -d handle=<you> http://127.0.0.1:8080/api/v1/signin | grep -c transition`, which should print `0` (the local client's `client_id` carries the scope; jacquard-oauth 0.12.0 `atproto.rs:306`).

**QA13 · note · Stages bullet 4 (`Account.id`).** The field is being removed, not renamed (`contract/zurfur/api/v1/account.proto:25,57,69,100` on `main`). Removing it is a contract change, which is the Engineer's call. The TDD already parks it "After 0.1" ("Parked: not in 0.1"). Point there, and leave the before-the-alpha timing to the Engineer.

**QA14 · note · Stages bullet 5 (shared cookies).** Correct: RFC 6265 §8.5 says "Cookies do not provide isolation by port." (fetched). But the check belongs to the dev-profile package's owner (the tech lead; QA Owl), so send it there. Add the reverse direction too: if the dev profile keeps the PostgreSQL session store that `api/src/main.rs:35` builds, a fixture user's session lands where the everyday stack reads sessions. Test both directions.

**QA15 · note · PDD correction 4 says "Wrong"; it is only ambiguous.** The column is "What 0.1 gives it", and 0.1 gives no DID operation: founding an Account minted DIDs before 0.1. Your wording is accurate (`ZURFUR_PLC_DIRECTORY_SUBMIT=false`, `.env.example:78`; sign-out ends only Zurfur's session). Relabel the correction.

**QA16 · note · Missed PDD wording: US-1.1, fourth criterion, "with its own field message".** The form shows one message for all three cases: "This handle is not valid" (`frontend/web/src/lib/server/forms/handle.ts`). Proposed: "…with a field message, before it calls the API."

*Checked and found accurate:* objective 1's commands (Caddy strips `/api/v1` before `/signin`; the field is `handle`; curl sends no `Origin`, so the CSRF check passes; the `422` body has no per-request field); Q1's spec quotes and "nothing uses the grant" (`public_records.rs` holds its own token); PDD correction 1's section title ("In one page") and its `plc.directory` citation (`resolver/limits.rs:12`); corrections 2 and 5 (a mismatched account's session is deleted first, then revoked within 10 s: `authenticator.rs:205-207`, `:77`).

## QA resolutions

2026-10-09 · Fox. All 16 accepted; none rejected. I re-checked the ones that turn on facts before changing anything: Bluesky's `request-manager.ts` (`get` and `setAuthorized` refuse "This request was initiated from another device" and delete the request; fetched 2026-10-09), `check_browser` on the branch (`authenticator.rs`: no cookie, no live sign-in, and the wrong cookie are three refusals), the question sheet's tech-lead answer 3 (identity owns `accounts().list` and `users().me`), and `adapter-atproto/NODE.json` on the branch (the `.understand` pointer is under `notes`). The atproto permission spec (fetched) says nothing about what `atproto` alone grants, so "can't write" stays marked unverified.

| QA | Resolution |
|---|---|
| QA1 | **Accepted.** Objective 2 rewritten: the browser check starts the sign-in with `curl`, so the browser lacks the cookie; the account check uses a second account you aren't signed in as, with the page's behaviour marked unverified and the named test as the fallback. |
| QA2 | **Accepted.** Objective 1 now runs on the identity branch first (the merge waits for it), then on `main`; it adds the log check and says `web-smoke.sh` proves routing only. |
| QA3 | **Accepted.** Q1 says why the investigation's precondition doesn't bind 0.1. Correction 1 says three slices under Q1's yes; correction 3 adds Q1 as a product decision, the sign-off sentence and the test. |
| QA4 | **Accepted.** The sign-out bullet states what stays stored, clears the old full-write sessions once under (a), marks "can't write" unverified, and says the residual under (b) or (c). |
| QA5 | **Accepted.** Objective 2 gains the stray-callback check for the cookie follow-up. |
| QA6 | **Accepted.** Objective 4 checks `refs` for the new page and `notes` for the stale pointer, and adds the approved-crates fix. |
| QA7 | **Accepted.** Objective 5: each PR names one `cargo test` command; it covers both adapters, the empty PostgreSQL character read included. |
| QA8 | **Accepted.** Bullets 1 and 2 became Q2, pre-answered; "undoes them" now reads "edited, and every database rebuilt". |
| QA9 | **Accepted.** "Not this release" lists identity's follow-up for the reads that bypass the Den. |
| QA10 | **Accepted.** Correction 6 is marked joint; I've written to Badger and Owl to agree one sentence for the PDD and den.proto. |
| QA11 | **Accepted.** New Q3 (the message's wording) and correction 8. |
| QA12 | **Accepted.** Objective 3 checks the address the sign-in actually sends, not the source. |
| QA13 | **Accepted.** "Removing", not "renaming"; it points to the TDD's parking and leaves the timing to you. |
| QA14 | **Accepted.** The cookie bullet is sent to the dev profile's owner, both directions. |
| QA15 | **Accepted.** Correction 4 relabelled "Ambiguous". |
| QA16 | **Accepted.** New correction 7. |
