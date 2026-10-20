# Teams: Zurfur

<!-- The teams plugin's whole configuration for this project: settings, teams, opt-outs.
     Section titles and keys are fixed; the Orchestrator reads them by name. Edit values freely.
     "—" means unset. Written once by /teams:orchestrate; never regenerated over your edits.
     A file written before QA moved to the top has a QA column: drop it and name the top-level QA in the qa row. -->

## Repository

| Key | Value | Notes |
|---|---|---|
| owner | engineer | ZurNetwork/ZurFur is the Engineer's and public. This file and the CLAUDE.md pointer are committed; the working papers never are (see docs) |
| docs | `/home/zuri/code/zurfur-workspaces/orchestrator/.understand` | the private corpus's `understand` branch, git-ignored here. Release documents are committed there, never as `docs/<version>` PRs into ZurFur; `scripts/private-papers-check.sh` enforces it in `just gate`, CI and `jj push`. One exception: the queue is not `<docs>/queue.md` but `/home/zuri/.claude/projects/-home-zuri-code-zurfur/memory/queue.md`, in Zurfur's project memory folder (the Engineer, 2026-10-16) |
| style | `/home/zuri/code/zurfur-design/project/37519361-code-style-semantic-rulings-rust.md`; `/home/zuri/.claude/projects/-home-zuri-code-zurfur/memory/feedback_frontend_review_rulebook.md` | the Rust semantic rulings (`$ZURFUR_DESIGN_DIR` overrides the corpus path) and the TypeScript review rules; also CLAUDE.md › Code style |
| decisions | `/home/zuri/code/zurfur-design` (`decisions/`, `project/`), indexed by `docs/design-index.md` | the single source of truth; rulings files in `<docs>/releases/<version>/` until synced |
| vcs | jj | colocated with git; git worktrees are retired |
| main | main | never pushed to directly; squash only, through the merge queue |
| workspaces | `/home/zuri/code/zurfur-workspaces` | one per branch; `/home/zuri/code/zurfur` is the Engineer's checkout; `orchestrator` holds `<docs>` and is never removed |
| teardown | — | unset on purpose: a `docker compose … down -v` run from a workspace's copied `.env` could reach the Engineer's own dev stack. Zurfur removes workspaces with CLAUDE.md's recipe (CLAUDE.md › Workspaces), not `/teams:cleanup` (R-211, 2026-10-19) |
| gate | `just gate` | exactly CI. Agents run it as `/home/zuri/.cache/zurfur-locks/build-slot.sh just gate` (two build slots) |
| light gate | `just gate-light` | for base merges and fix rounds: fmt, clippy, typos, design-index-check, nodes-check, the touched crates' tests, web checks when `frontend/` or `contract/` changed; agents wrap it the same way |

## Branches

| Key | Value | Notes |
|---|---|---|
| strategy | integration | |
| integration | `feature/{slug}` | cut at main's tip. Zurfur's names stand for the pack's through 0.2 (R-215, 2026-10-19): for 0.2, `{slug}` is `release-0-2`, so `feature/release-0-2` ↔ the version `pre-alpha-v0.2.0` ↔ `<docs>/releases/0.2/`. `bug/{slug}` is an off-cycle fix's branch. The pack-shaped names are decided at 0.3's scope, with the release-model question |
| slice | `feature/{slug}-{n}-{part}` | its PR targets the integration branch. A slice's base is the longest `feature/<x>` prefix of its name that exists on the remote; Machinery's slice-PR briefs name the base. The `feature/` prefix makes CI run on PRs into `feature/**` |
| fix | `feature/{slug}-m{n}-{team}-fix` | a milestone fix round's branch; its PR targets the integration branch (R-215) |
| engineer reviews | main | also the design corpus's and Vulpes's main |
| merge slice | merge | |
| merge main | squash | `gh pr merge <N> --squash` queues it |

## Tracker

| Key | Value | Notes |
|---|---|---|
| kind | none | no tickets since 2026-09-19; work rides descriptive bookmarks |
| project | — | |
| commit format | `[{slug}][{n}] {title}` | feature → main: `[{slug}] {title}`; chore and docs PRs into main: `{type}({scope}): {title}` |

## Release

| Key | Value | Notes |
|---|---|---|
| version format | `{stage}-v{version}` | e.g. `pre-alpha-v0.1.0` |
| stages | pre-alpha, alpha, beta, release | meanings in `<docs>/releases/stages.md` |
| documents | pdd, tdd, plan | Product Design Doc (signing locks the scope), Technical Design Doc, Delivery Plan |
| format | typst | shared `template.typ`; diagrams with fletcher |

## Lanes

| Lane | Agent | Model | Effort |
|---|---|---|---|
| plan | `teams:planner` | — | — |
| judgment | `teams:designer` | — | — |
| build | `teams:builder` | — | — |
| subsystem | `teams:myth` | — | — |
| glue | `teams:machinery` | — | — |

<!-- A Model or Effort set here is passed as the Agent call's model or effort; — = the agent file's.
     Only programmers may run on Sonnet: a build or subsystem Model pins every programmer to it
     (e.g. opus only); — = the build lead picks sonnet or opus per task. Security-nature work and
     every other role are always Opus, whatever is set here (their Effort may still be set). -->
<!-- Zurfur follows the pack: build leads pick sonnet or opus per task (the Engineer, 2026-10-16);
     the Sonnet trial's yardstick (R-119, 2026-10-14) stays as guidance for that choice. -->

## Limits

| Key | Value | Notes |
|---|---|---|
| max lanes | 2 | one per build slot (2026-10-13); milestone QA's planted-bug runs take a build slot too |
| small lanes | 4 | each one or two agents on a short slice; still two build slots |
| internal after | 3 | new retrospectives that wake the internal team's lead |

<!-- The Orchestrator gives each build lead a programmer budget (how many at once) that fits these. -->

## Reviews

| Key | Value | Notes |
|---|---|---|
| release reviews | on | compliance, architecture and security at the end of each release, before its last QA |
| security triggers | authentication; authorization and permissions; sessions and tokens; secrets, keys and signing; the private↔public data boundary; DID or handle correlation; plugin trust; the blob store; fetches of user-supplied URLs | a slice that touches one gets a security review of its plan and its code. A PR into main whose work hit one gets a fresh security review of its cumulative diff before the Engineer's review (R-209, 2026-10-19): off-cycle work has no release review, so this covers it |
| compliance triggers | personal data; age or adult content; copyright or takedowns; payments or tax; accessibility; a new dependency's license | a slice that touches one calls compliance in early |
| bot | — | `copilot`, paused since 2026-10-15 (R-149); set it back when the Engineer resumes it. CodeRabbit reviews only PRs into main, automatically, as part of the top-level review layer beside QA and the architects, and a merge into main waits for its review (the Engineer, 2026-10-16). Slice PRs into other branches get no CodeRabbit: its default auto-review covers only the default branch |
| comment prefix | `Claude:` | starts every comment posted through the Engineer's accounts |

## Teams

The Orchestrator is the tech lead; the Engineer is Product Owner, manager and stakeholder. A team is a named lead, spawned only when there's work. The qa row is the top-level QA and the architects' lead is the architect: both stand beside the Orchestrator, at every milestone and the release. Programmers aren't listed: each is short-lived, spawned and named by its build lead for one task, and ends when it's done. Review teams list their standing reviewers.

| Team | Mission | Owns (paths) | Lead | Reviewers |
|---|---|---|---|---|
| den | Atta and the Den tree: nodes, metadata, payload, levels, links, mounts | `backend/crates/atta/**`, `backend/crates/application/src/den{.rs,/**}`, `backend/crates/api/src/routes/den{.rs,/**}` | Badger | — |
| identity | User, Account, Character, handles, DIDs, profiles, the PDS boundary | `backend/crates/identity/**` (except `src/elements/achievement{.rs,/**}`, which is posts'), `backend/crates/kernel/src/elements/did{.rs,/**}`, `backend/crates/application/src/{user,account,character}{.rs,/**}`, `backend/crates/api/src/routes/accounts{.rs,/**}`, `backend/crates/adapter-atproto/**` | Fox | — |
| commission | Commission, Workflow boards, slots, the changelog | `backend/crates/workflow/**`, `backend/crates/application/src/commission{.rs,/**}`, `backend/crates/api/src/routes/commissions/**` | Wolf | — |
| web | The SvelteKit app | `frontend/web/**` | Fennec | — |
| posts | Posts, /post and /blob, and the Gallery | `backend/crates/gallery/**`, `backend/crates/kernel/src/elements/blob{.rs,/**}`, `backend/crates/identity/src/elements/achievement{.rs,/**}`, `backend/crates/application/src/{post,gallery}{.rs,/**}`, `backend/crates/api/src/routes/{posts,gallery}{.rs,/**}` | Bowerbird | — |
| moderation | Platform safety: moderator actions, reports, scanning, kill switches, blocks | `backend/crates/moderation/**`, `backend/crates/application/src/moderation{.rs,/**}`, `backend/crates/api/src/routes/moderation{.rs,/**}` | Meerkat | — |
| plugin | Plugins: Golems, Portals, the plugin API, plugin security and listings | `backend/crates/{plugin,plugin-api}/**`, `backend/crates/application/src/plugin{.rs,/**}`, `backend/crates/api/src/routes/plugins{.rs,/**}` | Octopus | — |
| plugin/studio | Zurfur Studio (`~/code/zurfur-studio`), a core plugin; answers to the plugin lead too | — (own repo, `/home/zuri/code/zurfur-studio`) | Chameleon | — |
| plugin/telegram | Unofficial Rusty and Mini Telegram (own GPL-3.0 repo); answers to the plugin lead too | — (own repo) | Pigeon | — |
| vulpes | Vulpes (`~/code/vulpes`): ACP reference impl, upstream of Zurfur and Wolven | — (own repo, `/home/zuri/code/vulpes`) | Reynard | — |
| wolven | Wolven (`~/code/wolven`): node graph and monotone policy fold over Vulpes | — (own repo, `/home/zuri/code/wolven`) | Lobo | — |
| payments | How money moves around a commission | — | Pelican | — |
| qa | Top-level QA: documents for the Engineer, every milestone, every PR into main | — | Owl | — |
| internal | Finds the most effective way for the teams to work and spend their time; owns CI, scripts and the way-of-working files | `.github/**`, `scripts/**`, `Justfile`, `CLAUDE.md`, `.claude/settings.json`, `.claude/TEAMS.md`, `.claude/pins.md` | Beaver | — |
| compliance | Release review: what Zurfur is legally bound to follow | — | Tortoise | Pangolin, Armadillo |
| architects | Reviews every milestone, before QA; release review: correct architecture | — | Termite | Caddis, Oriole |
| security | Every security-triggered slice (its plan, then its code), a triggered PR into main's cumulative diff, and the release review | — | Heron | Mongoose, Hedgehog |

**Shared ground** (no team owns it; one Spawn at a time, assigned by the Orchestrator): `contract/**`; `backend/crates/adapter-pg/**` (migrations only through `just migrate-add`); `backend/crates/adapter-mem/**`, `backend/crates/composition/**`, `backend/crates/cli/**`, `backend/crates/test-support/**`, `backend/crates/domain/**` (the façade and the unit of work), `backend/crates/kernel/**` (except the `did` and `blob` files above); `Cargo.toml`, `Cargo.lock`, `frontend/web/yarn.lock`; `docs/design-index.md` (generated by `just design-index`, never by hand). Anything no team owns is shared ground.

## Opt-outs

<!-- One line each: what, the date, the Engineer's words. Recognised: internal team, nodes, release reviews, main's protection. -->

- —
