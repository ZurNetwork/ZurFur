# 0.1 objectives: wolven

Stage: pre-alpha-v0.1.0 · Lead: Lobo · QA: Shrike

**No 0.1 work for Wolven.** The Delivery Plan gives Wolven no work package, and Zurfur's code, contract and design corpus never mention it. Wolven itself has been idle since 2026-08-22. The two objectives below are doors 0.1 must leave open.

## Objectives
1. **0.1 doesn't decide whether Zurfur uses Wolven.** Nothing of Wolven's enters Zurfur in 0.1: not its crate, its record types or its lexicons, and not the parent and child kinds it wants added to the Vulpes catalog. Wolven's spec puts "convergence with Zurfur's Account / User types" outside its own scope, so whether Zurfur ever reads Wolven stays your call, made later. (My reasoning, not a checked security fact: a Wolven node record can carry a Discord or Telegram id, so reading those records is how a Zurfur User could get tied to an outside community.) Done when: at sign-off, `git -C ~/code/zurfur grep -il -E 'wolven|got-paws' main -- Cargo.toml Cargo.lock backend frontend contract lexicons` prints nothing. Today it prints nothing on `main` and on the identity branch. Serves: the Delivery Plan's work breakdown, which has no Wolven work, and den.proto's shape for Users and Accounts, where any later convergence would land.
2. **0.1 settles nothing about who vouches for outside accounts on Wolven's behalf.** On 2026-10-08 you ruled that Zurfur keeps a private link for sign-in and publishes a Zurfur-attested claim only when the user asks. The ruling is in the unified identity study, under "Where does 'this Discord account is this DID' live?". You deferred two parts: who attests for Wolven, and who may run the lookup from an outside account to a DID. 0.1 must not settle either one by accident. The only 0.1 package that writes identity design pages is "Corpus updates": it syncs the Authenticators decision and writes the hardened resolver's decision page. Done when: you review that corpus PR and find no sentence that settles either deferred part. `gh pr diff <that PR> -R ZurNetwork/zurfur-design | rg -n -i 'externalAccount|outside account|attest'` lists every sentence to read. Serves: the corpus updates in "Sign in with Bluesky, hardened".

## Where the stages change our plan
- **The plan doesn't change:** at any stage it gives Wolven no work.
- **Pre-alpha helps in one place:** stages.md says a breaking change to the proto doesn't bump `v1`. So den.proto's shape for Users and Accounts costs less to change later, if Zurfur and Wolven ever converge.
- **Nothing relaxes:** pre-alpha loosens neither objective in 0.1. Both guard DID correlation and the public/private boundary.
- **"0.1" means four things:** Zurfur's `pre-alpha-v0.1.0`, Wolven's spec ("v0.1 (draft)"), Wolven's crate version (`0.1.0`), and the Vulpes protocol spec ("v0.1 (draft)"). The summary should keep them apart.

## Not this release
- **Wolven's own work** has been idle since 2026-08-22: the spec, record-types and roles-catalog PRs, and the uncommitted tree walk. 0.1 needs none of it.
- **That work needs rework before it can merge.**
  - Wolven builds on paired relationship kinds ("hasMember" and "memberOf", "childOf" and "parentOf"). Vulpes now uses one kind per relationship (the Vulpes protocol spec, "One kind per relationship"), and it dropped its relationship lexicon on 2026-08-22 and 23.
  - Wolven's Vulpes pin was never on Vulpes `main`. It is a commit from the middle of the review of the Vulpes "ACP core" PR. Whether a clean build can still fetch it is unverified.
  - Your "Vulpes is a library only" ruling of 2026-10-08 removes the hosted Vulpes that two of Wolven's requests assume: the Vulpes issues "wolven consumes the acp" and "wolven can't mint node dids".
- **What Wolven waits on upstream:** six Vulpes issues, all opened on 2026-08-24 and tracked under "wolven consumes the acp". The Vulpes team proposes. You rule on who attests for Wolven and who may run the lookup, and, after the library-only ruling, on how Wolven mints its node DIDs.
- **Stage tags for the sibling repos:** ask once, for both Wolven and Vulpes, when either first tags a release.
- **Nothing to cut, and nothing missing:** the plan has no Wolven work, and Zurfur 0.1 has no Wolven users.

## Questions for the Engineer (each pre-answered)
- **Does Wolven resume only after 0.1 is signed off, and only once you and the Vulpes team have settled what it waits on upstream?** *Proposed answer: yes.*
- **Should the uncommitted tree walk be committed to a Wolven branch now, so the pause can't lose it?** That is Wolven's `src/tree.rs` and its line in `src/lib.rs`. *Proposed answer: yes. The Orchestrator commits it on a new branch off the roles-catalog branch.* The Delivery Plan asks you the same about the uncommitted Atta draft.

## PDD corrections
None. I read `pdd.typ` in full. It never mentions Wolven, and none of its promises touches Wolven. It names no team and no sibling repo, so a line about Wolven alone would be uneven. Teams with no 0.1 work belong in the Orchestrator's summary.

## QA challenges

Shrike, 2026-10-09. Five must-fix, seven should-fix, one for the Engineer, one fine. Every command below was run, and its output read.

**QA1. Objective 1's check reads the wrong tree and misses one way in. Should-fix.**
- *Claim:* "`rg -il wolven Cargo.toml Cargo.lock backend frontend contract lexicons` prints nothing."
- *Evidence:* I ran it in `~/code/zurfur`. It printed nothing and exited 1, so it passes. But it reads the primary checkout's working copy, which today is a detached head with local edits (`git status`: `Cargo.lock` and `backend/crates/atta` modified), not `main`. Also, Wolven's spec ("Required ACP catalog addition") wants its parent and child kinds added to vulpes's ACP catalog. There they would be named `net.got-paws.acp.…`, which the word "wolven" doesn't match.
- *Fix:* Done when `git -C ~/code/zurfur grep -il -E 'wolven|got-paws' main -- Cargo.toml Cargo.lock backend frontend contract lexicons` prints nothing after both feature branches merge. I ran it today on `main` and on `origin/feature/identity-resolver`: no output, exit 1 on both.

**QA2. Objective 1's reason has no source and names the wrong records. "Holds at every stage" reads like a ruling. Should-fix.**
- *Claim:* the action records "name a person's DID, so letting them in would add a DID-correlation surface". In "Where the stages change our plan": "objective 1 … holds at every stage".
- *Evidence:* Wolven's spec, section "Action": a ban names a `subject` DID, and that DID is already public. What could tie a Zurfur User to an outside community is elsewhere: the node record's `platform` field (a Discord or Telegram id), the membership pairs, and the lookup from an outside account to a DID. The identity study (question 5, part (e)) calls that lookup "still a correlation surface". Under "Out of scope for v0.1", the spec lists "Convergence with Zurfur's Account / User types". So whether Zurfur ever reads Wolven is an open question for the Engineer, to be decided later.
- *Fix:* Cite those spec sections, and mark the reason as reasoning, not a checked security fact. Write "pre-alpha doesn't relax this boundary in 0.1" instead of "every stage".

**QA3. Objective 2's check can pass while the door closes (false green). Must-fix.**
- *Claim:* Done when the Technical Design Document's "One-way doors in 0.1" has no row about linked accounts, and the identity study still records parts (d) and (e) as deferred.
- *Evidence:* Nobody will edit either document to record a new ruling. The study is a dated snapshot, and a new ruling lands in the corpus instead. The one-way-doors table is a drafted list, so a missing row proves nothing. The only 0.1 work that writes identity design pages is "Corpus updates" (Delivery Plan, "Work breakdown"): design-sync the Authenticators decision, and write the hardened resolver's decision page. Today the corpus holds none of the Engineer's 2026-10-08 answers about outside accounts: `rg -il 'externalAccount|external.account|outside account' ~/code/zurfur-design` prints nothing.
- *Fix:* Done when the corpus-updates PR either leaves outside-account attestation and the lookup out or records both as open, and the `rg` above still finds no ruling on them. Under "Serves", name "the corpus updates", not only sign-in.

**QA4. Objective 2 says more is open than is, and misses what closed. Must-fix.**
- *Claim:* the open questions are "who attests that an outside account belongs to a DID" and the lookup. "Serves: 'Sign in with Bluesky, hardened', the only identity work in 0.1."
- *Evidence* (identity study, section 5):
  - Question 5 is answered (c): Zurfur attests a user's outside account when the user asks to publish it. What stays open is narrower: (d) "Who attests *for Wolven*?" and (e) who may run the lookup.
  - The same day, the Engineer answered question 6: "Vulpes is a LIBRARY ONLY." That removes "a hosted vulpes" from the three options in (d). It also removes the model behind two vulpes issues filed for Wolven, "wolven consumes the acp" and "wolven can't mint node dids": "vulpes-the-application is a facilitator … It mints, attests on other applications' behalf". 0.1 didn't close this, but the Engineer should hear that one of Wolven's premises has changed.
  - The deferral holds "until attaching sources is built or Wolven needs them". Wolven filed that need on 2026-08-24, so pausing Wolven (question 1) is what keeps (d) and (e) deferred.
  - Sign-in is not the only identity work in 0.1: the identity team also builds "Identity read ports and fixture actors" and "Identity host arms" for My Den (Delivery Plan, "Work breakdown").
- *Fix:* Restate the two open questions as the study words them. Add the library-only line. Correct "the only identity work".

**QA5. Wolven is not Zurfur's moderation. Should-fix.**
- *Claim:* the opening line, "Moderation is a non-goal of the Product Design Document", and "nothing a Wolven user would expect is left out, because moderation is already a non-goal".
- *Evidence:* In the board's team config (`~/code/zurfur/.agent-board.toml`), Zurfur's moderation belongs to the moderation team: Moderator Actions, reports, scanning, kill switches, blocks. Wolven's spec opens by calling Wolven an open standard for organising communities on atproto. The PDD's non-goal is about Zurfur's moderation, and Zurfur 0.1 has no Wolven users.
- *Fix:* Give the honest reason, which the plan and the code already prove: Zurfur doesn't use Wolven. Drop the moderation argument.

**QA6. The stages section misses the one place pre-alpha helps, and the name clash is wider than one spec. Should-fix.**
- *Evidence:* stages.md: in pre-alpha "the proto stays tagged `v1`: a breaking change does not bump it". So the shape den.proto gives Users and Accounts stays changeable in 0.1, and that shape is where Wolven's spec expects any future convergence. On names: Wolven's `Cargo.toml` says `version = "0.1.0"`, its spec PR is titled "Wolven v0.1 spec + lexicons", and vulpes's protocol is also "v0.1".
- *Fix:* Add one line for each.

**QA7. The vulpes pin was never on vulpes `main`. Must-fix.**
- *Claim:* "Wolven pins a vulpes commit from 2026-08-21, and vulpes `main` has moved since."
- *Evidence:* The pin (`rev = "907a86385c06"`) is on Wolven's record-types and roles-catalog branches only; Wolven's `main` has no vulpes dependency. `gh api repos/ZurNetwork/vulpes/compare/main...<pin>` returns `"status":"diverged"`, 7 ahead and 14 behind. The pin is a mid-review commit on the branch of the vulpes PR "the ACP core", and that PR has 35 more review commits after it; it was squash-merged on 2026-08-22. The branch is deleted (`gh api …/branches/feature/vul-acp-core` returns 404), and only that PR's own ref still reaches the commit (`git ls-remote`: `refs/pull/10/head`). Whether a clean `cargo` build can still fetch it is unverified.
- *Fix:* Say that the pin is an unreviewed commit from before the ACP core merged, not an older `main`.

**QA8. "Re-pin it when Wolven resumes" understates the job: the spec needs rework. Must-fix.**
- *Evidence:* Vulpes `main` dropped the relationship path on 2026-08-22 and 23 (vulpes PRs "CCS is attestations" and "drop the relationship path"). `lexicons/net.got-paws.acp.relationship.json` is gone from vulpes `main` (`git ls-tree origin/main`). `docs/acp.md`, "One kind per relationship": "Both sides use the *same* kind and say which side they are in `role` … never an `owns`/`ownedBy` pair." Wolven's spec builds on relationship pairs (`hasMember`/`memberOf`, and `childOf`/`parentOf` as two kinds). Its roles-catalog PR adds "MemberScope for the hasMember half", and `src/tree.rs` calls parent edges "CCS relationship pairs". The library-only ruling (QA4) changes things further.
- *Fix:* Write "Wolven's spec and its record-types and roles-catalog PRs need rework against today's vulpes before they can merge." Carry this into question 1.

**QA9. The list of vulpes pieces hands the Engineer's questions to a team, and it is incomplete. Must-fix.**
- *Claim:* "the payload schema …, the ruling on the lookup, and the parent/child relationship kinds. These belong to the vulpes team."
- *Evidence:* The lookup is part (e) of the study's question 5, which the Engineer deferred to himself. Part (d), who attests for Wolven, is "a cross-product call". Vulpes has six open issues filed for Wolven on 2026-08-24 (`gh issue list -R ZurNetwork/vulpes`): a tracking issue; edges (the membership grant, the parent and partnership kinds, and whose namespace they live in); one composed check of a pair; reaching a repo (port implementations, listing a repo's claims); minting (how a second app mints, and the key layout for a collective); and the outside-account lookup.
- *Fix:* The vulpes team proposes. The Engineer rules on (d), (e) and, after the library-only ruling, on how Wolven mints. Point to the vulpes tracking issue "wolven consumes the acp" for the rest.

**QA10. Question 1: Wolven is already paused, so the real questions are different. Ask the Engineer.**
- *Evidence:* Wolven has had no commit since 2026-08-21, its three PRs haven't been updated since 2026-08-22 (`gh pr list`), and `src/tree.rs` last changed on 2026-08-23. "You are the only reviewer" overstates: QA, Heron and Copilot review every slice (Delivery Plan, "Review gates"), and stages.md says only that the Engineer alone *sees* a pre-alpha. Resuming is blocked upstream anyway (QA8, QA9). `src/tree.rs` is also untracked in Wolven's only checkout. The Delivery Plan asks the Engineer the same thing about the uncommitted Atta draft, so that "nobody discards your work by accident" ("What blocks the start", row 4).
- *Fix:* Ask instead: "Resume Wolven only once you and the vulpes team settle what QA9 lists? *Proposed answer: yes.*" Then: "Commit the uncommitted tree walk to a Wolven branch now, so the pause can't lose it? *Proposed answer: yes.*"

**QA11. Question 2 comes too early, and its answer doesn't fit Wolven. Should-fix.**
- *Evidence:* stages.md defines every stage for Zurfur the product. Alpha is "A closed group, on a real domain", with invite codes, when "The MVP is done or close to done". None of that maps onto a standard and its crate. Wolven plans no release, and the Engineer's rule of 2026-10-08 is to ask at the slice that needs the answer. Vulpes would face the same question.
- *Fix:* Park it, and ask it once for both sibling repos when either one first tags a release.

**QA12. PDD correction: "None" is the better answer. Should-fix.**
- *Claim:* nothing in the PDD is wrong for Wolven, and the non-goals bullet should gain a sentence about Wolven.
- *Evidence:* The first half holds. `grep -i -E 'wolven|vulpes|team' pdd.typ` finds only "The teams" and the tech lead's sign-off line. The addition fails on three counts:
  1. It puts Wolven under Zurfur's moderation bullet, so a reader takes Wolven for Zurfur's moderation (QA5).
  2. It calls Wolven "the separate library". Wolven's spec calls it "an open standard" with a reference crate and a "Wolven reference instance" that runs bots. "Library only" is the Engineer's ruling about vulpes, not Wolven.
  3. The PDD names no team and no sibling repo; vulpes, plugin and moderation aren't named either. One line for Wolven alone would be uneven. Teams with no 0.1 work belong in the Orchestrator's summary of the nine pages.
- *Fix:* Answer "None".

**QA13. Bare ids. Should-fix.**
- The draft uses "vulpes issue 24", "PR 1/2/3", "risks R1 and R13", "parts (d) and (e)" and "question 5". Name each in words, for example "the vulpes issue 'wolven can't resolve a platform account to a did'" and "the risks 'your den.proto review lands late' and 'security reviews pile up'".

**QA14. What I checked and found true. Fine.**
- The Delivery Plan gives Wolven no work package (`plan.typ` never mentions Wolven).
- Zurfur's code, its contract and the corpus never mention Wolven. `rg` finds the word only in the board's team config.
- "One-way doors in 0.1" has no row about outside accounts.
- The identity study records (d) and (e) as deferred on 2026-10-08.
- Wolven's three PRs date from 2026-08-21. The spec and record-types PRs pass CI; the roles-catalog PR reports no checks.
- The tree walk is uncommitted, and the spec calls itself "v0.1 (draft)".
- Vulpes's consent and custody changes of 2026-08-25 exist.
- Wolven's action kinds are ban, temp-ban, kick and warn, and each names a subject DID.

## QA resolutions

Lobo, 2026-10-09. I re-checked each must-fix before accepting it. Vulpes: the pin is not an ancestor of `origin/main`, `main` has no relationship lexicon, and the protocol spec says "One kind per relationship". The six Vulpes issues for Wolven are open. The unified identity study records the "library only" ruling. All 14 are accepted, and the draft above is revised.

- **QA1. Accepted.** The check is now `git grep` on `main` for `wolven|got-paws`, run at sign-off.
- **QA2. Accepted.** The ban-names-a-DID reason is gone. The objective now rests on the spec's "out of scope: convergence with Zurfur's Account / User types". The correlation point is marked as my reasoning and moved to the node record's outside-platform id. "Every stage" now reads "pre-alpha loosens neither objective in 0.1".
- **QA3. Accepted.** Objective 2's check now sits where a ruling could land: your review of the "Corpus updates" PR, with an `rg` over its diff that lists the sentences to read. "Serves" now names the corpus updates.
- **QA4. Accepted.** Objective 2 now starts from your ruling (Zurfur attests when the user asks) and names only the two deferred parts. "The only identity work" is gone. The library-only consequence is under "Not this release". The point that pausing Wolven is what keeps the deferral alive is folded into question 1.
- **QA5. Accepted.** The moderation argument is dropped from the opening line and from "Nothing to cut". The reason given now is that Zurfur doesn't use Wolven.
- **QA6. Accepted.** Stages now has the den.proto line ("costs less to change", without claiming it stops being a door you review) and lists all four meanings of "0.1".
- **QA7. Accepted.** The pin is now described as a commit from the middle of a Vulpes PR's review, never on `main`, and the fetch is marked unverified.
- **QA8. Accepted.** "Re-pin" is now "needs rework before it can merge", with the one-kind-per-relationship change named.
- **QA9. Accepted.** The Vulpes team proposes; you rule on who attests for Wolven, the lookup and minting. The six issues are pointed to through their tracking issue.
- **QA10. Accepted.** The questions are now (1) resume only after sign-off and the upstream settlement, and (2) commit the tree walk now. "You are the only reviewer" is gone.
- **QA11. Accepted.** The stage-tag question is parked under "Not this release", to be asked once for both sibling repos.
- **QA12. Accepted.** The PDD corrections section now says "None", with the reason.
- **QA13. Accepted.** Issue, PR and risk numbers and the study's question letters are replaced by names, here and above.
- **QA14. Noted.** No change needed.
