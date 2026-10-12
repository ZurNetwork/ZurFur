# Roadmap: posts

Lead: Bowerbird · Stages covered: pre-alpha (after 0.1), alpha, beta, release

## In one screen

- **After 0.1:** posts has built nothing. The `posts` folder is the honest empty stub that den builds and posts reviewed.
- **Pre-alpha (P2 to P4):** posts' part of "security ASAP" comes first.
  - **P2:** every private byte moves into the gated blob store. Bytes are stored once, by content hash, behind moderation's scan gate, and read only through the Den's access check.
  - **P3:** a safe path for public bytes. They come from a separate host with no cookies, images are re-encoded, and no byte is public before a scan says "clear".
  - **P4:** a minimal Gallery of Public products (question 4), newest first, blurred by default. It never shows a Listed card.
- **Alpha (A1):** Portfolios. As proposed in question 7, each is a set of commissions you pick, shown at each visitor's level, so a commission made Private drops out everywhere. The beta's one-way doors get ruled before the beta builds anything.
- **Beta (B1 to B4):** Gallery Posts in the poster's own PDS. First a standalone post, then a post made from a product (snapshot, credits, one Gallery item per image), then public replies and shouts, then posts written in other apps, scanned before Zurfur serves them.
- **Release (R1 to R3):** Collections and artist lists, Dynamic collections and Gallery search on the Lens engine, then durability for real work.
- **Never relaxes:**
  - no public byte before a "clear" scan;
  - nothing private reaches a PDS;
  - the Gallery lists nothing a viewer can't open;
  - every read goes through the Den's one access check.
- **Needs you first:** the blob store's shape and its deletion window (questions 1 and 2), before P2 starts.

## Pre-alpha

- **P1 (0.1, fixed).** Posts builds nothing. Its only task is to review the `/post/<did>` stub, which answers the same bytes for any DID. Kea runs the "no post or public-blob byte moved" checks before your acceptance.

- **P2: every private byte lives in the gated blob store.**
  - **Features:** security-asap (its "gated blob store" item).
  - **What it builds:**
    - one byte-store port, with an S3-compatible adapter and a local one for `just dev` (Inventory Closures, item 6);
    - bytes keyed by content hash and stored once;
    - an upload that looks the same whether or not the bytes were already stored (The Den, decision 18);
    - bytes deleted only when nothing points at them, after a grace window;
    - pins, so moderation's pending and held bytes are never collected;
    - today's PostgreSQL mock byte table (`file_blob`) retired, with its bytes moved into the store.
  - **Depends on:**
    - commission: its upload and download move onto the store;
    - moderation: its P2 scan gate keeps its pending area on these pins, so the two land in the same slot;
    - the tech lead: who owns which part (see Cross-team dependencies);
    - you: questions 1 and 2.
  - **Done when** you run `just dev` and:
    1. you upload one image to two commissions, and both uploads get the same response while the bucket holds one object, named by its hash;
    2. you delete both files, and the object disappears only after the grace window;
    3. a stranger who asks for the file gets the one not-found;
    4. `rg -n file_blob backend/crates --glob '!**/migrations/**'` prints nothing.

- **P3: public bytes, served safely.**
  - **Features:** security-asap ("hardening", and the serving side of the gated blob store), plus proposal: a separate host for user bytes.
  - **What it builds:**
    - one byte route, which serves a file at the viewer's level through the Den's access check. Nothing reads a node except through Atta (The Den, decision 4).
    - Public images shown inline come from their own cookieless domain. Only raster types are shown, decoded and re-encoded with their metadata removed (OWASP File Upload Cheat Sheet: "decode and re-encode… explicitly removing unnecessary metadata"; "store the files on a different host"). Every other type is served only as a download, with `nosniff`.
    - A byte without a "clear" verdict is never served to anyone without standing. That includes files uploaded before the scan gate existed.
  - **Depends on:**
    - commission: a "make public" action on a product, which widens its ancestors (The Den, decision 12) and requires a maturity rating (Maturity Vocabulary, decision 4);
    - moderation: a readable verdict for each content hash;
    - the deployment owner: the second domain, with TLS;
    - you: question 3.
  - **Done when:**
    1. you make a product Public, and a signed-out browser loads it from the user-content domain without sending `zurfur.sid`;
    2. the loaded copy carries no metadata (`exiftool -G` shows no EXIF, XMP or IPTC group);
    3. a Public SVG or HTML file downloads and never renders inline;
    4. a Public file uploaded before the scan gate stays unserved until it has been scanned.

- **P4: a minimal Gallery.**
  - **Features:** gallery-view.
  - **What it builds:**
    - one page of Public items, newest first: Public products, joined by Public Characters once the Character slice gives them a public node with an image (question 4);
    - cards that show only what is itself Public (question 6), with unrated items left out;
    - blurring by default, behind an "I'm 18+" reveal;
    - results that come from a derived index that can be rebuilt, and pass through Atta (The Den, decision 27). So moderation's holds and every privatization apply with no Gallery code of its own;
    - never a Listed card: listing Listed items platform-wide is what the Den's traverse-only top level forbids (The Den, decision 5);
    - no count that includes anything the viewer can't see (The Den, decision 26);
    - a report link on every item.
  - **Not in this slot:** tags, search and the Lens engine. The Gallery stores nothing durable.
  - **Depends on:**
    - den: a query whose results pass through Atta;
    - commission: product ratings and "make public";
    - moderation: holds through `admit`, and the report path;
    - the owner of the maturity-labels UI (maturity-ui): the viewer's sensitivity setting;
    - web: the page;
    - you: questions 4 to 6.
  - **Done when:**
    1. a product made Public appears first for another signed-in user, blurred if it is rated above Safe;
    2. you make it Private, reload, and it's gone;
    3. a Listed-only commission contributes nothing;
    4. a moderator hold removes the item;
    5. Heron's review has passed.

**Exit (before alpha opens):**
- Every private byte is in the gated blob store, behind the scan gate. The mock store is gone.
- No public byte is served without a "clear" verdict, and inline public bytes come only from the user-content domain, re-encoded.
- The Gallery shows only Public, rated, unheld items, read through Atta.
- Posts' type names (post, blob and, if ruled, portfolio) are in the type catalog, which is due before the alpha.
- Questions 1 to 6 are answered, and P2 to P4 have passed security review.
- `/post/<did>` still answers the same for any DID.

## Alpha

- **A1: Portfolios.**
  - **Features:** portfolios.
  - **What it builds** (the shape I propose; question 7 decides):
    - A Portfolio is a directory under a new, always-public top-level `/portfolio`, mounted into its holder's `portfolios` folder. Its holder is a User or an Account, each with any number.
    - Its entries are links to commissions, each shown at the visitor's level (The Den, decision 10). A commission made Private drops out everywhere with no bookkeeping.
    - A Public commission shows its Public products. The User page and the Account page show the holder's Public portfolios.
    - This is Option 3 with 3A of the proposed "Portfolio Live vs Static" decision, built from the Den's links. The Den's link rule now gives that option's privacy guarantee for free.
  - **Depends on:**
    - den: the new top-level directory, its mounts and links;
    - identity: room on the User page and the Account page;
    - commission: Public products;
    - web: the screens;
    - you: question 7.
  - **Done when:**
    1. alice builds a portfolio of two commissions, and bob sees both on her page;
    2. alice makes one Private, and it vanishes from bob's view on reload while alice still sees it;
    3. deleting the portfolio leaves both commissions untouched.

- **The Gallery during the alpha** needs no slot of its own. It gains Public Characters when the Character slice delivers them, and it is used by the closed group with holds and reports live.

**Exit (before beta opens):**
- Portfolios are live.
- The beta's doors are ruled (questions 8 to 10), and the `app.zurfur.feed.post` lexicon text is checked against the field list ruled on 2026-07-04. It stays unpublished until B1, because a lexicon publishes only when its feature ships (The Lexicon Registry, decision 3).
- The publish permission set, if identity published it for Claims, already covers posts (see Cross-team dependencies).
- The end-of-release compliance review has looked at the public Gallery: a copyright takedown path and the content policy.

## Beta

- **B1: your first Gallery Post.**
  - **Features:** gallery-posts (standalone posts).
  - **What it builds:**
    - Publishing an image or text as an `app.zurfur.feed.post` into your own PDS.
    - The publish consent at first publish (Publish Consent & OAuth Scopes, decision 2).
    - The image goes through the scan gate and P3's re-encoding before it leaves Zurfur, because once it is in your PDS, Zurfur can't recall it (Boundary Contract, decision 7).
    - Alt text and a maturity rating, both required. Deleting a post deletes its record.
    - Your `posts` folder lists your real posts, and `/post/<did>/<record key>` and `/blob/<cid>` become real nodes. The Gallery reads `/post` as well.
    - The lexicon publishes here and is additive-only from then on.
    - A post deleted in another app leaves the Gallery within a bound you set (proposed: 5 minutes).
  - **Depends on:**
    - identity: an OAuth session with the publish scope, plus the parked fix for stored OAuth sessions that sign-out leaves behind ("before PDS writes use the sessions");
    - moderation: the scan gate covers post uploads (its B2);
    - whoever holds the lexicon authority DID and `_lexicon.zurfur.app`;
    - web: compose.
    - *Your note on a storage-routing crate:* this is the first slice that writes to both stores. If the application layer starts juggling both ports here, this is where that crate would earn its place. Posts doesn't plan it.
  - **Done when:**
    1. a post with alt text, rated Safe, appears in your PDS (in any atproto repo browser) and in the Gallery;
    2. its bytes in your PDS carry no metadata;
    3. deleting it in Zurfur removes it from both;
    4. deleting one in another app removes it from the Gallery within the bound.

- **B2: from commission to post.**
  - **Features:** gallery-posts (snapshot, credits, deduplication, tags).
  - **What it builds:**
    - Publishing a product as a post, with a snapshot taken at compose time that never points back at the commission (Post, rules).
    - Credits prefilled from the participants and removable before publishing. A credited person can hide their credit later (Gallery Posts, decision 3). Credits respect blocks.
    - The commission's changelog records the publish.
    - Byte-identical posts by several people show as one Gallery item with several sources (Gallery Posts, decision 2).
    - Community tags on posts.
    - A Portfolio entry shows its commission's post.
  - **Depends on:**
    - commission: the snapshot read, and a new changelog kind (an addition to the frozen kind list);
    - the tags owner: the `community.` namespace (The Den, decision 26);
    - moderation: credit disavow and hide, and tag delete;
    - the blocks owner;
    - identity: a character's `art` records, which point at posts (The Den, decision 14).
  - **Done when:**
    1. alice and bob each publish the same final, and the Gallery shows one item with both sources;
    2. bob unticks his credit before publishing, and his name appears nowhere;
    3. the record in alice's PDS holds no commission id.

- **B3: replies and shouts.**
  - **Features:** replies.
  - **What it builds:**
    - Public replies on posts and shouts on User and Account pages, as posts with `reply` set in the replier's own repo (The Replyable Trait, decisions 2 and 3), rendered flat.
    - The subject's owner can hide a reply, which suppresses it in Zurfur only.
    - Blocks cut rendering both ways (Blocks, decision 3).
    - No notifications until the release (question 11).
  - **Depends on:** the blocks owner, moderation (its B2 hide hold), identity (the pages) and web.
  - **Done when:**
    1. bob replies to alice's post, and the reply shows under it and lives in bob's PDS;
    2. alice hides it, and it leaves Zurfur but stays in bob's repo;
    3. a Bluesky block between them stops the reply rendering for both.

- **B4: posts made elsewhere.**
  - **Features:** proposal (not on the board). The PDS is canonical (Boundary Contract, decision 2), and the Den expects posts written by other apps (The Den, ruling 15).
  - **What it builds:** Zurfur picks up `app.zurfur.feed.post` records that other apps write into its users' repos. It scans their bytes before it serves or caches any of them, and it drops a post whose record is gone.
  - **Depends on:**
    - moderation: scanning at ingest;
    - identity: the hardened resolver, so fetching from user PDSes can't be turned against Zurfur's network (SSRF);
    - possibly the broker backbone (kafka).
  - **Done when:**
    1. a post created with a raw `createRecord` outside Zurfur appears only after its scan;
    2. deleting it outside Zurfur removes it.

**Exit (before release):**
- Posts and replies are live, and the lexicon is published.
- Deletes made elsewhere are honored within the bound.
- No byte Zurfur serves is unscanned, whichever app wrote it.

## Release

- **R1: Collections and lists.**
  - **Features:** collections.
  - **What it builds:**
    - Static collections of any referenceable thing, hand-ordered: Accounts and Users first, Characters and posts later (Collection as a Referenceable primitive, the coverage row).
    - Favorite, wishlist and will-not-commission artist lists as public records, display-only (that decision's decisions 3, 6 and 7). That adds a new lexicon, which is a door.
    - A member that a viewer can't see drops out for that viewer.
  - **Depends on:**
    - identity: profiles that show the lists;
    - den: a tree home for public collection records ("Other Class A records" is still open in The Den);
    - you: the record's fields, asked at that slot.
  - **Done when:**
    1. alice adds bob's Account to her favorites, and it is in her PDS and on her page;
    2. bob's Account deactivates, and it drops from her list's view.

- **R2: Dynamic collections and Gallery search.**
  - **Features:** collections (Dynamic).
  - **Depends on:** the Lens engine, from whoever builds the openings feed (openings-feed).
  - **Done when:**
    1. a saved "tag:fluffy, rated Safe" picks up new posts;
    2. no viewer ever sees a member they can't open.

- **R3: durable for real work.**
  - **Features:** proposal.
  - **What it builds:**
    - A replica of public blobs, so art survives a lost PDS (The Den, Open, "A replica of public blobs"). It still honors deletes.
    - Retention and quotas reset from real use.
    - The archive flow: delete from the repo, keep Zurfur's history, and name it as such (Boundary Contract, Open, "Gallery archive UX").
    - Export of your posts.
    - The notifications that posting and replies promised, plugged into the notifications card.
  - **Done when:**
    1. a post whose PDS has gone stays visible from the replica, marked as such, until its owner deletes it;
    2. a deleted post leaves the replica too.

**Exit:** artists can rely on the Gallery and their posts for real work: durable, exportable, and notified.

## One-way doors, and the slot that closes each

| Door | Kind | Closes at |
| --- | --- | --- |
| How private bytes are named and stored: the hash or hashes, and encryption at rest | Stored data. A second hash added later means re-reading every byte. | P2, live at alpha open |
| The grace window before unreferenced bytes are deleted | Deletes real data | The first collection run on real data (alpha open) |
| The user-content domain | Security boundary. Public links to it get shared. | P3, live at alpha open |
| The Gallery's API contract | Published contract | alpha open (P4) |
| Where a Portfolio lives in the tree, and its member kind | Stored data and published contract | A1 |
| The publish permission set's contents | Permission | Its first publication, which may be identity's, for Claims, in the alpha |
| Posts are always public (no Unlisted or Private) | Boundary | B1 |
| The post key `/post/<did>/<record key>`, which omits the collection | Stored data | B1, the first stored post |
| The `app.zurfur.feed.post` lexicon: fields, `reply` and labels | Published contract | B1 (publish-late) |
| The `snapshot` field's inner shape | Published contract | Its first publication (B2), additive afterwards |
| `/post/<did>` answers the same for a DID not on Zurfur | Security boundary (DID correlation) | Pinned in 0.1; it must still hold at B1 |
| The collection lexicon | Published contract | R1 |

## Cross-team dependencies (what you need, from whom, by which slot)

| Need | From | By |
| --- | --- | --- |
| Who owns what. Proposed: posts owns the byte store and the byte route, commission owns file nodes and uploads, moderation owns the scan gate. Moderation's roadmap lists the gated blob store as its own P2 dependency. | Tech lead | P2 |
| Upload and download moved onto the byte store | commission | P2 |
| The pending area and holds on the store's pins, landing in the same slot | moderation | P2 |
| "Make public" on a product (with its ancestors), with a required rating | commission | P3 |
| A "clear" verdict, readable by content hash | moderation | P3 |
| A second domain with TLS, which is not a `*.zurfur.app` name, since that is the handle namespace (Domains and Applications) | Deployment owner | P3 |
| Gallery results returned through Atta, so `admit` applies holds | den | P4 |
| **Correction:** my board reply of 2026-10-09 said the Gallery and the byte route would check holds themselves. They read through Atta (The Den, decisions 4 and 27), so `admit` covers them. Moderation's 0.1 page quotes the old proposal. | moderation, den | P4 |
| The report path on every Gallery item | moderation | P4 |
| The viewer's maturity setting and the 18+ reveal | maturity-ui owner | P4 |
| The Gallery page | web | P4 |
| Public Characters as nodes with an image | identity (Character slice) | When it lands |
| Under Character Deletion (decision 2), any Gallery appearance makes a Character permanently tombstone-only. The deletion check needs the Gallery's membership rule (question 4). | identity | P4 |
| A `/portfolio` top-level directory, its mounts and its links | den | A1 |
| Room for portfolios on the User and Account pages | identity | A1 |
| The publish permission set, when first published, carries `repo:app.zurfur.*` and `blob:*/*`, so posting needs no second consent (Publish Consent, decision 3) | identity | Its first publication (alpha) |
| Clearing the stored OAuth sessions that sign-out leaves behind | identity | B1 |
| The scan gate on post uploads (B1) and on ingested posts (B4) | moderation | B1, B4 |
| **Note for the consolidator:** moderation's B2 says "posts on hosted repos can be taken down". Posts are authored only by Users (Post, rules), and Users bring their own PDS, which Zurfur doesn't host. A post's take-down is a scan before upload plus Zurfur-side suppression, not a PDS takedown. | moderation | B2 |
| The snapshot read, and a changelog kind for "published" | commission | B2 |
| The `community.` tag namespace and tag delete | tags owner (den or posts, asked of the tech lead by moderation) | B2 |
| Block reads for credits and replies | blocks owner | B2, B3 |
| A character's `art` records point at posts. The board's character-presence card says "art records once the Gallery exists". | identity | B2 |
| The Lens engine | openings-feed owner | R2 |
| Notifications for publishing, credits and replies | notifications owner | R3 |
| The byte store's backend port is where your external-storage mounts would plug in (your note 2). Posts doesn't build them. | Whoever owns mounts | — |

## Not on the board (proposals, each with why)

- **A separate cookieless domain for public bytes (P3).** User bytes rendered from the app's own origin can run with the app's authority. Google moved such content to isolated origins for that reason ([Content hosting for the modern web](https://security.googleblog.com/2012/08), [Securely hosting user data](https://security.googleblog.com/2023/04/securely-hosting-user-data-in-modern.html); excerpts only, via search). OWASP says to store uploads "on a different host" ([File Upload Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html), read 2026-10-10).
- **Re-encode public images and remove their metadata (P3).** It defangs crafted images and drops metadata a person didn't mean to publish (OWASP, same page). Its own caveat applies: "rewriting does not guarantee that all malicious content is removed", so the separate domain stays.
- **Posts made elsewhere (B4).** Without it, Zurfur shows posts deleted in other apps, and posts written outside Zurfur would skip the scan.
- **Durable for real work (R3).** A PDS copy is publication, not backup (The Den, ruling 26, and its Open row on a replica).

## Questions for the Engineer (each pre-answered, in the order the slots need them)

**Before P2**

1. **Is the gated blob store your CrowFS design, or the corpus's plain port?**
   - Your CrowFS rulings of 2026-09-07 live only in my memory notes, not in the corpus: two hashes (BLAKE3 and the atproto SHA-256), every backend untrusted and checked on read, mark-and-sweep collection with a grace window, encryption at rest recommended. The corpus says "a `BlobStore` port with an S3-compatible adapter" and reference-counted collection (Inventory Closures, item 6).
   - *Answer:* the corpus's port, built to CrowFS's byte rules: both hashes from the first byte (a hash added later means re-reading every byte), mark-and-sweep with a grace window, and encryption at rest with server-held keys. CrowFS's own index isn't used, because the Den keeps meaning in PostgreSQL (The Den, decision 27). Then a decision page records it.
2. **The grace window and the upload quota.**
   - *Answer:* 30 days before unreferenced private bytes are deleted. In the alpha, 2 GiB per user beside the existing 25 MiB file cap, raised as real use shows.

**Before P3**

3. **Do public bytes get their own domain?**
   - *Answer:* yes, for Public bytes only, on a domain outside `*.zurfur.app` with no cookies. Private files stay downloads on the main origin, as today. The one-origin ruling still covers the app itself.

**Before P4**

4. **What does the alpha Gallery show?**
   - (a) Public products, plus Public Characters once they have a public node with an image.
   - (b) Every Public thing, by type: Accounts, Characters, commissions and products.
   - (c) Bring posting into the alpha. That moves a beta card and publishes the lexicon early.
   - *Answer:* (a). It is an art surface over what the alpha already has, and it needs no new record.
5. **Who sees the Gallery during the alpha?**
   - *Answer:* signed-in members only, while the alpha is a closed group. It opens to signed-out visitors at the release. A closed test shouldn't hand testers' work to search engines, and signed-out traffic needs rate limits and caching first.
6. **Does an alpha Gallery item name its artist?**
   - *Answer:* only when the commission's `participants` file is Public. Otherwise the card shows the work and the commission's name. `participants` starts Private and names the client too (My Den rulings, decision 9), so naming the artist by default would decide the client's privacy for them. Credits arrive with Gallery Posts.

**Before A1**

7. **What is an alpha Portfolio, and where does it live?**
   - (a) Hand-picked links to commissions, in a directory under a new always-public `/portfolio`, mounted into the holder's `portfolios` folder.
   - (b) A computed view: all the holder's Public, completed commissions, with no curation and no new directory.
   - (c) Move the Portfolios card to the beta, beside Gallery Posts.
   - *Answer:* (a). A user's own directory is Private, so under the Den's traversal rule a portfolio inside it could never be public (The Den, decision 9). (a) uses the link rule boards already use, and it settles the proposed "Portfolio Live vs Static" decision as Option 3 with 3A.

**Before B1** (asked now, because each is a door)

8. **Are posts always public?**
   - *Answer:* yes. The Post page's "Unlisted/Private as courtesies" note closes as no (The Den, decisions 19 and 20). Anything gated later, such as Subscription access, is private data and not a post. I offer one design-sync.
9. **The post key.**
   - *Answer:* keep `/post/<did>/<record key>`. `/post` holds only `app.zurfur.feed.post` records, so a record key is unique there. Any other public collection gets its own top-level directory, as `/profile` did.
10. **Is the board's beta "replies" card the public reply post?**
    - *Answer:* yes. Its gloss reads "Comments as feed.post replies; Replyable surfaces". The private replies on pins ship in the alpha with markup (Comments & Markup, decision 14). The Project MVP line that asks this gets the same design-sync.

**Before B2 and B3**

11. **Two decisions promise notifications that the board places at the release.** Gallery Posts (decision 4) says publishing tells the other participants. The Replyable Trait (decision 7) says a reply tells the subject's owner.
    - *Answer:* the beta ships without both. The commission's changelog records the publish, and both notifications arrive with the notifications card (R3).
