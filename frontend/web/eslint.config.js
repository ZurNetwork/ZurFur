import prettier from 'eslint-config-prettier';
import path from 'node:path';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, includeIgnoreFile } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';

const gitignorePath = path.resolve(import.meta.dirname, '.gitignore');

/**
 * `strictTypeChecked`/`stylisticTypeChecked` each bundle their own
 * `typescript-eslint/base` sub-config (parser setup, no `files` of its own).
 * `extends`-ing them under a `files`-scoped block re-scopes that parser
 * assignment to whatever `files` wraps it (the extends helper's own
 * behavior) — harmless for the plain-TS block below, but for the `.svelte`
 * block it would clobber `svelte.configs.recommended`'s `svelte-eslint-parser`
 * assignment with the raw TS parser. `ts.configs.base` is already applied
 * globally above, so the embedded copy is dropped here rather than relied on.
 */
const typeCheckedRules = (configs) => {
	const kept = configs.filter((c) => c.name !== 'typescript-eslint/base');
	if (kept.length === configs.length) {
		// Fail LOUD: if upstream renames the base sub-config this filter guards
		// against, silently keeping it would clobber svelte-eslint-parser.
		throw new Error("'typescript-eslint/base' not found — the parser-clobber guard is stale");
	}
	return kept;
};

/**
 * Every file the review-rulebook blocks below govern — one list so a future
 * extension (a new file kind) lands in every rule at once instead of drifting
 * per block.
 */
const RULEBOOK_FILES = [
	'src/**/*.ts',
	'src/**/*.js',
	'src/**/*.svelte',
	'src/**/*.svelte.ts',
	'src/**/*.svelte.js'
];

/** Every app source file the import rules govern. */
const SOURCE_FILES = [
	'src/**/*.ts',
	'src/**/*.js',
	'src/**/*.svelte',
	'src/**/*.svelte.ts',
	'src/**/*.svelte.js'
];

/** Below the runes seam: where Effect may live. */
const SERVER_SIDE = ['src/lib/server/**', 'src/**/*.server.ts', 'src/**/*.server.spec.ts'];

/** Specs and their harnesses. */
const SPECS = ['src/**/*.spec.ts', 'src/lib/testing/**'];

/** The only modules that mint a DenHref: the Den path builder, and the bare root route's fallback. */
const DEN_HREF_MINTERS = [
	'src/lib/server/den/den-path.ts',
	'src/lib/components/explorer/den-root.ts'
];

/** Effect is server-only, below the runes seam. */
const EFFECT_IMPORTS = [
	{
		name: 'effect',
		message:
			'Effect is server-only (DD 39944194) — move this under src/lib/server/** or a *.server.ts.'
	}
];

/** Effect's subpaths and companions are server-only too. */
const EFFECT_PATTERNS = [
	{
		group: ['effect/*', '@effect/*'],
		message: 'Effect is server-only (DD 39944194).'
	}
];

/**
 * A DenHref is a link the Den path builder built and checked; minting one
 * anywhere else would let a link skip the builder's refusals.
 */
const DEN_HREF_IMPORT = {
	name: '$lib/types/brand',
	importNames: ['denHref'],
	message:
		'Only the Den path builder (src/lib/server/den/den-path.ts) mints a DenHref — build Den links there.'
};

/** The same ban for every other spelling of the brand module: relative paths included. */
const DEN_HREF_PATTERN = {
	group: ['**/types/brand', '$lib/types/brand'],
	importNames: ['denHref'],
	message: DEN_HREF_IMPORT.message
};

export default defineConfig(
	includeIgnoreFile(gitignorePath),
	js.configs.recommended,
	// Untyped base TS parsing stays global (repo-root tooling — eslint.config.js,
	// vite.config.ts, prettier.config.js — sits outside `.svelte-kit/tsconfig.json`'s
	// `include`, so it can't carry a TS program). The typed configs below are
	// scoped to `src/**` and `.svelte`, where a program is available.
	ts.configs.base,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		languageOptions: { globals: { ...globals.browser, ...globals.node } },
		rules: {
			// typescript-eslint strongly recommend that you do not use the no-undef rule on TypeScript projects.
			// see: https://typescript-eslint.io/troubleshooting/faqs/eslint/#i-get-errors-from-the-no-undef-rule-about-global-variables-not-being-defined-even-though-there-are-no-typescript-errors
			'no-undef': 'off'
		}
	},
	{
		// Type-aware linting (strictTypeChecked + stylisticTypeChecked, which
		// together supersede `ts.configs.recommended`) needs a real TypeScript
		// program — wired via `projectService` + `tsconfigRootDir` for every
		// plain TS/JS file under src.
		files: ['src/**/*.ts', 'src/**/*.js'],
		extends: [
			...typeCheckedRules(ts.configs.strictTypeChecked),
			...typeCheckedRules(ts.configs.stylisticTypeChecked)
		],
		languageOptions: {
			parserOptions: {
				projectService: true,
				tsconfigRootDir: import.meta.dirname
			}
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
		extends: [
			...typeCheckedRules(ts.configs.strictTypeChecked),
			...typeCheckedRules(ts.configs.stylisticTypeChecked)
		],
		languageOptions: {
			parserOptions: {
				projectService: true,
				tsconfigRootDir: import.meta.dirname,
				extraFileExtensions: ['.svelte'],
				parser: ts.parser
			}
		}
	},
	{
		// Review rulebook: every switch over a union names every variant — a
		// `default` arm never stands in for the missing ones (it would silently
		// swallow future variants); non-union switches still need a `default`.
		files: RULEBOOK_FILES,
		rules: {
			'@typescript-eslint/switch-exhaustiveness-check': [
				'error',
				{
					considerDefaultExhaustiveForUnions: false,
					requireDefaultForNonUnion: true
				}
			]
		}
	},
	{
		// Review rulebook: production code never throws — errors are values
		// (tagged unions / Result-shaped returns; DD 39944194's error model).
		// SvelteKit 2's redirect()/error() helpers are called, not thrown, so
		// no framework carve-out is needed. Test harnesses may throw.
		// And null never crosses our own interfaces — `undefined` is the one
		// absence value (DD 39944194: strict-null `T | undefined`); platform
		// APIs that answer null get converted at the point of contact (`?? undefined`).
		// And test harnesses stay spec-only: src/lib/testing/** is exempt from
		// these bans, so production code must never import it.
		files: RULEBOOK_FILES,
		ignores: ['src/lib/testing/**', 'src/**/*.spec.ts'],
		rules: {
			'no-restricted-syntax': [
				'error',
				{
					selector: 'ThrowStatement',
					message:
						'Never throw — return an error value (tagged union / Result). Throws are test-harness-only.'
				},
				{
					selector: 'ImportDeclaration[source.value=/^\\$lib\\u002Ftesting/]',
					message:
						'Test harnesses ($lib/testing/**) are spec-only — they are exempt from the throw/null/assertion bans and must not reach production code.'
				},
				{
					selector: "Literal[raw='null']",
					message:
						'Never null — `undefined` is the absence value (DD 39944194). Convert platform nulls at contact: `x ?? undefined`.'
				},
				{
					selector: 'TSNullKeyword',
					message: 'Never `| null` in our types — model absence as `T | undefined` (DD 39944194).'
				},
				{
					// An expected error's message skips handleError and rides the page
					// source and its JSON: only a fixed literal (or none) may go there.
					selector:
						"CallExpression[callee.name='error'][arguments.length>1]:not([arguments.1.type='Literal'])",
					message:
						'error(status, message) takes only a literal message (or none): it ships in the page source, so it must never carry a path, an id or other data.'
				},
				{
					// A dynamic import would reach the brand module past the DenHref import ban.
					selector: 'ImportExpression[source.value=/types\\u002Fbrand$/]',
					message:
						'Import the brand module statically, so the rule that only the Den path builder mints a DenHref can see it.'
				},
				{
					selector:
						"ImportDeclaration[source.value='@sveltejs/kit'] ImportSpecifier[imported.name='error'][local.name!='error']",
					message:
						"Import SvelteKit's error() as `error`: the literal-message rule keys on that name."
				}
			]
		}
	},
	{
		// Review rulebook: parse, don't validate — a type assertion is a
		// validate-without-parsing, the checker's off-switch. Banned in
		// production code (`as const` is not an assertion and stays legal;
		// a type-predicate function is the sanctioned escape — its runtime
		// check travels with the claim). The ONE blessed assertion site is
		// the brand mint (src/lib/types/**): branding is unrepresentable
		// without it, and containing it there is the point of the pattern.
		files: RULEBOOK_FILES,
		ignores: ['src/lib/testing/**', 'src/**/*.spec.ts', 'src/lib/types/**'],
		rules: {
			'@typescript-eslint/consistent-type-assertions': ['error', { assertionStyle: 'never' }]
		}
	},
	{
		// DD 39944194 containment: Effect exists only below the runes seam —
		// src/lib/server/** and *.server.ts. Everything above the seam receives
		// plain data and never sees a fiber. And only the Den path builder mints
		// a DenHref (see DEN_HREF_IMPORT).
		files: SOURCE_FILES,
		ignores: [...SERVER_SIDE, ...DEN_HREF_MINTERS, ...SPECS],
		rules: {
			'no-restricted-imports': [
				'error',
				{
					paths: [...EFFECT_IMPORTS, DEN_HREF_IMPORT],
					patterns: [...EFFECT_PATTERNS, DEN_HREF_PATTERN]
				}
			]
		}
	},
	{
		// Above the seam, the DenHref minters and the specs keep only the Effect ban.
		files: [...DEN_HREF_MINTERS, ...SPECS],
		ignores: SERVER_SIDE,
		rules: {
			'no-restricted-imports': ['error', { paths: EFFECT_IMPORTS, patterns: EFFECT_PATTERNS }]
		}
	},
	{
		// Below the seam, only the Den path builder mints a DenHref.
		files: SERVER_SIDE,
		ignores: [...DEN_HREF_MINTERS, ...SPECS],
		rules: {
			'no-restricted-imports': ['error', { paths: [DEN_HREF_IMPORT], patterns: [DEN_HREF_PATTERN] }]
		}
	},
	{
		// Override or add rule settings here, such as:
		// 'svelte/button-has-type': 'error'
		rules: {}
	}
);
