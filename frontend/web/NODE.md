---
path: frontend/web
charted: 2026-09-12
fs:
  - name: src/
    role: all app code — hooks, $lib, routes
    node: true
  - name: static/
    role: robots.txt
    node: false
  - name: package.json
    role: yarn scripts (dev/build/check/lint/test) + pinned deps: svelte 5, kit 2, effect, sveltekit-superforms, @bufbuild/protobuf
    node: false
  - name: vite.config.ts
    role: the ONLY svelte config: adapter-node, runes:true, 127.0.0.1:5174 strictPort for Caddy, the two vitest projects
    node: false
  - name: eslint.config.js
    role: type-aware lint + the review rulebook as enforced rules
    node: false
  - name: prettier.config.js
    role: tabs, single quotes, no trailing comma, width 100
    node: false
  - name: tsconfig.json
    role: strict + noUncheckedIndexedAccess + exactOptionalPropertyTypes + erasableSyntaxOnly
    node: false
  - name: README.md
    role: stock sv-create scaffold readme, not doctrine
    node: false
  - name: yarn.lock
    role: yarn is the package manager
    node: false
  - name: .npmrc
    role: engine-strict=true — the declared node/yarn range is enforced on install
    node: false
  - name: .prettierignore
    role: lockfiles, static/ and the generated protobuf tree are not formatted
    node: false
  - name: .gitignore
    role: ignores this package's build output (.svelte-kit/, build/) and .env*
    node: false
  - name: .vscode/
    role: extensions.json — the recommended editor extensions for this package
    node: false
---

**Is:** A SvelteKit 2 / Svelte 5 app in forced runes mode, node adapter, talking to axum only from the server side, with a yarn-driven check/lint/test/build gate mirrored by `just check-all`.

**Conventions:** two vitest projects split by filename — `*.svelte.spec.ts` runs in real Chromium (`src/lib/server/**` excluded), every other `*.spec.ts` in node; `expect.requireAssertions` on. Specs sit next to code but drop the `+` for routes (`page.server.spec.ts`). eslint IS the rulebook enforcer: `effect` imports banned outside `src/lib/server/**` and `*.server.ts`; production code may not throw, mention `null`, or use type assertions (brand mints under `src/lib/types/**` are the one exemption; `src/lib/testing/**` is spec-only). Dev port 5174 strictPort (Caddy depends on it; `ZURFUR_WEB_PORT` overrides). The protobuf tree here regenerates with `buf generate` run from `contract/`, not from a yarn script.

**Entry points:** `src/hooks.server.ts` · `src/routes/+layout.server.ts` · `vite.config.ts`.

**Refs:** memory `feedback_frontend_review_rulebook`.
