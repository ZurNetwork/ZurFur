import prettier from 'eslint-config-prettier';
import path from 'node:path';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import { defineConfig, includeIgnoreFile } from 'eslint/config';
import globals from 'globals';
import ts from 'typescript-eslint';
import { PAPER_MOVES } from './src/lib/motion/paper-moves.ts';

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
// every file no other set names, so a new file can never land in none. One set
// sits outside src/: Storybook's two config files that run in the browser.
// ---------------------------------------------------------------------------

/** Every file kind ESLint lints under src/, by extension. */
const CODE_EXTENSIONS = ['.ts', '.mts', '.cts', '.js', '.mjs', '.cjs', '.svelte'];

/** Every file kind ESLint lints under src/. */
const CODE = CODE_EXTENSIONS.map((extension) => `src/**/*${extension}`);

/** The checks and every spec: Node globals, and they may throw. */
const CHECKS = ['src/checks/**', 'src/**/*.spec.ts', 'src/**/*.svelte.spec.ts'];

/** The gallery's pages. */
const ROUTES = ['src/routes/**'];

/** The gallery's own frame: the registry, the nav and the page layout. */
const GALLERY = ['src/gallery/**'];

/** The gallery's stores, carved out of its frame: the only files that keep settings in localStorage. */
const STORAGE = ['src/gallery/skin-store.ts'];

/** Storybook's stories and the frame they render in: the gallery host, beside the routes. */
const STORIES = ['src/stories/**'];

/** Storybook's config files that run in the browser: the preview's, and the manager's on its own origin. */
const STORYBOOK_BROWSER = ['.storybook/preview.ts', '.storybook/manager.ts'];

/** Storybook's config files that run in Node or only hand the preview over, with the package-root config files' rules. */
const STORYBOOK_NODE = ['.storybook/main.ts', '.storybook/vitest.setup.ts'];

/** The one place motion libraries may be imported. */
const MOTION = ['src/lib/motion/**'];

/** The one place heavy libraries may be loaded, and only by dynamic import. */
const LAZY = ['src/lib/lazy/**'];

/** The seam components read their moves and sounds through: the one home of Svelte's context. */
const ENVIRONMENT = ['src/lib/environment/**'];

/** Each set's `files` and `ignores`, by name. */
export const fileSets = {
	checks: { files: CHECKS, ignores: [] },
	routes: { files: ROUTES, ignores: CHECKS },
	gallery: { files: GALLERY, ignores: [...CHECKS, ...STORAGE] },
	storage: { files: STORAGE, ignores: CHECKS },
	stories: { files: STORIES, ignores: CHECKS },
	storybookBrowser: { files: STORYBOOK_BROWSER, ignores: [] },
	motion: { files: MOTION, ignores: CHECKS },
	lazy: { files: LAZY, ignores: CHECKS },
	environment: { files: ENVIRONMENT, ignores: CHECKS },
	components: {
		files: CODE,
		ignores: [
			...CHECKS,
			...ROUTES,
			...GALLERY,
			...STORAGE,
			...STORIES,
			...MOTION,
			...LAZY,
			...ENVIRONMENT
		]
	}
};

/** @typedef {keyof typeof fileSets} FileSetName */

/** @type {readonly FileSetName[]} */
const FILE_SET_NAMES = [
	'checks',
	'routes',
	'gallery',
	'storage',
	'stories',
	'storybookBrowser',
	'motion',
	'lazy',
	'environment',
	'components'
];

/**
 * One `files` or `ignores` glob as a regular expression: `**` spans any number
 * of folders and `*` anything within one segment, dots included, as ESLint
 * reads them. Any other glob syntax is refused, so a pattern ESLint would read
 * differently can never be read here quietly.
 *
 * @param {string} glob
 * @returns {RegExp}
 */
function globPattern(glob) {
	if (/[?[\]{}!()+@\\]/.test(glob)) throw new Error(`unsupported glob syntax in ${glob}`);
	const segments = glob.split('/');
	const source = segments
		.map((segment, index) => {
			const isLast = index === segments.length - 1;
			if (segment === '**') return isLast ? '.*' : '(?:[^/]+/)*';
			const text = segment.replace(/[.^$|]/g, '\\$&').replaceAll('*', '[^/]*');
			return isLast ? text : `${text}/`;
		})
		.join('');
	return new RegExp(`^${source}$`);
}

/**
 * @param {readonly string[]} globs
 * @param {string} relative
 */
const matchesAny = (globs, relative) => globs.some((glob) => globPattern(glob).test(relative));

/**
 * The file set a path lands in, read from `fileSets` as ESLint reads it; the
 * path is relative to this package, with forward slashes. Undefined for a path
 * no set names (outside src/ and the browser-side Storybook config, or not a
 * code file).
 *
 * @param {string} relative
 * @returns {FileSetName | undefined}
 */
export function fileSetOf(relative) {
	return FILE_SET_NAMES.find((name) => {
		const { files, ignores } = fileSets[name];
		return matchesAny(files, relative) && !matchesAny(ignores, relative);
	});
}

// ---------------------------------------------------------------------------
// The tiers. The sets form three tiers, strictest first: what ships (the
// components and their three carve-outs), the gallery host, and the checks. A
// relative import never reaches from one tier into a looser one, so no file
// can launder a looser set's freedoms through a re-export. A file no set reads
// (neither code nor .css) counts as the loosest tier unless imported as text or
// a URL, so code hidden under an unread extension never reaches what ships.
// ---------------------------------------------------------------------------

/** @typedef {'shipped' | 'host' | 'checks'} Tier */

/** @type {readonly Tier[]} */
const TIERS_STRICTEST_FIRST = ['shipped', 'host', 'checks'];

/** @type {Readonly<Record<FileSetName, Tier>>} */
const TIER_OF_SET = {
	components: 'shipped',
	motion: 'shipped',
	lazy: 'shipped',
	environment: 'shipped',
	routes: 'host',
	gallery: 'host',
	storage: 'host',
	stories: 'host',
	storybookBrowser: 'host',
	checks: 'checks'
};

/** @type {Readonly<Record<Tier, string>>} */
const TIER_LABEL = {
	shipped: 'a component',
	host: 'the gallery host',
	checks: 'a check or a spec'
};

/**
 * The loosest tier an import of `relative` can land in: the path as written,
 * with each code extension added (a TypeScript `.js` read as `.ts` too), and
 * as a folder's index. Every path inside src/ lands in some tier.
 *
 * @param {string} relative a path under src/, relative to this package
 * @returns {Tier}
 */
function loosestTierOf(relative) {
	const asTypeScript = relative.replace(/\.([mc]?)js$/, '.$1ts');
	const candidates = [
		relative,
		asTypeScript,
		...CODE_EXTENSIONS.map((extension) => `${relative}${extension}`),
		...CODE_EXTENSIONS.map((extension) => `${relative}/index${extension}`)
	];
	const ranks = candidates.flatMap((candidate) => {
		const name = fileSetOf(candidate);
		return name === undefined ? [] : [TIERS_STRICTEST_FIRST.indexOf(TIER_OF_SET[name])];
	});
	return TIERS_STRICTEST_FIRST[Math.max(...ranks)] ?? 'checks';
}

/** Queries that make Vite hand back a file's text or URL, never run it. */
const INERT_QUERIES = ['?raw', '?url'];

/**
 * True when an import of `target` with `suffix` (its query and hash, as
 * written) names a file no lint set reads: the last segment has an extension,
 * after its last dot and matched case-sensitively, that is neither code nor
 * `.css`, and the suffix is not exactly `?raw` or `?url`.
 *
 * @param {string} target
 * @param {string} suffix
 */
function isUnlintedTarget(target, suffix) {
	if (INERT_QUERIES.includes(suffix)) return false;
	const name = path.basename(target);
	const dot = name.lastIndexOf('.');
	if (dot === -1) return false;
	const extension = name.slice(dot);
	return extension !== '.css' && !CODE_EXTENSIONS.includes(extension);
}

// ---------------------------------------------------------------------------
// The bans, as entries the per-set arrays below merge.
// ---------------------------------------------------------------------------

const NETWORK_MESSAGE =
	'Components never reach the network: take the data as a prop and let the app fetch it.';
/** Why a component never keeps anything in the browser. */
export const STORAGE_MESSAGE =
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

/** Why the host keeps settings only through its stores. */
export const HOST_STORAGE_MESSAGE =
	"Only the gallery's stores (src/gallery/skin-store.ts) keep settings, and only in localStorage: go through them.";

/** Why no file puts a string into the page as HTML. */
export const rawHtmlMessage =
	'Raw HTML never reaches the page: render markup with Svelte, never through {@html}, createRawSnippet, svelte/internal, innerHTML, outerHTML, insertAdjacentHTML, setHTML, setHTMLUnsafe, createContextualFragment, DOMParser, parseHTMLUnsafe or document.write.';

/** Why markup never runs script nor loads another document. */
export const scriptMarkupMessage =
	"Markup never runs script or loads another document: no <script>, <iframe>, <frame>, <object> or <embed> element, no srcdoc attribute, and no <svelte:element> naming one. The gallery's one boot script lives only in src/app.html.";

/** Why nothing sets http-equiv. */
export const httpEquivMessage =
	'Nothing sets http-equiv: a <meta http-equiv> can send the page to another origin or rewrite its policy, and only the server sets those, as headers.';

/** Why markup never carries an inline event handler. */
export const inlineHandlerMessage =
	'An inline event-handler attribute runs its text as script: take a function as a prop and attach it with a lowercase on…={handler}, never an on… attribute in another letter case or with text in its value.';

/** The names a browser global is reached through besides its own. */
const GLOBAL_OBJECTS = '/^(?:window|self|globalThis)$/';

/** Browser storage, localStorage aside: refused in every source set. */
const STORAGE_NAMES_EXCEPT_LOCAL = ['sessionStorage', 'indexedDB', 'caches', 'cookieStore'];

/** Every browser storage a script can name as a global. */
const STORAGE_NAMES = ['localStorage', ...STORAGE_NAMES_EXCEPT_LOCAL];

/**
 * Storage, as globals (bare, and through window, self and globalThis) and as a
 * property of any object, which reaches what the globals rule can't see:
 * `document.defaultView.localStorage`, `top.localStorage`, a destructured
 * `{ localStorage } = window`, and `cookie` on any object. The browser's
 * private file store too: `navigator.storage` as a member, bare or
 * destructured, and as syntax through window, self and globalThis, read or
 * destructured, and `getDirectory` on any object. A computed name past
 * `navigator`, or navigator held in a variable, stays with review.
 *
 * @param {readonly string[]} names
 * @param {string} message
 */
function storageBans(names, message) {
	return {
		globals: names.map((name) => ({ name, message })),
		properties: [
			...[...names, 'cookie', 'getDirectory'].map((property) => ({ property, message })),
			{ object: 'navigator', property: 'storage', message }
		],
		syntax: [
			{
				selector: `MemberExpression[property.name='storage'][object.type='MemberExpression'][object.property.name='navigator'][object.object.name=${GLOBAL_OBJECTS}]`,
				message
			},
			{
				selector:
					"VariableDeclarator[init.type='MemberExpression'][init.property.name='navigator'] > ObjectPattern > Property[key.name='storage']",
				message
			}
		]
	};
}

/** The components and their carve-outs: no storage at all. */
const COMPONENT_STORAGE = storageBans(STORAGE_NAMES, STORAGE_MESSAGE);

/** The host outside its stores: no storage at all. */
const HOST_STORAGE = storageBans(STORAGE_NAMES, HOST_STORAGE_MESSAGE);

/** The gallery's stores: localStorage, and nothing else. */
const STORE_STORAGE = storageBans(STORAGE_NAMES_EXCEPT_LOCAL, HOST_STORAGE_MESSAGE);

const PROCESS_GLOBAL = { name: 'process', message: PROCESS_MESSAGE };

/** The HTML parser, by bare name and through window, self and globalThis. */
const HTML_GLOBAL = { name: 'DOMParser', message: rawHtmlMessage };

/** What the components' set and its two carve-outs refuse as globals. */
const COMPONENT_GLOBALS = [
	...NETWORK_GLOBALS,
	...COMPONENT_STORAGE.globals,
	PROCESS_GLOBAL,
	HTML_GLOBAL
];

/** What the host refuses as globals, outside its stores. */
const HOST_GLOBALS = [...NETWORK_GLOBALS, ...HOST_STORAGE.globals, PROCESS_GLOBAL, HTML_GLOBAL];

/** What the gallery's stores refuse as globals: every storage but localStorage. */
const STORE_GLOBALS = [...NETWORK_GLOBALS, ...STORE_STORAGE.globals, PROCESS_GLOBAL, HTML_GLOBAL];

/** Member reads the globals rule can't see. */
const NETWORK_PROPERTIES = [
	{ object: 'navigator', property: 'sendBeacon', message: NETWORK_MESSAGE },
	{ object: 'navigator', property: 'serviceWorker', message: CODE_LOADING_MESSAGE },
	{ object: 'process', property: 'env', message: PROCESS_MESSAGE }
];

/**
 * The script-side twins of {@html} and of the markup bans, as members: on any
 * object, the DOM calls that parse a string as HTML (innerHTML, outerHTML,
 * insertAdjacentHTML, setHTML, setHTMLUnsafe, createContextualFragment,
 * parseFromString and parseHTMLUnsafe), Svelte's createRawSnippet read off a
 * namespace import, and the srcdoc and httpEquiv properties; document.write
 * and document.writeln. The rest of the raw-HTML ban sits beside its kind:
 * DOMParser as a global, createRawSnippet and svelte/internal as imports, and
 * setAttribute, createAttribute and createElement naming a banned attribute or
 * element by a static string as syntax. A computed or variable name,
 * Reflect.set, Object.assign and a method borrowed through call or apply stay
 * with review.
 */
const HTML_PROPERTIES = [
	...[
		'innerHTML',
		'outerHTML',
		'insertAdjacentHTML',
		'setHTML',
		'setHTMLUnsafe',
		'createContextualFragment',
		'parseFromString',
		'parseHTMLUnsafe',
		'createRawSnippet'
	].map((property) => ({ property, message: rawHtmlMessage })),
	{ object: 'document', property: 'write', message: rawHtmlMessage },
	{ object: 'document', property: 'writeln', message: rawHtmlMessage },
	{ property: 'srcdoc', message: scriptMarkupMessage },
	{ property: 'httpEquiv', message: httpEquivMessage }
];

const CLOCK_MESSAGE =
	'Components never read the clock: take the time as a prop, so every render of the same props is the same.';

/** The clock's three faces, as members. */
const CLOCK_PROPERTIES = [
	{ object: 'Date', property: 'now', message: CLOCK_MESSAGE },
	{ object: 'performance', property: 'now', message: CLOCK_MESSAGE },
	{ object: 'Temporal', property: 'Now', message: CLOCK_MESSAGE }
];

/** What the components' set and its carve-outs refuse as members, element.animate() aside. */
const COMPONENT_PROPERTIES = [
	...NETWORK_PROPERTIES,
	...COMPONENT_STORAGE.properties,
	...HTML_PROPERTIES,
	...CLOCK_PROPERTIES
];

/** What the host refuses as members, outside its stores. */
const HOST_PROPERTIES = [...NETWORK_PROPERTIES, ...HOST_STORAGE.properties, ...HTML_PROPERTIES];

/** What the gallery's stores refuse as members. */
const STORE_PROPERTIES = [...NETWORK_PROPERTIES, ...STORE_STORAGE.properties, ...HTML_PROPERTIES];

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

/**
 * `text` as a regular expression that matches it in any letter case, read
 * case-sensitively: each letter becomes a class of its two cases.
 *
 * @param {string} text letters, `@` and `/` only
 */
const anyCase = (text) =>
	[...text]
		.map((character) =>
			/[a-z]/i.test(character)
				? `[${character.toLowerCase()}${character.toUpperCase()}]`
				: character
		)
		.join('');

const HOST_SELF_MESSAGE =
	'The host imports @zurfur/ui from its public entry, @zurfur/ui/styles, @zurfur/ui/tokens.json and @zurfur/ui/tokens.additions.json only, never a file inside it.';

/**
 * The host's deep self imports: everything past the public entry but exactly
 * `@zurfur/ui/styles`, `@zurfur/ui/tokens.json` and
 * `@zurfur/ui/tokens.additions.json`, as the exports map names
 * them (no query, no case variant, no deeper path), read case-sensitively,
 * and the package's name in any other letter case.
 */
const HOST_DEEP_SELF_IMPORTS = [
	{
		regex: '^@zurfur/ui/(?!(?:styles|tokens\\.json|tokens\\.additions\\.json)$)',
		caseSensitive: true,
		message: HOST_SELF_MESSAGE
	},
	{
		regex: `^(?!@zurfur/ui/)${anyCase('@zurfur/ui/')}`,
		caseSensitive: true,
		message: HOST_SELF_MESSAGE
	}
];

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

/** Imports Vite turns into a Worker or a WebAssembly loader, read as Vite reads them. */
const VITE_CODE_LOADING_IMPORTS = [
	{ regex: '[?&](worker|sharedworker)(&|$)', message: CODE_LOADING_MESSAGE },
	{ regex: '\\.wasm\\?init', message: CODE_LOADING_MESSAGE }
];

/**
 * Svelte's raw-HTML helper, createRawSnippet, by name (aliased and re-exported
 * too; a namespace import or export * of svelte names it as well), and
 * Svelte's internal modules, which hold the runtime's raw-HTML helper.
 */
const RAW_HTML_IMPORTS = [
	{ regex: '^svelte$', importNames: ['createRawSnippet'], message: rawHtmlMessage },
	{ regex: '^svelte/internal(/|$)', message: rawHtmlMessage }
];

/** Svelte's stores: a component takes its values as props, never from a store. */
const STORE_IMPORT = {
	regex: '^svelte/store(/|$)',
	message:
		'Components keep no stores: take the value as a prop and report a change through a callback.'
};

/** Svelte's context functions, which only src/lib/environment calls. */
const CONTEXT_FUNCTIONS = [
	'setContext',
	'getContext',
	'hasContext',
	'getAllContexts',
	'createContext'
];

/** Svelte's context functions by name (a namespace import or export * of svelte names them too). */
const CONTEXT_IMPORT = {
	regex: '^svelte$',
	importNames: CONTEXT_FUNCTIONS,
	message:
		"Svelte's context lives only in src/lib/environment: a component reads it once, through useEnvironment()."
};

const ENVIRONMENT_IMPORT_MESSAGE =
	'The seam imports only svelte, svelte/attachments and its own files in src/lib/environment.';

/**
 * The seam's allow-list, read case-sensitively: `svelte`, `svelte/attachments`
 * and paths that start with `./` and never climb out with a `..` segment.
 */
const ENVIRONMENT_IMPORTS_ALLOWED_ONLY = [
	{
		regex: '^(?!(?:svelte|svelte/attachments)$)(?!\\./)',
		caseSensitive: true,
		message: ENVIRONMENT_IMPORT_MESSAGE
	},
	{ regex: '(^|/)\\.\\.(/|$)', caseSensitive: true, message: ENVIRONMENT_IMPORT_MESSAGE }
];

/** The packages the controls wrap: the host shows them only through the public entry. */
const WRAPPED_PACKAGE_IMPORT = {
	regex: '^(bits-ui|@internationalized)(/|$)',
	message:
		"The gallery shows controls only through @zurfur/ui's public entry: bits-ui and @internationalized stay inside the components."
};

/** What every source set refuses as an import, before its own additions. */
const COMMON_IMPORTS = [
	ROOT_IMPORT,
	ENV_IMPORT,
	LIB_IMPORT,
	...SERVER_IMPORTS,
	...VITE_CODE_LOADING_IMPORTS,
	...RAW_HTML_IMPORTS
];

/** What every shipped set refuses as an import, beside the components' list. */
const SHIPPED_IMPORTS = [
	...COMMON_IMPORTS,
	DEEP_SELF_IMPORT,
	ANY_APP_IMPORT,
	GALLERY_IMPORT,
	STORE_IMPORT,
	...HEAVY_IMPORTS
];
const COMPONENT_IMPORTS = [...SHIPPED_IMPORTS, ...ADAPTER_MOTION_IMPORTS, CONTEXT_IMPORT];
const MOTION_IMPORTS = [...SHIPPED_IMPORTS, CONTEXT_IMPORT];
const LAZY_IMPORTS = COMPONENT_IMPORTS;
const ENVIRONMENT_IMPORTS = [
	...SHIPPED_IMPORTS,
	...ADAPTER_MOTION_IMPORTS,
	...ENVIRONMENT_IMPORTS_ALLOWED_ONLY
];
const HOST_IMPORTS = [
	...COMMON_IMPORTS,
	...HOST_DEEP_SELF_IMPORTS,
	APP_IMPORT_EXCEPT_PATHS,
	...ADAPTER_MOTION_IMPORTS,
	...HEAVY_IMPORTS,
	CONTEXT_IMPORT,
	WRAPPED_PACKAGE_IMPORT
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

const FETCH_MESSAGE =
	'No source file names fetch, a load({ fetch }) parameter and a fetcher prop included: take the data as a prop and let the app fetch it.';

/** Any name `fetch`: a parameter, a prop, a key, a member or a markup attribute. */
const FETCH_NAMES = [
	{ selector: "Identifier[name='fetch']", message: FETCH_MESSAGE },
	{
		selector:
			":matches(Property, PropertyDefinition, MethodDefinition, AccessorProperty, TSPropertySignature, TSMethodSignature, TSAbstractPropertyDefinition, TSAbstractMethodDefinition) > Literal.key[value='fetch']",
		message: FETCH_MESSAGE
	},
	{ selector: "MemberExpression > Literal.property[value='fetch']", message: FETCH_MESSAGE },
	{
		selector:
			"MemberExpression > TemplateLiteral.property[expressions.length=0][quasis.0.value.cooked='fetch']",
		message: FETCH_MESSAGE
	},
	{
		selector: ":matches(ImportSpecifier, ExportSpecifier) > Literal[value='fetch']",
		message: FETCH_MESSAGE
	},
	{ selector: "SvelteAttribute[key.name='fetch']", message: FETCH_MESSAGE },
	{ selector: "SvelteDirective[key.name.name='fetch']", message: FETCH_MESSAGE }
];

/** The element names that run script or load another document. */
const SCRIPT_MARKUP_NAMES = 'script|iframe|frame|object|embed';

/** The element names that run script or load another document, in any letter case. */
const SCRIPT_MARKUP_TAGS = `/^(?:${SCRIPT_MARKUP_NAMES})$/i`;

/** A qualified name, with or without a prefix, whose local part is `pattern`, in any letter case. */
const qualified = (/** @type {string} */ pattern) => `/^(?:[^:]*:)?(?:${pattern})$/i`;

/**
 * Markup that runs script or loads another document: the five elements wherever
 * they sit, a srcdoc attribute, and <svelte:element> naming one by a static
 * string. A component's own top-level script blocks parse as
 * SvelteScriptElement, not as markup. ESLint never reads .html, and src/app.html
 * is the only .html file src/ may hold, so its boot script is out of this ban's reach.
 */
const SCRIPT_MARKUP = [
	{
		selector: `SvelteElement[kind='html'][name.name=${SCRIPT_MARKUP_TAGS}]`,
		message: scriptMarkupMessage
	},
	{
		selector: ':matches(SvelteAttribute, SvelteShorthandAttribute)[key.name=/^srcdoc$/i]',
		message: scriptMarkupMessage
	},
	{
		selector: `SvelteElement[name.name='svelte:element'] > SvelteStartTag > SvelteAttribute[key.name='this'] > SvelteLiteral[value=${SCRIPT_MARKUP_TAGS}]`,
		message: scriptMarkupMessage
	},
	{
		selector: `SvelteSpecialDirective[kind='this'] > Literal.expression[value=${SCRIPT_MARKUP_TAGS}]`,
		message: scriptMarkupMessage
	},
	{
		selector: `SvelteSpecialDirective[kind='this'] > TemplateLiteral.expression[expressions.length=0][quasis.0.value.cooked=${SCRIPT_MARKUP_TAGS}]`,
		message: scriptMarkupMessage
	}
];

/** The start tag of an element markup renders as written: an HTML element or <svelte:element>. */
const RENDERED_START_TAG =
	":matches(SvelteElement[kind='html'], SvelteElement[name.name='svelte:element']) > SvelteStartTag";

/**
 * Inline event handlers. Svelte attaches an attribute as a listener only when
 * its name starts with a lowercase `on` and its value is one expression; any
 * other `on…` attribute renders as written, and the browser, which reads
 * attribute names in any letter case, runs its text. A component's props are
 * not markup. A spread can't be read here: ui/spreads refuses every spread but
 * the props a bits-ui component hands a {#snippet child(…)}, whose handlers are
 * functions; a child snippet anything else could render is refused.
 */
const INLINE_HANDLERS = [
	{
		selector: `${RENDERED_START_TAG} > :matches(SvelteAttribute, SvelteShorthandAttribute)[key.name=/^on/i][key.name=/[A-Z]/]`,
		message: inlineHandlerMessage
	},
	{
		selector: `${RENDERED_START_TAG} > SvelteAttribute[key.name=/^on[^A-Z]*$/]:not([value.length=1][value.0.type='SvelteMustacheTag'])`,
		message: inlineHandlerMessage
	}
];

/**
 * A call to one of `methods` on any object whose argument at `index` is a
 * string literal, or a template literal with no expressions, matching `regex`.
 *
 * @param {readonly string[]} methods
 * @param {number} index
 * @param {string} regex
 * @param {string} message
 */
function namedByLiteral(methods, index, regex, message) {
	const call = `CallExpression[callee.type='MemberExpression'][callee.property.name=/^(?:${methods.join('|')})$/]`;
	const argument = `arguments.${String(index)}`;
	return [
		{ selector: `${call}[${argument}.type='Literal'][${argument}.value=${regex}]`, message },
		{
			selector: `${call}[${argument}.type='TemplateLiteral'][${argument}.expressions.length=0][${argument}.quasis.0.value.cooked=${regex}]`,
			message
		}
	];
}

/** The attribute names markup bans, as setAttribute and createAttribute read them. */
const BANNED_ATTRIBUTES = [
	{ regex: qualified('on.*'), message: inlineHandlerMessage },
	{ regex: qualified('srcdoc'), message: scriptMarkupMessage },
	{ regex: qualified('http-equiv'), message: httpEquivMessage }
];

/**
 * The markup bans written as DOM calls: setAttribute and createAttribute (the
 * name first) and setAttributeNS and createAttributeNS (the name second)
 * naming an on… attribute, srcdoc or http-equiv, and createElement (the name
 * first) and createElementNS (the name second) naming one of the five
 * elements, each by a static string in any letter case. A name held in a
 * variable or reached through a computed method stays with review.
 */
const SCRIPT_CALLS = [
	...BANNED_ATTRIBUTES.flatMap(({ regex, message }) => [
		...namedByLiteral(['setAttribute', 'createAttribute'], 0, regex, message),
		...namedByLiteral(['setAttributeNS', 'createAttributeNS'], 1, regex, message)
	]),
	...namedByLiteral(['createElement'], 0, SCRIPT_MARKUP_TAGS, scriptMarkupMessage),
	...namedByLiteral(['createElementNS'], 1, qualified(SCRIPT_MARKUP_NAMES), scriptMarkupMessage)
];

/**
 * http-equiv, in any letter case, on any element or component in markup, the
 * <svelte:head> and <svelte:element> included, and as a quoted object key, the
 * shape a spread of attributes would carry it in.
 */
const HTTP_EQUIV = [
	{
		selector: ':matches(SvelteAttribute, SvelteShorthandAttribute)[key.name=/^http-equiv$/i]',
		message: httpEquivMessage
	},
	{
		selector: 'Property > Literal.key[value=/^http-equiv$/i]',
		message: httpEquivMessage
	}
];

/** `bind:innerHTML` and `bind:outerHTML`: {@html} written as a binding. */
const HTML_BINDING = {
	selector: "SvelteDirective[kind='Binding'][key.name.name=/^(?:innerHTML|outerHTML)$/i]",
	message: rawHtmlMessage
};

const PROPS_MESSAGE =
	'Destructure $props() into named props: a whole props object or a rest element forwards whatever the parent passes.';

/** P5: props come in typed, named and read-only; nothing forwards the rest or binds back. */
const PROPS_SYNTAX = [
	{
		selector: "Identifier[name='$bindable']",
		message:
			'No $bindable(): a control takes its value as a prop and reports a change through a callback.'
	},
	{
		selector:
			"VariableDeclarator[init.type='CallExpression'][init.callee.type='Identifier'][init.callee.name='$props'] > ObjectPattern.id > RestElement",
		message: PROPS_MESSAGE
	},
	{
		selector:
			"VariableDeclarator[id.type!='ObjectPattern'][init.type='CallExpression'][init.callee.type='Identifier'][init.callee.name='$props']",
		message: PROPS_MESSAGE
	},
	{
		selector:
			"CallExpression[callee.type='Identifier'][callee.name='$props']:not(VariableDeclarator > CallExpression.init)",
		message: PROPS_MESSAGE
	}
];

const MODULE_STATE_MESSAGE =
	'No module-level state in a shipped .ts or .js: a module is shared by every instance on every page. Keep state in the component, or inside a function.';

/** Outside any Svelte node: a plain .ts or .js file, never a component's script or markup. */
const OUTSIDE_SVELTE = ':not([type=/^Svelte/] *)';

/**
 * P6: no module-level state in a plain module: a `let` or `var` at the top
 * level (exported or not), and a Map, Set, WeakMap or WeakSet or a state rune
 * built outside a function (a class field counts as outside). A rune inside a
 * function, in the motion adapter's runes modules, is per call, not shared.
 */
const MODULE_STATE_SYNTAX = [
	{
		selector: 'Program > VariableDeclaration[kind=/^(?:let|var)$/]',
		message: MODULE_STATE_MESSAGE
	},
	{
		selector: 'Program > ExportNamedDeclaration > VariableDeclaration[kind=/^(?:let|var)$/]',
		message: MODULE_STATE_MESSAGE
	},
	{
		selector: `NewExpression[callee.type='Identifier'][callee.name=/^(?:Map|Set|WeakMap|WeakSet)$/]:not(:function *)${OUTSIDE_SVELTE}`,
		message: MODULE_STATE_MESSAGE
	},
	{
		selector: `CallExpression[callee.type='Identifier'][callee.name=/^\\$(?:state|derived)$/]:not(:function *)${OUTSIDE_SVELTE}`,
		message: MODULE_STATE_MESSAGE
	},
	{
		selector: `CallExpression[callee.type='MemberExpression'][callee.object.type='Identifier'][callee.object.name=/^\\$(?:state|derived)$/]:not(:function *)${OUTSIDE_SVELTE}`,
		message: MODULE_STATE_MESSAGE
	}
];

/** Why no shipped component holds a module block. */
export const scriptModuleMessage =
	"No <script module>: what it declares is shared by every instance. Keep each control's state in its own instance.";

/** P6: `<script module>`, in either spelling. */
const SCRIPT_MODULE = {
	selector:
		'SvelteScriptElement > SvelteStartTag > SvelteAttribute[key.name=/^(?:module|context)$/]',
	message: scriptModuleMessage
};

/**
 * The clock reads the members miss: through window, self and globalThis,
 * `new Date()` with no argument, and `Date()` called bare, which answers the
 * current time whatever it is handed.
 */
const CLOCK_SYNTAX = [
	{
		selector: `MemberExpression[object.type='MemberExpression'][object.object.name=${GLOBAL_OBJECTS}][object.property.name=/^(?:Date|performance|Temporal)$/][property.name=/^(?:now|Now)$/]`,
		message: CLOCK_MESSAGE
	},
	{
		selector: "NewExpression[callee.type='Identifier'][callee.name='Date'][arguments.length=0]",
		message: CLOCK_MESSAGE
	},
	{
		selector: `NewExpression[callee.type='MemberExpression'][callee.object.name=${GLOBAL_OBJECTS}][callee.property.name='Date'][arguments.length=0]`,
		message: CLOCK_MESSAGE
	},
	{
		selector: "CallExpression[callee.type='Identifier'][callee.name='Date']",
		message: CLOCK_MESSAGE
	},
	{
		selector: `CallExpression[callee.type='MemberExpression'][callee.object.name=${GLOBAL_OBJECTS}][callee.property.name='Date']`,
		message: CLOCK_MESSAGE
	}
];

/** What every shipped set refuses as syntax, beside the components' list. */
const SHIPPED_SYNTAX = [...PROPS_SYNTAX, ...MODULE_STATE_SYNTAX, SCRIPT_MODULE, ...CLOCK_SYNTAX];

/** Why a component never moves on its own. */
export const seamMotionMessage =
	'Components move only through the seam: {@attach moves.squish()} or {@attach moves.note(…)}, never a transition, an animation or a move class of their own.';

/** The move classes, as one alternation. */
const MOVE_NAMES = PAPER_MOVES.join('|');

/** A class list, as text, that names a move class. */
const MOVE_CLASS_TEXT = `/(?:^|\\s)(?:${MOVE_NAMES})(?:\\s|$)/`;

/**
 * The seam's rule for components: no transition:, in:, out: or animate:
 * directive, and no static class naming one of the move classes, as text,
 * as a string in a class expression, or as a class: directive.
 */
const COMPONENT_MOTION_SYNTAX = [
	{ selector: 'SvelteDirective[kind=/^(?:Transition|Animation)$/]', message: seamMotionMessage },
	{
		selector: `SvelteAttribute[key.name='class'] SvelteLiteral[value=${MOVE_CLASS_TEXT}]`,
		message: seamMotionMessage
	},
	{
		selector: `SvelteAttribute[key.name='class'] Literal[value=${MOVE_CLASS_TEXT}]`,
		message: seamMotionMessage
	},
	{
		selector: `SvelteAttribute[key.name='class'] Property > :matches(Identifier, Literal).key:matches([name=/^(?:${MOVE_NAMES})$/], [value=/^(?:${MOVE_NAMES})$/])`,
		message: seamMotionMessage
	},
	{
		selector: `SvelteDirective[kind='Class'][key.name.name=/^(?:${MOVE_NAMES})$/]`,
		message: seamMotionMessage
	}
];

const COMMON_SYNTAX = [
	...RULEBOOK_SYNTAX,
	IMPORT_META_ENV,
	IMPORT_META_ENV_COMPUTED,
	...FETCH_NAMES,
	...SCRIPT_MARKUP,
	...INLINE_HANDLERS,
	HTML_BINDING,
	...SCRIPT_CALLS,
	...HTTP_EQUIV
];

const COMPONENT_SYNTAX = [
	...COMMON_SYNTAX,
	...ANY_GLOB,
	ANY_DYNAMIC_IMPORT,
	...COMPONENT_STORAGE.syntax,
	...SHIPPED_SYNTAX,
	...COMPONENT_MOTION_SYNTAX
];
const MOTION_SYNTAX = [
	...COMMON_SYNTAX,
	...ANY_GLOB,
	ANY_DYNAMIC_IMPORT,
	...COMPONENT_STORAGE.syntax,
	...SHIPPED_SYNTAX
];
const LAZY_SYNTAX = [
	...COMMON_SYNTAX,
	...ANY_GLOB,
	DYNAMIC_IMPORT_EXCEPT_HEAVY,
	...COMPONENT_STORAGE.syntax,
	...SHIPPED_SYNTAX,
	...COMPONENT_MOTION_SYNTAX
];
/** The host's syntax bans outside its frame and stores: the pages, the stories and the browser-side Storybook config. */
const HOST_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT, ...HOST_STORAGE.syntax];
const GALLERY_SYNTAX = [
	...COMMON_SYNTAX,
	...GLOB_EXCEPT_PAGE_METAS,
	ANY_DYNAMIC_IMPORT,
	...HOST_STORAGE.syntax
];
const STORE_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT, ...STORE_STORAGE.syntax];

// ---------------------------------------------------------------------------
// The package's own rules, for what no built-in rule can see.
// ---------------------------------------------------------------------------

const PACKAGE_ROOT = import.meta.dirname;

/**
 * One property of an AST node, of any parser's node type.
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

/**
 * The specifier node of a type query's `import('…')`: `source` when it holds a
 * string literal, else the older `argument.literal`.
 *
 * @param {unknown} node the TSImportType
 * @returns {unknown}
 */
function importTypeSource(node) {
	const source = fieldOf(node, 'source');
	const holdsString =
		fieldOf(source, 'type') === 'Literal' && typeof fieldOf(source, 'value') === 'string';
	return holdsString ? source : fieldOf(fieldOf(node, 'argument'), 'literal');
}

/** @type {import('eslint').Rule.RuleModule} */
const importTiersRule = {
	meta: {
		type: 'problem',
		docs: { description: 'A relative import never reaches a looser file set, nor leaves src/.' },
		schema: [
			{
				type: 'object',
				properties: { tier: { enum: ['shipped', 'host'] } },
				required: ['tier'],
				additionalProperties: false
			}
		],
		messages: {
			looser:
				'{{importer}} never imports {{target}} ({{specifier}}): those files answer to looser rules.',
			unlinted:
				'{{importer}} never imports a file lint does not read ({{specifier}}): name a code or .css file, or import it with ?raw or ?url.',
			outside: 'A relative import never leaves src/ ({{specifier}}): import a package by its name.'
		}
	},
	create(context) {
		const option = fieldOf(context.options[0], 'tier');
		const importer = TIERS_STRICTEST_FIRST.find((tier) => tier === option) ?? 'shipped';
		const importerRank = TIERS_STRICTEST_FIRST.indexOf(importer);
		const folder = path.dirname(context.filename);

		/** @param {import('eslint').Rule.Node} node @param {unknown} source */
		function check(node, source) {
			const specifier = staticTextOf(source);
			if (specifier === undefined || !specifier.startsWith('.')) return;
			// Vite drops the query and the hash before it resolves a path.
			const bare = specifier.replace(/[?#].*$/s, '');
			const suffix = specifier.slice(bare.length);
			const target = path
				.relative(PACKAGE_ROOT, path.resolve(folder, bare))
				.split(path.sep)
				.join('/');
			if (!target.startsWith('src/')) {
				context.report({ node, messageId: 'outside', data: { specifier } });
				return;
			}
			const tier = loosestTierOf(target);
			const isLooser = TIERS_STRICTEST_FIRST.indexOf(tier) > importerRank;
			const isUnlinted = isUnlintedTarget(target, suffix);
			if (!isLooser && !isUnlinted) return;
			const label = TIER_LABEL[importer];
			context.report({
				node,
				messageId: isLooser ? 'looser' : 'unlinted',
				data: {
					importer: `${label.charAt(0).toUpperCase()}${label.slice(1)}`,
					target: TIER_LABEL[tier],
					specifier
				}
			});
		}

		/** @param {import('eslint').Rule.Node} node */
		const checkSource = (node) => {
			check(node, fieldOf(node, 'source'));
		};

		return {
			ImportDeclaration: checkSource,
			ExportAllDeclaration: checkSource,
			ExportNamedDeclaration: checkSource,
			ImportExpression: checkSource,
			/** @param {import('eslint').Rule.Node} node */
			TSImportType(node) {
				check(node, importTypeSource(node));
			},
			/** @param {import('eslint').Rule.Node} node */
			TSExternalModuleReference(node) {
				check(node, fieldOf(node, 'expression'));
			}
		};
	}
};

/**
 * `text` as a browser's URL parser reads its start: tabs and newlines dropped,
 * leading controls and spaces trimmed, in lower case.
 *
 * @param {string} text
 */
function urlReadingOf(text) {
	const unbroken = text.replace(/[\t\n\r]/g, '');
	let start = 0;
	while (start < unbroken.length && unbroken.charCodeAt(start) <= 0x20) start += 1;
	return unbroken.slice(start).toLowerCase();
}

/**
 * True when a browser would read `text` as a URL on another origin: `http:`,
 * `https:` or protocol-relative, with `\` read as `/`, as the network scan reads it.
 *
 * @param {string} text
 */
function readsAsRemote(text) {
	const read = urlReadingOf(text);
	return read.startsWith('http:') || read.startsWith('https:') || /^[/\\]{2}/.test(read);
}

/**
 * True when a browser would read `text` as a `javascript:` URL.
 *
 * @param {string} text
 */
function readsAsScriptUrl(text) {
	return urlReadingOf(text).startsWith('javascript:');
}

/**
 * Visitors that hand `check` every string in a file: a string literal, each
 * text part of a template literal, and each text part of a markup attribute.
 *
 * @param {(node: import('eslint').Rule.Node, text: unknown) => void} check
 * @returns {import('eslint').Rule.RuleListener}
 */
function stringVisitors(check) {
	return {
		Literal(node) {
			check(node, node.value);
		},
		TemplateElement(node) {
			check(node, node.value.cooked ?? node.value.raw);
		},
		/** @param {import('eslint').Rule.Node} node */
		SvelteLiteral(node) {
			check(node, fieldOf(node, 'value'));
		}
	};
}

/** @type {import('eslint').Rule.RuleModule} */
const noRemoteStringRule = {
	meta: {
		type: 'problem',
		docs: { description: 'No string holds a URL a browser would fetch from another origin.' },
		schema: [],
		messages: {
			remote:
				'No remote URL in source ({{text}}): nothing here reaches the network; take the URL as a prop.'
		}
	},
	create(context) {
		return stringVisitors((node, text) => {
			if (typeof text !== 'string' || !readsAsRemote(text)) return;
			context.report({ node, messageId: 'remote', data: { text: text.trim().slice(0, 40) } });
		});
	}
};

/** @type {import('eslint').Rule.RuleModule} */
const noScriptUrlRule = {
	meta: {
		type: 'problem',
		docs: { description: 'No string holds a javascript: URL.' },
		schema: [],
		messages: {
			script:
				'No javascript: URL in source ({{text}}): following it runs its text as script; take a function as a prop and attach it with on…={handler}.'
		}
	},
	create(context) {
		return stringVisitors((node, text) => {
			if (typeof text !== 'string' || !readsAsScriptUrl(text)) return;
			context.report({ node, messageId: 'script', data: { text: text.trim().slice(0, 40) } });
		});
	}
};

/**
 * A path as written in an import, relative to this package with forward
 * slashes, resolved from the importing file; undefined for a bare specifier.
 *
 * @param {string} filename the importing file
 * @param {string} specifier
 * @returns {string | undefined}
 */
function packagePathOf(filename, specifier) {
	if (!specifier.startsWith('.')) return undefined;
	const bare = specifier.replace(/[?#].*$/s, '');
	return path
		.relative(PACKAGE_ROOT, path.resolve(path.dirname(filename), bare))
		.split(path.sep)
		.join('/');
}

/** `relative` with forward slashes, relative to this package. */
const packageRelative = (/** @type {string} */ filename) =>
	path.relative(PACKAGE_ROOT, filename).split(path.sep).join('/');

/**
 * True when `target` is `folder` or a path inside it.
 *
 * @param {string} target
 * @param {string} folder
 */
const isInside = (target, folder) => target === folder || target.startsWith(`${folder}/`);

/** The package's own name, in any letter case, alone or with a subpath. */
const SELF_SPECIFIER = /^@zurfur\/ui(?:\/|$)/i;

/** The folder the motion adapter lives in. */
const MOTION_FOLDER = 'src/lib/motion';

/** The folder the seam lives in. */
const ENVIRONMENT_FOLDER = 'src/lib/environment';

/** The public entry, which re-exports the seam's provideEnvironment for the host. */
const PUBLIC_ENTRY = 'src/lib/index.ts';

/**
 * Visitors that hand `check` the source of every import, re-export, dynamic
 * import, type import and import-equals in a file.
 *
 * @param {(node: import('eslint').Rule.Node, source: unknown) => void} check
 * @returns {import('eslint').Rule.RuleListener}
 */
function sourceVisitors(check) {
	/** @param {import('eslint').Rule.Node} node */
	const checkSource = (node) => {
		check(node, fieldOf(node, 'source'));
	};
	return {
		ImportDeclaration: checkSource,
		ExportAllDeclaration: checkSource,
		ExportNamedDeclaration: checkSource,
		ImportExpression: checkSource,
		/** @param {import('eslint').Rule.Node} node */
		TSImportType(node) {
			check(node, importTypeSource(node));
		},
		/** @param {import('eslint').Rule.Node} node */
		TSExternalModuleReference(node) {
			check(node, fieldOf(node, 'expression'));
		}
	};
}

/** @type {import('eslint').Rule.RuleModule} */
const noMotionImportRule = {
	meta: {
		type: 'problem',
		docs: {
			description:
				"No shipped file but the adapter's own and the public entry imports the motion adapter, by path or through the entry; a plain module may pull in its stylesheet."
		},
		schema: [],
		messages: {
			motion:
				'{{specifier}} reaches src/lib/motion: components move only through the seam (useEnvironment()), never by importing the motion adapter.'
		}
	},
	create(context) {
		if (packageRelative(context.filename) === PUBLIC_ENTRY) return {};
		const isComponent = context.filename.endsWith('.svelte');
		return sourceVisitors((node, source) => {
			const specifier = staticTextOf(source);
			if (specifier === undefined) return;
			const target = packagePathOf(context.filename, specifier);
			if (target === undefined) {
				if (SELF_SPECIFIER.test(specifier)) {
					context.report({ node, messageId: 'motion', data: { specifier } });
				}
				return;
			}
			// The styles entry pulls in the move classes' stylesheet; no component may.
			const isStylesheet = !isComponent && target.endsWith('.css');
			if (isInside(target, MOTION_FOLDER) && !isStylesheet) {
				context.report({ node, messageId: 'motion', data: { specifier } });
			}
		});
	}
};

/**
 * The name an import or export specifier reads from its module: an
 * identifier's name or a string name's value.
 *
 * @param {unknown} name
 * @returns {string | undefined}
 */
function specifierName(name) {
	const identifier = fieldOf(name, 'name');
	if (typeof identifier === 'string') return identifier;
	const value = fieldOf(name, 'value');
	return typeof value === 'string' ? value : undefined;
}

/** @type {import('eslint').Rule.RuleModule} */
const noProvideEnvironmentRule = {
	meta: {
		type: 'problem',
		docs: {
			description:
				'Only the host and the checks take provideEnvironment; no shipped file outside the seam imports it.'
		},
		schema: [],
		messages: {
			provide:
				'Only the host and the checks call provideEnvironment: a component reads the environment with useEnvironment() ({{specifier}}).',
			whole:
				'Import the seam by name, never as a namespace or with export *: either would hand over provideEnvironment ({{specifier}}).'
		}
	},
	create(context) {
		const isEntry = packageRelative(context.filename) === PUBLIC_ENTRY;

		/** @param {unknown} source */
		function reachesSeam(source) {
			const specifier = staticTextOf(source);
			if (specifier === undefined) return undefined;
			const target = packagePathOf(context.filename, specifier);
			const reaches =
				target === undefined
					? SELF_SPECIFIER.test(specifier)
					: isInside(target, ENVIRONMENT_FOLDER);
			return reaches ? specifier : undefined;
		}

		/**
		 * @param {import('eslint').Rule.Node} node
		 * @param {readonly unknown[]} specifiers
		 * @param {boolean} isReexport
		 */
		function check(node, specifiers, isReexport) {
			const specifier = reachesSeam(fieldOf(node, 'source'));
			if (specifier === undefined) return;
			for (const part of specifiers) {
				const type = fieldOf(part, 'type');
				if (type === 'ImportNamespaceSpecifier') {
					context.report({ node, messageId: 'whole', data: { specifier } });
					continue;
				}
				const read =
					type === 'ImportSpecifier' ? fieldOf(part, 'imported') : fieldOf(part, 'local');
				const isProvide = specifierName(read) === 'provideEnvironment';
				if (isProvide && !(isReexport && isEntry)) {
					context.report({ node, messageId: 'provide', data: { specifier } });
				}
			}
		}

		return {
			/** @param {import('eslint').Rule.Node} node */
			ImportDeclaration(node) {
				const specifiers = fieldOf(node, 'specifiers');
				check(node, Array.isArray(specifiers) ? specifiers : [], false);
			},
			/** @param {import('eslint').Rule.Node} node */
			ExportNamedDeclaration(node) {
				const specifiers = fieldOf(node, 'specifiers');
				check(node, Array.isArray(specifiers) ? specifiers : [], true);
			},
			/** @param {import('eslint').Rule.Node} node */
			ExportAllDeclaration(node) {
				const specifier = reachesSeam(fieldOf(node, 'source'));
				if (specifier === undefined) return;
				context.report({ node, messageId: 'whole', data: { specifier } });
			}
		};
	}
};

/** The only files that may write <svelte:element>, relative to this package. */
export const SVELTE_ELEMENT_FILES = ['src/lib/surfaces/Card.svelte'];

/**
 * The files that may also hand their {#snippet child(…)} to one of our own
 * components: the component's default import and the attribute that takes it.
 * That component must render the snippet only with what bits-ui hands it.
 */
const SNIPPET_HANDOFF_FILES = [
	{ file: 'src/lib/controls/IconButton.svelte', source: './Tooltip.svelte', attribute: 'trigger' }
];

/** The one module whose components hand a {#snippet child(…)} its props. */
const BITS_UI = 'bits-ui';

/** What bits-ui's {#snippet child(…)} hands over to be spread. */
const SNIPPET_PROPS = ['props', 'wrapperProps'];

/**
 * The nearest {#snippet} block above `node`, or undefined.
 *
 * @param {unknown} node
 * @returns {unknown}
 */
function nearestSnippet(node) {
	let current = fieldOf(node, 'parent');
	while (current !== undefined && current !== null) {
		if (fieldOf(current, 'type') === 'SvelteSnippetBlock') return current;
		current = fieldOf(current, 'parent');
	}
	return undefined;
}

/**
 * The variable `name` resolves to where `node` sits, or undefined.
 *
 * @param {import('eslint').SourceCode} sourceCode
 * @param {import('eslint').Rule.Node} node
 * @param {string} name
 */
function variableAt(sourceCode, node, name) {
	/** @type {import('eslint').Scope.Scope | null} */
	let scope = sourceCode.getScope(node);
	let variable = undefined;
	while (scope !== null && variable === undefined) {
		variable = scope.set.get(name);
		scope = scope.upper;
	}
	return variable;
}

/**
 * The import that alone binds a component element's root name (`Switch` in
 * `<Switch.Root>`), as its source and specifier, or undefined.
 *
 * @param {unknown} element
 * @param {import('eslint').SourceCode} sourceCode
 * @returns {{ source: string, specifier: unknown, isMember: boolean } | undefined}
 */
function componentImport(element, sourceCode) {
	if (fieldOf(element, 'type') !== 'SvelteElement' || fieldOf(element, 'kind') !== 'component') {
		return undefined;
	}
	const name = fieldOf(element, 'name');
	let root = name;
	while (fieldOf(root, 'type') === 'SvelteMemberExpressionName') root = fieldOf(root, 'object');
	const rootName = fieldOf(root, 'name');
	if (fieldOf(root, 'type') !== 'Identifier' || typeof rootName !== 'string') return undefined;
	const variable = variableAt(
		sourceCode,
		/** @type {import('eslint').Rule.Node} */ (element),
		rootName
	);
	if (variable === undefined || variable.defs.length !== 1) return undefined;
	const [definition] = variable.defs;
	if (definition === undefined || definition.type !== 'ImportBinding') return undefined;
	const declaration = definition.parent;
	if (fieldOf(declaration, 'type') !== 'ImportDeclaration') return undefined;
	if (
		fieldOf(declaration, 'importKind') === 'type' ||
		fieldOf(definition.node, 'importKind') === 'type'
	) {
		return undefined;
	}
	const source = fieldOf(fieldOf(declaration, 'source'), 'value');
	if (typeof source !== 'string') return undefined;
	return { source, specifier: definition.node, isMember: root !== name };
}

/**
 * The attribute whose whole value is `identifier` (`{child}` or
 * `name={child}`), as its name and the element it sits on, or undefined.
 *
 * @param {unknown} identifier
 * @returns {{ name: string, element: unknown } | undefined}
 */
function attributeValuedBy(identifier) {
	const parent = fieldOf(identifier, 'parent');
	let attribute = undefined;
	if (fieldOf(parent, 'type') === 'SvelteShorthandAttribute') {
		if (fieldOf(parent, 'value') === identifier) attribute = parent;
	} else if (
		fieldOf(parent, 'type') === 'SvelteMustacheTag' &&
		fieldOf(parent, 'expression') === identifier
	) {
		const holder = fieldOf(parent, 'parent');
		const value = fieldOf(holder, 'value');
		const isWhole = Array.isArray(value) && value.length === 1 && value[0] === parent;
		if (fieldOf(holder, 'type') === 'SvelteAttribute' && isWhole) attribute = holder;
	}
	const startTag = fieldOf(attribute, 'parent');
	const name = fieldOf(fieldOf(attribute, 'key'), 'name');
	if (fieldOf(startTag, 'type') !== 'SvelteStartTag' || typeof name !== 'string') return undefined;
	return { name, element: fieldOf(startTag, 'parent') };
}

/**
 * True when `inner`'s source lies within `outer`'s.
 *
 * @param {unknown} inner
 * @param {unknown} outer
 */
function liesWithin(inner, outer) {
	const within = fieldOf(inner, 'range');
	const around = fieldOf(outer, 'range');
	if (!Array.isArray(within) || !Array.isArray(around)) return false;
	const [start, end] = within;
	const [from, to] = around;
	return (
		typeof start === 'number' &&
		typeof end === 'number' &&
		typeof from === 'number' &&
		typeof to === 'number' &&
		start >= from &&
		end <= to
	);
}

/**
 * True when `snippet`, a {#snippet child(…)}, can only ever be rendered by a
 * bits-ui component: it sits right under one, which takes it as `child`, and
 * nothing inside that component names it; or it sits anywhere else and every
 * use of its name, at least one, is the value of a `child` attribute on one.
 * A listed file may also hand it to the one component and attribute its entry
 * names. A use the scope analysis can't resolve counts as a use.
 *
 * @param {unknown} snippet
 * @param {import('eslint').SourceCode} sourceCode
 * @param {{ source: string, attribute: string } | undefined} handoff
 */
function isRenderedByBitsUi(snippet, sourceCode, handoff) {
	const isBitsUi = (/** @type {unknown} */ element) =>
		componentImport(element, sourceCode)?.source === BITS_UI;
	const id = fieldOf(snippet, 'id');
	const name = fieldOf(id, 'name');
	if (typeof name !== 'string') return false;
	const scopes = sourceCode.scopeManager?.scopes ?? [];
	const named = scopes
		.flatMap((scope) => scope.references)
		.filter((reference) => reference.identifier.name === name && reference.identifier !== id);
	const parent = fieldOf(snippet, 'parent');
	if (fieldOf(parent, 'type') === 'SvelteElement' && fieldOf(parent, 'kind') !== 'html') {
		const inside = named.filter((reference) => liesWithin(reference.identifier, parent));
		return inside.length === 0 && isBitsUi(parent);
	}
	const variable = scopes
		.flatMap((scope) => scope.variables)
		.find((candidate) =>
			candidate.defs.some(
				(definition) => definition.type === 'FunctionName' && definition.node === snippet
			)
		);
	if (variable === undefined || variable.defs.length !== 1) return false;
	const uses = named.filter(
		(reference) => reference.resolved === variable || reference.resolved === null
	);
	/** @param {{ name: string, element: unknown }} attribute */
	const isHandoff = (attribute) => {
		if (handoff === undefined || attribute.name !== handoff.attribute) return false;
		const target = componentImport(attribute.element, sourceCode);
		return (
			target !== undefined &&
			target.source === handoff.source &&
			!target.isMember &&
			fieldOf(target.specifier, 'type') === 'ImportDefaultSpecifier'
		);
	};
	return (
		uses.length > 0 &&
		uses.every((reference) => {
			const attribute = attributeValuedBy(reference.identifier);
			if (attribute === undefined) return false;
			return (attribute.name === 'child' && isBitsUi(attribute.element)) || isHandoff(attribute);
		})
	);
}

/**
 * True when a spread attribute spreads exactly what bits-ui hands the
 * nearest {#snippet child(…)}: it sits on an HTML element, its argument is a
 * bare name, that name resolves to `props` or `wrapperProps` destructured
 * from that snippet's one parameter, nothing nearer shadowing it, and only a
 * bits-ui component can render that snippet (see isRenderedByBitsUi).
 *
 * @param {import('eslint').Rule.Node} node the SvelteSpreadAttribute
 * @param {import('eslint').SourceCode} sourceCode
 * @param {{ source: string, attribute: string } | undefined} handoff
 */
function isSnippetPropsSpread(node, sourceCode, handoff) {
	const element = fieldOf(fieldOf(node, 'parent'), 'parent');
	if (fieldOf(element, 'type') !== 'SvelteElement' || fieldOf(element, 'kind') !== 'html') {
		return false;
	}
	const argument = fieldOf(node, 'argument');
	const name = fieldOf(argument, 'name');
	if (fieldOf(argument, 'type') !== 'Identifier' || typeof name !== 'string') return false;
	const snippet = nearestSnippet(node);
	if (fieldOf(fieldOf(snippet, 'id'), 'name') !== 'child') return false;
	const params = fieldOf(snippet, 'params');
	const pattern = Array.isArray(params) && params.length === 1 ? params[0] : undefined;
	if (fieldOf(pattern, 'type') !== 'ObjectPattern') return false;
	const variable = variableAt(sourceCode, node, name);
	if (variable === undefined || variable.defs.length !== 1) return false;
	const [definition] = variable.defs;
	if (definition === undefined || definition.type !== 'Parameter') return false;
	if (definition.node !== snippet) return false;
	const property = fieldOf(definition.name, 'parent');
	const key = fieldOf(property, 'key');
	const isHandedProps =
		fieldOf(property, 'type') === 'Property' &&
		fieldOf(property, 'parent') === pattern &&
		fieldOf(property, 'value') === definition.name &&
		fieldOf(property, 'computed') === false &&
		fieldOf(key, 'type') === 'Identifier' &&
		SNIPPET_PROPS.some((allowed) => fieldOf(key, 'name') === allowed);
	return isHandedProps && isRenderedByBitsUi(snippet, sourceCode, handoff);
}

/** @type {import('eslint').Rule.RuleModule} */
const spreadsRule = {
	meta: {
		type: 'problem',
		docs: {
			description:
				'A spread carries only the props a bits-ui component hands its {#snippet child(…)} onto an HTML element; <svelte:element> only in its listed files.'
		},
		schema: [],
		messages: {
			spread:
				'A spread passes attributes nobody reads here, an inline handler included: spread only the props or wrapperProps of the nearest {#snippet child(…)}, onto an HTML element inside it, and let only bits-ui render that snippet: put it right under a bits-ui component, or hand it to one as child and nowhere else (SNIPPET_HANDOFF_FILES lists the exceptions).',
			element:
				'<svelte:element> picks its tag at run time: write the element itself. Only the files on SVELTE_ELEMENT_FILES may use it.'
		}
	},
	create(context) {
		const file = packageRelative(context.filename);
		const mayUseElement = SVELTE_ELEMENT_FILES.includes(file);
		const handoff = SNIPPET_HANDOFF_FILES.find((entry) => entry.file === file);
		return {
			/** @param {import('eslint').Rule.Node} node */
			SvelteSpreadAttribute(node) {
				if (isSnippetPropsSpread(node, context.sourceCode, handoff)) return;
				context.report({ node, messageId: 'spread' });
			},
			/** @param {import('eslint').Rule.Node} node */
			"SvelteElement[name.name='svelte:element']"(node) {
				if (mayUseElement) return;
				context.report({ node, messageId: 'element' });
			}
		};
	}
};

/** The package's own rules, as an inline plugin. */
const uiPlugin = {
	meta: { name: 'ui' },
	rules: {
		'import-tiers': importTiersRule,
		'no-remote-string': noRemoteStringRule,
		'no-script-url': noScriptUrlRule,
		'no-motion-import': noMotionImportRule,
		'no-provide-environment': noProvideEnvironmentRule,
		spreads: spreadsRule
	}
};

/**
 * The host's rules outside its frame and stores: no network, no storage of any
 * kind, no raw HTML or script markup, no environment, no dynamic or glob
 * import, and relative imports only within the shipped and host tiers.
 */
const HOST_RULES = {
	tier: /** @type {const} */ ('host'),
	globals: HOST_GLOBALS,
	properties: [...HOST_PROPERTIES, ANIMATE_PROPERTY],
	imports: HOST_IMPORTS,
	syntax: HOST_SYNTAX,
	seam: { motionImport: false, provideEnvironment: false }
};

/**
 * Each source set's tier, its options for the four restricted rules, and
 * which of the seam's import rules it takes: no import reaching the motion
 * adapter (every shipped set but the adapter itself) and no provideEnvironment
 * (every shipped set but the seam). A
 * later flat-config block replaces an earlier one's options for the same rule,
 * so each set gets one block per rule holding every entry it needs.
 */
export const boundaryRules = {
	components: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: [...COMPONENT_PROPERTIES, ANIMATE_PROPERTY],
		imports: COMPONENT_IMPORTS,
		syntax: COMPONENT_SYNTAX,
		seam: { motionImport: true, provideEnvironment: true }
	},
	motion: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: COMPONENT_PROPERTIES,
		imports: MOTION_IMPORTS,
		syntax: MOTION_SYNTAX,
		seam: { motionImport: false, provideEnvironment: true }
	},
	lazy: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: [...COMPONENT_PROPERTIES, ANIMATE_PROPERTY],
		imports: LAZY_IMPORTS,
		syntax: LAZY_SYNTAX,
		seam: { motionImport: true, provideEnvironment: true }
	},
	environment: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: [...COMPONENT_PROPERTIES, ANIMATE_PROPERTY],
		imports: ENVIRONMENT_IMPORTS,
		syntax: COMPONENT_SYNTAX,
		seam: { motionImport: true, provideEnvironment: false }
	},
	routes: HOST_RULES,
	gallery: {
		tier: /** @type {const} */ ('host'),
		globals: HOST_GLOBALS,
		properties: [...HOST_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: GALLERY_SYNTAX,
		seam: { motionImport: false, provideEnvironment: false }
	},
	storage: {
		tier: /** @type {const} */ ('host'),
		globals: STORE_GLOBALS,
		properties: [...STORE_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: STORE_SYNTAX,
		seam: { motionImport: false, provideEnvironment: false }
	},
	stories: HOST_RULES,
	storybookBrowser: HOST_RULES
};

/**
 * The four restricted-rule blocks and the package-rules block for one source set.
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
		},
		{
			name: `ui/${name}/package-rules`,
			files,
			ignores,
			rules: {
				'ui/import-tiers': ['error', { tier: rules.tier }],
				'ui/no-remote-string': 'error',
				'ui/no-script-url': 'error',
				...(rules.seam.motionImport ? { 'ui/no-motion-import': 'error' } : {}),
				...(rules.seam.provideEnvironment ? { 'ui/no-provide-environment': 'error' } : {})
			}
		}
	];
}

/** Every set but the checks: the components, their three carve-outs, the host, its stores and the browser-side Storybook config. */
/** @type {readonly (keyof typeof boundaryRules)[]} */
const SOURCE_SETS = [
	'components',
	'motion',
	'lazy',
	'environment',
	'routes',
	'gallery',
	'storage',
	'stories',
	'storybookBrowser'
];

// ---------------------------------------------------------------------------
// The load bans: timers, load starters and media. Kept in one block of their
// own. A later flat-config block replaces an earlier one's options for the
// same rule, so these entries run ESLint's own restricted-name rules under the
// `ui-load` names, beside the boundary blocks' entries rather than over them.
// ---------------------------------------------------------------------------

const { builtinRules } = await import('eslint/use-at-your-own-risk');

/**
 * ESLint's built-in rules, by name. Its rule map is read through the package's
 * unstable entry; a missing rule fails the config load rather than linting less.
 *
 * @param {string} name
 * @returns {import('eslint').Rule.RuleModule}
 */
function builtinRule(name) {
	const rule = builtinRules.get(name);
	if (rule === undefined) throw new Error(`ESLint's built-in rule ${name} not found`);
	return rule;
}

/** Why a component never schedules work of its own. */
export const timerMessage =
	'Components never schedule work of their own: no timers, frames or idle callbacks. Take the moment as a prop, or move through the seam.';

/** Why nothing outside the checks starts a load of its own. */
export const loadStarterMessage =
	'Nothing outside the checks starts a load of its own: no Image, Audio, createElement or createElementNS. Render the element in markup, from a prop.';

/** Why the motion adapter builds only spans. */
export const motionElementMessage =
	"The motion adapter builds only spans, with createElement('span'), and never calls createElementNS.";

/** Why no markup holds media or a picture a stylesheet loads. */
export const mediaMessage =
	'No markup loads media of its own: no <picture>, <source>, <video>, <audio>, <iframe>, <object>, <embed> or <track>, no srcset, imagesrcset or poster, no SVG <image>, <use> or <feImage> with an href, no url() or other image function in a style attribute or style: directive, no value in a style attribute, no value in a style: directive but a few that never hold a picture, no background attribute, no input with a src, no value or url() but url(#…) in an SVG mask, filter, clip-path, fill, stroke, marker or cursor, and no <svelte:element> naming a prefixed tag such as svg:image.';

/** Why a component never reaches an inline style from code. */
export const inlineStyleMessage =
	"Components never reach an element's inline style from code: no .style, attributeStyleMap, or setAttribute or createAttribute naming style or background. Style it in markup, with a class or a token.";

/** The timers' globals, bare and through window, self and globalThis. */
const TIMER_GLOBALS = [
	'setTimeout',
	'setInterval',
	'requestAnimationFrame',
	'requestIdleCallback',
	'scheduler'
].map((name) => ({ name, message: timerMessage }));

/** A timer reached as a member: an abort signal that fires after a delay. */
const TIMER_PROPERTIES = [{ object: 'AbortSignal', property: 'timeout', message: timerMessage }];

const IMAGE_GLOBAL = { name: 'Image', message: loadStarterMessage };
const AUDIO_GLOBAL = { name: 'Audio', message: loadStarterMessage };

/** The load starters every set but the checks refuses. */
const STARTER_GLOBALS = [IMAGE_GLOBAL, AUDIO_GLOBAL];

/** createElement and createElementNS on any object, destructured too. */
const ELEMENT_PROPERTIES = ['createElement', 'createElementNS'].map((property) => ({
	property,
	message: loadStarterMessage
}));

/**
 * The motion adapter's one way to build an element: `createElement('span')`,
 * called with that one string literal. Any other use of `createElement`, by
 * name or computed, destructured, and every `createElementNS` is refused.
 */
const MOTION_ELEMENT_SYNTAX = [
	{
		selector:
			"MemberExpression[property.name='createElement']:not(CallExpression[arguments.length=1][arguments.0.type='Literal'][arguments.0.value='span'] > MemberExpression.callee)",
		message: motionElementMessage
	},
	{
		selector: 'MemberExpression[property.value=/^createElement(?:NS)?$/]',
		message: motionElementMessage
	},
	{ selector: "MemberExpression[property.name='createElementNS']", message: motionElementMessage },
	{
		selector: 'ObjectPattern > Property[key.name=/^createElement(?:NS)?$/]',
		message: motionElementMessage
	},
	{
		selector: 'ObjectPattern > Property[key.value=/^createElement(?:NS)?$/]',
		message: motionElementMessage
	}
];

/** The media elements, in any letter case. */
const MEDIA_TAGS = '/^(?:picture|source|video|audio|iframe|object|embed|track)$/i';

/** The functions that load a picture inside a CSS value, in any letter case. */
const STYLE_LOAD = '/(?:^|[^a-z0-9_])(?:-webkit-)?(?:url|src|image|image-set|cross-fade)\\s*\\(/i';

/** A style attribute in any letter case, or a style: directive. */
const STYLE_HOLDERS = ['SvelteAttribute[key.name=/^style$/i]', 'SvelteStyleDirective'];

/**
 * The style: properties that may take a value from an expression. A property
 * joins only if its CSS grammar can't hold a picture; widening the list gets a
 * security review. Written in lower case, so any other letter case is refused.
 */
const STYLE_VALUE_PROPERTIES = [
	'left',
	'top',
	'width',
	'border-radius',
	'box-shadow',
	'font-family',
	'font-size',
	'font-weight',
	'line-height',
	'letter-spacing',
	'background-color'
];

/** The style: properties that may take a value, as an exact-name pattern. */
const STYLE_VALUE_NAMES = `/^(?:${STYLE_VALUE_PROPERTIES.join('|')})$/`;

/** The SVG attributes that take a paint, mask, filter, marker or cursor, in any letter case. */
const SVG_REFERENCE_ATTRIBUTES =
	'/^(?:mask|filter|clip-path|fill|stroke|marker|marker-start|marker-mid|marker-end|cursor)$/i';

/** A picture-loading function in an attribute's text, but a url() naming a #fragment. */
const NON_FRAGMENT_LOAD =
	'/(?:^|[^a-z0-9_])(?:-webkit-)?(?:url\\s*\\((?!\\s*[\'"]?\\s*#)|(?:src|image|image-set|cross-fade)\\s*\\()/i';

/** An attribute or its shorthand, either written out or as `{name}`. */
const ATTRIBUTE = ':matches(SvelteAttribute, SvelteShorthandAttribute)';

/**
 * A value reaching a style: a style attribute, on an element or a component,
 * holding any expression or written as `{style}`, and a style: directive
 * holding an expression or written as a shorthand, unless its property is on
 * STYLE_VALUE_PROPERTIES. A static value goes through STYLE_LOAD instead.
 */
const STYLE_VALUES = [
	{ selector: 'SvelteAttribute[key.name=/^style$/i] > SvelteMustacheTag', message: mediaMessage },
	{ selector: 'SvelteShorthandAttribute[key.name=/^style$/i]', message: mediaMessage },
	{
		selector: `SvelteStyleDirective[shorthand=true]:not([key.name.name=${STYLE_VALUE_NAMES}])`,
		message: mediaMessage
	},
	{
		selector: `SvelteStyleDirective:not([key.name.name=${STYLE_VALUE_NAMES}]) > SvelteMustacheTag`,
		message: mediaMessage
	}
];

/**
 * Attributes that load a picture from their value: the old background
 * attribute, whatever it holds; src on an input, which loads as type="image";
 * and the SVG paint, mask, filter, marker and cursor attributes holding an
 * expression, or a picture-loading function but url(#…), each on anything and
 * in any letter case.
 */
const LOADING_ATTRIBUTES = [
	{ selector: `${ATTRIBUTE}[key.name=/^background$/i]`, message: mediaMessage },
	{
		selector: `SvelteElement[name.name=/^input$/i] > SvelteStartTag > ${ATTRIBUTE}[key.name=/^src$/i]`,
		message: mediaMessage
	},
	{
		selector: `SvelteAttribute[key.name=${SVG_REFERENCE_ATTRIBUTES}] > SvelteMustacheTag`,
		message: mediaMessage
	},
	{
		selector: `SvelteShorthandAttribute[key.name=${SVG_REFERENCE_ATTRIBUTES}]`,
		message: mediaMessage
	},
	{
		selector: `SvelteAttribute[key.name=${SVG_REFERENCE_ATTRIBUTES}] > SvelteLiteral[value=${NON_FRAGMENT_LOAD}]`,
		message: mediaMessage
	}
];

/**
 * <svelte:element> naming a prefixed tag by any static text: inside an <svg>,
 * `svg:image` or `svg:script` builds the real SVG element, which the exact-name
 * bans above never see. Any string or template text in `this` holding a `:`.
 */
const PREFIXED_ELEMENT = [
	{
		selector: `SvelteElement[name.name='svelte:element'] > SvelteStartTag > SvelteAttribute[key.name='this'] > SvelteLiteral[value=/:/]`,
		message: mediaMessage
	},
	{ selector: "SvelteSpecialDirective[kind='this'] Literal[value=/:/]", message: mediaMessage },
	{
		selector: "SvelteSpecialDirective[kind='this'] TemplateElement[value.cooked=/:/]",
		message: mediaMessage
	}
];

/**
 * Media in markup: the media elements wherever they sit, as an element or a
 * component in any letter case and as <svelte:element> naming one by a static
 * string; srcset, imagesrcset and poster on anything; SVG image, use and
 * feImage with an href or any prefixed href; and a picture-loading function in
 * the text or the string literals of a style attribute or a style: directive.
 */
const MEDIA_SYNTAX = [
	{ selector: `SvelteElement[name.name=${MEDIA_TAGS}]`, message: mediaMessage },
	{
		selector: `SvelteElement[name.name='svelte:element'] > SvelteStartTag > SvelteAttribute[key.name='this'] > SvelteLiteral[value=${MEDIA_TAGS}]`,
		message: mediaMessage
	},
	{
		selector: `SvelteSpecialDirective[kind='this'] > Literal.expression[value=${MEDIA_TAGS}]`,
		message: mediaMessage
	},
	{
		selector: `SvelteSpecialDirective[kind='this'] > TemplateLiteral.expression[expressions.length=0][quasis.0.value.cooked=${MEDIA_TAGS}]`,
		message: mediaMessage
	},
	{
		selector:
			':matches(SvelteAttribute, SvelteShorthandAttribute)[key.name=/^(?:srcset|imagesrcset|poster)$/i]',
		message: mediaMessage
	},
	{
		selector:
			'SvelteElement[name.name=/^(?:image|use|feimage)$/i] > SvelteStartTag > :matches(SvelteAttribute, SvelteShorthandAttribute)[key.name=/^(?:[^:]*:)?href$/i]',
		message: mediaMessage
	},
	...STYLE_HOLDERS.flatMap((holder) => [
		{ selector: `${holder} SvelteLiteral[value=${STYLE_LOAD}]`, message: mediaMessage },
		{ selector: `${holder} Literal[value=${STYLE_LOAD}]`, message: mediaMessage },
		{ selector: `${holder} TemplateElement[value.raw=${STYLE_LOAD}]`, message: mediaMessage }
	]),
	...STYLE_VALUES,
	...LOADING_ATTRIBUTES,
	...PREFIXED_ELEMENT
];

/** A member named `name`, by dot, by a string key or by a template with no expressions. */
const memberNamed = (/** @type {string} */ name) => [
	`MemberExpression[computed=false][property.name='${name}']`,
	`MemberExpression[computed=true][property.value='${name}']`,
	`MemberExpression[computed=true][property.expressions.length=0][property.quasis.0.value.cooked='${name}']`,
	`ObjectPattern > Property[key.name='${name}']`,
	`ObjectPattern > Property[key.value='${name}']`
];

/**
 * An element's inline style reached from code: the `style` and
 * `attributeStyleMap` members, read or written, destructured too (a property
 * assignment, setProperty, cssText and Object.assign all go through them), and
 * setAttribute, setAttributeNS, createAttribute and createAttributeNS naming
 * style or background by a static string in any letter case.
 */
const INLINE_STYLE_SYNTAX = [
	...['style', 'attributeStyleMap'].flatMap(memberNamed).map((selector) => ({
		selector,
		message: inlineStyleMessage
	})),
	...namedByLiteral(
		['setAttribute', 'createAttribute'],
		0,
		qualified('style|background'),
		inlineStyleMessage
	),
	...namedByLiteral(
		['setAttributeNS', 'createAttributeNS'],
		1,
		qualified('style|background'),
		inlineStyleMessage
	)
];

/** ESLint's restricted-name rules, under names of their own. */
const loadPlugin = {
	meta: { name: 'ui-load' },
	rules: {
		globals: builtinRule('no-restricted-globals'),
		properties: builtinRule('no-restricted-properties'),
		syntax: builtinRule('no-restricted-syntax'),
		markup: builtinRule('no-restricted-syntax')
	}
};

/**
 * What each source set refuses here: the components and the seam take the
 * timers, both load starters, both element calls and any reach into an inline
 * style, the motion adapter Image, Audio and every element but a span, and
 * every other set Image, Audio and both element calls.
 */
export const loadRules = {
	components: {
		globals: [...TIMER_GLOBALS, ...STARTER_GLOBALS],
		properties: [...TIMER_PROPERTIES, ...ELEMENT_PROPERTIES],
		syntax: INLINE_STYLE_SYNTAX
	},
	environment: {
		globals: [...TIMER_GLOBALS, ...STARTER_GLOBALS],
		properties: [...TIMER_PROPERTIES, ...ELEMENT_PROPERTIES],
		syntax: INLINE_STYLE_SYNTAX
	},
	motion: { globals: STARTER_GLOBALS, properties: [], syntax: MOTION_ELEMENT_SYNTAX },
	lazy: { globals: STARTER_GLOBALS, properties: ELEMENT_PROPERTIES, syntax: [] },
	routes: { globals: STARTER_GLOBALS, properties: ELEMENT_PROPERTIES, syntax: [] },
	gallery: { globals: STARTER_GLOBALS, properties: ELEMENT_PROPERTIES, syntax: [] },
	storage: { globals: STARTER_GLOBALS, properties: ELEMENT_PROPERTIES, syntax: [] },
	stories: { globals: STARTER_GLOBALS, properties: ELEMENT_PROPERTIES, syntax: [] },
	storybookBrowser: { globals: STARTER_GLOBALS, properties: ELEMENT_PROPERTIES, syntax: [] }
};

/** Every .svelte file under src/, the checks' harnesses included. */
export const MEDIA_FILES = ['src/**/*.svelte'];

/**
 * The load bans' block for one source set.
 *
 * @param {keyof typeof loadRules} name
 * @returns {import('eslint').Linter.Config}
 */
function loadBlock(name) {
	const { files, ignores } = fileSets[name];
	const { globals, properties, syntax } = loadRules[name];
	/** @type {import('eslint').Linter.RulesRecord} */
	const rules = { 'ui-load/globals': ['error', { globals, checkGlobalObject: true }] };
	if (properties.length > 0) rules['ui-load/properties'] = ['error', ...properties];
	if (syntax.length > 0) rules['ui-load/syntax'] = ['error', ...syntax];
	return { name: `ui-load/${name}`, files, ignores, rules };
}

/** @type {import('eslint').Linter.Config[]} */
const LOAD_BLOCKS = [
	{ name: 'ui-load/plugin', plugins: { 'ui-load': loadPlugin } },
	...SOURCE_SETS.map(loadBlock),
	{
		name: 'ui-load/media',
		files: MEDIA_FILES,
		rules: { 'ui-load/markup': ['error', ...MEDIA_SYNTAX] }
	}
];

export default defineConfig(
	includeIgnoreFile(gitignorePath),
	{
		name: 'ui/plugin',
		plugins: { ui: uiPlugin }
	},
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
		name: 'ui/storybook-node-config',
		// Storybook's main config runs in Node, and the story tests' setup only
		// hands the preview to Vitest: both take the package-root config files' rules.
		files: STORYBOOK_NODE,
		languageOptions: { globals: globals.node }
	},
	{
		name: 'ui/storybook-unused-vars',
		// The TypeScript-aware twin of no-unused-vars, which reads parameter names
		// in a function type as unused variables.
		files: ['.storybook/*.ts'],
		rules: { 'no-unused-vars': 'off', '@typescript-eslint/no-unused-vars': 'error' }
	},
	{
		name: 'ui/story-template-args',
		// A story's template snippet is handed (args, context), and the rule reads every
		// snippet parameter as checked, a leading one included. A story that reads only
		// the context names the first one exactly `_args`; nothing else is let through.
		files: ['src/stories/**/*.stories.svelte'],
		rules: { '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_args$' }] }
	},
	{
		name: 'ui/storybook-browser-config',
		// The preview and the manager run in the browser, under the host's bans below.
		files: STORYBOOK_BROWSER,
		languageOptions: { globals: globals.browser }
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
		name: 'ui/button-link',
		// Button's href is a LinkHref, checked again when Button renders; Kit's
		// resolve() belongs to the app that builds the link, not to the library.
		files: ['src/lib/controls/Button.svelte'],
		rules: { 'svelte/no-navigation-without-resolve': 'off' }
	},
	{
		name: 'ui/spreads',
		// Every .svelte file in every set, the host's and the checks' included.
		files: ['**/*.svelte'],
		rules: { 'ui/spreads': 'error' }
	},
	{
		name: 'ui/no-at-html',
		// Every .svelte file in every set, the host's and the checks' included.
		files: ['**/*.svelte'],
		rules: { 'svelte/no-at-html-tags': 'error' }
	},
	...SOURCE_SETS.flatMap(boundaryBlocks),
	...LOAD_BLOCKS
);
