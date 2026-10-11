# Ask: each team's roadmap, pre-alpha to release

From the Orchestrator, for the Engineer, 2026-10-10.

**The Engineer asked for one roadmap covering pre-alpha, alpha, beta and release, divided by minor version.** Each team lead writes their team's part in one file. A consolidator then merges every team's file into one roadmap, and a QA challenges it before the Engineer reads it.

The Engineer signed the 0.1 product design document on 2026-10-10. The 0.1 tech design and plan are under their review.

## Read first

- `.understand/releases/stages.md`: the four stages and who sees each.
- `.understand/releases/roadmap/inputs/release-board.md`: **the scope of record**. It lists every feature the Engineer sorted into the alpha, beta and release lanes on 2026-09-12. Anything not on it is new, so mark it as a proposal.
- `.understand/releases/roadmap/inputs/engineer-notes.md`: the Engineer's notes for later stages.
- The corpus page "Project MVP": `~/code/zurfur-design/project/589826-project-mvp.md`. Open the corpus pages it names for your features, under `~/code/zurfur-design/`.
- Release 0.1, in `.understand/releases/0.1/`:
  - `pdd.typ` (signed);
  - `plan.typ` and `tdd.typ` (under review);
  - `objectives/<team>.md` and `objectives/join-<team>.md`;
  - `rulings-2026-10-09.md`.
- Your team's charter in `.agent-board.toml`.

## The frame

- **0.1 is fixed.** It is `pre-alpha-v0.1.0`, as plan v4 says. Start your roadmap after it.
- **One slot is one minor version.** Name slots relative to their stage: pre-alpha `P1` (= 0.1, fixed), `P2`, `P3`…; alpha `A1`, `A2`…; beta `B1`…; release `R1`, `R2`…. The consolidator turns slots into real version numbers across all teams. Its default is one continuous minor sequence (0.2, 0.3…) through alpha and beta, with `release-v1.0.0` as R1.
- **Size a slot as one coherent, demoable increment**, about the size of 0.1 or smaller. No dates.
- **What each stage needs from you:**
  - **Pre-alpha:** fast, breaking changes allowed, only the Engineer looks.
  - **Alpha:** a real domain and real people's data, for a closed group. So the shape of stored data and anything published become real one-way doors at alpha. Also, "the MVP is done or close to done": every alpha card on the Release Board must have a home by the end of alpha.
  - **Beta:** a bigger group; the post-MVP roadmap; the beta cards.
  - **Release:** the release cards, and whatever "used for real work" demands of your area.
- **Security and the private/public data boundary never relax with the stage.** The board's "security ASAP" card is due before alpha opens.

## What to write

One file: `.understand/releases/roadmap/teams/<team>.md`, using the template below. Keep the top section to one screen.

```markdown
# Roadmap: <team>

Lead: <name> · Stages covered: <which>

## In one screen
<5–10 lines: what your area looks like at the end of each stage>

## Pre-alpha
- **P2: <outcome>.** Features: <Release Board card names, or "proposal: …">. Depends on: <other team + what, or none>. Done when: <a check the Engineer could do by hand>.
- …
Exit: <what must be true for your area before alpha opens>

## Alpha
(same shape: A1, A2…, then Exit)

## Beta
## Release

## One-way doors, and the slot that closes each
## Cross-team dependencies (what you need, from whom, by which slot)
## Not on the board (proposals, each with why)
## Questions for the Engineer (each pre-answered)
```

## Rules

- **Read-only, apart from your one file.** Don't edit anything else: not the corpus, code, other teams' files or the 0.1 documents. Run no `jj` or `git` command that changes anything.
- **Domain forks are the Engineer's.** Write a needed decision as a question carrying your answer. Don't settle it.
- **Plain words.** No bare ids. Name a decision or feature in words, with its card or page id at most in parentheses.
- **Don't plan other teams' work.** Name what you need from them under "Cross-team dependencies"; the consolidator reconciles.
- On the board: `register` with your name, your team and role `lead`. Answer the Orchestrator (`claude`) by board mail if it asks. Your last message is your report.
