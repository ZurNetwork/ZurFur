# frontend/web — SvelteKit + ZesTTY (the Dialect)

SvelteKit 2 / Svelte 5 runes app. Two binding DDs shape everything here — fetch
them (via `docs/confluence-design-index.md`) before asserting from memory:
**39944194** (Frontend Stack: Effect server-only under `src/lib/server/**`, the
runes seam, the containment eslint rule) and **46596098** (The Dialect: ZesTTY
is the authoring language, committed-twins mode).

## The twins loop (DD 46596098 D2)

- Author `.zts` (ZesTTY — `match`/enums/expression-`if`/`Result`), **under
  `src/` only**. Its generated `.ts` twin is committed beside it, marked
  `@generated` — **never hand-edit a twin**; edit the `.zts` and regenerate.
- `just gen-zts` regenerates twins + the `.zts-twins` style-ignore list;
  `just zts-check` is the gate (stale/orphan/scope/list guards) — runs in CI's
  web job and inside `just gate`. Commit `.zts` + `.ts` together.
- Twins are **style-exempt but semantics-live**: prettier/two eslint style
  rules skip them; the DD 39944194 Effect-containment rule deliberately DOES
  lint them — that is how containment reaches `.zts`-authored code (D6).
- Hard fence: nothing ZesTTY-related under `src/lib/server/api/generated/**`
  (CI-wiped, protobuf-owned). The wiring canary `src/lib/zts/wiring-sample.*`
  is permanent — the gate must always have a pair to check.
- Toolchain = npm **exact pins** (`@zestty/*@0.2.0`, bumped by Engineer
  ruling 2026-08-06); bumps are deliberate, DD-recorded rulings, never
  drive-by. v0.2.0 adds wildcard + literal `match`, `newtype`, `?`, and
  `@zestty/core` `ResultPipe`/`and_then` — the sweep-wait ruling is
  satisfied; the sweep may begin once the seam revisit (DD 46596098 D3
  trigger) is ruled.
- **`kind`** is the discriminant for every client-facing union (DD D4);
  generated protobuf messages keep `case` and cross via per-message adapters.
- **No `Option`** — strict-null `T | undefined`, even if ZesTTY ships one
  (DD D7 pre-bind, extending 39944194 D7).
- **No formatter touches `.zts`** — the formatting story is an open DD row
  (no prettier parser exists; candidate = the compiler's own emit as
  canonical). Until ruled, `.zts` is formatted by hand.

## Editor DX (as of 0.2.0)

zestty.nvim runs the repo-pinned language server: hover, go-to-def,
exhaustiveness squiggles, and — since 0.2.0 — semantic completions
(upstream #13 shipped).

## Memory check

Forms/error-channel questions: the one-channel superform pattern (values,
field errors, and the backend `Problem`-as-`message` all ride the superform —
see `lib/server/forms/`). Result↔Effect seam questions: DD 46596098 D3
(`runApi` returns `Result`; Effect keeps Layers/Schema/orchestration). Check
the repo memories (`project_dd_zestty_adoption`, `project_zts_compiler`)
before asserting toolchain facts — they carry the verified empirics.
