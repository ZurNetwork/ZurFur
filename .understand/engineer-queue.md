# The Engineer's queue

Kept by the Orchestrator. **The item at the top blocks the most work.** Each item is one specific task.
**Every question already carries Claude's answer.** Adjust any answer you disagree with, in the file or in chat. One-way doors are marked; they wait for your review. Last updated: 2026-10-11.

New rules on 2026-10-11:
- You review only PRs into `main` (and the corpus's and Vulpes's `main`).
- Copilot and QA review slice PRs, plus security when it applies, and the Orchestrator merges them with a merge commit.
- PRs into `main` squash. After #259 they go through a merge queue.

---

**1. Merge #266 (docs bundle) into `main`.**
1. **Run** `cd ~/code/zurfur && gh pr update-branch 266; sleep 15; gh pr checks 266 --watch --fail-fast && gh pr merge 266 --squash`
   Done when: #266 shows as merged.

**2. Read den.proto on #255 (D-19).** It is held for you; the Orchestrator merges it into `feature/my-den` after you've read it.
1. **Open** GitHub → #255 → `Files changed` → `den.proto`.
   Done when: you say "OK" or what to change.

**3. Merge design#47 into the corpus.**
1. **Run** `gh pr merge 47 -R ZurNetwork/zurfur-design --squash`
   Done when: design#47 shows as merged.

**4. Merge #259 (toolchain + CI + merge queue).** *Not ready yet.* Pika is folding in #236 and #244, then Hoopoe re-checks it. After it merges:
1. **Run** `cd ~/code/zurfur && jj git fetch && jj rebase -s ywryypxk -d main@origin` (moves your checkout onto `main`).

---

## Questions (each pre-answered; adjust any)

- **A Character's one ownership edge vs. "public character, anonymous owner".** The corpus (Characters on ATProto) says a Character has exactly one ownership edge, always Character ↔ User. Its flow 13 has no public owner record; ownership is proven by a holder-held credential or a key-control challenge. **Claude's answer:** keep the one-edge rule. Flow 13 is the case where the edge exists but is never published, and it's proven privately. *(Vulpes #30 follow-up; it touches the corpus.)*
- **SEC-3: a hidden real child can hide the viewer's own mount of the same name.** **Claude's answer (Heron's (a)):** mount names and real names never collide. Needed before the first folder that holds both (bookmarks, commissions brought into an Account, links).
- **A claim column for the sign-in binding.** **Claude's answer:** no column; clearing the binding stays the single-use marker (Hedgehog agrees). A `claimed_at` for a sweeper can come later. Fox builds #242 on this answer.
- **A tombstoned Character under `include-deleted`.** **Claude's answer:** a later narrow, name-only read; not in this slice.
- **The hosting pick (R-56) and the staging domain's name (R-58).** Beaver is widening the comparison. The pick and names come to you with Hoopoe's QA.

---

**Later:** your Explorer click-through now happens on the `feature/web-explorer-shell` → `main` PR, after #249. Optional: `! sudo pacman -S --needed typst`.
