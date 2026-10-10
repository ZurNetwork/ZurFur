# The Engineer's queue

Kept by the Orchestrator. **The item at the top blocks the most work.** Each item is one specific task.
**Every question already carries Claude's answer.** Adjust any answer you disagree with, in the file or in chat. One-way doors are marked; they wait for your review. Last updated: 2026-10-10 (0.1 signed; Phase 1 building).

---

**1. Reconnect the board, then merge four PRs.** *The new `board` is installed (2026-10-10). Trunk mode turns on once #252 is merged, because until then the trunk's config lacks the compliance, architects and security teams, and they'd close.*
1. **Run** `/mcp` → `plugin:agent-board:board` → Reconnect.
   Done when: `board sync --help` works in this session's tools.
2. **Merge** #252 (three teams), #244 (the Rust pin; Internal's CI PR stacks on it), then #236, #237 and #250 into `main`.
   Done when: all five show as merged. I then turn on trunk mode and restack what sits on them.

**2. Click through the Explorer shell, on fake data.** *The UI you asked for. QA'd by Crow and cleared by Heron. PRs #245 → #246 → #247 → #248 → #249 are open. Copilot recommends approval on all but #247, which has one last fix in progress. I'll put the five up for merging, in order, once that fix is verified.* I set up a workspace of your own, with the web dependencies installed.
1. **Run** `cd ~/code/zurfur-workspaces/zuri && just dev-mock` (it already sits on the newest shell slice)
   Done when: the dev server prints a local address.
2. **Open** http://127.0.0.1:5174 and **click** "Open My Den". You're signed in as alice.
   Done when: the tree shows `~` with its folders.
3. **Try** the tree with the keyboard (Tab in, arrows, a letter to jump), then the mouse (a click opens a folder; only the arrow expands it).
   Done when: it behaves like a file explorer.
4. **Turn on** "Show archived and deactivated".
   Done when: "Winter YCH" shows "Archived" in commissions and "Former Studio" shows "Deactivated" in accounts.
5. **Open** `/den/commissions/0192d00d-0b16-7000-8000-0000000b16b1/attachments`, then **click** "More".
   Done when: 25 entries become 40, in place.
6. **Narrow** the window to phone width and **use** ☰.
   Done when: the drawer opens with the tree.
7. **Tell me** anything that feels wrong, in plain words. Your answers to the shell's questions are in `.understand/20261008-web-explorer-shell.md` → "Questions for you" and "Questions from the build" (Q1 to Q10, all pre-answered).

**3. Merge the PRs that are ready.** CI is green on all of them and Copilot is clean.
1. **Merge** #236 (the `cid` cleanup), #237 (three team rules in CLAUDE.md), #244 (the Rust version pin) and #250 (agents talk on the board; once Copilot has reviewed it) into `main`.
2. **Merge** #238 (the domain words) and #239 (the guarded client) into `feature/identity-resolver`.
   Done when: all five show as merged. I then open the resolver PR, and after it #240 → #241 → #242 → #243 come up in turn.

**4. den.proto: approved as a whole (D-6, 2026-10-09).** *Your answers to D-4, D-7 to D-10, D-14 and D-16 go into it. Under D-19 you still read the text itself on its PR, when "den.proto into the contract" opens it. Nothing to do until then.*

**5. Optional: install Typst,** so you can recompile the documents yourself. I compiled these PDFs with a throwaway build in my scratch folder (Typst 0.15.1).
1. **Run** `! sudo pacman -S --needed typst`
   Done when: `typst --version` prints 0.15.1 or later.
