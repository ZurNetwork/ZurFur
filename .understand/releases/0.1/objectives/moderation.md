# 0.1 objectives: moderation

Stage: pre-alpha-v0.1.0 · Lead: Meerkat · QA: Lapwing

**0.1 holds no moderation work.** The Delivery Plan's "Work breakdown" gives moderation no package. 0.1 isn't deployed, so the only people in it are you and the fixture users. That alone makes it safe, because `main` already takes uploads and free text through the API. 0.1 needs two things from us anyway.

## Objectives

1. **den.proto and Atta's one host check leave room for take-down and hide.** At the alpha, a moderated item must look like the one not-found in the Den to everyone it is hidden from, in paths and listings. That must need no new field and no second check inside the Den.
   - *Checked 2026-10-09* against den.proto revision 2.1 (`.understand/20261007-den-proto-draft.md`) and the den lead's amendment "hiding soft-deleted nodes after access resolves" (`.understand/20261006-my-den-architecture.md`; the Technical Design Document's D-15).
     - The host's `admit` check refuses without giving a reason.
     - A refused node on a path is the one not-found, and a refused entry drops out before the page is cut.
     - So a hold can use the same check, and den.proto needs **no change**.
   - An owner-only "removed" state, if you ever want one, would be a new access case, which old clients show as a card. It would not be a soft-delete word: take-down isn't delete, and "Show archived and deactivated" would list it.
   - Hide needs the host to tell a viewer with standing from one who opens a node only through its public level. The amendment needs the same distinction for archived commissions, and leaves how to make it to Atta's read side.
   - *Proposed, not agreed:* the Gallery and the blob route aren't Den reads, so they would check holds themselves (the posts lead's board reply, 2026-10-09). Neither exists in 0.1.
   - Done when both hold:
     - the den lead's 0.1 page records that a hold goes through `admit` with no new field (it already does, under "Not this release");
     - Lapwing's review of `admit`'s inputs is on Atta's read-side PR.

     Re-checked at the alpha's den.proto review. Serves: your den.proto review, and Atta's read side.
2. **No build that anyone but you can reach goes out before moderation's floor stands, whatever its stage.** Pre-alpha has no sign-in allowlist, so any Bluesky user could upload to a pre-alpha build put on a domain. The floor:
   - the scan gate in front of every upload route;
   - a report path, which also covers the free text no scan reads (notes, markup, the linked channel);
   - your answers on database roles (today the app connects as the database superuser) and on user text and file names in changelog entries (today they stay there for good).
   - Done when: the tech lead has added the floor to the deployment checklist (the hardened-resolver design, "Deployment checklist"), and the Product Design Document's non-goal "Deployment" points there (correction 2). Serves: that checklist, which 0.1 parks until a release deploys, and two of the Release Board's "Security-first" items, CSAM scanning and a report path.

## Where the stages change our plan

- Our study set two decisions for "before any public deploy": database roles, and user text and file names in changelog entries. *Proposed (question 1):* who can reach a build sets the deadline, not its stage. 0.1 isn't deployed, so both decisions stay reversible until the first build that others can reach.
- None of the study's 20 questions is asked in 0.1. Each waits for the slice that needs it.
- Nothing relaxes. The rigor applies in full to the first build that others can reach, and to the first slice that changes an upload path. 0.1 is neither.
- den.proto stays at `v1` through breaking changes in pre-alpha, so it can still change until the alpha. We checked it now anyway, because you are reviewing the text that will ship.

## Not this release

- **The study's proposed MVP set:**
  - the scan gate and the report button;
  - Moderator Actions and their audit log;
  - take-down and the evidence store;
  - lockdown;
  - tag delete and Index suppression;
  - Blocks;
  - the ToS and legal pages.

  The Inventory Closures decision ships four of these slivers and defers the rest. Moderator Actions and lockdown come from your 2026-10-08 rulings, which aren't in the corpus yet. Blocks is an alpha item on the Release Board. While 0.1 is undeployed, none of them has anyone to protect.
- **Nothing to cut** from 0.1's plan for moderation.
- **Lead time for the alpha.** Some scanning services vet their applicants: PhotoDNA's approval goes through a third-party vetting service (Microsoft's PhotoDNA page, read 2026-10-08 and 2026-10-09). Zurfur has no legal entity (the Payments & Billing decision). Whether a vetted service needs one is unverified. So the questions about the legal entity and counsel *may* be on the alpha's critical path.

## Questions for the Engineer (each pre-answered)

1. **When is moderation's first deadline?** The question covers two things:
   - the two "Security-first" items that moderation owns, CSAM scanning and a report path, which Project MVP sequences "ahead of feature work, ASAP";
   - the study's two decisions above.

   *Proposed answer:* whichever comes first:
   - the first build that anyone but you can reach, whatever its stage;
   - for the scan gate, the first slice that adds or changes an upload path. If you take the study's proposal that an upload becomes a node only once it is scanned clear, an upload slice built before the gate would have to be rebuilt.

   0.1 is neither: it isn't deployed and adds no upload. Today's commission file upload stays unscanned until the gate lands.

## PDD corrections

I read `pdd.typ` (version 1, 2026-10-08) in full.

1. **"Goals and non-goals", the non-goal "Moderation, posts and the Gallery."** Missing: why it is safe to leave moderation out. The posts lead also rewrites this non-goal (the posts page, correction 1), so add these sentences after that wording: "0.1 needs no moderation, because it isn't deployed: the only people in it are you and the fixture users. Today's API, with no web screen, still takes two kinds of input. One is a commission file upload, served without a scan. The other is free text that other Participants read: notes, markup and the linked channel. That is harmless only while no one else can reach the build."
2. **"Goals and non-goals", the non-goal "Deployment."** Missing: moderation's floor. Proposed wording: "*Deployment.* 0.1 runs on a developer machine. The deployment checklist (the hardened-resolver design, "Deployment checklist") waits for the first build that anyone but you can reach, whatever its stage. Moderation's floor joins it (moderation's question 1): the scan gate in front of every upload route, a report path, and your answers on database roles and on user text and file names in changelog entries."
3. **"What 0.1 delivers on the Release Board", the row "Security-first: the items that cannot be escaped".** Missing: the order Project MVP sets for moderation's two items. Add to "What 0.1 gives it": "Two of the untouched items, CSAM scanning and a report path, are moderation's. Project MVP sequences them ahead of feature work. 0.1 builds My Den first, because it isn't deployed. Both are due before the first build that anyone but you can reach (moderation's question 1)."
- **The sign-in scope:** the identity lead's page covers it in full (its objective 3, question 1 and correction 3). Nothing to add.

## QA challenges

Lapwing (moderation QA), 2026-10-09. **12 challenges: 2 must-fix, 7 should-fix, 3 notes.** The headline holds. The Delivery Plan's "Work breakdown" gives moderation no work package, and I found nothing in 0.1 that closes a moderation door. But the reason given ("shows nobody's content to anyone else") is wrong, and the deadline is tied to a stage name, not to who can reach the build.

*Checked, and they hold:* the upload and download routes, with no web screen for them (`api/src/routes/commissions/mod.rs` and `frontend/web/src/routes` on `main`); the file-upload changelog entry keeps `"filename"` (`application/src/commission/files/upload.rs:102`); the dev database user `admin`, which the postgres image creates "with superuser power" (docker-library postgres docs, read 2026-10-09); "Inventory Closures" item 13; the "Security-first" bullet's wording; den.proto 2.1's fallbacks and its "own review" sentence ("Note for the security review"); PhotoDNA's "subject to approval by third-party vetting service" (Microsoft's PhotoDNA Cloud Service page, read 2026-10-09); all four PDD section titles, word for word.

1. **QA-1 · must-fix · Meerkat.** "Shows nobody's content to anyone else" is false, and four places lean on it: the opening line, the stages bullet "0.1 has neither", PDD correction 1, and your question 1. On the dev profile, bob, a Participant, sees alice's commission title and her attachments' file names (PDD, "Look inside a commission I take part in"). On `main`, the changelog route serves one Participant's notes and markup text to the others (`application/src/commission/changelog/read.rs`, Participant-only), and the download route serves the bytes. The same stages bullet says rigor applies "from the first slice that takes uploads", and `main` already takes them. What makes 0.1 safe is the third leg alone: it isn't deployed, so the only people are you and the fixture users. Rest the claim on that, everywhere. (The opening line also credits this reasoning to the PDD's non-goal, which doesn't contain it.)
2. **QA-2 · must-fix · the Engineer, through your question 1; Meerkat rewords.** The deadline is named by stage, but the risk is reach. `stages.md` says pre-alpha is "the Engineer only", not "never deployed". I found no sign-in allowlist on `main`, and invite codes arrive with the alpha (`stages.md`). So a pre-alpha build put on a domain would open the unscanned upload to any Bluesky user. Your question 1's own answer gets it right ("no build carrying it is deployed before then"). Make objective 2, the stages section and corrections 2 and 3 match: "before any build that anyone but you can reach, whatever its stage". The stages section also states that deadline as fact while question 1 asks it. Word it as the proposed answer.
3. **QA-3 · should-fix · the Engineer, through your question 1.** "Ahead of feature work" also means "before the first new upload slice". Project MVP sequences CSAM scanning "ahead of feature work, ASAP". The study's answer on where an upload waits ("no node until clear") changes how an upload creates its node, so an upload slice built before it gets rebuilt. Add to the proposed answer: whichever comes first, the first slice that adds or changes an upload path, or the first build others can reach.
4. **QA-4 · should-fix · the tech lead, then Meerkat.** Objective 2's "Done when" points at the wrong page. The first deploy will read the deployment checklist (`.understand/20261008-hardened-resolver-design.md`, "Deployment checklist", which the tech lead keeps) and the TDD's "Parked: not in 0.1" row "The deployment checklist", not 0.1's PDD. Correction 2 would also say the checklist "holds" moderation's items before anyone has added them. Make it: done when the tech lead has added them to that checklist, and the PDD line points there. Consider making the objective the boundary itself: no build with today's upload route goes where others can reach it before the scan gate stands. That is what 0.1 needs from you.
5. **QA-5 · should-fix · Meerkat.** The "one gap" in correction 1 is several. Besides the file upload, `main` takes free text that other Participants read: notes, written into the changelog (`application/src/commission/notes/attach.rs`); markup text (the `"markup"` field, `application/src/commission/markup.rs:81`); and the linked-channel pointer (raw text, no scheme allowlist). Scanning doesn't cover these; a report path does. Objective 2 and question 1 narrow the same way. The study's changelog question covers "every changelog entry that holds user text or a file name" (study, "Questions for the Engineer"), and commission titles sit in other entries too. Name the API-only write routes as a group, and use the study's scope.
6. **QA-6 · should-fix · Meerkat with the den lead.** "Nothing to change" holds for den.proto, but not yet for Atta's read side. The admit amendment leaves open how the host learns whether the viewer has standing or opens a node only through its public level: it is "settled when Atta's read side … builds the trait" (amendment, "The method, in words"). Hide needs that distinction (the study, "The first list": owner and participants still see it). So does lockdown's "Public off" ("Lockdown mode": "at Atta's resolver … public Den nodes, Listed cards"). Add to objective 1: your QA reviews admit's inputs when Atta's read side builds them. Also, `objectives/den.md` says the lockdown check "waits for the first slice with writes", but "Public off" is a read gate; settle that with the den lead. And "they will check holds themselves" reads as settled. I can't read that board reply, and `objectives/posts.md` says your questions to posts get "answers in the posts study". Say "proposed".
7. **QA-7 · should-fix · Meerkat; any owner-facing "removed" state is the Engineer's.** Drop the second later route, a moderation word in the soft-delete field. Your ruling is that take-down isn't delete (study, "Keeping the evidence"). A word in `soft_delete` would also list under `includeDeleted`, which the contract defines as "also list soft-deleted nodes the viewer may open" (den.proto draft, `GetDenRootRequest`). Keep only the new access case.
8. **QA-8 · should-fix · Meerkat; scope is the Engineer's.** "The MVP moderation set" overstates the corpus. "Inventory Closures" item 13 ships four slivers and defers the rest. Moderator Actions and lockdown come from the Engineer's 2026-10-08 rulings, which aren't in the corpus yet, and the study only proposes them for the MVP. Say "the study's proposed MVP set". (Blocks is in the alpha: Project MVP, the Release Board amendment.)
9. **QA-9 · should-fix · Meerkat with the posts lead; the tech lead for the order.** Two collisions. First, the posts page's correction 1 also rewrites the non-goal "Moderation, posts and the Gallery." Agree one text, or offer yours as sentences to add. Second, correction 3 raises the board's order for all thirteen "Security-first" items but argues only for moderation's two. That 0.1 builds features ahead of eleven untouched items is a release question for the tech lead to put to the Engineer. Keep correction 3 to your two.
10. **QA-10 · note · Meerkat.** Objective 1's "Done when" waits on an approval that may move. You write that den.proto "can still change until the alpha", and `objectives/den.md` proposes the alpha as its one-way door. Make it: done when the den lead confirms, on the admit amendment, that a hold needs no field and goes through admit. Re-check at the alpha's den.proto review.
11. **QA-11 · note · Meerkat.** Soften "are on the alpha's critical path" to "may be". The vetting is real (checked above). Whether a vetted service needs a legal entity is unverified, and the study lists Arachnid Shield as "sign up for an API key" (marked excerpt there).
12. **QA-12 · note · the identity lead; whether it is fixed in 0.1 is the Engineer's.** The sign-in scope bullet checks out. `OAUTH_SCOPES` is `"atproto transition:generic"` on `main` (`adapter-atproto/src/lib.rs:97`, not the authenticator file) and at the top of the identity stack (`adapter-atproto/src/authenticator.rs:43`, on the close-out and browser-binding slices). The spec quotes match (read 2026-10-09). The spec also says a client may use bare `atproto` alone "if they only need account authentication". Fix the file name. Identity has no objectives page yet, so send this to the identity lead and the tech lead on the board now, not only through the fold.

## QA resolutions

Meerkat, 2026-10-09. Before adopting any challenge, I checked it myself. I read:
- the commission routes on `main` (`api/src/routes/commissions/mod.rs`: files, notes, the linked channel, markup);
- `api` and `application`, for a sign-in allowlist (none);
- where the deployment checklist sits in the hardened-resolver design;
- the posts and den pages, and identity's page on the sign-in scope.

| QA | Resolution |
|---|---|
| 1 | **Adopted.** "Not deployed" is now the only reason given, in the opening line, the stages section, correction 1 and question 1. |
| 2 | **Adopted.** The deadline is now the first build that anyone but you can reach, whatever its stage. The stages section words it as the proposed answer to question 1. |
| 3 | **Adopted** into question 1's proposed answer. It applies to the scan gate only, and only if the study's "no node until scanned clear" proposal is taken. |
| 4 | **Adopted.** Objective 2 is now the boundary itself. It is done when the tech lead adds the floor to the deployment checklist and the PDD points there. |
| 5 | **Adopted.** Objective 2, correction 1 and question 1 now name the API-only free text (notes, markup, the linked channel), which the report path covers. The changelog question uses the study's scope: user text and file names. |
| 6 | **Adopted in part.** Objective 1 now names the line between standing and public reach, and adds Lapwing's review of `admit`'s inputs on Atta's read-side PR. The posts lead's reply is marked "proposed, not agreed". Lockdown is not ruled, so 0.1 gets no lockdown action. I've told the den lead on the board that "public off" is a read gate. |
| 7 | **Adopted.** Only the new access case remains. |
| 8 | **Adopted.** The list is now "the study's proposed MVP set", with its corpus sources named. |
| 9 | **Adopted.** Correction 1 now adds sentences after the posts lead's wording. Correction 3 speaks only for moderation's two items. Building ahead of the other eleven is the tech lead's to put to you. |
| 10 | **Adopted.** Folded into objective 1's "Done when". |
| 11 | **Adopted.** The line now says "may", and whether a vetted service needs a legal entity is marked unverified. |
| 12 | **Resolved without a board message.** Identity's page now carries the scope question in full, so my bullet points there. |
