# Roadmap: commission

Lead: Wolf · Stages covered: pre-alpha (P2–P7), alpha (A1–A2), beta (B1–B5), release (R1–R2)

## In one screen

- **After 0.1 (P1, fixed):** a commission you take part in shows in My Den, read-only, from PostgreSQL. Everything else about commissions still runs on pre-Den routes, most of them outside the contract.
- **End of pre-alpha (P2–P7):** an artist and a client can run a whole commission inside the Den. They create it, seat the client by invitation, upload work through moderation's scan gate, pin and discuss it, and move its status and lifecycle. They slot characters with the keeper's consent, place it on the Account's board, grant view, make it Listed, and archive or delete it. Every change lands in the changelog in the same transaction. The old routes, the old drawn markup and the free-text notes are gone. You have reviewed every stored shape and `commission.proto`.
- **End of alpha (A1–A2):** every alpha card in our area has a home. Request for Approval and hand-settled invoices close the loop (A1). Ask-for-Art and the status tags open the client-first door (A2).
- **End of beta (B1–B5):** phases and a discussion on the commission page, the changelog hash chain, YCH templates, and commission trees. Golems get seats in commissions (with the plugin team). Before Golems can join Accounts, the MVP-only rule that membership gives view is retired.
- **Release (R1–R2):** tiered disputes, for real work (R1). Capacity guards and the openings feed, so an artist advertises only what they can take (R2).
- **The fork that blocks us first:** where a commission's brief, budget and seats live in the tree (question 1). It needs a ruling during P2, because seats (P4) build on it.

## Pre-alpha

- **P2: A commission you can work from My Den.** Create, rename, move it through its lifecycle and status, set a deadline, archive or delete it, and read its changelog a page at a time. Today's use cases become named actions on the commission node, and the first commission acts enter the contract.
  Features: commission-core (lifecycle, the two status axes, ownership that never moves by itself), changelog (each entry written with its change, read a page at a time, its sentence rendered at read), fact-deletion (our half: hard delete only while fact-free, archive otherwise; this is the board's "soft-delete gate"), closed-door (born Private). Also question 1 put to you as a decision page (no code).
  Depends on: den, for named actions reaching our handlers through Atta, a commission's payload served and verified (its hash and MAC), and the changelog rendered as a typed file with pages. Web, for the create form, the commission header and its actions, and the changelog view. Questions 2 and 4 ruled.
  Done when: in My Den, create "Test 1"; it shows Private and Draft. Rename it. Move it to Batched, then Active. Set "Waiting for Input" and a deadline. Open `changelog`: each act is one sentence, and "More" pages back. Archive it: it leaves `commissions` and returns with the toggle. Create "Test 2" and delete it: it's gone. A write to someone else's commission gets "Nothing here", the same answer a read gets.

- **P3: Files and pins.** Upload into `attachments` and `products` through the scan gate, place a pin, and talk in its thread.
  Features: markup (a pin opens a thread), the Comment entity (changelog notes become comments), the submission prompt for status, a product registered as a fact.
  Depends on: moderation, for the scan gate in front of every upload. This slot changes the upload path, so under your ruling on moderation's first deadline (D-26) the gate must be there first. Den, for files stored once by content hash with an upload that looks the same either way, and each file a node. Web, for the upload, the pin layer and the thread.
  Done when: upload "sketch-02.png". It shows "processing", then appears in `attachments`. The changelog says a file was added and doesn't print its name. The upload asks whether to set "Waiting for Approval", and skipping leaves status alone. Pin the file and reply twice. Edit your reply. Delete the first comment: it shows "deleted", and its changelog line carries no text. Upload a final into `products`: delete now refuses and offers archive. The old markup and notes routes answer "Nothing here", and their tables are gone.

- **P4: Bring people in.** Seats, invitations, applications and the `participants` file.
  Features: seats-handshake (seats with requirements; invite → accept; apply → accept → accept; 10 applications a day per user; a filled seat refuses; pending applications close when it fills), the roles Creator, Client and Commission Admin (with the owner-only reserve and ownership transfer), seat visibility ceilings, `participants` shown at the viewer's standing, each participant's own title for the commission. Proposal: "Waiting on you" (question 3).
  Depends on: question 1 ruled. Identity, for finding a user's DID from their handle to invite them. Den, for a new participant's mount (0.1's read side) and for ceilings mapped onto card, node and subtree. Moderation, for the blocked-pair check (we add the hook; it's enforced once Blocks land). Web.
  Done when: as alice, declare a Client seat ("send your refs") and invite bob. As bob, "Waiting on you" lists it; accept, and the commission appears in bob's `commissions`. As carol (dev profile, or a second Bluesky account), apply to a vacant Creator seat; alice accepts; carol accepts and is seated. Carol's 11th application that day is refused. Alice makes bob Commission Admin, and bob still can't archive. `participants` shows the roles to bob and "Nothing here" to a stranger. Every act is in the changelog.

- **P5: Slots that hold characters, with the keeper's consent.**
  Features: slots (open; filled with a character on Zurfur; filled with an off-platform character; a slot's own files; the slot note as a comment), the keeper's request → accept, and "This character is private" with Request Access.
  Depends on: identity, for character storage, keepers and a way to make a character (all left out of 0.1 by your ruling on D-1), plus the keeper's list of requests. Den, for slot folders and the files inside them. Moderation, for slot files going through the same scan gate. Web.
  Done when: alice adds an open slot "Hero" and an off-platform slot "Kael" with a back reference. She asks for bob's "Abco" in "Hero". Bob sees the request in "Waiting on you" and accepts, and "Hero" shows Abco. Bob makes Abco Private: alice's slot shows "This character is private" with Request Access. A second request is refused, and bob's decline is silent.

- **P6: The board.** A Workflow with columns, and commissions placed and ordered on it.
  Features: workflows (create one; columns are child nodes, each with its own level; a column can be archived; one board per Account in the alpha), board-ordering (fractional keys never renumbered, placement after an anchor, one card per board, a move keeps the card's alias and archived state, the soft column cap), each card's own alias and archived state, and the board's default visibility filled into a new commission's form.
  Depends on: den, for symlinks (a card shows at the lowest of its own level, its column's and its commission's, and opening it lands on the commission's real path). Identity, for members' standing on the Account. Web, for the board with drag and drop. Question 7 ruled.
  Done when: in Supreme Arts, create "Queue" with Open, Sketching and Done. Place "Test 1" in Open and a second commission after it. Drag the second to the top, refresh, and the order holds. Move Test 1 to Sketching. Rename its card "Abco ref" on this board only. Archive the card: it leaves the board, and the commission is unchanged. A member who isn't a participant gets no card for a Private commission. No board act writes to the changelog.

- **P7: Who else sees it.** View grants, bringing a commission into an Account, Listed and Public, and the brief.
  Features: view-grants (Presentation gives a card, Description opens the commission node, Total opens everything; revoke is quiet), bringing a commission into an Account (the record the corpus marks as an MVP-only exception), surfaces (frozen, placed as question 1 rules), client-progress (a Listed card on a public board), "make Listed or Public" with the maturity rating required first, and an Account's bookmarks of outside commissions.
  Depends on: den, for view-grant mounts, Listed cards and the act that widens a node. Identity, for an Account's `commissions` folder holding members' mounts, and a Public Account. Web.
  Done when: grant dave Description: he sees the commission's header and its open seats, but no files. Raise it to Total: he sees the files. Revoke it: the commission leaves his Den. Bring it into Supreme Arts: every member sees it, and the changelog says so. Making it Listed asks for a maturity rating first. With Supreme Arts Public, a signed-out browser sees the card on "Queue" (title, maturity, column) and can't open it.

**Exit, before the alpha opens:**
- The loop above works after your real sign-in on `just dev`, from PostgreSQL, and on the dev profile.
- No commission read or write goes around the Den. Today's routes outside it are retired: notes, drawn markup, placement by index and the routes outside the contract. (The owner's list goes in 0.1.)
- `commission.proto` carries every commission act the alpha ships. You review it as a whole, because the first alpha tag closes it.
- You've reviewed every shape under "One-way doors" that closes at A1.
- Security: the security reviewer (Heron, at xhigh) has no open must-fix on any slot. Moderation's entry kinds and its redaction path write into our changelog port. An upload quota is in place (proposal 2).

## Alpha

- **A1: Close the loop.** The Request for Approval, and invoices settled by hand.
  Features: request-approval, invoices (issued with lines prefilled from slots; never edited, only voided and reissued; the issuer's "paid" mark is a fact; the payer may mark "sent"), and our half of reputation (on Completed, both sides are asked to rate; a rating is registered as a fact).
  Depends on: whichever team owns reputation, for the rating itself. Web.
  Done when: as the Creator, "Request approval" sets "Waiting for Approval". As the Client, "Approve" is recorded and the status clears. Issue an invoice: two filled slots prefill two lines. Editing it is refused; void and reissue instead. Mark it paid: delete now refuses. Move the commission to Completed: both sides are asked to rate. Every act is in the changelog.

- **A2: Ask for Art.** The client writes the ask, and an artist applies by link.
  Features: ask-for-art (a commission the client creates, with a vacant Creator seat and the "Looking for Artist" tag), tags (owner tags on commissions; the reserved status tags). "Commissions Open" on an Account is identity's half.
  Depends on: den, for attributes in their namespaces, read only through Atta. Identity, for tags on Accounts. Web.
  Done when: as a client, create "Need a ref sheet" with a vacant Creator seat, requirements and the tag, and make it Public. Send the link to a second account. It opens, shows the ask, and lets them apply; the handshake seats the artist, and the client stays owner. Removing the tag ends the ask. A Private commission's tags never show to a stranger.

**Exit:** the alpha cards in our area are built: commission-core, slots, seats-handshake, markup, changelog, surfaces, view-grants, client-progress, workflows, board-ordering, invoices, request-approval, ask-for-art, tags, plus our halves of fact-deletion, closed-door and reputation. You've reviewed each door the alpha slots opened, and the closed group's commission fixes have landed.

## Beta

- **B1: A richer work loop.** Phases you check off, the client approving a phase, and a discussion on the commission page.
  Features: phases (an ordered list declared at creation and prefilled from the board; check-off is explicit; the client's approval reuses A1's act; a file may name its phase; a prompt, never an automatic move), commission-comments (comments on the commission page with no anchor; never public replies).
  Depends on: web.
  Done when: a commission created from "Queue" starts with the board's phases. Check off "Sketch". As the Client, approve "Sketch". Tag an upload "Lines". Comment on the commission page and reply. No status moves by itself.

- **B2: The changelog hash chain.** Tamper-evident history.
  Features: hash-chain. Each entry's hash covers the one before. Alpha entries are chained once from a starting point. An operator check names the first broken entry, and a redaction keeps the hash while removing the content.
  Depends on: moderation, for how a redaction fits the chain.
  Done when: the check passes on a commission. Alter one entry by hand on a dev database, and the check names it. A moderator redacts an entry, and the chain still verifies.

- **B3: YCH.** A template that clients claim.
  Features: ych (a prepared commission with open slots and a vacant Client seat, copied for each buyer).
  Depends on: none beyond P4 and P5. Who owns the copy is your call when B3 starts.
  Done when: alice marks "Winter YCH" as a template. As bob, claim it: a new commission appears with the template's content and open slots, and bob in the Client seat. The template is unchanged.

- **B4: Commission trees.** A big piece split into child commissions.
  Features: trees (at most one parent, checked for cycles; Admin or owner creates children; detach and re-parent; a parent's facts include its children's; a child is never wider than its parent; bulk acts on a subtree run in the frontend), and inert relationship labels.
  Depends on: den and you, for how a parent's limit on a child composes with the Den's level rule (open on the Den page). Web.
  Done when: create the child "Lineart" under "Big piece". Seat a second artist in the child only: they see the child, not the parent. Deliver a product in the child: deleting the parent refuses. Detach the child: it stands alone.

- **B5: Golems in commissions** (with the plugin team's plugin API).
  Features: our half of plugin-api (a Golem sits only in a seat and is bounded by its ceiling; plugin-written content goes through the one write gate; a plugin appends to the changelog with its own sentence, rate-limited). Our half of achievements (commission events for EXP) and of kafka (those events through the outbox). Retiring the membership-view exception before Golems join Accounts (question 8).
  Depends on: plugin, for the API, Golem identity and the "write to the changelog" scope. The EXP owner. The event backbone's owner.
  Done when: install a test Golem on Supreme Arts and invite it to a Description-ceiling seat. It appends "Bookkeeper sent a payment reminder" and can't read the brief. A Golem member of Supreme Arts sees no commission that was brought into the Account.

**Exit:** phases, commission-comments, hash-chain, ych and trees are built, plus our halves of plugin-api, achievements and kafka. The membership-view exception is gone.

## Release

- **R1: Disputes, for real work.**
  Features: disputes (any participant raises one; deadlines freeze; the steps are direct, then mediation after 7 quiet days, then arbitration; the outcomes; a warning whose public display the wronged party chooses, and which expires; the grace windows; paid-marks frozen while a dispute is open). Proposal: a participant exports the commission (proposal 3).
  Depends on: moderation, for mediators and arbitrators. Identity, for warnings on the profile. The notifications owner, so the other side hears of it.
  Done when: as the Client, raise a dispute on an Active commission: the deadline freezes and "Late" stops. As the Creator, marking the invoice paid is refused. After 7 days, request mediation. A moderator records "No fault", and the commission returns to Active.

- **R2: Open for commissions, safely.** Capacity guards and the openings feed.
  Features: capacity-guards (an artist sets a capacity on the board, and going past it warns), openings-feed (Listed and Public openings as a feed and RSS, never filtered on anything a card hides).
  Depends on: den, for queries that count and filter only what the viewer can see. Whoever owns the Lens engine. Whether a platform-wide feed may exist at all is open on the Den page; it's yours when R2 starts.
  Done when: set "Queue" to 3 with 3 Active: accepting a 4th warns. Supreme Arts' opening leaves the feed while it's full, and an RSS reader shows the openings.

**Exit:** disputes, capacity-guards and openings-feed are built, and our halves of notifications, telegram and trello-import are served (below).

## One-way doors, and the slot that closes each

| Door | Why it's a door | Closes |
|---|---|---|
| Comments replace the drawn markup and the free-text notes | The Comments & Markup decision says the old markup table goes "before any row ships". Dropping your own pre-alpha markup or notes rows deletes your data, so that PR comes to you (D-36). | P3 (its PR); A1 at the latest |
| Each changelog entry's payload, including "file added" carrying the file's id, never its name | Entries are append-only and never rewritten, and the beta chain hashes them | A1; later kinds when they ship |
| Where the brief, budget and seats live, and how Presentation, Description and Total map onto card, node and subtree (question 1) | Stored shape, contract, and what every viewer sees | Ruled in P2, built in P4 and P7, closes at A1 |
| `commission.proto` | The alpha is the first published client; the first alpha tag closes the contract (D-19) | First alpha tag |
| Slots, and the keeper-consent record | Stored | A1 |
| Card storage: the symlink, the position key's format, the alias, the archived state | Stored; position keys are never renumbered | A1 |
| Seats, applications, invitations, roles | Stored | A1 |
| The record that brings a commission into an Account (MVP-only) | Real members gain view through it; removing it changes who sees what | Opens at A1; removal planned in B5 |
| Invoices; the approval act | Invoices are immutable facts; phases reuse the approval act | A1 |
| The status tag names (`system.` namespace) | Asks and the later feed match on them | A2 |
| The chain's hash input; phase, YCH and tree storage | Stored, and hashes are kept | B1–B4, each in its own slot |
| Published records | None in our area. A commission never goes to atproto; publishing a product is the posts team's (beta). | — |

## Cross-team dependencies (what you need, from whom, by which slot)

| What we need | From | By |
|---|---|---|
| Named actions routed through Atta to our handlers; a commission's payload served and verified; the changelog as a typed file with pages | den | P2 |
| Files stored once by content hash, with an upload that looks the same either way | den | P3 |
| The scan gate and its pending area in front of commission uploads | moderation | P3 |
| Fixture DIDs agreed for every seeded world, as in 0.1 | identity | each slot |
| Finding a user's DID from their handle, to invite them | identity | P4 |
| A block-pair check for invitations, applications and slot requests | moderation, with identity reading blocks | when Blocks land, before A1 |
| Character storage, keepers, making a character, a keeper's request list | identity | P5 |
| Slot folders and the files inside them, within the depth cap | den | P5 |
| Symlinks: the lowest-of rule, opening onto the real path | den | P6 |
| Members' standing on an Account; a Public Account; an Account's `commissions` folder with members' mounts | identity | P6, P7 |
| View-grant mounts; Listed cards; the widening act | den | P7 |
| Moderator entry kinds and the redaction path. We build the changelog port; moderation calls it with its audit record. | moderation, with us | before A1 |
| Deployment for the alpha | internal | before A1 |
| Who owns reputation (the rating and its totals). We provide the Completed prompt and the fact. | Orchestrator, to name the owner | A1 |
| Attributes in namespaces, read only through Atta | den | A2 |
| Tags on Accounts ("Commissions Open") | identity | A2 |
| How the redaction rule fits the chain | moderation | B2 |
| A ruling on how a parent's limit composes with the Den's level rule | den, then you | B4 |
| The plugin API, Golem seats, the scope to write to the changelog; the event backbone; the EXP owner | plugin; internal; to be named | B5 |
| Mediators and arbitrators; warnings on the profile | moderation; identity | R1 |
| Queries that filter only on what the viewer can see; the Lens engine's owner | den; to be named | R2 |
| The Workflow use cases live under `application/src/account/workflow`, outside our charter's paths. Add that path to our charter, or agree it with identity. | Orchestrator | P6 |
| **What others need from us.** Posts: a product's bytes for publishing (beta), and a commission's public face for Portfolios (alpha). Plugin: commission create and card placement for the Trello importer, and commission events plus the linked channel for the Telegram bot (release). The notifications owner: commission events as one of its intakes (release). Web: an act and a read for each slot. | — | as named |

## Not on the board (proposals, each with why)

1. **"Waiting on you" (P4).** A plain list of your pending invitations, slot requests and approval requests. The My Den rulings give a pending invitation no mount and no card, and say notifications announce it. Notifications ship at release, so without this list nobody in the alpha could find an invitation except through a link.
2. **An upload quota per user (by P3, before the alpha).** Commission files have a 25 MiB cap per file and no total (an open row on the Den page). Real people's storage and abuse make a total necessary before the alpha.
3. **Export a commission (R1).** A participant downloads the record they're allowed to see: the changelog and the files. Real work needs it, and the end-of-release compliance review may ask for it (not verified). The changelog page also lists a privileged "full read for legal export" with no owner.
4. **Finish the markup rules for real use (R1 or R2).** Whether pins survive a replaced file, who may annotate what, and how long comments are kept after a commission closes. These are open rows on "Blocking Gaps for v1"; the alpha deletes pins along with their file.
5. **Retire the MVP-only membership view (B5).** It was ruled "to be removed after the MVP", and the reason it was allowed (no Golems in the alpha) ends when Golems join Accounts.
6. **Gift, fan and surprise art, and a keeper's standing permission ("anyone", "ask me", "nobody but me").** Approved for the roadmap and unplaced. Proposed: after R2, because each one gives up a keeper's protection.
7. **Your notes for later stages.** A storage-routing crate isn't needed for us: commission data is all private storage, and the first strain would be the posts team's publishing. External-storage mounts don't reach commission files in any slot here, because a Private commission can't be enforced on storage we don't control.

## Questions for the Engineer (each pre-answered)

**Needed during P2 (they block P2 to P4):**

1. **Where do a commission's brief, budget, seats and the rest of its content live in the Den?** This is open on the Den page ("Shape of the Commission directory"); it blocks P4 and closes at the alpha. *Proposed:* each piece lives at its level, the same way view grants work.
   - **Presentation** content is the card: title, maturity, stage.
   - **Description** content is the commission node's own payload, so everyone who can open the node sees it, as the My Den rulings require. That includes a vacant seat and its requirements.
   - **Total** content (brief, price, references, the linked channel) is a sixth fixed entry, a Private `brief` file that shows at the viewer's standing, like `participants`.

   Surface and tab settings become groups inside those homes, and the alpha freeze holds. A seat's ceiling then caps its occupant at card, node or everything, as a grant does. Recorded through a decision page.
2. **How do commission acts appear in the contract?** *Proposed:* typed calls in `commission.proto`, each naming the commission by its id (its node key), one call per named action. Not one generic "act on a node" call, which would carry untyped data. To be agreed with the den lead.
3. **How does someone see a pending invitation in the alpha, when notifications come at release?** *Proposed:* proposal 1, "Waiting on you", and the link sent off-platform stays the main way people arrive.
4. **Can a commission move to "Disputed" in the alpha?** Project MVP lists Disputed in the alpha lifecycle, but the Commission page leaves that move to the disputes work, which is release. *Proposed:* yes (P2). Any participant can make and withdraw it as a plain move that freezes the deadline. Tiers and warnings wait for R1.

**Needed later (asked at their slot; listed now so none is a surprise):**

5. **What is the Request for Approval? (A1)** *Proposed:* the Creator's "approval requested" sets "Waiting for Approval", and the Client's "approved" (naming the request) clears it. Both are changelog entries. Neither is a fact, and neither moves the lifecycle. Phases (B1) reuse the act with a phase named.
6. **How is a keeper's consent stored, and when does a slot count as a character fact? (P5)** These are open rows on "Character Deletion". *Proposed:* a slot request is its own record beside the seat application, run by the same request → accept steps. A commission is active while it's Draft, Batched, Active or Disputed and not archived, so a slot in it counts as a character fact.
7. **Where do a card's position, alias and archived state live? (P6)** This is open in the My Den rulings. *Proposed:* in the card symlink's own payload. A move is one write that changes its column and position and keeps its alias and archived state.
8. **What happens to the MVP-only membership view when Golems join Accounts? (B5)** *Proposed:* remove it. Each member's view becomes a per-user grant at the level the owner picked, logged once, and Golem members get none.
9. **A dispute withdrawn within 10 minutes "leaves no permanent changelog entry".** That line on the Commission page contradicts an append-only changelog. *Proposed (R1):* the entry stays, marked as a quick withdrawal, and it never becomes a warning.
10. **Which card is the beta "replies"?** The Comments & Markup decision moved private replies (pin threads) into the alpha, and we build them in P3. *Proposed:* the board's "replies" card is the public reply post, which is the posts team's and stays beta.
