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

/** The one place motion libraries may be imported. */
const MOTION = ['src/lib/motion/**'];

/** The one place heavy libraries may be loaded, and only by dynamic import. */
const LAZY = ['src/lib/lazy/**'];

/** Each set's `files` and `ignores`, by name. */
export const fileSets = {
	checks: { files: CHECKS, ignores: [] },
	routes: { files: ROUTES, ignores: CHECKS },
	gallery: { files: GALLERY, ignores: [...CHECKS, ...STORAGE] },
	storage: { files: STORAGE, ignores: CHECKS },
	stories: { files: STORIES, ignores: CHECKS },
	motion: { files: MOTION, ignores: CHECKS },
	lazy: { files: LAZY, ignores: CHECKS },
	components: {
		files: CODE,
		ignores: [...CHECKS, ...ROUTES, ...GALLERY, ...STORAGE, ...STORIES, ...MOTION, ...LAZY]
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
	'motion',
	'lazy',
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
 * no set names (outside src/, or not a code file).
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
// components and their two carve-outs), the gallery host, and the checks. A
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
	routes: 'host',
	gallery: 'host',
	storage: 'host',
	stories: 'host',
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

/** What the components' set and its two carve-outs refuse as members, element.animate() aside. */
const COMPONENT_PROPERTIES = [
	...NETWORK_PROPERTIES,
	...COMPONENT_STORAGE.properties,
	...HTML_PROPERTIES
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

/** What every source set refuses as an import, before its own additions. */
const COMMON_IMPORTS = [
	ROOT_IMPORT,
	ENV_IMPORT,
	LIB_IMPORT,
	...SERVER_IMPORTS,
	...VITE_CODE_LOADING_IMPORTS,
	...RAW_HTML_IMPORTS
];

const COMPONENT_IMPORTS = [
	...COMMON_IMPORTS,
	DEEP_SELF_IMPORT,
	ANY_APP_IMPORT,
	GALLERY_IMPORT,
	...ADAPTER_MOTION_IMPORTS,
	...HEAVY_IMPORTS
];
const MOTION_IMPORTS = [
	...COMMON_IMPORTS,
	DEEP_SELF_IMPORT,
	ANY_APP_IMPORT,
	GALLERY_IMPORT,
	...HEAVY_IMPORTS
];
const LAZY_IMPORTS = COMPONENT_IMPORTS;
const HOST_IMPORTS = [
	...COMMON_IMPORTS,
	...HOST_DEEP_SELF_IMPORTS,
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
 * not markup, and a spread can't be read here.
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
	...COMPONENT_STORAGE.syntax
];
const LAZY_SYNTAX = [
	...COMMON_SYNTAX,
	...ANY_GLOB,
	DYNAMIC_IMPORT_EXCEPT_HEAVY,
	...COMPONENT_STORAGE.syntax
];
const ROUTES_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT, ...HOST_STORAGE.syntax];
const GALLERY_SYNTAX = [
	...COMMON_SYNTAX,
	...GLOB_EXCEPT_PAGE_METAS,
	ANY_DYNAMIC_IMPORT,
	...HOST_STORAGE.syntax
];
const STORE_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT, ...STORE_STORAGE.syntax];
const STORIES_SYNTAX = [...COMMON_SYNTAX, ...ANY_GLOB, ANY_DYNAMIC_IMPORT, ...HOST_STORAGE.syntax];

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
				check(node, fieldOf(fieldOf(node, 'argument'), 'literal'));
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

/** The package's own rules, as an inline plugin. */
const uiPlugin = {
	meta: { name: 'ui' },
	rules: {
		'import-tiers': importTiersRule,
		'no-remote-string': noRemoteStringRule,
		'no-script-url': noScriptUrlRule
	}
};

/**
 * Each source set's tier and its options for the four restricted rules. A
 * later flat-config block replaces an earlier one's options for the same rule,
 * so each set gets one block per rule holding every entry it needs.
 */
export const boundaryRules = {
	components: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: [...COMPONENT_PROPERTIES, ANIMATE_PROPERTY],
		imports: COMPONENT_IMPORTS,
		syntax: COMPONENT_SYNTAX
	},
	motion: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: COMPONENT_PROPERTIES,
		imports: MOTION_IMPORTS,
		syntax: COMPONENT_SYNTAX
	},
	lazy: {
		tier: /** @type {const} */ ('shipped'),
		globals: COMPONENT_GLOBALS,
		properties: [...COMPONENT_PROPERTIES, ANIMATE_PROPERTY],
		imports: LAZY_IMPORTS,
		syntax: LAZY_SYNTAX
	},
	routes: {
		tier: /** @type {const} */ ('host'),
		globals: HOST_GLOBALS,
		properties: [...HOST_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: ROUTES_SYNTAX
	},
	gallery: {
		tier: /** @type {const} */ ('host'),
		globals: HOST_GLOBALS,
		properties: [...HOST_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: GALLERY_SYNTAX
	},
	storage: {
		tier: /** @type {const} */ ('host'),
		globals: STORE_GLOBALS,
		properties: [...STORE_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: STORE_SYNTAX
	},
	stories: {
		tier: /** @type {const} */ ('host'),
		globals: HOST_GLOBALS,
		properties: [...HOST_PROPERTIES, ANIMATE_PROPERTY],
		imports: HOST_IMPORTS,
		syntax: STORIES_SYNTAX
	}
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
				'ui/no-script-url': 'error'
			}
		}
	];
}

/** Every set but the checks: the components, their two carve-outs, the host and its stores. */
/** @type {readonly (keyof typeof boundaryRules)[]} */
const SOURCE_SETS = ['components', 'motion', 'lazy', 'routes', 'gallery', 'storage', 'stories'];

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
		// Storybook's main config runs in Node, like the package-root config files.
		files: ['.storybook/main.ts'],
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
		name: 'ui/storybook-browser-config',
		// The manager's, the preview's and the story tests' setup run in the browser.
		files: ['.storybook/manager.ts', '.storybook/preview.ts', '.storybook/vitest.setup.ts'],
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
		name: 'ui/no-at-html',
		// Every .svelte file in every set, the host's and the checks' included.
		files: ['**/*.svelte'],
		rules: { 'svelte/no-at-html-tags': 'error' }
	},
	...SOURCE_SETS.flatMap(boundaryBlocks)
);
