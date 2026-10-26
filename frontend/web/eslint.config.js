import prettier from 'eslint-config-prettier';
import { readFileSync } from 'node:fs';
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
	group: ['**/types/brand', '**/types/brand.*', '$lib/types/brand', '$lib/types/brand.*'],
	importNames: ['denHref'],
	message: DEN_HREF_IMPORT.message
};

/** Every file kind ESLint lints here, src/ and the package-root config files alike. */
const LINTED_FILES = [
	'**/*.js',
	'**/*.mjs',
	'**/*.cjs',
	'**/*.ts',
	'**/*.mts',
	'**/*.cts',
	'**/*.svelte'
];

/**
 * The packages web declares, read from its own manifest when the config
 * loads: `dependencies` and `devDependencies`.
 *
 * @returns {string[]}
 */
function declaredPackages() {
	const manifest = JSON.parse(
		readFileSync(path.resolve(import.meta.dirname, 'package.json'), 'utf8')
	);
	const section = (key) => Object.keys(Object(manifest[key]));
	return [...section('dependencies'), ...section('devDependencies')];
}

/** `text` as a regular expression that matches it literally. */
const literally = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** The declared packages, each alone or with a subpath, as a regular expression. */
const DECLARED = `(?:${declaredPackages().map(literally).join('|')})(?:/|$)`;

/** What web may import besides its packages: Node's built-ins, Kit's aliases and relative paths. */
const LOCAL = '(?:node:|\\$(?:app|lib|env)(?:/|$)|\\.{1,2}(?:/|$))';

const UNDECLARED_MESSAGE = 'Import only packages web declares; `@zurfur/ui` wraps bits-ui.';

/**
 * Any import that is neither a declared package nor local, read
 * case-sensitively, so no other spelling reaches past the manifest's names.
 */
const UNDECLARED_IMPORT = {
	regex: `^(?!${DECLARED})(?!${LOCAL})`,
	caseSensitive: true,
	message: UNDECLARED_MESSAGE
};

/** The allow-list for a dynamic import, which no-restricted-imports never reads. */
const ALLOWED_SPECIFIER = new RegExp(`^(?:${DECLARED}|${LOCAL})`, 'u');

/** The app's source folder and the folder `$lib` names. */
const SOURCE_ROOT = path.resolve(import.meta.dirname, 'src');
const LIB_ROOT = path.join(SOURCE_ROOT, 'lib');

/**
 * A field of an AST node, or undefined when there is none.
 *
 * @param {unknown} node
 * @param {string} key
 * @returns {unknown}
 */
const fieldOf = (node, key) => (Object(node) === node ? Reflect.get(Object(node), key) : undefined);

/**
 * The text of a string literal, or of a template literal with no expressions.
 *
 * @param {unknown} node
 * @returns {string | undefined}
 */
function staticTextOf(node) {
	const type = fieldOf(node, 'type');
	if (type === 'Literal') {
		const value = fieldOf(node, 'value');
		return typeof value === 'string' ? value : undefined;
	}
	if (type !== 'TemplateLiteral') return undefined;
	const expressions = fieldOf(node, 'expressions');
	const quasis = fieldOf(node, 'quasis');
	if (!Array.isArray(expressions) || expressions.length > 0 || !Array.isArray(quasis)) {
		return undefined;
	}
	const cooked = fieldOf(fieldOf(quasis[0], 'value'), 'cooked');
	return typeof cooked === 'string' ? cooked : undefined;
}

/** `specifier`'s path segments, split on either slash. */
const segmentsOf = (specifier) => specifier.split(/[/\\]/u);

/** Whether `specifier` is relative: `.`, `..`, or either followed by a slash. */
const isRelative = (specifier) => /^\.{1,2}(?:\/|$)/u.test(specifier);

/** Whether `specifier` is `$lib` or under it. */
const isLib = (specifier) => /^\$lib(?:\/|$)/u.test(specifier);

/**
 * Where a relative or `$lib` specifier lands, its query and hash dropped as
 * Vite drops them; undefined for any other specifier.
 *
 * @param {string} specifier
 * @param {string} folder the importing file's folder
 * @returns {string | undefined}
 */
function landingOf(specifier, folder) {
	const bare = specifier.replace(/[?#].*$/su, '');
	if (isLib(bare)) return path.join(LIB_ROOT, bare.slice('$lib'.length));
	if (isRelative(bare)) return path.resolve(folder, bare);
	return undefined;
}

/** Whether `target` is the source folder or inside it. */
function isInsideSource(target) {
	const relative = path.relative(SOURCE_ROOT, target);
	const leaves = relative === '..' || relative.startsWith(`..${path.sep}`);
	return !leaves && !path.isAbsolute(relative);
}

/** The spec harness that builds Kit's real server-side fetch. */
const KIT_FETCH_HARNESS = 'src/lib/testing/kit-fetch.ts';

/** The Kit internals that harness imports, which no exports map offers. */
const KIT_INTERNALS = [
	'../../../../node_modules/@sveltejs/kit/src/runtime/server/fetch.js',
	'../../../../node_modules/@sveltejs/kit/src/runtime/server/cookie.js'
];

/** @type {import('eslint').Rule.RuleModule} */
const importSpecifiersRule = {
	meta: {
		type: 'problem',
		docs: {
			description:
				'An import names a package by its name and a local file inside src/; a dynamic import answers to the allow-list.'
		},
		schema: [
			{
				type: 'object',
				properties: { internals: { type: 'array', items: { type: 'string' } } },
				additionalProperties: false
			}
		],
		messages: {
			nodeModules:
				'Import a package by its name, never through a node_modules path ({{specifier}}).',
			dotSegment: 'A package, $app, $env or node: import holds no . or .. segment ({{specifier}}).',
			outside:
				'A relative or $lib import never leaves src/ ({{specifier}}): import a package by its name.',
			undeclared: UNDECLARED_MESSAGE
		}
	},
	create(context) {
		const folder = path.dirname(context.filename);
		const importerInSource = isInsideSource(context.filename);
		const listed = fieldOf(context.options[0], 'internals');
		/** The exact static specifiers this file may import past the path checks. */
		const internals = new Set(Array.isArray(listed) ? listed : []);

		/**
		 * @param {import('eslint').Rule.Node} node
		 * @param {unknown} source
		 * @param {boolean} dynamic
		 */
		function check(node, source, dynamic) {
			const specifier = staticTextOf(source);
			if (specifier === undefined) return;
			if (!dynamic && internals.has(specifier)) return;
			/** @param {'nodeModules' | 'dotSegment' | 'outside' | 'undeclared'} messageId */
			const report = (messageId) => {
				context.report({ node, messageId, data: { specifier } });
			};
			const segments = segmentsOf(specifier);
			if (segments.some((segment) => /^node_modules(?:[?#]|$)/iu.test(segment))) {
				report('nodeModules');
				return;
			}
			const local = isRelative(specifier) || isLib(specifier);
			if (!local) {
				const named = segmentsOf(specifier.replace(/^node:/u, ''));
				if (named.some((segment) => segment === '.' || segment === '..')) {
					report('dotSegment');
					return;
				}
			}
			const landing = landingOf(specifier, folder);
			const governed = isLib(specifier) || importerInSource;
			if (landing !== undefined && governed && !isInsideSource(landing)) {
				report('outside');
				return;
			}
			if (dynamic && !ALLOWED_SPECIFIER.test(specifier)) report('undeclared');
		}

		/** @param {import('eslint').Rule.Node} node */
		const checkStatic = (node) => {
			check(node, fieldOf(node, 'source'), false);
		};

		return {
			ImportDeclaration: checkStatic,
			ExportAllDeclaration: checkStatic,
			ExportNamedDeclaration: checkStatic,
			/** @param {import('eslint').Rule.Node} node */
			ImportExpression(node) {
				check(node, fieldOf(node, 'source'), true);
			},
			/**
			 * Reads `source` when it holds a string literal, else the older
			 * `argument.literal`, so the check keeps working across both shapes.
			 *
			 * @param {import('eslint').Rule.Node} node
			 */
			TSImportType(node) {
				const source = fieldOf(node, 'source');
				const holdsString =
					fieldOf(source, 'type') === 'Literal' && typeof fieldOf(source, 'value') === 'string';
				const specifier = holdsString ? source : fieldOf(fieldOf(node, 'argument'), 'literal');
				check(node, specifier, false);
			},
			/** @param {import('eslint').Rule.Node} node */
			TSExternalModuleReference(node) {
				check(node, fieldOf(node, 'expression'), false);
			}
		};
	}
};

/** web's own rules, as an inline plugin. */
const webPlugin = {
	meta: { name: 'web' },
	rules: { 'import-specifiers': importSpecifiersRule }
};

/**
 * One no-restricted-imports entry holding every ban a block's files answer
 * to: the allow-list always, and Effect's and the DenHref's bans where asked.
 * A later flat-config block replaces an earlier one's options for the same
 * rule, so each block carries its whole list. The test tools' bans join here,
 * as one more flag, when they land.
 *
 * @param {{ readonly effect: boolean, readonly denHref: boolean }} bans
 */
function importRules({ effect, denHref }) {
	return [
		'error',
		{
			paths: [...(effect ? EFFECT_IMPORTS : []), ...(denHref ? [DEN_HREF_IMPORT] : [])],
			patterns: [
				UNDECLARED_IMPORT,
				...(effect ? EFFECT_PATTERNS : []),
				...(denHref ? [DEN_HREF_PATTERN] : [])
			]
		}
	];
}

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
					selector: 'ImportExpression[source.value=/types\\u002Fbrand(\\.[a-z]+)?$/]',
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
		// Every file imports only the packages web declares. The blocks below
		// replace this one's options for their files, each carrying the
		// allow-list beside its own bans; a file none of them names (a server
		// spec, the Den path builder, a config file) keeps the allow-list alone.
		name: 'web/declared-imports',
		files: LINTED_FILES,
		rules: { 'no-restricted-imports': importRules({ effect: false, denHref: false }) }
	},
	{
		// Every file's specifiers, static and dynamic: no node_modules path, no
		// . or .. in a package name, no relative or $lib path out of src/, and a
		// dynamic import answers to the same allow-list as a static one.
		name: 'web/import-specifiers',
		files: LINTED_FILES,
		plugins: { web: webPlugin },
		rules: { 'web/import-specifiers': 'error' }
	},
	{
		// The spec harness for Kit's server fetch reaches Kit's internals by path
		// on purpose; it may import exactly these two, statically, and nothing
		// else past the checks.
		name: 'web/kit-internals',
		files: [KIT_FETCH_HARNESS],
		rules: { 'web/import-specifiers': ['error', { internals: KIT_INTERNALS }] }
	},
	{
		// DD 39944194 containment: Effect exists only below the runes seam —
		// src/lib/server/** and *.server.ts. Everything above the seam receives
		// plain data and never sees a fiber. And only the Den path builder mints
		// a DenHref (see DEN_HREF_IMPORT).
		name: 'web/client-imports',
		files: SOURCE_FILES,
		ignores: [...SERVER_SIDE, ...DEN_HREF_MINTERS, ...SPECS],
		rules: { 'no-restricted-imports': importRules({ effect: true, denHref: true }) }
	},
	{
		// Above the seam, the DenHref minters and the specs keep only the Effect ban.
		name: 'web/minter-and-spec-imports',
		files: [...DEN_HREF_MINTERS, ...SPECS],
		ignores: SERVER_SIDE,
		rules: { 'no-restricted-imports': importRules({ effect: true, denHref: false }) }
	},
	{
		// Below the seam, only the Den path builder mints a DenHref.
		name: 'web/server-imports',
		files: SERVER_SIDE,
		ignores: [...DEN_HREF_MINTERS, ...SPECS],
		rules: { 'no-restricted-imports': importRules({ effect: false, denHref: true }) }
	},
	{
		// Override or add rule settings here, such as:
		// 'svelte/button-has-type': 'error'
		rules: {}
	}
);
