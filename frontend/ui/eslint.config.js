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
 * `typescript-eslint/base` sub-config. Extending them under a `files`-scoped
 * block would re-scope that parser assignment and, for `.svelte` files,
 * replace `svelte-eslint-parser` with the raw TS parser. `ts.configs.base` is
 * applied globally below, so the embedded copy is dropped here.
 *
 * @template {{ name?: string }} Config
 * @param {readonly Config[]} configs
 * @returns {Config[]}
 */
const typeCheckedRules = (configs) => {
	const kept = configs.filter((c) => c.name !== 'typescript-eslint/base');
	if (kept.length === configs.length) {
		throw new Error("'typescript-eslint/base' not found: the parser-clobber guard is stale");
	}
	return kept;
};

// ---------------------------------------------------------------------------
// The file sets. Every source file under src/ lands in exactly one of them:
// each set's `ignores` keeps it off the others, and the components' set takes
// every file no other set names, so a new file can never land in none.
// ---------------------------------------------------------------------------

/** Every file kind ESLint lints under src/. */
const CODE = [
	'src/**/*.ts',
	'src/**/*.mts',
	'src/**/*.cts',
	'src/**/*.js',
	'src/**/*.mjs',
	'src/**/*.cjs',
	'src/**/*.svelte'
];

/** The checks and every spec: Node globals, and they may throw. */
const CHECKS = ['src/checks/**', 'src/**/*.spec.ts', 'src/**/*.svelte.spec.ts'];

/** The gallery's pages. */
const ROUTES = ['src/routes/**'];

/** The gallery's own frame: the registry, the nav and the page layout. */
const GALLERY = ['src/gallery/**'];

/** The one place motion libraries may be imported. */
const MOTION = ['src/lib/motion/**'];

/** The one place heavy libraries may be loaded, and only by dynamic import. */
const LAZY = ['src/lib/lazy/**'];

/** Each set's `files` and `ignores`, by name. */
export const fileSets = {
	checks: { files: CHECKS, ignores: [] },
	routes: { files: ROUTES, ignores: CHECKS },
	gallery: { files: GALLERY, ignores: CHECKS },
	motion: { files: MOTION, ignores: CHECKS },
	lazy: { files: LAZY, ignores: CHECKS },
	components: { files: CODE, ignores: [...CHECKS, ...ROUTES, ...GALLERY, ...MOTION, ...LAZY] }
};

// ---------------------------------------------------------------------------
// The bans, as entries the per-set arrays below merge.
// ---------------------------------------------------------------------------

const NETWORK_MESSAGE =
	'Components never reach the network: take the data as a prop and let the app fetch it.';
const STORAGE_MESSAGE =
	'Components never touch browser storage: take the value as a prop and let the app keep it.';
const PROCESS_MESSAGE =
	'The process environment belongs to the server: take the value as a prop instead.';
const CODE_LOADING_MESSAGE =
	'Components never load code at run time: import it statically, or load heavy libraries from src/lib/lazy.';

/** Network and code loading, by bare name and through window, self and globalThis. */
const NETWORK_GLOBALS = [
	{ name: 'fetch', message: NETWORK_MESSAGE },
	{ name: 'XMLHttpRequest', message: NETWORK_MESSAGE },
	{ name: 'WebSocket', message: NETWORK_MESSAGE },
	{ name: 'EventSource', message: NETWORK_MESSAGE },
	{ name: 'Worker', message: CODE_LOADING_MESSAGE },
	{ name: 'SharedWorker', message: CODE_LOADING_MESSAGE },
	{ name: 'require', message: CODE_LOADING_MESSAGE }
];

/** Browser storage, other than localStorage. */
const STORAGE_GLOBALS_EXCEPT_LOCAL = [
	{ name: 'sessionStorage', message: STORAGE_MESSAGE },
	{ name: 'indexedDB', message: STORAGE_MESSAGE },
	{ name: 'caches', message: STORAGE_MESSAGE },
	{ name: 'cookieStore', message: STORAGE_MESSAGE }
];

const LOCAL_STORAGE_GLOBAL = { name: 'localStorage', message: STORAGE_MESSAGE };

const PROCESS_GLOBAL = { name: 'process', message: PROCESS_MESSAGE };

/** What the components' set and its two carve-outs refuse as globals. */
const COMPONENT_GLOBALS = [
	...NETWORK_GLOBALS,
	LOCAL_STORAGE_GLOBAL,
	...STORAGE_GLOBALS_EXCEPT_LOCAL,
	PROCESS_GLOBAL
];

/** The gallery may keep its own settings in localStorage; everything else stays refused. */
const HOST_GLOBALS = [...NETWORK_GLOBALS, ...STORAGE_GLOBALS_EXCEPT_LOCAL, PROCESS_GLOBAL];

/** Member reads the globals rule can't see. */
const NETWORK_PROPERTIES = [
	{ object: 'navigator', property: 'sendBeacon', message: NETWORK_MESSAGE },
	{ object: 'navigator', property: 'serviceWorker', message: CODE_LOADING_MESSAGE },
	{ object: 'document', property: 'cookie', message: STORAGE_MESSAGE },
	{ object: 'process', property: 'env', message: PROCESS_MESSAGE }
];

/** Web Animations, outside the motion adapter. */
const ANIMATE_PROPERTY = {
	property: 'animate',
	message: 'Animate through the motion adapter in src/lib/motion, never with element.animate().'
};

/** The package's own source, reached past its public entry. */
const DEEP_SELF_IMPORT = {
	regex: '^@zurfur/ui/',
	message: 'Import @zurfur/ui from its public entry only, never a file inside it.'
};

const ROOT_IMPORT = {
	regex: '^/',
	message:
		'A path starting with / resolves against whichever project compiles this file: import relatively instead.'
};

const ENV_IMPORT = {
	regex: '^\\$env(/|$)',
	message: 'Environment values belong to the app: take the value as a prop instead.'
};

const LIB_IMPORT = {
	regex: '^\\$lib(/|$)',
	message: "$lib resolves to the importing project's src/lib: import ui's own files relatively."
};

const ANY_APP_IMPORT = {
	regex: '^\\$app(/|$)',
	message: "SvelteKit's $app modules belong to the app: take what you need as a prop or a callback."
};

const APP_IMPORT_EXCEPT_PATHS = {
	regex: '^\\$app(?!/paths$)(/|$)',
	message: "The gallery uses only $app/paths from SvelteKit's $app modules."
};

const GALLERY_IMPORT = {
	regex: '^\\$gallery(/|$)',
	message: "The gallery's frame is the host's: components never import it."
};

/** Server-only libraries, the generated contract, and web's own files. */
const SERVER_IMPORTS = [
	{
		regex: '^(effect|@effect/[^/]+)(/|$)',
		message: 'Effect is server-only: components take plain data as props.'
	},
	{
		regex: '^@bufbuild/',
		message: 'The wire format is the server half: components take plain data as props.'
	},
	{
		regex: '(^|/)server/api/generated(/|$)',
		message: 'The generated contract is the server half: components take plain data as props.'
	},
	{
		regex: '^web(/|$)',
		message: "Components never import web's files: web imports ui, never the other way round."
	},
	{
		regex: '(^|/)node_modules(/|$)',
		message: 'Import a package by its name, never through a node_modules path.'
	},
	{
		regex: '^(\\.{1,2}/)+web(/|$)',
		message: "Components never import web's files: web imports ui, never the other way round."
	}
];

const MOTION_IMPORT_MESSAGE =
	'Motion libraries are imported only in src/lib/motion: use the motion adapter.';

/** gsap and Svelte's motion modules: allowed only in src/lib/motion. */
const ADAPTER_MOTION_IMPORTS = [
	{ regex: '^gsap(/|$)', message: MOTION_IMPORT_MESSAGE },
	{ regex: '^svelte/(transition|animate|motion|easing)(/|$)', message: MOTION_IMPORT_MESSAGE }
];

/** three and lottie-web: loaded only by dynamic import in src/lib/lazy. */
const HEAVY_IMPORTS = [
	{
		regex: '^(three|lottie-web)(/|$)',
		message: 'Heavy libraries load only by dynamic import in src/lib/lazy.'
	}
];

/** What every source set refuses as an import, before its own additions. */
const COMMON_IMPORTS = [ROOT_IMPORT, ENV_IMPORT, LIB_IMPORT, DEEP_SELF_IMPORT, ...SERVER_IMPORTS];

const COMPONENT_IMPORTS = [
	...COMMON_IMPORTS,
	ANY_APP_IMPORT,
	GALLERY_IMPORT,
	...ADAPTER_MOTION_IMPORTS,
	...HEAVY_IMPORTS
];
const MOTION_IMPORTS = [...COMMON_IMPORTS, ANY_APP_IMPORT, GALLERY_IMPORT, ...HEAVY_IMPORTS];
const LAZY_IMPORTS = COMPONENT_IMPORTS;
const HOST_IMPORTS = [
	...COMMON_IMPORTS,
	APP_IMPORT_EXCEPT_PATHS,
	...ADAPTER_MOTION_IMPORTS,
	...HEAVY_IMPORTS
];

/** The review rulebook: never throw, never null. */
const RULEBOOK_SYNTAX = [
	{
		selector: 'ThrowStatement',
		message: 'Never throw: return an error value (a tagged union or a Result). Only specs throw.'
	},
	{
		selector: "Literal[raw='null']",
		message:
			'Never null: undefined is the one absence value. Convert platform nulls at contact: `x ?? undefined`.'
	},
	{
		selector: 'TSNullKeyword',
		message: 'Never `| null` in our types: model absence as `T | undefined`.'
	}
];

const IMPORT_META_ENV = {
	selector: "MemberExpression[object.type='MetaProperty'][property.name='env']",
	message: 'import.meta.env belongs to the app: take the value as a prop instead.'
};

const IMPORT_META_ENV_COMPUTED = {
	selector: "MemberExpression[object.type='MetaProperty'][property.value='env']",
	message: IMPORT_META_ENV.message
};

const GLOB_MESSAGE =
	'import.meta.glob resolves against whichever project compiles this file and can reach the server half: import files by name.';

const ANY_GLOB = [
	{
		selector: "MemberExpression[object.type='MetaProperty'][property.name=/^glob/]",
		message: GLOB_MESSAGE
	},
	{
		selector: "MemberExpression[object.type='MetaProperty'][property.value=/^glob/]",
		message: GLOB_MESSAGE
	}
];

/** The gallery's one glob: the page metas under src/routes, as that literal pattern. */
const GLOB_EXCEPT_PAGE_METAS = [
	{
		selector:
			"MemberExpression[object.type='MetaProperty'][property.name=/^glob/]:not(CallExpression[arguments.0.type='Literal'][arguments.0.value='/src/routes/**/meta.ts'] > MemberExpression.callee)",
		message:
			"The gallery's one glob is import.meta.glob('/src/routes/**/meta.ts'): find pages through it."
	},
	ANY_GLOB[1]
];

const ANY_DYNAMIC_IMPORT = {
	selector: 'ImportExpression',
	message: CODE_LOADING_MESSAGE
};

const DYNAMIC_IMPORT_EXCEPT_HEAVY = {
	selector: "ImportExpression:not([source.type='Literal'][source.value=/^(three|lottie-web)$/])",
	message: "src/lib/lazy loads only import('three') and import('lottie-web'), by literal name."
};

const COMMON_SYNTAX = [...RULEBOOK_SYNTAX, IMPORT_META_ENV, IMPORT_META_ENV_COMPUTED];

const COMPONENT_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT];
const LAZY_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, DYNAMIC_IMPORT_EXCEPT_HEAVY];
const ROUTES_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT];
const GALLERY_SYNTAX = [...COMMON_SYNTAX, ...GLOB_EXCEPT_PAGE_METAS, ANY_DYNAMIC_IMPORT];

/**
 * Each source set's options for the four restricted rules. A later flat-config
 * block replaces an earlier one's options for the same rule, so each set gets
 * one block per rule holding every entry it needs.
 */
export const boundaryRules = {
	components: {
		globals: COMPONENT_GLOBALS,
		properties: [...NETWORK_PROPERTIES, ANIMATE_PROPERTY],
		imports: COMPONENT_IMPORTS,
		syntax: COMPONENT_SYNTAX
	},
	motion: {
		globals: COMPONENT_GLOBALS,
		properties: NETWORK_PROPERTIES,
		imports: MOTION_IMPORTS,
		syntax: COMPONENT_SYNTAX
	},
	lazy: {
		globals: COMPONENT_GLOBALS,
		properties: [...NETWORK_PROPERTIES, ANIMATE_PROPERTY],
		imports: LAZY_IMPORTS,
		syntax: LAZY_SYNTAX
	},
	routes: {
		globals: HOST_GLOBALS,
		properties: [...NETWORK_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: ROUTES_SYNTAX
	},
	gallery: {
		globals: HOST_GLOBALS,
		properties: [...NETWORK_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: GALLERY_SYNTAX
	}
};

/**
 * The four restricted-rule blocks for one source set.
 *
 * @param {keyof typeof boundaryRules} name
 * @returns {import('eslint').Linter.Config[]}
 */
function boundaryBlocks(name) {
	const { files, ignores } = fileSets[name];
	const rules = boundaryRules[name];
	return [
		{
			name: `ui/${name}/no-restricted-globals`,
			files,
			ignores,
			rules: {
				'no-restricted-globals': ['error', { globals: rules.globals, checkGlobalObject: true }]
			}
		},
		{
			name: `ui/${name}/no-restricted-properties`,
			files,
			ignores,
			rules: { 'no-restricted-properties': ['error', ...rules.properties] }
		},
		{
			name: `ui/${name}/no-restricted-imports`,
			files,
			ignores,
			rules: { 'no-restricted-imports': ['error', { patterns: rules.imports }] }
		},
		{
			name: `ui/${name}/no-restricted-syntax`,
			files,
			ignores,
			rules: { 'no-restricted-syntax': ['error', ...rules.syntax] }
		}
	];
}

/** Every set but the checks: the components, their two carve-outs and the host. */
/** @type {readonly (keyof typeof boundaryRules)[]} */
const SOURCE_SETS = ['components', 'motion', 'lazy', 'routes', 'gallery'];

export default defineConfig(
	includeIgnoreFile(gitignorePath),
	{
		name: 'ui/inline-config',
		// An inline comment can never switch a rule off: the config is the only word.
		linterOptions: { noInlineConfig: true }
	},
	js.configs.recommended,
	ts.configs.base,
	svelte.configs.recommended,
	prettier,
	svelte.configs.prettier,
	{
		name: 'ui/config-files',
		// The package-root config files run in Node and sit outside the TS program.
		files: ['*.ts', '*.js'],
		languageOptions: { globals: globals.node }
	},
	{
		name: 'ui/source-globals',
		files: CODE,
		ignores: CHECKS,
		languageOptions: { globals: globals.browser }
	},
	{
		name: 'ui/checks-globals',
		files: CHECKS,
		languageOptions: { globals: { ...globals.browser, ...globals.node } }
	},
	{
		name: 'ui/no-undef',
		rules: {
			// typescript-eslint advise against no-undef on TypeScript projects: the compiler checks names.
			'no-undef': 'off'
		}
	},
	{
		name: 'ui/typed-ts',
		files: CODE,
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
		name: 'ui/typed-svelte',
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
		name: 'ui/rulebook-switch',
		// Every switch over a union names every variant; a non-union switch needs a default.
		files: CODE,
		rules: {
			'@typescript-eslint/switch-exhaustiveness-check': [
				'error',
				{ considerDefaultExhaustiveForUnions: false, requireDefaultForNonUnion: true }
			]
		}
	},
	{
		name: 'ui/rulebook-assertions',
		// A type assertion validates without parsing; a type predicate is the sanctioned escape.
		files: CODE,
		ignores: CHECKS,
		rules: {
			'@typescript-eslint/consistent-type-assertions': ['error', { assertionStyle: 'never' }]
		}
	},
	{
		name: 'ui/no-eval',
		// Lint guards against accidental coupling; computed tricks are left to review.
		files: CODE,
		ignores: CHECKS,
		rules: {
			'no-eval': 'error',
			'no-new-func': 'error',
			'@typescript-eslint/no-implied-eval': 'error'
		}
	},
	{
		name: 'ui/gallery-nav-links',
		// The nav links pages the registry finds at run time, so no typed route can
		// name them for resolve(); it prefixes each with resolve('/') instead.
		files: ['src/gallery/GalleryNav.svelte'],
		rules: { 'svelte/no-navigation-without-resolve': 'off' }
	},
	{
		name: 'ui/no-at-html',
		files: ['**/*.svelte'],
		rules: { 'svelte/no-at-html-tags': 'error' }
	},
	...SOURCE_SETS.flatMap(boundaryBlocks)
);
