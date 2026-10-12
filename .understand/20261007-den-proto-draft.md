# `den.proto`: the Den read's contract (draft for one ruling)

2026-10-07 · drafted by Fennec (web lead) · reviewed by Badger (den lead): **agreed** after two rounds (round 2 folded in the pre-code path security review, sheet §12) · for the Engineer's single ruling under P1 (a). Design only: no repo file is changed.

> **Changed 2026-10-08 after Crow's QA.** What you rule on changed:
> - The spelling question now has three options, and the recommendation is now `includeDeleted`.
> - Two new questions: what a card carries (Q-B), and DIDs against the 64-character id rule (Q-C).
> - An open answer whose content the app doesn't recognize now shows "content not shown yet", where it used to be an error.
> - Clients may show folders before files, keeping the server's order within each group, as your mockup does (from the Explorer shell's QA).
>
> Items marked "den lead to confirm" change the den side; Badger wasn't live to confirm them.
>
> *Since then (2026-10-08) the den lead has confirmed those items and revision 2, and both of the den lead's revision-2 wordings are in.*

## Revision 2 (2026-10-08, WP-25)

By Fennec, with the den lead's side in mind, so that you review the text that would ship. Only this file changed. The release's technical document numbers the questions differently: Q-A to Q-D here are its D-7 to D-10, Q-E is its D-14, and Q-F is its D-16.

- **The soft-deleted mark is now a word** (the product document's D-4, answer (a)). The yes/no `soft_deleted` becomes `string soft_delete = 3` in `DenOpen`, JSON key `softDelete`, with a word naming the soft-deleted state, today `archived` (a commission) or `deactivated` (an Account), and absent when the node isn't soft-deleted. It is a string, not an enum, because the versioning contract makes every v1 vocabulary a documented string (R8), as `role`, `outcome`, `lifecycle` and `visibility` already are. **Fallback:** a word the client doesn't know still marks the node as soft-deleted, with a generic soft-deleted label, never the unknown value itself: never as live, and never with another value's word, the same cautious direction as `own_level`'s "never `public`". The label's text is the Engineer's word (the web shell's Q4). *Where:* §2 `DenOpen`; §1, the "open" bullet and the `includeDeleted` paragraph; choice 2; a second JSON snippet in §2; the appendix row `DenOpen.soft_delete`; §5.
- **Typing the path of an archived node you could see only as a card gives the same 404 as any other miss** (Q-D). *Where:* §1 "Not found." and the `includeDeleted` paragraph; the proto's header comment; the appendix row "One 404 problem".
- **Q-A to Q-D answered in place.** Each question now has Claude's answer under it (the four answers had been stacked under Q-D), and the text no longer hedges with "spelling: Q-A" or "the recommendation". The parameters are `includeDeleted` and `pageToken`; the My Den rulings' `include-deleted` (decision 13) is corrected by a later design-sync. *Where:* §1 "What you are ruling on", "The page token" and `includeDeleted`; the appendix; §5.
- **The id rule, as proposed text** (Q-C): the amended R6 wording and the changed `session.proto` `did` comment. `account.proto` needs no change. *Where:* §2, "The id-rule amendment (Q-C)".
- **Confirmations folded in.** "Den lead to confirm" on Q-B and choices 5 to 7 now reads confirmed, with two details from that confirmation: the web reloads the first page only once, and the 400's detail never names the parameter. The appendix's "tech lead to confirm" on optional content now reads confirmed, as choice 4 already did. *Where:* §1, the card bullet, Q-B, choices 5 to 7; the appendix row on `oneof content`.
- **Checks re-run.** `buf lint` and `buf build` exit 0. The counts are unchanged (11 messages, 26 fields, 3 `oneof`s, 2 rpcs): the field changed its type and name, not its number. *Where:* §2, "Counts" and "Checked".

**Den lead:** confirmed revision 2 (2026-10-08; "Den lead confirmation, revision 2", at the end), and both of the den lead's wordings are in: §1's `includeDeleted` paragraph and the appendix row `DenOpen.soft_delete`.

**Two questions this revision raises:**

- **Q-E. The word's field name.** A published field name can't change after the alpha.
  - (a) `soft_delete` (JSON `softDelete`): a noun, like every other vocabulary field in the contract (`role`, `outcome`, `lifecycle`, `visibility`, `kind`). It names what both words are instances of: "Archiving a commission is its soft delete" (My Den rulings, decision 13), and an Account's soft delete is its deactivation (Account Deletion, decision 2).
  - (b) Keep `soft_deleted` (JSON `softDeleted`): the draft's name, but it reads as yes/no while it holds a word.
  - (c) `deleted`, to match `includeDeleted`. But Account Deletion (decision 6) keeps "deleted" for the hard delete, a different state from "deactivated".
  - **Rec: (a).**

  **Claude's answer (2026-10-08; the Engineer adjusts if wrong):** (a) `soft_delete`.

  **Engineer:**

- **Q-F. Serving the soft-deleted word in this slice.** You ruled that this slice serves metadata only (question sheet P13 (a)). The word isn't metadata, since The Den keeps metadata a closed set (its decision 24), but the My Den rulings' decision 13 requires the mark.
  The slice also serves the soft-deleted word, projected from today's rows and unverified, like the name and level. Where the state lives for good stays open.

  **Claude's answer (2026-10-08; the Engineer adjusts if wrong):** yes

  **Engineer:**

### Revision 2.1 (2026-10-08): Owl's fixes

From Owl's QA of revision 2, at the end of this file. Rows 8 to 16 belong to the den lead and the tech lead (the architecture amendment and the release document) and aren't handled here. The proto's comments changed, so `buf lint` and `buf build` were re-run: both exit 0.

- **Row 1, the unknown-word fallback:** now "a generic soft-deleted label, never the unknown value itself" in the `DenOpen.soft_delete` comment, §1's "open" bullet, choice 2 and "Revision 2". The label's text stays the Engineer's word, under the web shell's Q4.
- **Row 2, the id rule:** a Den path segment is no longer called an id. The proposed R6 (§2, "The id-rule amendment (Q-C)") makes it a node's key: unique only within its real parent, at most 2048 characters, opaque apart from refusing empty, `.` and `..` pieces. Q-C's question, recommendation and answer follow, matching the release document's D-9.
- **Row 3, one word per type:** dropped. The `DenOpen.soft_delete` comment says "a word naming its soft-deleted state: today `archived` … or `deactivated` …", and so do §1 and "Revision 2". §5 says the privacy argument holds while each type has one word, and that a second word gets its own review. The appendix row says the same.
- **Row 4, stale status lines:** the box at the top, the "Den lead" line in "Revision 2" and §6 now say the den lead confirmed revision 2 and both wordings are in. "What you are ruling on" now counts six questions, Q-E and Q-F being under "Revision 2".
- **Row 5, the page token:** the `next_page_token` comment now promises the property, "reveals no names, hashes or counts", not the contents.
- **Row 6, the word isn't metadata:** Q-F above, with Claude's answer. The release document's "Metadata only" bullet is the tech lead's.
- **Row 7, tolerant-reader tests:** step 3 of the web shell plan now also tests an unknown `own_level` (no level, never Public) and an unknown `soft_delete` word (the generic soft-deleted label, never live).
- **Note 17:** `GetDenNodeResponse.node` now reads "in the shape its parent's listing uses". `DenCrumb.name` had the same flaw (a crumb can be an archived commission missing from its parent's listing), so it now reads "the one its parent's listing uses".
- **Note 18:** recorded under Q-A as a known, reversible choice.
- **Note 19:** §2 "Checked" now says the breaking check covers only the `session.proto` comment, since `den.proto` is new, and that retyping field 3 is free only because revision 1 was never published.
- **Note 20 (optional):** taken, as an optional line in the Q-C subsection for the den.proto package; `account.proto` still needs no change for Q-C.

## 1. For the Engineer: what a Den answer looks like

**Two addresses, one read.** `GET /api/v1/den` reads your own root ("My Den"); `GET /api/v1/den/<path>` reads anything below it, such as `/api/v1/den/commissions/01a0ef9c…/attachments`. A path always starts at *your* root. No field names a viewer or a root: the backend takes you from the session cookie, so nobody can ask for someone else's Den. Answers carry Den paths only, never where a node really lives (such as `/commission/<uuid>`).

**Every node is described the same way**, whether it's the one you landed on or a row in a listing:

- its **path**, a list of pieces the web sends back unchanged when you click (left off a card, Q-B);
- its **name**, from what it holds: a User's display name, else handle, else DID; a commission's title; a file's file name. Later, your custom title replaces a commission's name on your own mount;
- its **type**, a provisional working name such as `commission`, which the web only uses to pick an icon;
- its **kind** (directory, file or symlink), plus a **mount** mark when the row is one of your mounts. A mount shows its target's name, type and kind;
- its **access**, exactly one of two:
  - **open**: you can open it. Only then does it also carry the node's **own level** (private, listed or public; a mount shows its target's, a symlink its own), the **"content not shown yet"** flag, and, when it is soft-deleted, its **word**, which names that state: today `archived` for a commission, `deactivated` for an Account (a mount shows its target's). A word the web doesn't know still shows the node as soft-deleted, with a generic soft-deleted label, never the unknown value itself: never as live, and never with another value's word;
  - **card**: you may know it exists but can't open it. A card carries only its name and type, plus the mount mark when the row is your own mount (Q-B (b); den lead confirmed). So a card viewer never learns the level set for others, its path (for an Account, the DID that leads to its handle), its kind or whether it was archived. An access the web doesn't recognize shows as a card.

**A folder answer** adds **breadcrumbs** (your root first, down to the folder's parent) and **one page of entries** in the server's order (today: by name, as you are shown it), which the web keeps. What you can't see is simply absent: no count, no "N hidden". The web refuses an answer with no node as a contract error, so a stray body, such as another endpoint's answer, can never show up as an empty folder. An open node whose content the web doesn't recognize shows "content not shown yet", never an empty folder.

**The page token.** When more entries exist, the page ends with a token the web sends back as `?pageToken=…`. Today it is only a position in your own filtered list: no names, no hashes, no counts. The page is cut *after* hiding what you can't see, so a short page reveals nothing. A malformed token gets one fixed error before the path is even looked at; a valid one used elsewhere just starts at that position.

**`includeDeleted`** is a query parameter, `?includeDeleted=true`, sent by the UI toggle on each request and never stored. It affects listings only: archived commissions, and a deactivated Account for its Owner, then appear, each marked with its word. It never shows anything you couldn't otherwise open. A typed path ignores it: an archived commission you can open still opens, marked `archived`; a deactivated Account opens, marked `deactivated`, for its Owner and is the 404 for everyone else; an archived node you could see only as a card is the 404 (Q-D).

**A path to a file** (an attachment, the changelog) answers like any node: its breadcrumbs, its entry (open, flagged "content not shown yet") and an empty "file" content. The web links file rows to that page. File content fills that "file" part in a later slice. A folder flagged "content not shown yet" (a Workflow, your `posts`, an Account's `commissions`) answers an empty page, whatever it holds, so neither the flag nor the page depends on the data.

**Not found.** Absent, nonexistent, inside a node you can't open, or a malformed path (an empty one included): always **404** with one problem document (type `urn:zurfur:error:node-not-found`, code `node_not_found`), the same title, detail and headers. A typed path to an archived node you could see only as a card answers the same 404 as any other miss (Q-D), as a deactivated Account does for anyone but its Owner.

**A bad request.** A query parameter the Den doesn't know, a wrong spelling included, gets one fixed **400**. A known parameter with a bad value gets one fixed **422**. Both are checked before the path is looked at, so neither depends on what exists.

### What you are ruling on

**You review revision 2, as fixed in 2.1 (2026-10-08, WP-25):** the text below is what would ship, with Claude's answers to Q-A to Q-D and the soft-deleted word written in.

**Approve the draft as a whole, or name what to change** (P1 (a)), and answer six questions: Q-A to Q-D below, and Q-E (the word's field name) and Q-F (serving the word in this slice) under "Revision 2" above.

- **Q-A. How query parameters are spelled.** This is the rule for every query parameter from now on: today `includeDeleted` and `pageToken`, later "show hidden". It can't change after the alpha.
  - (a) `include-deleted`, as the My Den rulings wrote it. No contract field can hold a `-`, so the contract would say one thing and the server serve another.
  - (b) `include_deleted`, the proto field name. Then one round trip mixes styles: the answer carries `nextPageToken` and the client sends it back as `?page_token=`.
  - (c) `includeDeleted`, the field's JSON name. It matches your ruling that wire names are lowerCamelCase (versioning contract, R1), and Google's own REST APIs, such as Drive's `pageSize`, `pageToken` and `nextPageToken`.
  - **Rec: (c),** then correct the My Den rulings' spelling through design-sync. *(Changed 2026-10-08: the draft first recommended (b), before (c) was on the list.)*
  - **A known choice, and reversible.** `http.proto` names a query parameter by "the field path in the request message", so a client generated from the route annotations would send `include_deleted` and get the fixed 400. Accepting both spellings later only loosens validation, which is additive, so the strict answer is safe now.

  **Claude's answer (2026-10-08; the Engineer adjusts if wrong):** (c) `includeDeleted`, the lowerCamelCase JSON name, which matches the wire-name rule and `pageToken`. The My Den rulings' `include-deleted` (decision 13) is corrected by a later design-sync, with the den.proto package.

  **Engineer:**

- **Q-B. What does a card carry?** Your actor-card ruling says a card of an Account or Character shows "only its name, its type and the viewer's resolved level". But a path piece is the node's key, and an actor's key is its DID, whose public DID document names its handle. You chose not to put the handle on a card.
  - (a) As first drafted: name, type, kind, the mount mark and the path, so a later "Accept" or "Request Access" can address the card. This widens the actor-card ruling.
  - (b) Name and type only, plus the mount mark when the row is your own mount (it describes your entry, not the node). The path and kind come back, with their own review, in the slice that first acts on a card. This also hides a commission card's creation time, which its id embeds.
  - **Rec: (b).** No card exists in this slice, so neither answer costs code now. *(Den lead confirmed, 2026-10-08.)*

  **Claude's answer (2026-10-08; the Engineer adjusts if wrong):** (b) Name and type only, plus the mount mark on your own mount. Path and kind return, with their own review, in the slice that first acts on a card.

  **Engineer:**

- **Q-C. DIDs, Den path pieces and the 64-character id rule.** Your versioning rule says every id is a stable, unique, opaque string of at most 64 characters. A DID can be up to 2048 characters (the atproto spec's "hard limit of 2048 characters"), and a Den path piece can be a DID. But a path piece isn't an id: it is a node's key, unique only within its real parent (every commission has an `attachments`). `/me` already documents its DID under the 64-character rule.
  - **Rec:** amend the rule. A DID may be up to 2048 characters, and everything else in the id rule holds for it (opaque, never parsed by clients). A Den path piece is not an id but a node's key: unique only within its real parent, at most 2048 characters, and opaque to clients, except that a client building a Den path refuses empty, `.` and `..` pieces. The Den read enforces its own path rules: at most 16 pieces, each 1 to 2048 bytes of ASCII letters, digits and `. _ : % ~ -`, never `.` or `..`.

  **Claude's answer (2026-10-08; the Engineer adjusts if wrong):** Amend the id rule: a DID may be up to 2048 characters, and a Den path piece is a key, not an id, with the same bound, opaque except for refusing empty, `.` and `..` pieces. The amended wording, and `session.proto`'s `did` comment, are in §2, "The id-rule amendment (Q-C)".

  **Engineer:**

- **Q-D. Typing the path of an archived node you can only see as a card.** Listings hide an archived node from a card-only viewer, whether or not `includeDeleted` is set. If typing its path still showed the card, that viewer could tell the node had been archived, and the draft otherwise keeps that fact to people who can open the node. *(Raised by the den lead after the QA round, 2026-10-08.)*
  - **Rec:** typing that path gives the same 404 as any other miss. That is already the rule for a member who isn't the Owner of a deactivated Account. It adds one sentence where the draft states its not-found rules, and no code in this slice, since no card exists yet.

  **Claude's answer (2026-10-08; the Engineer adjusts if wrong):** Yes. Typing the path of an archived node you can only see as a card gives the same 404 as any other miss. Written into §1 "Not found." and the `includeDeleted` paragraph, and into the proto's header comment.

  **Engineer:**

**Choices inside the draft** (no ruling exists yet; approving the draft approves them):

1. **One node shape** for the landed node and every listing row. A mount is a mark on the row, not a kind.
2. **Facts only openers see sit inside "open":** the own level, "content not shown yet", the soft-deleted word. An unknown level shows no level, never Public; an unknown word still shows the node as soft-deleted, with a generic soft-deleted label, never the unknown value itself.
3. **A path to a file answers 200** with an empty "file" content, flagged "content not shown yet". The web links file rows.
4. **Content is optional on the client** *(changed 2026-10-08)*. The node is required: an answer without one is a contract error, which stops a stray body. An open node whose content the client doesn't recognize shows "content not shown yet". So a new kind of content added later degrades gracefully in an older client, including the command-line tool, which you ruled today uses a client generated from this contract. *(Tech lead confirmed, 2026-10-08. This adjusts the path-handling review's "content required" rule, and its purpose still holds: a stray body has no node, so it fails as a contract error and never shows up as an empty folder.)*
5. **Pagination.** The server picks the page size. A page token from an older server format, or a malformed one, gets the fixed 422, and the web then reloads the folder from its first page, once; a second 422 shows the error state, so a web bug can't loop. *(Den lead confirmed, 2026-10-08.)*
6. **Errors.** An unknown query parameter gets a fixed 400: a new registry entry, type `urn:zurfur:error:bad-request`, code `unknown_parameter`, whose detail never names the parameter. The not-found is type `urn:zurfur:error:node-not-found`, code `node_not_found`, with one fixed title and detail. *(Den lead confirmed the 400, 2026-10-08.)*
7. **Order** is promised only as "the order the server sends; clients keep it within each kind", so a client may show folders before files, as your mockup does *(added after the shell's QA, 2026-10-08)*. Today the order is by name, as the viewer is shown it. Ties between cards are broken by a keyed hash of the real path, never by the key itself, so the order can't reveal which commission came first. Board columns will list placements in board order, so the field doesn't promise name order forever. *(Den lead confirmed with this wording, 2026-10-08.)*

**Already settled** (for information, not to rule):

- Two calls, one per address: the contract's lint forces it.
- Name-then-key order is the tech lead's call, which you kept as a code convention (sheet §11, C12).

**Before the alpha** (no answer needed now): the type names, such as `commission`, are provisional. Either the type-catalog decision lands before the alpha, or these names ship as they are. Renaming them after that is breaking at review, though older clients only fall back to a generic icon.

**Claude's answer (2026-10-08; the Engineer adjusts if wrong):** Approved as a whole, revision 2 as fixed in 2.1, with the answers above, Q-E (a), Q-F yes, and the product document's D-4 (a): the soft-deleted mark is a word, today `archived` or `deactivated`, in `soft_delete`.

**Engineer (the draft as a whole):**

## 2. `den.proto`

```proto
// My Den: a signed-in viewer reads their own Den, one node at a time.
//
// Only messages are generated; the service block declares routes only. A
// Den path is relative to the viewer's own root, and the viewer comes from
// the session alone: no field names a viewer or a root. Only Den paths
// travel; no answer carries a node's real path. Every vocabulary here is a
// string with its known values documented; clients tolerate unknown values
// with the fallback stated on the field. Absent, nonexistent, inside a node
// the viewer can't open, or a malformed path: every case is the same 404
// problem, `urn:zurfur:error:node-not-found` with code `node_not_found`,
// with the same title, detail and headers. So is a soft-deleted node the
// viewer could see only as a card, and a deactivated Account for anyone but
// its Owner.

syntax = "proto3";

package zurfur.api.v1;

import "google/api/annotations.proto";

// `GET /api/v1/den`: the viewer's own root; the session is the argument.
message GetDenRootRequest {
  // Query parameter `includeDeleted`: also list soft-deleted nodes the viewer may open.
  bool include_deleted = 1;
  // Query parameter `pageToken`: a `nextPageToken` from an earlier page; absent for the first page.
  string page_token = 2;
}

// The viewer's own root and one page of its entries.
message GetDenRootResponse {
  // The root itself, its `segments` empty, its name the viewer's own name and its
  // own level `private`; required, or the answer is a contract violation.
  DenNode node = 1;
  // What the root holds. A client that finds no case it knows shows "content not shown yet".
  oneof content {
    // One page of the root's entries.
    DenListing listing = 2;
  }
}

// `GET /api/v1/den/{path=**}`: a node below the viewer's root.
message GetDenNodeRequest {
  // The Den path below the root: each segment percent-encoded once on its own, joined by `/`.
  string path = 1;
  // Query parameter `includeDeleted`: also list soft-deleted nodes the viewer may open.
  bool include_deleted = 2;
  // Query parameter `pageToken`: a `nextPageToken` from an earlier page; absent for the first page.
  string page_token = 3;
}

// The node at a Den path, its breadcrumbs, and what it holds.
message GetDenNodeResponse {
  // The node the path lands on, in the shape its parent's listing uses; required, or
  // the answer is a contract violation.
  DenNode node = 1;
  // The folders above it, the root first, down to its parent.
  repeated DenCrumb crumbs = 2;
  // What an open node holds; unset for a card. On an open node, a client that finds
  // no case it knows shows "content not shown yet", never an empty folder.
  oneof content {
    // One page of an open directory's entries.
    DenListing listing = 3;
    // An open file.
    DenFile file = 4;
    // An open symlink.
    DenLink link = 5;
  }
}

// One node as the Den shows it to this viewer.
message DenNode {
  // Its Den path from the viewer's root: opaque pieces, sent back unchanged. Empty on a card.
  repeated string segments = 1;
  // Its display name, taken from what it holds; never its key.
  string name = 2;
  // Its type: a provisional working name such as `commission`; unknown values get a generic icon.
  string type = 3;
  // `directory` | `file` | `symlink`, a mount's being its target's; unset on a card;
  // unknown values get a generic marker.
  string kind = 4;
  // Set when the entry is one of the viewer's mounts rather than a real child of its folder.
  bool mount = 5;
  // The viewer's resolved access; unset or unknown renders as a card, never as open.
  oneof access {
    // The viewer can open it.
    DenOpen open = 6;
    // The viewer may know it exists but can't open it.
    DenCard card = 7;
  }
}

// What a viewer who can open a node sees of it, beyond its entry.
message DenOpen {
  // The node's own level: `private` | `listed` | `public`; a mount shows its target's, a
  // symlink its own. An unknown value shows no level, never `public`.
  string own_level = 1;
  // Set when this view doesn't show the node's content yet; fixed per type, never read from the node.
  bool content_not_shown = 2;
  // Set only when the node is soft-deleted, to a word naming its soft-deleted state:
  // today `archived` (a commission) or `deactivated` (an Account). A mount shows its
  // target's, a symlink its own. An unknown value still marks the node soft-deleted,
  // with a generic soft-deleted label, never the unknown value itself: never as live,
  // and never with another value's word.
  string soft_delete = 3;
}

// A node the viewer may know exists but can't open. Its entry carries only its name, its
// type and, on the viewer's own mount, the mount mark; this message carries nothing more yet.
message DenCard {}

// An open file's content; empty, because this read serves no file content yet.
message DenFile {}

// An open symlink's content; empty, because links don't open yet.
message DenLink {}

// One folder above the node, as a breadcrumb.
message DenCrumb {
  // Its Den path from the viewer's root; empty for the root.
  repeated string segments = 1;
  // Its display name, the one its parent's listing uses; for the root, the viewer's own name.
  string name = 2;
}

// One page of a directory's entries, after everything the viewer can't see is left out.
message DenListing {
  // The entries, in the order the server sends; clients keep it within each kind and may
  // show directories first. Today: by name, as the viewer is shown it.
  repeated DenNode entries = 1;
  // An opaque position in this viewer's listing that reveals no names, hashes or counts; absent on the last page.
  string next_page_token = 2;
}

// The Den read. Every route requires a session, checked before anything
// else (401 `not_authenticated` otherwise), and answers Problems per
// problem.proto. Query parameters are spelled as their field's JSON name:
// `includeDeleted`, `pageToken`. Then the query is checked, before the path
// is resolved, so neither check depends on what exists:
// - an unknown query parameter answers one fixed 400 problem,
//   `urn:zurfur:error:bad-request`, code `unknown_parameter`;
// - `includeDeleted` other than `true` or `false`, a repeated parameter, or a
//   page token that is malformed or from a format this server no longer reads
//   answers one fixed 422 `invalid_request` problem.
// A well-formed page token only sets a position: on any listing it starts
// there, past the end it gives an empty page, and on a file, a symlink or a
// card it is ignored. An open directory whose type sets `content_not_shown`
// answers an empty listing with no token, whatever it holds.
service DenService {
  // The viewer's own root. Its entries are the system folders and mounts
  // the viewer's access makes.
  rpc GetDenRoot(GetDenRootRequest) returns (GetDenRootResponse) {
    option (google.api.http) = {
      get: "/api/v1/den"
    };
  }

  // The node at a Den path below the root. The server splits the raw path
  // on `/`, then percent-decodes each segment exactly once. More than 16
  // segments, or a segment that isn't 1 to 2048 bytes of ASCII letters,
  // digits and `. _ : % ~ -` (or is `.` or `..`), is the uniform 404; so is
  // an empty path (`/api/v1/den/`). A path never continues past a file, a
  // symlink or a card.
  rpc GetDenNode(GetDenNodeRequest) returns (GetDenNodeResponse) {
    option (google.api.http) = {
      get: "/api/v1/den/{path=**}"
    };
  }
}
```

**Counts:** 11 messages, 26 fields (3 `oneof`s), 1 service, 2 rpcs. Counted from the built descriptor. Unchanged in revisions 2 and 2.1: `string soft_delete = 3` replaces `bool soft_deleted = 3`, and 2.1 changed comments only.

**Checked (revision 2.1, 2026-10-08):** in a scratch copy of `contract/` holding this file and the Q-C `session.proto` comment below, `buf lint` (the repo's `STANDARD` config) and `buf build` both exit 0 with no output, run inside `contract/` and as CI runs them (`buf lint contract`), with buf 1.72.0, CI's pin. A camelCase field name planted in this file makes `buf lint` fail (exit 100), so the file is linted. A round trip through the built descriptor emits `softDelete` as a JSON string, leaves it out when empty, and passes an unknown word through unchanged. `buf breaking` against the repo's contract (`WIRE_JSON`) exits 0, but `den.proto` is a new file there, so that check covers only the `session.proto` comment. Changing `bool soft_deleted` to `string soft_delete` at number 3 costs nothing only because revision 1 was never published.

### One answer, as JSON

alice opens `/den/commissions/01a0ef9c…`. Defaults are left out on the wire, so `false` flags and the root's empty path don't appear:

```json
{
  "node": { "segments": ["commissions", "01a0ef9c…"], "name": "Untitled",
            "type": "commission", "kind": "directory", "mount": true,
            "open": { "ownLevel": "private" } },
  "crumbs": [ { "name": "alice" },
              { "segments": ["commissions"], "name": "commissions" } ],
  "listing": { "entries": [
    { "segments": ["commissions", "01a0ef9c…", "attachments"], "name": "attachments",
      "type": "commission.attachments", "kind": "directory", "open": { "ownLevel": "private" } },
    { "segments": ["commissions", "01a0ef9c…", "changelog"], "name": "changelog",
      "type": "commission.changelog", "kind": "file",
      "open": { "ownLevel": "private", "contentNotShown": true } } ] }
}
```

(The type strings are illustrative working names; P10 keeps them provisional. The root's breadcrumb carries the viewer's own name, here "alice": a display name, else the handle, else the DID. The web labels it "My Den". The root's own level is `private`.)

**The soft-deleted word on the wire.** With `?includeDeleted=true`, alice's `commissions` folder would list an archived commission as below. It is illustrative: the seed holds nothing archived, and the mock's hand-made archived commission shows it. A node that isn't soft-deleted has no `softDelete` key, as in the answer above.

```json
{ "segments": ["commissions", "0199c2d4…"], "name": "Old sketch",
  "type": "commission", "kind": "directory", "mount": true,
  "open": { "ownLevel": "private", "softDelete": "archived" } }
```

### The id-rule amendment (Q-C), as proposed text

Two contract files change with Q-C. They change in the den.proto package (WP-13), not now; the scratch check above included the `session.proto` change.

**`contract/VERSIONING.md`, ruling R6, as it would read.** Four changes: the "except" clause for a DID, the sentence on Den path segments, "its bound" in place of "the bound", and the closing note. The date is your ruling's.

> | R6 | **Ids are opaque** (resolves §8 Q2, taking the Microsoft Graph side of the AIP/Graph contradiction). Every id is a stable, unique, opaque string of at most 64 characters, except a DID, which may be up to 2048 characters: atproto's limit ("in the context of atproto, there is a current hard limit of 2048 characters", https://atproto.com/specs/did). A Den path segment is not an id: it is a node's key, unique only within its real parent, at most 2048 characters, and opaque to clients, except that a client building a Den path refuses an empty, `.` or `..` segment. Clients must not parse ids, derive meaning from their structure, or order by them; ordering, timestamps, and any other derived fact are provided as explicit fields or response order. Changes to id length or format within its bound are **not breaking**. Binds consumers only — the backend keeps full internal knowledge of its own formats (`Uuid` parsing, `ORDER BY id`, the embedded v7 timestamp). *Amended YYYY-MM-DD at the den.proto review: the 2048-character bound for a DID, and the Den path segment as a key, not an id. Raising a documented bound is a review flag (§1 item 17), taken here before the first published client.* |

**`contract/zurfur/api/v1/session.proto`, `GetMeResponse.did`.** One change, "at most 2048 characters":

```diff
   // The visitor's atproto DID — always present for a live session. Opaque to
-  // clients like every id (VERSIONING.md R6), though DIDs are additionally a
-  // public, externally-specified format.
+  // clients like every id (VERSIONING.md R6), at most 2048 characters, though
+  // DIDs are additionally a public, externally-specified format.
   string did = 1;
```

**`account.proto` needs no change.** Its header's "≤64-char strings" still holds: its `did` fields are always an Account's `did:plc`, and "the overall identifier length is 32 characters" (the did:plc spec, web.plc.directory/spec/v0.1/did-plc, read 2026-10-08). `commission.proto` carries no DID. *Optional, in the den.proto package:* the header restates the bound rather than pointing at R6, so one bound lives in two places. It could read "Ids are OPAQUE to clients (VERSIONING.md R6): stable, unique strings within R6's bound — do not parse, do not order by them."

## 3. Appendix: the rulings behind each field

How to read the ids: "P" and a number is a question on the My Den question sheet, with the option you chose ("P4 (c)"). "9." and a number is the tech lead's numbered answer in that sheet's section 9, and "§12" is its path-handling security review. "D" and a number is a decision on the My Den rulings page (2026100701) or on The Den (2026093001, called "Den DD"). "R" and a number is a versioning-contract ruling (`contract/VERSIONING.md`).

| Field or rule | Ruling behind it |
| --- | --- |
| Two routes, `/den` and `/den/{path=**}`; one Query behind both | 9.7; 2026100701 D1 (P3). Two rpcs, because buf `STANDARD` lint (`RPC_REQUEST_RESPONSE_UNIQUE`) forbids sharing a request or response, and one rpc with two bindings would make `path` an implicit query parameter on the root route |
| No viewer or root field anywhere | P2 note ("user can see its own and not others'"); architecture SEC-0 |
| Only Den paths travel, never a real path | path-handling security review (sheet §12, its F12): until symlinks open (P29), a mount target's real path only repeats the segment, so it adds surface and nothing else |
| `GetDenNodeRequest.path`, split, then decoded once | 9.11, 9.18; architecture SEC-1b; `http.proto` leaves `%2F` encoded inside a `**` variable. Once only: a DID may contain `%`, so a second decode would corrupt a key (Badger's review) |
| `includeDeleted` (query, per request, listings only) | P8, P41 (a), P42 (a soft-deleted Account: its Owner only, so another member's typed path is the 404; sheet §12), C6; 2026100701 D13. Spelling: Q-A (c), which follows R1 (lowerCamelCase wire names); D13's `include-deleted` is corrected by a later design-sync. A typed path to an archived node seen only as a card is the 404: Q-D |
| `pageToken`, `nextPageToken` | 9.7; C12; 2026100701 D24; contract R3 (opaque cursor), R7 (pagination lands beside the list); sheet §12 item 5: a version byte plus an offset into the viewer's filtered listing, no names, unkeyed hashes or counts; parsed before resolution, so a bad token can't reveal what exists |
| Authentication first, then the query, then the path | sheet §12 item 2: a 401, 400 or 422 never depends on the path |
| An unknown query parameter answers a fixed 400 | `contract/VERSIONING.md` §6, "Server obligation": reject unknown fields in query strings with 400 and an RFC 9457 problem. The error registry (23592962) has no 400 entry yet, so this adds one |
| `GetDenNodeResponse.node`, `crumbs` | web I5 / den I-W2 (agreed rows); P2 (1) |
| `oneof content { listing \| file \| link }`; `node` required, content not | contract R4 (an explicit discriminant). Sheet §12 item 1 required the content case, so that a stray body, such as the `/accounts` listing, can't show as an empty folder. A required `node` stops that alone, because no other answer carries one. Content is optional so that a new content case reaches an older client, such as the command-line tool (ruled 2026-10-08, generated from this contract), as "content not shown yet" rather than an error (Crow's QA, row 3; 40992770 D6; R8). *Tech lead confirmed, 2026-10-08; it adjusts a §12 item* |
| `DenNode.segments`; the segment rules on `GetDenNode` | P6 (a): a key is a segment; web I5 / den I-W2: paths travel as segment lists, so no tier parses one; 9.4: the cap must fit the longest D6 key, a DID of up to 2048 characters (atproto.com/specs/did), which the 64-character id rule (R6) doesn't allow, hence Q-C (the amended R6 is in §2); sheet §12 item 4: `Segment` is an ASCII allowlist, which also closes Unicode look-alikes of `..` and `/`; 16 pieces (9.4, §12 item 1) |
| `DenNode.name` | P10, P11 (a), P17 (b), P24 (b); 2026100701 D6 |
| `DenNode.type` | P10 (provisional names, opaque to the web) |
| `DenNode.kind`, `DenNode.mount` | Den DD D11: a mount is an entry the system makes, a symlink a real node. A mount shows its target's name, type and level (2026100701 D5, D6), so it shows its target's kind too; the mount guard (2026100701 D23) admits a file target such as `/blob/<cid>`, so "mount" can't stand in for a kind (Badger's review) |
| `oneof access { open \| card }`, unset as a card | 9.17; P4 (c); 2026100701 D5 |
| `DenOpen.own_level` | P4 (c): to openers only; a mount shows its target's, a symlink its own. The unknown-value fallback (no level, never Public) follows R8 (a) |
| `DenOpen.content_not_shown` | P9 (a); 2026100701 D15 |
| `DenOpen.soft_delete` | P8 (archiving is the soft delete), P41 (a) ("opens it, marked archived"); architecture §6 item 16. The words: the product document's D-4 (a), from 2026100701 D13 (archiving is a commission's soft delete) and Account Deletion (23003138) D2 and D6 (a soft-deleted Account is "deactivated"). A string, not an enum: R8, like every vocabulary in the contract. Absent when not soft-deleted: R4 (absence means "not set"), and an empty string is never a value. The unknown-word fallback follows R8 (a), leaning the same way as `own_level`'s. No promise of one word per type: a second word for a type is a new value, a review obligation under R8 (b). Named a noun: Q-E. Served in this slice although it isn't metadata: Q-F. Den side: The Den keeps metadata closed (Den DD D24), so the mark comes from the type's host arm, which already reads its row's `archived_at` or `deleted_at`; the den module sends the word, and Atta never learns it *(den lead confirmed, 2026-10-08)*. Which soft-deleted nodes a viewer gets is decided after Atta resolves each node's access, by one host-trait method that may refuse a node for a listing or a path without saying why: Atta drops a refused entry before it sorts and cuts the page, and treats a refused node on a path, or as a link's target, as missing. A placement's archived state is open (2026100701 D10); if it lands on the placement's symlink, it is that symlink's own mark |
| `DenCard {}`; a card's entry has no path and no kind | P13 (a) (metadata only), P18 (a), P32 (a), P33; 2026100701 D12 (an actor card shows "only its name, its type and the viewer's resolved level"); Q-B (b) |
| `DenListing.entries` order, filter before the page, no count | 9.10; 9.7; Den DD D26. The order is promised only as "the server's, kept by clients", because board columns will list placements in board order (2026100701 D10; 62488577) |
| A path to a file answers 200 with an empty `DenFile` | architecture §6 item 10; P9 (a); P2 (only folders open). File content grows inside `DenFile` later, as fields old readers ignore |
| An empty `DenLink` now | Den DD D10 (an open symlink must answer something); P29 (a): the target's real path goes inside it later, additively (Badger's review) |
| A flagged directory answers an empty listing with no token | P9 (a); 2026100701 D15 (the flag is never computed from data, so the page mustn't be either); 9.11, 9.24 (a Workflow lists nothing); P2 (1), (2) (Badger's review) |
| One 404 problem for every not-found | 9.16; C12; 2026100701 D21; 23592962. An archived node seen only as a card: Q-D (raised by the den lead, 2026-10-08) |
| No hash, payload or attributes | P13 (a); Den DD D25 (the hash goes only to openers, so it would join `DenOpen` later) |

## 4. Not in this draft (additive later)

- `showHidden` (dotfiles, P30): a request field, once a dotfile exists, spelled under Q-A's rule (the field's JSON name).
- A tombstoned Character (P38): a new `access` case when Character delete exists; today's readers would show it as a card.
- A symlink's landing (P29 (a)): the target's real path, as fields inside `DenLink`, with the real-path read. It is the first real path on the wire, so it gets its own review.
- File content, card bodies (P33's maturity), the payload hash: new fields in `DenFile`, `DenCard` and `DenOpen`.
- `pageSize`: an optional request field, if a client ever needs one.

## 5. Note for the security review

- **Card paths (now Q-B).** If a card carries its path (Q-B (a)), an actor card's path piece is its DID, which resolves publicly to its handle (atproto DID spec: "The handle claimed by the DID is found in the alsoKnownAs array"). A commission's or file's path piece is an app-minted UUIDv7 (`domain/src/elements/commission/entity.rs:76`), which embeds its creation time. Under Q-B's answer, (b), neither travels on a card. No card exists in this slice (P18 (a)).
- **The soft-deleted word (revision 2).** It sits inside `DenOpen`, so only a viewer who can open the node sees it, and it adds nothing that viewer couldn't already tell from the node's `type` and the old yes/no, because today each type has one word. That holds only while it does: a second word for one type (a moderation state riding this field, say) would carry more than the type, so it gets its own review. A card viewer never sees it: under Q-D, an archived node seen only as a card is missing from listings and the 404 on a typed path, so to that viewer an archived node looks the same as one that doesn't exist. A deactivated Account stays the 404 for everyone but its Owner.

## 6. Disagreements for the Engineer

None between den and web. Identity (which owns node names) and commission (which owns the commission card) haven't read the draft: their leads weren't live on 2026-10-07 or 2026-10-08. Revision 2 adds none; the den lead confirmed it (2026-10-08).

## QA challenges (Crow, 2026-10-08)

**Fit to rule on once two items are added to "What you are ruling on".** The proto keeps the decided privacy rules: one not-found for every miss, no viewer or root field, Den paths only, no counts, timestamps or hashes, the node's own level only to openers, the page cut after filtering, and the session checked first, then the query, then the path.
**Must-fix (2):** a card carries its path and kind, more than the actor-card ruling allows, and an actor's path piece is its DID, which leads to the handle that ruling left out (row 1). The spelling question leaves out a third option, `includeDeleted` (row 2).
**Should-fix (6):** one premise is out of date since today's command-line ruling, and five gaps in the contract text. None of them blocks your ruling; each fix is a line or two.

| # | Severity | Draft section | Finding | Ruling or source | Suggested change |
| --- | --- | --- | --- | --- | --- |
| 1 | must-fix | §1 "What you are ruling on"; §5 security note | A card carries `segments`, `kind` and `mount` as well as its name and type. The actor-card ruling says a Listed Account or Character card shows "only its name, its type and the viewer's resolved level", and you turned down adding an Account's handle to it. But an actor's path piece is its DID, and a DID document names its handle, so the path gives a card viewer the handle that ruling withheld. A commission's UUIDv7 piece gives its creation time. The draft's §5 notes only that second half, and doesn't put either to you. | My Den rulings, decision 12 "Actor cards and Character levels" (the actor-card question, P32 (a)); atproto DID spec: "The handle claimed by the DID is found in the `alsoKnownAs` array" (atproto.com/specs/did, checked 2026-10-08) | Add a ruling item: "A card carries its Den path and kind, beyond the card ruling's list. An actor card's path is its DID." Options: accept it (a card can't be addressed without a path), or keep the path off cards until "Accept" or "Request Access" needs one. No card exists in this slice, so either answer costs no code now. |
| 2 | must-fix | "New question for the Engineer" (the spelling question) | It offers only `include-deleted` or `include_deleted`. A third spelling, `includeDeleted`, follows your lowerCamelCase field-naming ruling and Google's own REST convention: the `pageSize` and `pageToken` query parameters, answered by a `nextPageToken` field. Under the draft, one round trip mixes the two styles: the body returns `nextPageToken` and the client sends it back as `?page_token=`. No existing proto has a query parameter, so this answer sets the rule for every later one, including "show hidden", which the tech lead spelled `show-hidden`. The spelling can't change after the alpha. | Versioning contract (`contract/VERSIONING.md`), ruling R1; `contract/google/api/http.proto`, "the parameter name is the field path in the request message"; Google Drive v3 `files.list` reference (developers.google.com, checked 2026-10-08); question sheet §10, the tech lead's answer on "show hidden" | Restate the question with three options, each argued, as the rule for all query parameters. |
| 3 | should-fix | §1 item 5; `GetDenNodeResponse.content` | The case for making content required rests on "while the web is the only client". Today you ruled that the command-line tool's user commands call the API through a typed client generated from this contract, and that tool runs on machines away from the server. After the alpha, a new kind of content would reach an older tool as an unknown field, which is dropped. "Content required" would then make it a contract error, so an additive change would break the tool. The stray-body risk this rule guards against is already stopped by `node` being required: a stray body such as the `/accounts` listing has no `node`. | API contract DD 40992770, decision 6 (unknown fields are ignored, which is what lets the contract grow additively); versioning contract §6 (unknown values get a defined fallback); your command-line ruling of 2026-10-08 (`.understand/20261007-unified-identity-investigation.md`, "Ruled 2026-10-08") | Correct the premise in item 5 and offer the alternative: `node` stays required, and an open node with no content the client recognizes shows "content not shown yet". |
| 4 | should-fix | §1 item 6; the segment rules | "Path pieces are not ids" doesn't hold up. A piece is the node's key (the ruling that a key is its path segment), and an actor's key is its DID. `session.proto` already documents `did` as an id under the 64-character id rule. But atproto allows a DID of up to 2048 characters. So the real conflict is with the id rule itself, not just with path pieces. | My Den rulings, decision 2; versioning contract ruling R6; `session.proto`, `GetMeResponse.did`; atproto DID spec: "a current hard limit of 2048 characters" (checked 2026-10-08) | Put item 6 as a change to the 64-character id rule for DIDs and keys, so `did` fields and Den path pieces follow one rule. |
| 5 | should-fix | service comment; §1 item 7 | The draft doesn't say what an unknown query parameter does. The versioning contract requires the server to reject unknown query-string fields with a 400 problem. Without that, a client that follows the decided spelling `include-deleted` is silently ignored and gets the filtered list. The error registry has no 400 entry yet. | Versioning contract §6, "Server obligation"; API response DD 23592962, the starter code registry | State that an unknown query parameter answers one fixed 400 problem (a new registry entry), checked with the other query checks before the path. |
| 6 | should-fix | `DenOpen.own_level` | The file's header promises that every vocabulary states its fallback for unknown values. `type` and `kind` do; `own_level` doesn't. | Versioning contract ruling R8 (a) | Add one, for example "an unknown value shows no level, never Public". |
| 7 | should-fix | `GetDenNode` route | `{path=**}` matches zero segments, so `/api/v1/den/` also binds `GetDenNode`, with an empty path. The architecture notes that axum's catch-all doesn't match an empty path, so that URL would get the router's plain 404, not the Den's problem document. The draft doesn't say what it answers, and the test that compares the declared routes with the real ones has to agree. | `http.proto`: "`**` matches zero or more URL path segments"; architecture §3, "The root read"; API contract DD 40992770, decision 12 | State that an empty path answers the uniform Den 404 (or the root), and pin it in the route test. |
| 8 | should-fix | `DenListing.entries` | It promises "sorted by name and then by segment" for every listing. Board columns will list their placements, which are symlinks, in board order rather than by name, so the promise would have to change, and changing what a field means is breaking once it's published. | My Den rulings, decision 10 (placements are symlinks in their column); Board Ordering DD 62488577; versioning contract §1 item 18 | Word it as "in the order the server sends; clients keep it", with "today: by name, then by segment" as information. |
| 9 | note | §1 items 9 and 1 | The listing order is already settled: the tech lead set it (sheet §9, the listing-order answer), and you kept it as a code convention (sheet §11, C12). Item 1, the two calls, is forced by the lint rules. | Sheet §11 C12; buf `STANDARD` lint | Mark both as information, not questions. |
| 10 | note | `DenNode.type` | You allowed provisional type names for this slice, but the alpha publishes them. If the type catalog decision renames them after the alpha, that is a vocabulary change and counts as breaking at review. Older clients would show a generic icon, so they degrade rather than break. | Versioning contract ruling R8 (b); API contract DD 40992770, decision 10 (the baseline reset ends at the first published client); sheet §11 C13 (the alpha is that client) | Tell the Engineer that either the type catalog decision lands before the alpha, or the provisional names ship as they are. |
| 11 | note | JSON example; `DenCrumb` | The root's breadcrumb reads "alice" here and "My Den" in the approved architecture's fixture tables. The root node's own level isn't stated; the architecture draws it as Private. | Architecture §4b; My Den rulings, decision 22 | Say what the wire carries (the user's name, which the web labels "My Den"), and that the root's own level is `private`. |
| 12 | note | service comment | Three cases aren't stated: which `include_deleted` values are accepted (`true` and `false` only?), what a token sent to a file or card path does, and what happens to a token minted by an older server after a deploy. Encrypted keyset tokens are planned for PostgreSQL, and a 422 in the middle of browsing would be a surprise. | Sheet §12, item 5 | One line each. |
| 13 | note | §6 "Disagreements" | Only den and web agreed the draft. Identity owns the node names, and commission owns the commission card, whose title and maturity are confirmed in the My Den rulings. Neither team is recorded as having read the draft. | My Den rulings, Context, "Confirmed, no change" | Say whether identity and commission saw it. |
| 14 | note | §3 "Why this shape" | The table cites rulings by bare id ("9.7", "P4 (c)", "D5"), which is hard to follow while ruling. | — | Add a few words to each id, or mark §3 as an appendix. |

**If other open items land differently:** a top-level `/board` would bring boards into an Account as mounts or symlinks, which the mount mark and `kind` already cover. One permanent personal Account per user would add a root entry but no field. Letting members see a soft-deleted Account (you said "may change") would change no field. A maturity gate on your own Den would likely add a field later, which is additive. Only the type catalog (row 10) and board ordering (row 8) touch text that can't change after the alpha.

## QA resolutions (Fennec, 2026-10-08)

**Count:** 14 challenges. 9 resolved in the draft, 3 routed to the Engineer as questions (rows 1, 2, 4), 1 routed as information for the alpha (row 10), 1 recorded as open with the teams (row 13). None rejected.

**What still needs a confirmation:**
- Den lead (Badger), for rows 1, 5, 7, 8 and 12: cards without a path or kind, the 400, the empty path, the order wording, and the token rules.
- Tech lead, for row 3: it adjusts the path-handling review's "content required" rule.

I checked each cited source before resolving: the versioning contract (R1, R6, R8, §6), `http.proto`, `session.proto`, the My Den rulings (D10, D12), the API contract decision (D6, D12), the 2026-10-08 command-line ruling, and the atproto DID spec and Google Drive's `files.list` reference, both read 2026-10-08.

| # | Outcome | How |
| --- | --- | --- |
| 1 | **Routed to the Engineer (Q-B)**; draft written to the recommendation; den lead to confirm | Q-B puts both options to the Engineer: (a) cards carry their path and kind, or (b) name and type only, plus the viewer's own mount mark. I recommend (b). The proto comments now say a card's `segments` and `kind` are empty, and `DenCard` says what a card's entry carries. The security note now covers the DID-to-handle path as well as the UUIDv7 creation time |
| 2 | **Routed to the Engineer (Q-A, restated)** | Three options, each argued, as the rule for every query parameter. The recommendation changed from `include_deleted` to `includeDeleted` (R1; Drive's `pageSize`/`pageToken`/`nextPageToken`), with the date. The plain-words section, the proto comments and the appendix use the recommended spelling, marked "spelling: Q-A". "Show hidden" becomes `showHidden` under the same rule |
| 3 | **Resolved**; tech lead to confirm | Accepted: the "only client" premise is out of date since the command-line ruling. `node` stays required, which alone stops a stray body. Content is optional, and an open node with no case the client knows shows "content not shown yet". Choices item 4, the proto comments, the plain words and the appendix row all changed |
| 4 | **Routed to the Engineer (Q-C)** | "Path pieces are not ids" is withdrawn. Q-C proposes amending the 64-character id rule so that DIDs and Den path pieces may reach atproto's 2048-character limit. If it's ruled, `session.proto`'s `did` comment needs the same change (identity's file) |
| 5 | **Resolved**; den lead to confirm | An unknown query parameter now answers one fixed 400 (versioning contract §6, "Server obligation"): a new registry entry, type `urn:zurfur:error:bad-request`, code `unknown_parameter`. It is checked with the other query checks, before the path. In the service comment, choices item 6, the plain words and the appendix |
| 6 | **Resolved** | `own_level`: an unknown value shows no level, never `public` (R8 (a)). Also in choices item 2 |
| 7 | **Resolved**; den lead to confirm | An empty path (`/api/v1/den/`) is the uniform 404, like any refused path. It goes through the explicit `/den/` route the path-handling review asked for, and the router-vs-descriptor test (API contract D12) must pin it. In the `GetDenNode` comment and the plain-words "Not found" |
| 8 | **Resolved**; den lead to confirm | `DenListing.entries` now promises "the order the server sends; clients keep it", with "today: by name, then by segment" as information. In choices item 7 and the appendix |
| 9 | **Resolved** | The two calls and the order moved to "Already settled", as information |
| 10 | **Routed as information** | "Before the alpha": either the type-catalog decision lands first, or the provisional type names ship as they are. No answer needed now |
| 11 | **Resolved** | The root's crumb carries the viewer's own name, which the web labels "My Den", and the root's own level is `private`. In the JSON note, `GetDenRootResponse.node` and `DenCrumb.name` |
| 12 | **Resolved**; den lead to confirm | `includeDeleted` accepts `true` or `false` only. A repeated parameter, or a token that is malformed or from a format the server no longer reads, gets the fixed 422, after which the web reloads the folder from its first page. A token on a file, symlink or card is ignored. In the service comment and choices item 5 |
| 13 | **Open, recorded** | §6 now says identity and commission haven't read the draft, because their leads weren't live. Their reading stays open; it doesn't block the Engineer's ruling |
| 14 | **Resolved** | §3 is now an appendix, with a short legend explaining the id families |

**Checked after the fixes:** the revised proto block passes `buf lint` (the repo's `STANDARD` config) and `buf build` beside the v1 corpus in a scratch copy (both exit 0). No field was added or renamed; only comments changed.

## Den lead confirmation (Badger, 2026-10-08)

**Short:** four confirmed, one small objection (the order note in the proto comment), and one gap the QA round didn't mark, put to the Engineer. None blocks the ruling, and none costs code in this slice. Atta today is still a skeleton (`backend/crates/atta/src/lib.rs`); everything below needs only what the My Den rulings already give it: the host trait, filtering before the page, and a token parsed in `application`'s den module before the path is resolved.

| Item | Den lead | Why, and the change if any |
| --- | --- | --- |
| What a card carries (Q-B, row 1) | **confirmed** | Name and type, plus the access case, is exactly the actor-card ruling (My Den rulings, decision 12), and the access case *is* the "resolved level" of decision 5. The mount mark describes the viewer's own entry, not the node, so it adds nothing about the target. |
| Pagination and the token rules (choice 5, row 12) | **confirmed** | Matches the path-handling review's token (a version byte plus an offset, parsed before the path), "page tokens carry no names" (decision 24) and filtering before the page. `true`/`false` only, a repeated parameter as the 422, and a token ignored on a file, symlink or card: all fine. One guard for the web: reload from page one **once**; a second 422 shows the error state, so a web bug can't loop. |
| The 400 for an unknown query parameter (choice 6, row 5) | **confirmed** | The versioning contract requires it ("Server obligation", §6), and a new error type is additive (its §1 item 22). A general type `bad-request` with a specific code matches the existing 422 (`invalid-request` plus specifics), so an unknown body field can reuse the type later with its own code. The detail stays fixed and never echoes the parameter's name. |
| The empty path is the 404 (row 7) | **confirmed** | The explicit `/den/` route runs the same order as the other Den routes (session, then query, then path), so `/api/v1/den/?bogus=1` is the 400, as on any path. A trailing or doubled `/` gives an empty piece, which is the same 404. |
| Order: "the order the server sends" (choice 7, row 8) | **confirmed**, with **one small objection** | The promise is right, and it also leaves room for keyset pages later. *Objection:* the informational "today: by name, then by segment" can't hold for cards. Under Q-B (b) a card has no segment, and The Den (decision 26) lets a listing "filter, count or sort only by what the viewer can see at each node's level". Same-named cards sorted by their hidden key would leak which key sorts first; for a commission that is which one was created first. **Change:** the `DenListing.entries` comment says "Today: by name, as the viewer is shown it." In code, ties between cards are broken by a keyed hash of the real path, never the key itself. That is no contract change, and no card exists in this slice. |
| *Not marked; for the Engineer:* an archived node the viewer sees only as a card | **gap**, recommendation in the next column | A listing hides it, with or without `includeDeleted` (the flag "never shows a viewer a node they could not otherwise open", decision 13). If a typed path still answered the card, the gap between list and path would tell a card viewer it was archived, a fact the draft keeps to openers (choice 2). **Rec:** a typed path to a soft-deleted node that the viewer could see only as a card is the uniform 404, as it already is for a non-Owner member on a soft-deleted Account (path-handling review, P42). That is one sentence in §1's `includeDeleted` paragraph and no code now. |

## Den lead confirmation, revision 2 (2026-10-08)

By Badger (den lead). Read against The Den (2026093001), the My Den rulings (2026100701) and Account Deletion (23003138).

**Short:** confirmed. The proto doesn't change. Two wording fixes in §1, and the den-side note needs one more sentence, because the hiding of soft-deleted nodes can't run where the approved architecture drew it. Nothing contradicts a My Den ruling except decision 13's spelling, which is already Q-A. None of this blocks the ruling.

| Item | Den lead | Why, and the change if any |
| --- | --- | --- |
| Where the word comes from (the den-side note) | **confirmed**, one addition | The rows already hold it: `archived_at` on a commission, `deleted_at` on an Account. The den module is the host, so it reads the word from its own answer for that node and builds `DenOpen`; Atta carries nothing. In code the word is a closed enum rendered through `Display`, so an empty or made-up word can't be sent. **Addition:** the approved architecture drew the hiding inside a type's mounts answer (its request diagram; its item 15 left it open). That can't hold. A mounts answer also serves typed paths, so a Participant's own archived commission would be the 404, against "still opens it, marked archived" (decision 13). And the host can't tell a card viewer from an opener before Atta resolves access, so Q-D would need a second copy of the level rule. **Instead:** after Atta resolves a node's access, it asks the host whether to keep the node, for a listing or for a path. Atta drops a refused entry before it sorts and cuts the page, and treats a refused node on a path, or as a link's target, as missing. Atta learns that the host may refuse a node, never why. It is one method on the one host trait, and it is the only place this check runs, so the listing, the typed path and a link can't disagree. The den redraws that step of the diagram for the Engineer's approval before building it. |
| "A mount shows its target's word; a symlink shows its own" | **confirmed** | This is decision 5's rule for levels: a mount has nothing of its own, and a symlink is a real node. No symlink type has a word today, so an opener sees a link to an archived commission unmarked, and following it lands on the commission, marked `archived` (decision 11). Whether a link should hint at its target's state stays with My Den's open row on a placement's archived state. The word belongs to the node itself and never passes to what is inside it: a deactivation "does not cascade" (Account Deletion, decision 2). |
| Q-D, as written | **confirmed**, two wording fixes | The proto header says what I meant: "a soft-deleted node the viewer could see only as a card". §1 says "archived node", and the Owner-only rule covers a deactivated Account, so no case is missed. In §1's `includeDeleted` paragraph, though, "your own archived commission" reads as owner-only, while decision 13 opens it for "a viewer who has standing" (P41 (a): a Participant). Also, "every other member" is narrower than the header's "anyone but its Owner". Proposed wording below. |
| Serving the word before payload verification | **confirmed**, noted | The slice may serve nodes before payload verification exists, but metadata only (sheet P13 (a)), and the word isn't metadata. Decision 13 and P41 (a) require the mark, so the slice serves it as a fact projected from today's rows (P14 (a)). Only openers get it, and it exposes nothing beyond the unverified name and level that P13 (a) already accepts. Where the archived state lives for good (a payload field, a `system.` attribute under The Den's decision 26, or metadata) stays open with the storage shape. The wire word is the same in each case. Parked, no question now. |
| Q-D against The Den, decision 26 | **holds** | Q-D hides a node from a card viewer by a fact the card doesn't show. Decision 26 says a query may "filter, count or sort only by what the viewer can see", to stop a viewer narrowing a hidden value by re-running a query with other filters. A card viewer gets the same answer whatever they send (`includeDeleted` changes nothing for them). A card that disappears looks the same as a revoked grant, which is quiet (decision 4), or as a link whose target narrowed to Private. Decision 13, the later ruling, requires the hiding. Written down so QA needn't work it out again. |
| Contradictions with a My Den ruling | **none** | Every revision-2 change, checked against decisions 4, 5, 10, 11, 12, 13, 15, 21 and 24. The only mismatch is decision 13's `include-deleted`, already Q-A. |

*Both proposed wordings below were applied to the draft by the tech lead, 2026-10-08.*

**Proposed wording, §1, the last sentence of the `includeDeleted` paragraph:**

> A typed path ignores it: an archived commission you can open still opens, marked `archived`; a deactivated Account opens, marked `deactivated`, for its Owner and is the 404 for everyone else; an archived node you could see only as a card is the 404 (Q-D).

**Proposed wording, appendix row `DenOpen.soft_delete`, its "Den side" sentence:**

> Den side: The Den keeps metadata closed (Den DD D24), so the mark comes from the type's host arm, which already reads its row's `archived_at` or `deleted_at`; the den module sends the word, and Atta never learns it *(den lead confirmed, 2026-10-08)*. Which soft-deleted nodes a viewer gets is decided after Atta resolves each node's access, by one host-trait method that may refuse a node for a listing or a path without saying why: Atta drops a refused entry before it sorts and cuts the page, and treats a refused node on a path, or as a link's target, as missing.

**Q-E, the den lead's view:** (a) `soft_delete`. The field holds a different word for each type, so it is named after what both words are instances of, the soft delete (decision 13; Account Deletion, decision 2). `deleted` would take the word Account Deletion keeps for the hard delete (its decision 6), and nothing on the den side reads the field as yes/no.

## QA, revision 2 (Owl, 2026-10-08)

By Owl (den QA), for the tech lead, before the Engineer sees revision 2 or the den lead's amendment (`.understand/20261006-my-den-architecture.md`, "Proposed amendment (2026-10-08)"). Read against the My Den rulings, The Den, Account Deletion, the error model and the API contract decision in the corpus, `contract/VERSIONING.md`, the release's Technical Design Document (`.understand/releases/0.1/tdd.typ`) and the web shell's "Questions from the build".

**Verdict: fix first.** Two sentences of contract text are wrong. Each is a one-line fix, and nothing else blocks.

- **The proto checks out.** I re-ran `buf lint` and `buf build` on Fennec's scratch copy, both inside `contract/` and the way CI runs them: each exits 0 with no output (buf 1.72.0, CI's pin). `buf breaking` against the repo exits 0. The draft's proto block is byte-identical to the checked file. The scratch copy differs from the repo's contract only by the new `den.proto` and the `session.proto` comment. The counts hold. `softDelete` goes out as a JSON string, is left out when empty, and passes an unknown word through.
- **The word matches the rulings.** Archiving is a commission's soft delete, and a soft-deleted Account is "deactivated". Only openers see the word. A typed path to an archived node seen only as a card gets the same 404 in all five places that say so.
- **Must-fix 1:** the contract says a node with an unknown word shows "no word", but the web's answer shows "Inactive". The two have to agree.
- **Must-fix 2:** the amended id rule calls a Den path segment an id, which makes it "stable, unique". A segment is unique only within its parent: every commission has an `attachments`.
- **The amendment** keeps decision 13, the card rule (Q-D), cutting the page after filtering (SEC-5) and the one not-found (SEC-6). Atta never learns the flag, the word or the reason. Its diagram parses. It has five should-fixes: pass the viewer to the new method the way its siblings get it; put the link-target choice to the Engineer; add three missing tests, one of them a leak check; and give the release document's D-15 real options and a clear approval path.
- **Counts:** 2 must-fix, 11 should-fix (5 on the amendment), 7 notes (3 on the amendment).

| # | Severity | Where | What's wrong | Proof | Fix |
| --- | --- | --- | --- | --- | --- |
| 1 | must-fix | `DenOpen.soft_delete` comment; §1, the "open" bullet; choice 2; the "Fallback" in "Revision 2"; web shell, "Questions from the build", Q4 | The contract's fallback for an unknown word is "a generic marker and no word" (§1 and choice 2: "with no word"). The web's answer to Q4 shows "Inactive", which is a word. The Engineer would approve both on the same day. Also, "Inactive" means nearly the same as "deactivated", the confusion that "never with another value's word" is meant to prevent. | this draft, §2 and §1; `.understand/20261008-web-explorer-shell.md`, Q4; `contract/VERSIONING.md` R8 (a): each vocabulary has one defined fallback | Make the contract say what the web does, without naming a UI word: "an unknown value still marks the node soft-deleted, with a generic soft-deleted label, never the unknown value itself: never as live, and never with another value's word". Use the same wording in §1, choice 2 and "Revision 2". The label stays the Engineer's word under Q4; add to Q4 that "Inactive" may read as "deactivated". |
| 2 | must-fix | §2, "The id-rule amendment (Q-C)", the R6 wording; Q-C; release document D-9 | "Every id is a stable, unique, opaque string of at most 64 characters, except a DID and a Den path segment" makes a segment an id. The binding contract would then call it unique, and it isn't: a key is unique only within its real parent, and the same segment recurs (every commission has `attachments`, every user `commissions`). A client keying anything by one segment would mix nodes up. Also, the web has to look inside segments to refuse empty, `.` and `..` pieces, which "clients must not parse ids" could be read to forbid. | My Den rulings, decision 2 ("unique within that parent"); `contract/VERSIONING.md` R6; architecture, SEC-1a | Keep the one 2048 bound, but don't call a segment an id: "...of at most 64 characters, except a DID, which may be up to 2048 characters: atproto's limit (…). A Den path segment is not an id: it is a node's key, unique only within its real parent, at most 2048 characters, and opaque to clients apart from the path builder's refusal of empty, `.` and `..` pieces." Q-C and D-9 follow. The Engineer rules on the wording. |
| 3 | should-fix | `DenOpen.soft_delete` comment ("to its type's word"); §5, second bullet | The comment promises one word per type, and §5's privacy argument rests on that ("the word follows from the type"). The moderation study proposes a "Hide" that keeps a commission visible to its owner and participants. If that state ever rides this field, a commission has two words: the promise breaks (a change of meaning, breaking at review), and the word carries more than the type. | `.understand/20261008-moderation-study.md`, §4.1 "Hide" (proposed, not ruled); `contract/VERSIONING.md` R8 (b), §1 item 18 | Drop the one-to-one promise: "to a word naming the node's soft-deleted state; today `archived` (a commission) or `deactivated` (an Account)". In §5, say the argument holds while each type has one word, and that a second word gets its own review. |
| 4 | should-fix | The box at the top; "Revision 2", "Still open"; §6; "What you are ruling on" | The status lines the Engineer reads first are out of date. They say "the den lead hasn't read revision 2" ("Still open" and §6) and "Badger wasn't live to confirm them" (the top box). But the den lead's revision-2 confirmation is at the end, and the tech lead applied both of its wordings. Also, "answer four questions" sits beside a fifth, Q-E. | this file, "Den lead confirmation, revision 2 (2026-10-08)" | Say the den lead confirmed revision 2 and both wordings are in. Say "five questions; Q-E is under 'Revision 2'". |
| 5 | should-fix | `DenListing.next_page_token` comment | The proto promises a token "free of names, hashes and counts", with no "today". The path-handling review rules out only names, *unkeyed* hashes and counts, and it plans encrypted keyset tokens once listings come from PostgreSQL. The den lead breaks ties between cards with a keyed hash. A keyset cursor holds the last sort key, so it would break the published wording without leaking anything. | question sheet §12, item 5; My Den rulings, decision 24 ("carries no names"); den lead confirmation, the "Order" row | Promise the property, not the contents: "an opaque position in this viewer's listing that reveals no names, hashes or counts". §1 already says "Today". |
| 6 | should-fix | den lead confirmation, revision 2, "Serving the word before payload verification"; release document, "Data", "Metadata only" | The Engineer answered P13 with (a), "metadata only". The word isn't metadata: The Den keeps metadata a closed set, and the soft-deleted state isn't in it. The den lead reconciled the two rulings himself ("Parked, no question now"), and the release document still says "metadata only". The reading is fair (the name and level also go out unverified), but how was it reached without asking the Engineer? | question sheet P13 (a); The Den, decision 24; My Den rulings, decision 13 | Put one line to the Engineer, with Claude's answer: "The slice also serves the soft-deleted word, projected from today's rows and unverified, like the name and level. Where the state lives for good stays open." Adjust the release document's "Metadata only" bullet. |
| 7 | should-fix | Web shell plan, step 3 "Wire to den.proto", its tolerant-reader tests | The step tests an unknown access, kind and content. It doesn't test an unknown `own_level` (no level, never Public) or an unknown `soft_delete` word (soft-deleted, never live). The versioning contract wants each fallback pinned by a test. | `contract/VERSIONING.md` R8 (a), "pinned by a §6-style test"; `.understand/20261008-web-explorer-shell.md`, step 3 | Add both tests to step 3 (the web owns it). |
| 8 | should-fix | **Amendment:** "The method, in words", Inputs | `admit` takes no viewer: the host holds one from the den Query. Its siblings `lift` and `mounts` get the viewer from Atta. So one request hands the host the viewer through two channels, and nothing makes them agree: `admit` could decide for a different viewer than `lift`. Keeping `includeDeleted` off Atta's signatures doesn't need this, since Atta already carries the viewer. | architecture §3 (`lift(viewer, …)`, `mounts(viewer, …)`); the amendment's own diagram (`mounts(viewer, commissions)`) | Pass the viewer to `admit` as the siblings do, and keep only `includeDeleted` in the host. Or make the host's viewer the only one, for every method. The tech lead's implementation call. |
| 9 | should-fix | **Amendment:** Inputs ("Atta asks about a link's target as `Path`"); the rule table's "Open, `Path`" column | Two product choices sit inside an implementation detail. (1) Asking about a link's target as a path means a board placement pointing at an archived commission stays in listings with `includeDeleted` off, for anyone who can open the commission; on a Public board, that is every visitor. Asked as a listing, it would be hidden. (2) The table opens an archived node on a typed path for any viewer who can open it, while decision 13 names "a viewer who has standing". How was each reached? A placement's archived state is an open row. | My Den rulings, decision 13 and the open row "A placement's position, alias and archived state"; The Den, decision 10 (a link's level) | Put both to the Engineer with Claude's answers, or leave the link-target choice open until the first slice with a symlink (the seed has none). Either way, no code in this slice. |
| 10 | should-fix | **Amendment:** "Tests that prove it" | Three cases have no test. (a) Refused entries after the cut must not leave a next-page token. If the host refuses every entry past position N, a page of N has no token; otherwise a card viewer learns that hidden archived entries exist. The "page never short" test doesn't cover this. (b) bob, a Participant who isn't the owner, types the archived commission's path and gets 200, marked `archived`. Decision 13 says "a viewer who has standing", and the den lead reworded §1 for exactly this case, but the listed test uses alice. (c) The Owner types the deactivated Account's path with the flag off and gets 200, marked `deactivated`, tested at the den Query or the route. | SEC-5 (cut the page after filtering; never a count); My Den rulings, decision 13; question sheet P42 | Add the three rows. (a) runs against Atta's fake host. (b) and (c) need test-only seeds, because the dev world holds nothing archived. |
| 11 | should-fix | **Amendment / release document D-15:** the options | D-15's option (b), "keep the approved diagram, and accept that it breaks decision 13 and D-10", isn't a real alternative, so the Engineer sees no choice. There are two real ones. The den Query could hide entries after Atta answers: cheap, but the page is then cut before the hiding, so pages come out short and the token's offset counts hidden rows (it breaks SEC-5). Or each host arm could filter, with a second copy of the level rule (the amendment's own reason against it). | release document, D-15; the amendment's "Why" | Replace (b) with those two, one line each, with why each loses. |
| 12 | should-fix | **Amendment / release document D-15:** its status | The release document lets D-15 "proceed on Claude's answer", while the amendment is "for the Engineer's approval". The Engineer's rule is that architecture diagrams are approved before programmers start, and this changes an approved one. Who decided it could go ahead first? | release document, the "Decisions needed: technical" intro and D-15; the Engineer's diagram-approval rule (2026-10-06) | Ask the tech lead which rule governs. If approval comes first, say that WP-12 (Atta's read side) waits for it. |
| 13 | should-fix | release document, "Sequence: opening a folder", the paragraph under it | "A typed path to one's own archived commission still opens it" reads as owner-only. Decision 13 opens it for "a viewer who has standing", which is why the den lead reworded the same sentence in §1. | My Den rulings, decision 13; den lead confirmation, revision 2, the "Q-D, as written" row | "A typed path to an archived commission still opens it, marked, for a viewer with standing." |
| 14 | note | **Amendment:** its Mermaid block | It parses. I ran `mermaid.parse()` under jsdom 25.0.1, with mermaid 11.4.1 and with 11.17.2, and a deliberately broken block fails in both. The amendment names 11.4.1, while the file's "Validation" line names 11.17.2. The `alt` reads as three exclusive cases, but a typed path to a folder runs the path check at each step and then the listing check on the folder's entries. | my runs, 2026-10-08; the architecture file, "Conventions" | Name 11.17.2, and say the cases can occur in one request (a note, or `opt` blocks). |
| 15 | note | **Amendment:** "What it replaces" | The list leaves out three places that change. Diagram 1's E5 label lists the host-trait methods, and `admit` joins them. The lists of Atta's own value types (diagram 2's legend, the release document's Feature 3 architecture) gain the reach and `Admission` types. And the release document's SEC-5 and SEC-6 rows don't mention the refusal step. | architecture §1 (E5), §2 legend; release document, Feature 3, "Architecture" and "Security" | Add those places. |
| 16 | note | **Amendment:** how it reaches the Engineer | The Engineer approves diagrams on a rendered page with few boxes and short labels, keeps the Mermaid as the agents' record, and has QA compare the page with the design. The amendment has only the Mermaid. | the Engineer's rules on approval diagrams (2026-10-07) and on QA checking them (2026-10-08) | Build the approval page for the redraw, and send it to QA before the Engineer. |
| 17 | note | `GetDenNodeResponse.node` comment | "As its parent lists it": with `includeDeleted` off, an archived commission opened by its typed path isn't in its parent's listing at all. | §1, the `includeDeleted` paragraph | "In the shape its parent's listing uses". |
| 18 | note | Q-A; the service comment | `http.proto` says a query parameter's name is "the field path in the request message". So a client generated from these annotations would send `include_deleted` and get the fixed 400. Accepting both spellings later loosens validation, which is additive, so the strict answer is safe now. | `contract/google/api/http.proto`, "Rules for HTTP mapping"; `contract/VERSIONING.md` §1 item 15, §7.1 (a ProtoJSON parser must accept both names in a body) | Record it under Q-A as a known, reversible choice. |
| 19 | note | "Checked (revision 2)"; release document D-6 | `buf breaking` against the repo passes, but `den.proto` is a new file there, so the check covers only the `session.proto` comment. Changing `bool soft_deleted` to `string soft_delete` at number 3 costs nothing only because revision 1 was never published. | my run; `contract/VERSIONING.md` §1 items 4 and 8 | Say what the breaking check covers. |
| 20 | note | §2, "`account.proto` needs no change" | The reasoning holds: Account DIDs are always did:plc, and the did:plc spec says "the overall identifier length is 32 characters" (checked 2026-10-08). But the file's header restates "≤64-char" instead of pointing at the id rule, so one bound lives in two places. | `contract/zurfur/api/v1/account.proto`, header; web.plc.directory/spec/v0.1/did-plc | Optional, in the den.proto package: have the header point at the id rule. |

**Checked and sound.** Each of these was checked by its exit code and its output, or against its page.

- **`buf`.**
  - `buf lint` and `buf build` exit 0 with no output, both inside `contract/` and run as `buf lint contract`, the way CI runs it.
  - `buf breaking` against the repo's `contract/` exits 0.
  - A camelCase field planted in a copy fails `buf lint` with exit 100, so the file is linted.
- **The file itself.**
  - The draft's proto block and the checked file are both 7377 bytes, with the same SHA-256 (`c1104f64…`).
  - The scratch copy differs from the repo only by `den.proto` and the `session.proto` comment shown in §2.
  - The built descriptor holds 11 messages, 26 fields, 3 `oneof`s, 1 service and 2 rpcs, and `soft_delete` is field 3, a string, with JSON name `softDelete`.
  - Round trips through `buf convert`: `archived` stays, an empty word is left out, an unknown word passes through, and `soft_delete` is accepted as input.
- **Sources.**
  - The atproto DID spec ("a current hard limit of 2048 characters"; "The handle claimed by the DID is found in the `alsoKnownAs` array") and the did:plc spec (32 characters), both read 2026-10-08.
  - Q-E's quotes match My Den rulings decision 13 and Account Deletion decisions 2 and 6.
  - `role`, `outcome`, `lifecycle` and `visibility` are strings in `account.proto` and `commission.proto`.
  - The den side's claims hold in code: a commission has `archived_at` and an Account has `deleted_at`. `Commission::is_archived` is still inverted, so the release document's "a bug to fix first" stands.
- **The rest of revision 2.**
  - The Q-D sentence agrees in the header comment, §1 "Not found.", §1 `includeDeleted`, the appendix's "One 404 problem" row and §5.
  - Q-A to Q-D each carry their answer in place.
  - The confirmations are folded in (Q-B, choices 5 to 7, the appendix row on content).
  - The release document's D-6 to D-10 and D-14 match Q-A to Q-E. Its den.proto table matches the proto, and its `soft_delete` row doesn't say "no word", so row 1's fix doesn't touch it.
- **The amendment.**
  - Listings hide soft-deleted nodes unless the flag is set.
  - Typed paths open them for openers, and a deactivated Account opens only for its Owner.
  - Cards are refused for every reach.
  - Refusals happen before the sort and the cut, and the token counts kept entries only.
  - A refusal leaves through the missing child's exit.
  - Atta gets only Admit or Refuse.

## QA verification, revision 2.1 (Owl, 2026-10-08)

By Owl, checking the fixes for "QA, revision 2" above: Fennec's "Revision 2.1" in this file, the den lead's revised amendment in `.understand/20261006-my-den-architecture.md`, the release document `.understand/releases/0.1/tdd.typ`, and the web shell's Q4 and step 3.

**Verdict: ready for the Engineer.** All 20 rows are closed, and nothing new contradicts a ruling. One new should-fix: the amendment's question 2 leaves out one way to open a commission without standing. It is a clause to add and needs no new QA round. There are also two new notes.

**The proofs, re-run on revision 2.1.** Each was judged by its exit code and its output.

- **`buf`.**
  - `buf lint` and `buf build` exit 0 with no output, both inside `contract/` and as CI runs them.
  - `buf breaking` against the repo exits 0.
  - A planted camelCase field fails `buf lint` (exit 100).
- **The file itself.**
  - The draft's proto block is byte-identical to the scratch file (SHA-256 `29029a05…` on both).
  - The scratch copy still differs from the repo only by `den.proto` and the `session.proto` comment.
  - The descriptor still holds 11 messages, 26 fields, 3 `oneof`s and 2 rpcs.
  - `softDelete` still round-trips: it is kept, left out when empty, and an unknown word passes through.
- **Mermaid.** The amendment's one block parses under jsdom 25.0.1 with mermaid 11.17.2 and with 11.4.1. A broken control block fails under both. The file's other four blocks parse too.
- **The release document.**
  - It compiles with the pinned typst 0.15.1: exit 0, no warnings, 20 pages.
  - My compile's text is identical to the committed `tdd.pdf`.
  - Page 16 shows D-15 with its "Awaiting your approval" badge, three real options and "required before dependent work starts".
  - Page 17 shows Figure 5, the redraw, which agrees with the amendment's Mermaid (path checks at each step, then listing checks, then the word), and D-16.

| Row | Closed? | Where it is fixed |
| --- | --- | --- |
| 1 | closed | "a generic soft-deleted label, never the unknown value itself" in the proto, §1, choice 2 and "Revision 2". The shell's Q4 answer is now "Set aside", a generic label that agrees with the contract (and collides with no corpus term) |
| 2 | closed | Proposed R6: a DID up to 2048 characters; a Den path segment is a key, not an id, unique only within its real parent. Q-C and the release document's D-9 match |
| 3 | closed | "a word naming its soft-deleted state: today …" in the proto, §1 and "Revision 2"; §5 and the appendix say a second word gets its own review |
| 4 | closed | The top box, "Revision 2", §6 and "six questions" are current |
| 5 | closed | `next_page_token`: "reveals no names, hashes or counts" |
| 6 | closed | Q-F here; D-16 in the release document; the release document's "Data" bullet |
| 7 | closed | The shell's step 3 now tests an unknown own level and an unknown word |
| 8 | closed | `admit` takes the viewer from Atta, like `lift` and `mounts` |
| 9 | closed | The amendment's questions 1 and 2, with Claude's answers; the link-target reach is left open, and the rule table and tests follow it |
| 10 | closed | The three tests (no token after a refused tail; bob as a Participant; the Owner's typed path) |
| 11 | closed | D-15 options (a), (b) and (c), each with why it loses |
| 12 | closed | D-15 is "Awaiting your approval"; WP-12 waits for it (the amendment's "Status", the release document's intro and D-15) |
| 13 | closed | "for a viewer with standing" under "Sequence: opening a folder" |
| 14 | closed | mermaid 11.17.2 named; `opt` blocks, with a note that one request can run all the checks |
| 15 | closed | "What it replaces" lists §1's E5, §2's legend and the release document's Feature 3; the release document's lines are already updated |
| 16 | closed | Figure 5 under D-15 is the rendered approval page; I compared it with the Mermaid |
| 17 | closed | `GetDenNodeResponse.node`, and `DenCrumb.name` too |
| 18 | closed | Under Q-A |
| 19 | closed | §2 "Checked" |
| 20 | closed | The optional line in the Q-C subsection |

**New in revision 2.1**

| # | Severity | Where | What's wrong | Proof | Fix |
| --- | --- | --- | --- | --- | --- |
| 21 | should-fix | **Amendment:** question 2 and the rule table's "opened without standing" row; release document D-15's answer | Question 2 names two ways to open an archived commission without standing: its own level and a view grant. The rulings define a third. Members of an Account that a commission was brought into see it "at that level" through the commission-side record, and that isn't standing, because standing never crosses the mount. Under Claude's answer, a studio's members would lose an archived commission brought into the studio, even with "Show archived" on. Also, D-15's answer gives Claude's answer to question 2, but not the den lead's disagreement on view grants. | My Den rulings, decision 8 ("Members see the commission at that level, through the mount of D7") and decision 3 (standing never crosses a mount) | Add the brought-in case to question 2 and to that table row. In D-15's answer, add "the den lead would count a view grant with standing". No code in 0.1, because bringing a commission into an Account is a later slice. |
| 22 | note | **Amendment:** "Standing is the host's own knowledge" | "Whether it lifted this viewer at this node" misses standing inherited from an ancestor, since standing covers the whole real subtree. Today's two arms are each lifted at themselves (a commission's Participant, an Account's Owner), and a miss refuses, so nothing leaks. | My Den rulings, decision 3 | Say "whether a lift on the node's real path covers this viewer", or have Atta pass whether standing contributed to the level. The tech lead's implementation call. |
| 23 | note | Release document: the summary's "serves metadata only", and the den.proto table's `DenOpen` row; this draft's line mapping Q-A to Q-D onto D-7 to D-10 | Small leftovers. The summary still says "metadata only", though D-16 adds the word. The table lists the words without "today". The mapping line leaves out Q-E (D-14) and Q-F (D-16). | release document, the summary's "Data" line and "Contract: den.proto"; "Revision 2", first paragraph | One phrase each. |

**Against the rulings.** Nothing in revision 2.1 contradicts one:

- Q-F puts the gap between the Engineer's answer to P13 ("metadata only") and decision 13 to the Engineer.
- The new R6 matches decision 2.
- Claude's answer to question 2 narrows decision 13 for viewers without standing, but it is put to the Engineer as a question, with the den lead's opposing view recorded. It isn't decided silently.
