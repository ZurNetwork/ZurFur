# 0.1 objectives: vulpes

Stage: pre-alpha-v0.1.0 · Lead: Reynard · QA: Rook

**No 0.1 work for Vulpes.** No work package, and Zurfur doesn't depend on it. 0.1 writes no ACP record and checks no claim. The only DID it mints comes from today's "found an Account", through Zurfur's own minter.

## Objectives
1. **No ACP record enters 0.1, fixtures included.**
   - A keeper stays Zurfur's own record. For a Private character it stays that way at every stage (the Characters on ATProto decision). A Public character's claims are the alpha's "Claims — character ownership" item.
   - The fixture world is the one place 0.1 could cross this line. A fixture character is Public only if the in-memory fake models its repo, and that repo's first record is an ownership claim.

   Done when, on `main`:
   - (a) `git grep -n vulpes main -- '*Cargo.toml' Cargo.lock` prints nothing;
   - (b) `git grep -n got-paws main -- backend frontend contract lexicons` prints nothing (both are true today);
   - (c) the fixture seed makes Abco, Ember and Kael-sona Private and writes nothing to the in-memory record store.

   Checked as one line each in Heron's review of the identity read ports and in the end-of-slice security review. Serves: the identity read ports and fixture actors (My Den).
2. **The resolver's decision page keeps room for the vulpes ports.** Your 2026-10-08 answer, one hardened resolver for every lookup, covers these ports too. Done when: the hardened resolver's decision page (corpus updates) keeps the design's "vulpes ports" row:
   - the ports are built later in `adapter-atproto`, over the guarded client;
   - a caller may raise the per-request body cap for status lists (vulpes allows 1 MiB per copy, 16 copies at most);
   - the `at://` path is settled when the port is built.

   Serves: "Sign in with Bluesky, hardened".

## Where the stages change our plan
- **Nothing changes for Vulpes.** 0.1 uses none of it.
- **Name clashes.** Vulpes's tag `v0.1.0` and its roadmap's "ACP v0.1" are not Zurfur's `pre-alpha-v0.1.0`.
- **Stays strict.** Objective 1 guards DID correlation and the public/private boundary at every stage.

## Not this release
- **Vulpes code:**
  - the pure-library ruling's three code cuts (committed, on no remote);
  - the claim record's catch-up;
  - PR #33, the pure-library write-up.

  The cuts land before Zurfur takes vulpes as a dependency.
- **Zurfur consuming the ACP:** the port implementations, the PDS test bed, the first attestor, the ownership claims.
- **Minting through vulpes** (the alpha's "DID operations"). Its minter lacks a handle-less mint, an owner key made on the user's device and an "attach a PDS" update, and may need a PDS key role (unified-identity study, "Out of scope").
- **Zurfur's half of the 2026-10-08 identity split.** Zurfur signs in and keeps the private list of linked accounts. Until a slice links an outside account, it lives in the unified-identity study, question 6.

## Questions (each pre-answered)
- **For you: does vulpes code pause until release sign-off?** *Proposed: yes.* Kit and Tod idle. The three cut branches are pushed, with no PR, so they aren't on one disk only.
- **For you: does PR #30 join your queue in 0.1?** *Proposed: yes, below every 0.1 item, once rebased.* The Characters on ATProto decision names vulpes's `characters-atproto.md` as its "source of record". On vulpes `main`, that file still describes co-owner claims, which the 2026-09-12 rulings removed.
- **For the tech lead: does the in-memory fake model character repos in 0.1?** *Proposed: no, so every fixture character is Private (objective 1).*

## PDD corrections
I read `pdd.typ` in full; it never names Vulpes.
- **Endorse identity's corrections 1 and 4.** They fix the summary's "without asking Bluesky's servers" and the Release Board's "No DID operations", which my draft also raised.
- **"What 0.1 delivers on the Release Board", row "View grants & visibility".** "Character keepers" can read as the alpha's ownership claims. *Proposed wording:* "The read side of the visibility rules for you, Account members, character keepers (Zurfur's own record: 0.1 writes, reads and checks no ownership claim) and commission Participants. No grants."

## QA challenges

Rook, 2026-10-09. Two must-fix, eight should-fix, four nits. "No 0.1 work for Vulpes" holds. Neither objective needs a new domain decision, except the fixture call in QA2.

**Checked and true** (read-only, 2026-10-09):
- Objective 1, Done when (a): run in `~/code/zurfur`, it printed nothing (exit 1). `git grep -il -E 'vulpes|got-paws'` over the same paths also printed nothing on `main`, `origin/feature/identity-resolver-6-close-out`, `origin/feature/identity-resolver-7-browser-binding` and `origin/feature/web-explorer-shell-2c-follow-ups`.
- `send_capped` is crate-private (`adapter-atproto/src/guarded_http/client.rs:50` on the close-out branch). Its 1 MiB test is `a_raised_cap_accepts_a_body_the_default_refuses` (`client/tests.rs:202`: a 700 KiB body under a 1 MiB cap), and the 512 KiB default is in `limits.rs:18`. `git grep -n send_capped main -- backend/crates/adapter-atproto` prints nothing today, as expected before the merge.
- Vulpes `origin/main`:
  - `MAX_STATUS_LIST_BYTES = 1 << 20` (`src/acp/status.rs:231`).
  - `docs/ROADMAP.md`: the active milestone is "ACP v0.1", and line 54 still lists "The first attestor".
  - Tag `v0.1.0` is annotated "vulpes v0.1.0 — renamed from zurid" (2026-08-10), at Cargo version 0.1.0.
- PRs #33 and #30 are open. #30 edits `NODE.md` files that `origin/main` replaced with `NODE.json`, so it does need the rebase. The three code cuts are committed and stacked (`feature/cut-shells`, then `feature/cut-postgres`, then `feature/injected-rng`), and are on no remote.
- The one-edge ruling is real: the Characters on ATProto decision, 2026-09-12 panel, change (1). The one-resolver answer does name the vulpes ports: the unified-identity study, question 4, option (a). The Delivery Plan has no vulpes work package. The PDD, TDD and plan never name vulpes, the ACP or claims.

**QA1 · must-fix · Opening paragraph: "0.1 mints no DID".**
- Evidence: the PDD keeps today's Account pages ("found an Account, delete one"; "Goals and non-goals"). The code:
  - Founding an Account mints its did:plc through Zurfur's own minter (`backend/crates/application/src/account/create.rs:45`).
  - Deleting an Account tombstones its DID (`account/delete.rs:67`).
  - The everyday stack wires the real minter (`composition/src/runtime.rs:129`). Sending to a directory is off by default (`composition/src/config.rs:80-84`; `.env.example`: `ZURFUR_PLC_DIRECTORY_SUBMIT=false`).

  The identity page's PDD correction 4 says the same. This page also misses the PDD's matching error, in the "Login / logout, and DID operations" row: "No DID operations."
- Fix: "0.1 mints no DID through vulpes. Its only minting is today's 'found an Account', through Zurfur's own minter, and it stays local. 0.1 writes no ACP record and checks no claim." Under PDD corrections, endorse identity's correction 4 rather than adding a third wording.

**QA2 · must-fix · Objective 1: "No ACP record and no vulpes call enters 0.1."**
- Evidence: the draft doesn't name the one place 0.1 could cross this line.
  - TDD, "Feature 3 › Data", the fixture world: "a character is Public only if the fake models its repo." The tech lead's answer leaves it to whether the in-memory fake models the repo (My Den question sheet, "9. Tech-lead answers", item 22).
  - The Characters on ATProto decision, 2026-09-12 panel, change (6): "First repo content at publicize is the CCS `owned` half, and nothing else."
  - The in-memory record store already exists (`adapter-mem/src/public_records.rs`).

  So a faithful fake repo for a Public Abco or Ember holds an ownership claim record. That fake would be built in "the identity read ports and fixture actors", which starts when the PDD is signed.
- Fix: add to objective 1: "Every 0.1 fixture character is Private (the fake models no repo), or a modeled repo carries no ownership half." Put the choice to the tech lead as a question, with the proposed answer "all three Private". Send it to Fox on the board so identity's QA checks it. Done when: the fixture seeds every character as `Presence::Private`, and the in-memory record store holds no record for any of them.

**QA3 · should-fix · Objective 1's title, "shaped like the ownership edge that will later back them", and its first bullet, "no 0.1 security review covers that".**
- Evidence: the Characters on ATProto decision, ownership bullet: "A private Character has a DID and keys but no repo, so the claim/attestation pair only materializes at publicize — ownership of a private Character is an Index fact until then." So no claim ever backs a Private character's keeper. A claim between a User and a Private character is ruled out by the design at every stage, not merely unreviewed in 0.1. Kael-sona is Private in every version of the fixture.
- Fix: "My Den's keepers stay Zurfur's own records. A Private character's keeper is a Zurfur-only fact at every stage (the Characters on ATProto decision). A Public character's claim pair belongs to the alpha's 'Claims — character ownership' item, which has its own security review."

**QA4 · should-fix · Objective 1: "Each Character has exactly one keeper, and it is a User", and Done when (b) and (c).**
- Evidence:
  - The Character page's summary row says "one rendered Keeper — the User named by the claims edge; administratively a User (single Character) or an Account (shared Character)". It keeps "co-Keepership = priority-ordered rotation keys ONLY (administrative lane)". Its own prose still says "has one or more Keepers" and "Co-Keepership is native in the target model", so the corpus isn't uniform either.
  - (b) can't fail and can't be checked by hand. `Character` already holds a single `owner_id: UserId` (`domain/src/elements/character/entity.rs`), so two keepers can't be expressed. The fixture doesn't exist yet, and the check names no command or file.
  - (c) is a code change in identity's Character module that no work package owns. The same comment is stale in another way too: "**Stub.** … Only the id type exists so far", while `attributes`, `entity` and `presence` all exist.
- Fix:
  - Say "one rendered keeper, a User. A shared character is administered by an Account, which is never its keeper."
  - Drop (b), or replace it with "the `Character` type holds a single `owner_id: UserId` (true on `main`)".
  - Move (c) out of the objective. Make it a request to the identity read ports and fixture actors to rewrite the whole module comment to match the Character page. Add a note for the Orchestrator that the Character page's prose needs a corpus sync.

**QA5 · should-fix · Objective 1, Done when (a): `rg -il 'vulpes|got-paws' Cargo.toml Cargo.lock backend frontend contract`.**
- Evidence: it prints nothing today, but it has three gaps:
  - `rg` reads whatever change is checked out, not `main`.
  - It leaves out the root `lexicons/` folder, where a record schema would land. Wolven's page includes that folder.
  - `-i 'vulpes'` matches prose. A doc comment or `NODE.json` note saying that `send_capped` exists for vulpes status lists (objective 2's own subject) would fail the check, with no dependency added.
- Fix: check the two things meant, on `main`. `git grep -n vulpes main -- '*Cargo.toml' Cargo.lock` checks that there's no dependency. `git grep -n got-paws main -- backend frontend contract lexicons` checks that there's no ACP record name. Both print nothing today (exit 1).

**QA6 · should-fix · Objective 2, Done when (b): "`git grep -n send_capped main -- backend/crates/adapter-atproto` still finds it."**
- Evidence:
  - It checks a name. It passes if the cap is hard-coded under the same name, and fails on a harmless rename.
  - The function has one caller: `send_http`, with the 512 KiB default (`client.rs:106`). The resolver design's own review check 8 records "`send_capped` has one caller", so a later clean-up may well inline it.
  - Your "Where the stages change our plan" calls both objectives cheap to undo in code. Restoring a crate-private function takes minutes.
- Fix: check the behaviour: `git grep -n a_raised_cap_accepts_a_body_the_default_refuses main -- backend/crates/adapter-atproto`. Better, drop (b) and keep (a): the corpus page is the door that matters. Consider making objective 2 a one-line request to the author of the corpus updates (the tech lead, QA Magpie) rather than one of your two objectives.

**QA7 · should-fix · Objective 2, Done when (a): "status lists capped at 1 MiB per call".**
- Evidence: vulpes's limit is per copy, not per fetch.
  - `MAX_STATUS_LIST_BYTES = 1 << 20` (`src/acp/status.rs:231`) sits beside `MAX_STATUS_COPIES = 16` (`:238`).
  - The status port "Returns every copy it can find". An implementation SHOULD "cap the response at `MAX_STATUS_LIST_BYTES` and bound the copies it returns" (`src/acp/ports.rs`, the status port's doc).
  - A list address may also be `at://<did>/…`, resolved "through the repo path" (`src/acp/record.rs:476-494`). In Zurfur that read would go over XRPC, which calls `send_http` at the 512 KiB default (`client.rs:98-107`), not `send_capped`.
  - Unverified: how such a list travels over XRPC, and so its size on the wire.

  A corpus page that says "per call" will be read as per port call.
- Fix: word the row as "each status-list copy capped at vulpes's per-copy limit (1 MiB today), at most 16 copies per fetch; how the `at://` path gets the same cap is settled when the port is built". Or keep numbers out of the page: "a per-request cap the caller can raise, for status lists".

**QA8 · should-fix · "Where the stages change our plan": "Checks, not gates … No vulpes review is added to any work package", beside "What stays strict".**
- Evidence: the ASK says "Security and data-boundary rigor never relaxes with the stage." Objective 1's no-record rule is that kind of rigor, but no reviewer carries it:
  - In the Delivery Plan's "Review gates", Heron reviews the identity read ports for "the read that pairs a user with their characters".
  - The end-of-slice security review runs "the question sheet's 19 findings as the checklist".

  As drafted, the rule is only a hand check at sign-off.
- Fix: ask the tech lead to add one line to Heron's review of the identity read ports and to the end-of-slice checklist: "no vulpes dependency, no ACP record name, no fixture character with an ownership half." That adds a line to reviews already planned, not a vulpes review.

**QA9 · should-fix · Question: "Does vulpes's own work pause until 0.1 is signed off, PRs #33 and #30 included? Proposed answer: yes … Nothing in 0.1 reads either."**
- Evidence:
  - "Signed off" is ambiguous. The Engineer signs the PDD once to approve scope (Delivery Plan, "What blocks the start", row 6, before Phase 2) and again at release sign-off. The two are weeks apart.
  - Something in 0.1 may read #30's file. The Characters on ATProto decision names vulpes `docs/characters-atproto.md` as its "source of record", and settles disagreements with it only on timing. On vulpes `origin/main` that file still says "Co-ownership = multiple priority-ordered rotation keys + an `ownership` claim from each co-owner". That is the many-keeper shape objective 1 rules out, and whoever builds the identity read ports and follows the link reads it.
  - The pause has no exit condition, and the three code cuts exist on one disk only. On `origin/main`, vulpes's default build still carries its own reqwest client (`Cargo.toml`: `default = ["minter", "directory"]`, `directory = ["dep:reqwest", …]`). The shells cut removes it.
- Fix: split the question.
  - (a) Vulpes code pauses until release sign-off: yes.
  - (b) PR #30, rebased, goes into the Engineer's queue below every 0.1 item, as a short docs read, because it backs objective 1. PR #33 can wait.

  Under "Not this release", add the exit condition: "the three cuts land before Zurfur takes vulpes as a dependency". Add a pre-answered question: "Push the three cut branches (no PR) so the pause doesn't rest on one disk? Proposed: yes."

**QA10 · should-fix · PDD corrections, second bullet: "Summary, first bullet: 'without asking Bluesky's servers' overstates."**
- Evidence: the identity page's PDD correction 1 corrects the same sentence, with fuller wording: it also says nobody has yet signed in through the hardened stack with a real account. Two wordings for one sentence would reach the Orchestrator's summary, and the draft itself says the section is identity's.
- Fix: replace the bullet with "Endorse identity's correction 1 (the summary's first bullet) and correction 4 (the 'Login / logout, and DID operations' row)."

**QA11 · nit · PDD corrections, first bullet: a new Release Board row for "Claims — character ownership".**
- Evidence: the table lists the alpha items 0.1 touches, then "All other alpha items — Nothing — None", which already covers this one. The misreading you fear starts at the words "character keepers" in the "View grants & visibility" row. Also, "My Den lists your characters from Zurfur's own records" holds only on the dev profile, from in-memory fixtures: under the side-by-side scope call, the everyday stack serves no My Den.
- Fix: amend that row instead: "…for you, Account members, character keepers (Zurfur's own record; 0.1 writes, reads and checks no ownership claim) and commission Participants."

**QA12 · nit · "Not this release": "Vulpes's minter lacks a handle-less mint and an input for an owner key made on the user's device."**
- Evidence: true as far as it goes. On vulpes `origin/main`, `src/minter.rs:318` has only `mint(&self, handle: &Handle)`. The unified-identity study ("Out of scope, found on the way") lists more gaps: an "attach a PDS" update operation, which vulpes's strict guard refuses today, and perhaps a PDS key role. Zurfur's own minter already has a handle-less mint (`adapter-atproto/src/did_minter.rs:131`).
- Fix: "…lacks at least three things, and perhaps a fourth (unified-identity study, 'Out of scope'): a handle-less mint, an input for an owner key made on the user's device, an 'attach a PDS' update, and perhaps a PDS key role."

**QA13 · nit · "Not this release": "Recording the 2026-10-08 identity split on Zurfur's side … Parked until the first slice that links an outside account."**
- Evidence: the ruling ("Vulpes is a LIBRARY ONLY"; Zurfur keeps the private list and sign-in) lives in the unified-identity study, question 6, and in the unmerged PR #33. The corpus records only half of it: vulpes is "a stateless crate that computes claim halves and verifies public reads" (the Characters on ATProto decision, the kill-test paragraph). Zurfur's half, the private list of linked accounts and sign-in, is unrecorded. Meanwhile, the corpus updates already record the resolver answer from the same study and the same day.
- Fix: keep it parked if you like, but say where it lives meanwhile. Add a pre-answered question: "Fold one paragraph on the split into the hardened resolver's decision page? Proposed: yes."

**QA14 · nit · Plain words and length.**
- Evidence: the PDD corrections cite "US-3.1" and "US-1.1", but the ASK's rule is "Plain words. No bare ids." The page has 937 words above the QA sections, and the ASK asks for one screen.
- Fix: name the stories ("Open My Den", "Sign in with my handle"). Trim the opening paragraph to its first two sentences, and objective 2's preamble to one.

## QA resolutions

Reynard, 2026-10-09. I checked each challenge's evidence before answering. 11 accepted, 3 accepted in part, none declined outright.

- **QA1 · accepted.** I checked `application/src/account/create.rs`: "found an Account" calls `ports.did_minter.mint`. The opening now says 0.1's only DID minting is that one, through Zurfur's own minter. The PDD corrections endorse identity's correction 4 instead of rewording it.
- **QA2 · accepted, in part.** Objective 1 now names the fixture world, and its Done when (c) checks the seed. The fixture call goes to the tech lead as a question, with "every fixture character Private" as the proposed answer. I checked the source: the tech lead's rule (question sheet, section 9, item 22, and the approved architecture's item 24). I sent Fox a note on the board so identity hears this before the identity read ports start. I declined the other fix, "a modeled repo with no ownership half": the Characters on ATProto decision makes the `owned` half the first content of a public repo, so that fake would be unfaithful.
- **QA3 · accepted.** I dropped "shaped like the ownership edge that will later back them". Objective 1 now says a Private character's keeper stays Zurfur's record at every stage, and it puts a Public character's claims under the alpha's claims item. I checked both against the decision's ownership bullet.
- **QA4 · accepted.** I dropped the one-keeper rule from objective 1. On `main`, `Character` holds a single `owner_id: UserId`, so 0.1 can't express two keepers, and that check could never fail. Two items move out of the page:
  - The stale module comment ("one or more Keepers", "Stub") goes to identity as a request, in the same board note.
  - The Character page's mixed prose ("has one or more Keepers"; "Co-Keepership is native in the target model") goes to the Orchestrator for a corpus sync.
- **QA5 · accepted.** Done when (a) and (b) are now your two `git grep` checks on `main`, with `lexicons` included. I ran both: no output, exit 1. The `'*Cargo.toml'` pathspec does reach nested crates: it finds `jacquard` in `adapter-atproto/Cargo.toml`.
- **QA6 · accepted, in part.** I dropped the `send_capped` name check. Objective 2 stays an objective, now checked only on the page. That page is written in 0.1 and records your 2026-10-08 answer, which named these ports. That's the "decision that must not be closed off" the ASK asks for.
- **QA7 · accepted.** The row now says a caller may raise the per-request cap for status lists. It quotes vulpes's limits as per copy (1 MiB) and per fetch (16 copies), and leaves the `at://` path to the port's build.
- **QA8 · accepted.** Objective 1 now adds one line to Heron's review of the identity read ports and one to the end-of-slice checklist. "Checks, not gates" is gone.
- **QA9 · accepted.** I split the question in two:
  - Code pauses until release sign-off, and the cut branches get pushed.
  - PR #30 joins the queue, rebased, below every 0.1 item.

  The exit condition sits under "Not this release". I named release sign-off, not the PDD's signing.
- **QA10 · accepted.** The PDD corrections now endorse identity's corrections 1 and 4 instead of a second wording.
- **QA11 · accepted.** The correction now amends the "View grants & visibility" row instead of adding a row. The amended text says nothing about which stack serves My Den, so the dev-profile point doesn't bite.
- **QA12 · accepted.** The minter bullet lists all four gaps, the fourth as "may need".
- **QA13 · accepted, in part.** "Not this release" now says where Zurfur's half lives meantime. I didn't make it a question. The split's home is the sign-in decision, Authenticators, which identity owns, and nothing in 0.1 builds on it. I pass the suggestion to the Orchestrator for identity and the tech lead.
- **QA14 · accepted.** No story ids remain. The page above the QA sections went from 937 words to about 680.
