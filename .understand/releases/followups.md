# Follow-ups found during 0.1 Phase 1

Kept by the Orchestrator. Each gets an owner and a version when the roadmap is cut.

- **No problem+json on routing misses** (found by Jay, QA of #254). The `api` crate has no fallback for 404 or 405, so a missing route or wrong method answers without the RFC 9457 body the error-model decision requires. This predates 0.1. The owner is shared ground (the router's composition), so the Orchestrator assigns it. Proposed: 0.2, one slice with a test per status.
- **Den route pin count collision** between `[my-den][2]` (9 → 11) and `[my-den][6]` (9 → 8). Whichever merges second sets it to 10.
- **WP-23's squash** names #254's declared baseline reset (the tech lead's call, 2026-10-10).
- **The API Response Shape page's error registry** lacks the two new den errors, `node_not_found` and `unknown_parameter` (Badger). Fold them into a design-sync.
- **`frontend/web/src/lib/server/accounts.ts:35`** says "exactly 9 endpoints" (flagged to Fennec).
- **`MemProfileSource`** answers one fixed profile for every DID. The dev profile (WP-21) should read the fixture profiles from the seeded cache (Stoat).
- **`Workflow::find` still checks `WorkflowName` on load** (Raven). Once WP-18 tightens the name rule, a legacy board name would fail `find`. Fold into WP-18.
- **`FileName` still uses a custom `try_new`** instead of `FromStr`/`TryFrom`. Fold into WP-18's name-rule rewrite.
- **#262 and #258 conflict** on `adapter-mem/src/fixture.rs`, the `pub mod fixture;` line and the regenerated `adapter-pg/src/queries.rs`. The second to merge restacks: one line, then `just gen-queries`.
- **Build contention:** rustc hit EAGAIN and out-of-memory with many agents building at once (Raven, Mole). `CARGO_BUILD_JOBS=4` works around it. This is logged for Internal.
- **Create's read-back loads the owner's whole list** to find the new row (`create.rs`). Read it back by id instead (Wolf, after #254). Small; commission owns it. Proposed: 0.2.
- **WP-19 inputs (from Badger; Vole isn't on the board yet):** every 422 cause uses `Problem::invalid_query()`. Add a test that `GET /den` and `GET /den/x` answer the 401 problem without a session, proving the routes the pin claims are mounted. Pass both to WP-19's builder in their brief.
- **The session layer is duplicated** between `main.rs` and the PostgreSQL test setup (Badger declined extracting it in #256). Proposed: a small slice to share it, so the 7-day expiry can't drift.
- **Migration `20261009002000_client_session_by_session_id`** on the identity branch has a round-minute stamp (possibly hand-typed), but it's unique across all refs, so it's left as is (renaming would break dev databases that applied it).
- **Login CSRF / cleanup:** add "a failed sign-in's leftover session row" to the TDD's parked item "Sign-out leaving OAuth sessions stored" (Fox, #240).
- **Tombstoned Characters under `include-deleted`:** a later narrow read returning a name-only type (Stoat and Magpie). An Engineer question when the tombstone slice comes.
- **Vulpes `docs/acp.md` still names the claim's other party `subject`** in three places: the claim record's payload example, "Relationships", and the paired-verification steps. R-28 says `did`. It's binding spec text, so it needs its own change with a changelog entry. Reynard, after Vulpes #30 merges.
- **WP-16's gate S must require** (Mongoose on #258): the arm takes the id for both identity reads only from `Viewer`; a recording-port test shows both reads are called only with the viewer's DID; the user arm gives standing at `/user/<did>` only when that DID is the viewer.
- **WP-18's gate S must require** (Mongoose on #262): participant mounts only from `Viewer`, with a recording-port test; a file node resolves through the commission-scoped link before `filename(key)`, and another commission's key gets the one not-found with equal statements; workflows, files and slots are read only beneath nodes where Atta found the viewer's standing.
- **A hard delete never deletes the file blob** (Mongoose; predates 0.1). The delete use case doesn't call `FileStore::delete`, so the bytes and the name stay in `file_blob`, readable by key. Commission owns it; the `just dev` walk (WP-39) checks it. Proposed: a 0.2 fix.
- **WP-14 must build one per-request, per-viewer record** for every soft-delete arm, because `admit` gets neither the viewer's role nor the archived state. Until WP-16, every deactivated Account is refused for everyone. WP-16's equal-statements test includes "absent" against "deactivated, viewer not the Owner" (Heron).
- **Carry-over row 66:** the amendment's table in `.understand/20261006-my-den-architecture.md` (line 696) still refuses an archived commission opened through a view grant. The code follows the TDD, which is right. Badger corrects the amendment before WP-14 reads it.
- **Merge note:** merge #257 right before #260, with nothing using `atta` in between (Heron).
- **Identity stack merge order:** slice 4 (#240) must never reach `main` without slice 6 (#243), whose hickory log filter keeps the handle out of warn/error logs. They land together as one feature squash.
- **Debug logs carry hosts and the authorization server's error body word for word.** Don't turn debug on in production without filtering (Hedgehog).
- **Vulpes `characters-atproto.md`:** flow 1 and "Three facts must agree" still describe the one-sided claim shape. Flows 2 and 3 say a private Character has no DID until publicize, which contradicts Actor Addressing (every Character gets a DID at creation). The tech lead's call: a follow-up PR, not #30 (Rook).
