# 0.1 joined: identity's share

2026-10-09 · Fox (identity lead). Sized for your ruling that 0.1 joins the halves, on the pre-filled option (b): no character storage. Seams agreed on the board with Badger (den); Fennec's (web) assumptions about identity hold. Sizes use the plan's units: S = 1, M = 2, L = 3.

## 1. New and changed packages, under (b)

- **Sign-in asks only for `atproto`** (S, new; your ruling today). One constant, a test that the sign-in address carries no `transition`, and the stored sessions cleared once (question 2). *Needs:* the sign-in stack merged (WP-1). *Lands:* a slice on the identity branch, before sign-in reaches `main`. *Security review:* yes.
- **Your real root** (M, new). On the everyday stack, the user arm names your root from your own PDS, through the hardened resolver: display name, else a handle that checks out both ways, else your DID. It reads the cache first, keeps one lookup per DID in flight, and falls back to your DID after a short wait, so an outage never stalls My Den. *Needs:* the identity arms (WP-16); sign-in on `main` (WP-5), then `feature/my-den` taking `main`. The profile ports are already in the application's ports (`application/src/app.rs`), so the den Query stays blind to profiles (agreed with Badger). *Lands:* an identity slice on `feature/my-den`. *Security review:* yes.
- **Your real Accounts on PostgreSQL** (M, changed: identity read ports WP-15, arms WP-16). No new read is needed (section 3). New: a PostgreSQL seeder (alice, bob, one Account, with DIDs agreed with commission's Raven) for den's PostgreSQL test harness, and identity's cases on that harness. A deactivated Account shows only to its Owner. A stranger's Account and a missing one trigger the same store calls: the deterministic half of the timing re-test. No Den answer names a User other than the caller: the cross-persona guard, extended to the Den. A stored name the Den refuses falls back (question 1). *Needs:* WP-15, WP-16, den's harness. *Lands:* an identity slice on `feature/my-den`. *Security review:* yes.
- **The reads that bypass the Den, identity's half** (S, moved into 0.1). The Accounts list keeps only the Accounts that den's new "may this viewer open it" check opens; `/me` asks the same check about your own root. Wire shapes stay unchanged, so the web needs nothing. *Needs:* den's check, your real root. *Lands:* an identity slice on `feature/my-den`, merged before the end-of-slice security review starts. *Security review:* yes, and it is on that review's checklist.

**Total:** 5 units for joining, plus 1 for the scope slice. In the plan's model they run in units 7 to 10, beside the arms, so the critical path doesn't grow. Sign-in's slack drops from 9 units to about 5, because sign-in must now be on `main` (and in `feature/my-den`) before the joined live run, not just before sign-off.

## 2. Option (c), sized apart: characters in PostgreSQL

- **Character storage** (M): a `character` table, keyed by the Character's DID under the actor table, holding its keeper, its name, Private only, and a tombstone time; PostgreSQL create (`unimplemented!` today), find and keeper reads. It replaces the empty answer and its pinning test.
- **A way to make one** (M to L): without it, (c) changes nothing you see, because nothing in 0.1 creates a character. The use case exists (`application/src/character/create.rs`). Missing: a route, a contract message (your review), and a command-line command or a form, in a release where My Den is read-only.
- **(c) costs 4 to 5 units more than (b), and opens two forks:** a write in a read-only release, and the keys. A Character minted today gets the old custodial key list (`adapter-atproto/src/did_minter.rs`: cold key, then operational key). That is not the owner-first list with a key you hold, which Characters on ATProto rules. It's harmless while nothing goes to the public directory, but every such Character would need re-minting before the alpha.

## 3. Already covered ("both adapters, no stubs")

- Your `accounts` folder lists through today's own-view membership query, which already runs on PostgreSQL (`adapter-pg/queries/account/list_for_user.sql`). The read ports package writes the Owner-only deactivated read and the keeper read in both adapters; the PostgreSQL keeper read answers empty, named as an exemption.
- Users and the profile cache are PostgreSQL tables today. The hardened profile read already shows an unconfirmed handle as the DID (identity branch, `adapter-atproto/src/profile.rs`). The arms call only ports, so one codebase serves both stacks.

## 4. Risks, then questions

- **An outage could stall My Den.** Every Den answer's first breadcrumb carries the root's name; the web runs up to 17 Den reads per page; failed lookups aren't cached, by design; and one port call may take 20 s. *Answer:* the short wait and DID fallback in "Your real root", with a test whose PDS never answers.
- **`main`'s profile read lacks the guards.** It still uses jacquard's own client: bare reqwest, a Bluesky fallback, the DID document's id unchecked (the hardened-resolver design, its `me` row). *Answer:* "Your real root" goes live only after sign-in's merge. Restacking `feature/my-den` is yours to name.
- **Correlation.** Zurfur resolves only your own DID, taken from the session and never from a path. 0.1 resolves no other user's name: there's no participants entry and no roster. Zurfur's server makes the fetch, so your PDS host learns that Zurfur looked you up, not where you are. *Answer:* the cross-persona guard, extended to the Den.
- **Two stacks, one cookie.** Both stacks now serve My Den on 127.0.0.1, and cookies don't isolate by port (RFC 6265, section 8.5). If the dev profile keeps the PostgreSQL session store, each stack accepts the other's sessions. *Answer (proposed to the dev profile's owner):* the dev profile uses the in-memory store.
- **Heron's queue** gains four reviews, three of them around units 9 to 10, next to the arms'.

**Q1. What does an Account show when its stored name breaks the Den's name rules?** `AccountName` checks only for an empty name and the length (`domain/src/elements/account/value.rs`), so a real Account's name can hold control or format characters, bidirectional overrides included. *Proposed:* its handle, never a cleaned-up name, and the read goes on; from now on, new Account names refuse the same characters. Den's question 5 proposes "Unnamed" for every name that fails; for an Account, the handle is always valid and tells you which Account it is.
**Q2. How are the old full-write sign-in sessions cleared?** `just db-reset` would now also wipe your real Accounts, which your Den shows. *Proposed:* the scope slice deletes only the stored sign-in sessions. Nothing reads them back (no `restore` outside tests, per Magpie's check).
**Q3. (b) or (c)?** *Proposed:* (b). (c) adds 4 to 5 units and two forks, for a folder that only a new create surface could fill.

## 5. Revised objectives

- **Objective 5 extends to the everyday stack.** Also done when: after your real sign-in on `just dev`, `/den` opens your root; `accounts` lists the same Accounts as the Accounts page, and a deactivated one of yours appears only with the toggle on; `characters` shows the web's empty-folder text. The named `cargo test` commands add the PostgreSQL cases: identity's harness cases, the cross-persona guard over the Den, and a root name that falls back to your DID, within the short wait, when your PDS never answers.
- **New objective 6: the reads that bypass the Den go through it.** Done when: the Accounts list and `/me` call den's check, with a test per read in both adapters, and their answers for alice and bob are byte-identical to today's.
- "Not this release" loses two lines: real users' roots, and the follow-up for the reads that bypass the Den.
