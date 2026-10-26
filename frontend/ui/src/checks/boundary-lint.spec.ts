import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	boundaryRules,
	fileSetOf,
	inlineHandlerMessage,
	scriptMarkupMessage
} from '../../eslint.config.js';
import { isRemote } from './scan/network';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** Planted files: virtual paths, one per file set, that never exist on disk. */
const planted = {
	component: 'src/lib/x.ts',
	componentSvelte: 'src/lib/X.svelte',
	stray: 'src/elsewhere.ts',
	motion: 'src/lib/motion/x.ts',
	motionSvelte: 'src/lib/motion/X.svelte',
	lazy: 'src/lib/lazy/x.ts',
	lazySvelte: 'src/lib/lazy/X.svelte',
	gallery: 'src/gallery/x.ts',
	gallerySvelte: 'src/gallery/X.svelte',
	routes: 'src/routes/x.ts',
	routesSvelte: 'src/routes/X.svelte',
	check: 'src/checks/planted.ts',
	spec: 'src/lib/planted.spec.ts'
} as const;

const plantedPaths = Object.values(planted);

/** Real files a case lints planted text at; the file on disk is never read or touched. */
const real = {
	publicEntry: 'src/lib/index.ts',
	layoutLoad: 'src/routes/+layout.ts'
} as const;

/**
 * The real config, unchanged but for one parser option: the project service
 * refuses a file that isn't on disk, so the planted paths parse against the
 * package's tsconfig as a default project instead, all of them at once.
 */
const plantedOverride: Linter.Config = {
	languageOptions: {
		parserOptions: {
			projectService: {
				allowDefaultProject: plantedPaths,
				defaultProject: 'tsconfig.json',
				// The parser caps the default project at 8 files unless told otherwise.
				maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: plantedPaths.length
			}
		}
	}
};

const plantedLinter = new ESLint({ cwd: uiRoot, overrideConfig: plantedOverride });

/** The real config, as the CLI and editors load it. */
const realLinter = new ESLint({ cwd: uiRoot });

/** Every rule that guards the boundary; an allowed case must fire none of them. */
const boundaryRuleIds: ReadonlySet<string> = new Set([
	'no-restricted-globals',
	'no-restricted-properties',
	'no-restricted-imports',
	'no-restricted-syntax',
	'no-eval',
	'no-new-func',
	'@typescript-eslint/no-implied-eval',
	'@typescript-eslint/consistent-type-assertions',
	'svelte/no-at-html-tags',
	'ui/import-tiers',
	'ui/no-remote-string',
	'ui/no-script-url'
]);

async function lintPlanted(filePath: string, code: string): Promise<readonly Linter.LintMessage[]> {
	const [result] = await plantedLinter.lintText(code, { filePath });
	if (result === undefined) throw new Error(`no lint result for ${filePath}`);
	const fatal = result.messages.filter((message) => message.fatal === true);
	expect(fatal).toEqual([]);
	return result.messages;
}

async function expectRefused(filePath: string, code: string, ruleId: string): Promise<void> {
	const messages = await lintPlanted(filePath, code);
	expect(messages.map((message) => message.ruleId)).toContain(ruleId);
}

/** Refused by the syntax entry that carries `syntaxMessage`, not by any other. */
async function expectSyntaxMessage(
	filePath: string,
	code: string,
	syntaxMessage: string
): Promise<void> {
	const messages = await lintPlanted(filePath, code);
	const matching = messages.filter(
		(message) => message.ruleId === 'no-restricted-syntax' && message.message === syntaxMessage
	);
	expect(matching).not.toEqual([]);
}

/** Refused by the markup ban itself, not by any other syntax entry. */
async function expectScriptMarkupRefused(filePath: string, code: string): Promise<void> {
	await expectSyntaxMessage(filePath, code, scriptMarkupMessage);
}

/** Refused by the inline-handler ban itself, not by any other syntax entry. */
async function expectInlineHandlerRefused(filePath: string, code: string): Promise<void> {
	await expectSyntaxMessage(filePath, code, inlineHandlerMessage);
}

async function expectAllowed(filePath: string, code: string): Promise<void> {
	const messages = await lintPlanted(filePath, code);
	const boundary = messages.filter(
		(message) => message.ruleId !== null && boundaryRuleIds.has(message.ruleId)
	);
	expect(boundary).toEqual([]);
}

type Case = readonly [label: string, code: string, ruleId: string];

type Allowed = readonly [label: string, code: string];

const GLOBALS = 'no-restricted-globals';
const PROPERTIES = 'no-restricted-properties';
const IMPORTS = 'no-restricted-imports';
const SYNTAX = 'no-restricted-syntax';
const TIERS = 'ui/import-tiers';
const REMOTE = 'ui/no-remote-string';
const SCRIPT_URL = 'ui/no-script-url';

type PathCase = readonly [label: string, filePath: string, code: string];

/** A module that exports one string, as source text. */
function stringModule(text: string): string {
	return `export const r = ${JSON.stringify(text)};`;
}

/** Strings a browser reads as remote, and as local, when it meets them as a URL. */
const remoteSamples: readonly string[] = [
	'https://x/a.png',
	'http://x/a.png',
	'HTTPS://x',
	'HtTp://x',
	' https://x',
	'\t\n https://x',
	'\u0001https://x',
	'ht\ttps://x',
	'h\nttp://x',
	'https:x',
	'http:',
	'//x/a.png',
	'\\\\x/a',
	'/\\x',
	'\\/x'
];

const localSamples: readonly string[] = [
	'/a.png',
	'./a.png',
	'../a.png',
	'data:image/png;base64,AAAA',
	'a.png',
	'x https://y',
	'httpx://x',
	'ftp://x',
	'mailto:a@b',
	'/ /x',
	''
];

const componentRefusals: readonly Case[] = [
	// Network and code loading.
	['bare fetch', "export const r = fetch('/x');", GLOBALS],
	['window.fetch', "export const r = window.fetch('/x');", GLOBALS],
	['globalThis.fetch', "export const r = globalThis.fetch('/x');", GLOBALS],
	['self.fetch', "export const r = self.fetch('/x');", GLOBALS],
	['XMLHttpRequest', 'export const r = new XMLHttpRequest();', GLOBALS],
	['WebSocket', "export const r = new WebSocket('wss://x');", GLOBALS],
	['EventSource', "export const r = new EventSource('/x');", GLOBALS],
	['navigator.sendBeacon', "export const r = navigator.sendBeacon('/x');", PROPERTIES],
	['Worker', "export const r = new Worker('/w.js');", GLOBALS],
	['SharedWorker', "export const r = new SharedWorker('/w.js');", GLOBALS],
	['navigator.serviceWorker', 'export const r = navigator.serviceWorker;', PROPERTIES],
	['eval', "export const r: unknown = eval('1');", 'no-eval'],
	['new Function', "export const r = new Function('return 1');", 'no-new-func'],
	// Storage.
	['localStorage', 'export const r = localStorage;', GLOBALS],
	['window.localStorage', 'export const r = window.localStorage;', GLOBALS],
	['sessionStorage', 'export const r = sessionStorage;', GLOBALS],
	['globalThis.sessionStorage', 'export const r = globalThis.sessionStorage;', GLOBALS],
	['indexedDB', 'export const r = indexedDB;', GLOBALS],
	['self.indexedDB', 'export const r = self.indexedDB;', GLOBALS],
	['caches', 'export const r = caches;', GLOBALS],
	['cookieStore', 'export const r: unknown = cookieStore;', GLOBALS],
	['document.cookie', 'export const r = document.cookie;', PROPERTIES],
	// The server half's environment.
	['process.env', 'export const r = process.env.X;', GLOBALS],
	['process.env, by property', 'export const r = process.env.X;', PROPERTIES],
	['globalThis.process', 'export const r = globalThis.process;', GLOBALS],
	['import.meta.env', 'export const r: unknown = import.meta.env;', SYNTAX],
	["import.meta['env']", "export const r: unknown = import.meta['env'];", SYNTAX],
	[
		"import.meta.glob('/src/lib/server/**')",
		"export const r = import.meta.glob('/src/lib/server/**');",
		SYNTAX
	],
	['a relative import.meta.glob', "export const r = import.meta.glob('./*.ts');", SYNTAX],
	["import '/src/x'", "import '/src/x';", IMPORTS],
	["import('/src/x')", "export const r = import('/src/x');", SYNTAX],
	["export * from '/src/x'", "export * from '/src/x';", IMPORTS],
	// Imports.
	['$app/navigation', "import { goto } from '$app/navigation';\nexport { goto };", IMPORTS],
	['$app/paths', "import { resolve } from '$app/paths';\nexport { resolve };", IMPORTS],
	['$env/dynamic/public', "import { env } from '$env/dynamic/public';\nexport { env };", IMPORTS],
	// Never '$lib/x' from src/lib/x.ts: a file re-exporting itself hangs the type-aware rules.
	['$lib/y', "import { x } from '$lib/y';\nexport { x };", IMPORTS],
	[
		'$gallery/registry',
		"import { registry } from '$gallery/registry';\nexport { registry };",
		IMPORTS
	],
	['effect', "import { Effect } from 'effect';\nexport { Effect };", IMPORTS],
	['effect/Schema', "import 'effect/Schema';", IMPORTS],
	['@effect/platform', "import '@effect/platform';", IMPORTS],
	["export * from 'effect'", "export * from 'effect';", IMPORTS],
	['@bufbuild/protobuf', "import '@bufbuild/protobuf';", IMPORTS],
	['the generated contract', "import './server/api/generated/zurfur_pb';", IMPORTS],
	['web/…', "import 'web/src/lib/x';", IMPORTS],
	['a path through node_modules', "import '../../node_modules/web/src/lib/server/x';", IMPORTS],
	['a path into web', "import '../../../web/src/lib/server/api/generated/x';", IMPORTS],
	['a deep import of the package', "import '@zurfur/ui/src/lib/x';", IMPORTS],
	// Code Vite turns into a Worker or a WebAssembly loader.
	['?worker', "import W from './w.ts?worker';\nexport { W };", IMPORTS],
	['?sharedworker', "import W from './w.ts?sharedworker';\nexport { W };", IMPORTS],
	['?worker&inline', "import W from './w.ts?worker&inline';\nexport { W };", IMPORTS],
	['?worker&url', "import W from './w.ts?worker&url';\nexport { W };", IMPORTS],
	['?url&worker', "import W from './w.ts?url&worker';\nexport { W };", IMPORTS],
	['?sharedworker&inline', "import W from './w.ts?sharedworker&inline';\nexport { W };", IMPORTS],
	['.wasm?init', "import init from './a.wasm?init';\nexport { init };", IMPORTS],
	// Motion outside the adapter.
	['gsap', "import { gsap } from 'gsap';\nexport { gsap };", IMPORTS],
	["export { gsap } from 'gsap'", "export { gsap } from 'gsap';", IMPORTS],
	['three', "import * as three from 'three';\nexport { three };", IMPORTS],
	['lottie-web', "import lottie from 'lottie-web';\nexport { lottie };", IMPORTS],
	['svelte/transition', "import { fade } from 'svelte/transition';\nexport { fade };", IMPORTS],
	['svelte/animate', "import { flip } from 'svelte/animate';\nexport { flip };", IMPORTS],
	['svelte/motion', "import { Tween } from 'svelte/motion';\nexport { Tween };", IMPORTS],
	['svelte/easing', "import { cubicOut } from 'svelte/easing';\nexport { cubicOut };", IMPORTS],
	["import('three')", "export const r = import('three');", SYNTAX],
	['a computed import(name)', "const name = 'three';\nexport const r = import(name);", SYNTAX],
	['el.animate()', 'export const r = document.body.animate([], 100);', PROPERTIES],
	// The rulebook.
	['throw', "export function f(): never {\n\tthrow new Error('x');\n}", SYNTAX],
	['null', 'export const r = null;', SYNTAX],
	['a null type', 'export type R = string | null;', SYNTAX],
	['as', "export const r = 'x' as string;", '@typescript-eslint/consistent-type-assertions']
];

describe('the boundary lint over planted text', { timeout: 120_000 }, () => {
	it('plants only at paths that are not on disk', () => {
		const onDisk = plantedPaths.filter((relative) => existsSync(`${uiRoot}${relative}`));
		expect(onDisk).toEqual([]);
	});

	it('plants over real paths only where the file is on disk', () => {
		const missing = Object.values(real).filter((relative) => !existsSync(`${uiRoot}${relative}`));
		expect(missing).toEqual([]);
	});

	describe('the components (src/lib)', () => {
		it.each(componentRefusals)('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.component, code, ruleId);
		});

		it.each([
			['a peer dependency', "import { untrack } from 'svelte';\nexport { untrack };"],
			['a relative import', "import { y } from './y';\nexport { y };"],
			['as const', "export const r = ['a'] as const;"],
			['?raw', "import text from './a.txt?raw';\nexport { text };"],
			['?url', "import href from './a.png?url';\nexport { href };"]
		])('allows %s', async (_label, code) => {
			await expectAllowed(planted.component, code);
		});
	});

	describe('a svelte component', () => {
		it.each([
			[
				'fetch in a script',
				'<script lang="ts">\n\tconst r = fetch(\'/x\');\n</script>\n\n{r}\n',
				GLOBALS
			],
			[
				'gsap in a script',
				'<script lang="ts">\n\timport { gsap } from \'gsap\';\n</script>\n\n{gsap.version}\n',
				IMPORTS
			],
			[
				'{@html}',
				'<script lang="ts">\n\tconst x = \'<b>x</b>\';\n</script>\n\n{@html x}\n',
				'svelte/no-at-html-tags'
			]
		])('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.componentSvelte, code, ruleId);
		});
	});

	describe('a stray file outside src/lib', () => {
		it.each([
			['fetch', "export const r = fetch('/x');", GLOBALS],
			['localStorage', 'export const r = localStorage;', GLOBALS],
			['gsap', "import 'gsap';", IMPORTS],
			['throw', "export function f(): never {\n\tthrow new Error('x');\n}", SYNTAX]
		])('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.stray, code, ruleId);
		});
	});

	describe('no inline comment switches a rule off', () => {
		it.each([
			[
				'eslint-disable-next-line',
				"// eslint-disable-next-line no-restricted-globals\nexport const r = fetch('/x');"
			],
			['eslint-disable', "/* eslint-disable */\nexport const r = fetch('/x');"],
			[
				'a rule set to off',
				'/* eslint no-restricted-globals: "off" */\nexport const r = fetch(\'/x\');'
			]
		])('still refuses fetch under %s', async (_label, code) => {
			const messages = await lintPlanted(planted.component, code);
			expect(messages.map((message) => message.ruleId)).toContain(GLOBALS);
			const ignoredComment = messages.filter(
				(message) => message.ruleId === null && message.message.includes('noInlineConfig')
			);
			expect(ignoredComment.map((message) => message.severity)).toEqual([1]);
		});
	});

	describe('the motion adapter (src/lib/motion)', () => {
		it.each([
			['gsap', "import { gsap } from 'gsap';\nexport { gsap };"],
			["export { gsap } from 'gsap'", "export { gsap } from 'gsap';"],
			['svelte/transition', "import { fade } from 'svelte/transition';\nexport { fade };"],
			['svelte/animate', "import { flip } from 'svelte/animate';\nexport { flip };"],
			['svelte/motion', "import { Tween } from 'svelte/motion';\nexport { Tween };"],
			['svelte/easing', "import { cubicOut } from 'svelte/easing';\nexport { cubicOut };"],
			['el.animate()', 'export const r = document.body.animate([], 100);']
		])('allows %s', async (_label, code) => {
			await expectAllowed(planted.motion, code);
		});

		it.each([
			['three', "import * as three from 'three';\nexport { three };", IMPORTS],
			['lottie-web', "import lottie from 'lottie-web';\nexport { lottie };", IMPORTS],
			["import('three')", "export const r = import('three');", SYNTAX],
			['fetch', "export const r = fetch('/x');", GLOBALS],
			['localStorage', 'export const r = localStorage;', GLOBALS],
			['$lib/y', "import '$lib/y';", IMPORTS],
			['throw', "export function f(): never {\n\tthrow new Error('x');\n}", SYNTAX]
		])('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.motion, code, ruleId);
		});
	});

	describe('the lazy loader (src/lib/lazy)', () => {
		it.each([
			["import('three')", "export const r = import('three');"],
			["import('lottie-web')", "export const r = import('lottie-web');"]
		])('allows %s', async (_label, code) => {
			await expectAllowed(planted.lazy, code);
		});

		it.each([
			['a static three', "import * as three from 'three';\nexport { three };", IMPORTS],
			['a static lottie-web', "import 'lottie-web';", IMPORTS],
			['gsap', "import { gsap } from 'gsap';\nexport { gsap };", IMPORTS],
			["import('gsap')", "export const r = import('gsap');", SYNTAX],
			["import('/src/x')", "export const r = import('/src/x');", SYNTAX],
			['a computed import(name)', "const name = 'three';\nexport const r = import(name);", SYNTAX],
			['fetch', "export const r = fetch('/x');", GLOBALS],
			['el.animate()', 'export const r = document.body.animate([], 100);', PROPERTIES],
			['null', 'export const r = null;', SYNTAX]
		])('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.lazy, code, ruleId);
		});
	});

	const hostAllowed: readonly Allowed[] = [
		['localStorage', "export const r = localStorage.getItem('x');"],
		['$app/paths', "import { resolve } from '$app/paths';\nexport { resolve };"],
		['@zurfur/ui', "import * as ui from '@zurfur/ui';\nexport { ui };"],
		['$gallery/…', "import { groupNames } from '$gallery/groups';\nexport { groupNames };"],
		['?raw', "import text from './a.txt?raw';\nexport { text };"]
	];

	const hostRefusals: readonly Case[] = [
		['fetch', "export const r = fetch('/x');", GLOBALS],
		['new WebSocket', "export const r = new WebSocket('wss://x');", GLOBALS],
		['sessionStorage', 'export const r = sessionStorage;', GLOBALS],
		['document.cookie', 'export const r = document.cookie;', PROPERTIES],
		['process.env', 'export const r = process.env.X;', GLOBALS],
		['import.meta.env', 'export const r: unknown = import.meta.env;', SYNTAX],
		['$lib/y', "import { x } from '$lib/y';\nexport { x };", IMPORTS],
		['?worker', "import W from './w.ts?worker';\nexport { W };", IMPORTS],
		['?sharedworker', "import W from './w.ts?sharedworker';\nexport { W };", IMPORTS],
		['?worker&inline', "import W from './w.ts?worker&inline';\nexport { W };", IMPORTS],
		['.wasm?init', "import init from './a.wasm?init';\nexport { init };", IMPORTS],
		['a deep import of the package', "import '@zurfur/ui/src/lib/x';", IMPORTS],
		[
			'$env/static/public',
			"import { PUBLIC_X } from '$env/static/public';\nexport { PUBLIC_X };",
			IMPORTS
		],
		['$app/navigation', "import { goto } from '$app/navigation';\nexport { goto };", IMPORTS],
		['$app/state', "import { page } from '$app/state';\nexport { page };", IMPORTS],
		['$app/paths/extra', "import '$app/paths/extra';", IMPORTS],
		['gsap', "import { gsap } from 'gsap';\nexport { gsap };", IMPORTS],
		['three', "import 'three';", IMPORTS],
		['svelte/transition', "import { fade } from 'svelte/transition';\nexport { fade };", IMPORTS],
		["import '/src/x'", "import '/src/x';", IMPORTS],
		["import('/src/x')", "export const r = import('/src/x');", SYNTAX],
		['el.animate()', 'export const r = document.body.animate([], 100);', PROPERTIES],
		['throw', "export function f(): never {\n\tthrow new Error('x');\n}", SYNTAX]
	];

	const galleryAllowed: readonly Allowed[] = [
		...hostAllowed,
		[
			"import.meta.glob('/src/routes/**/meta.ts')",
			"export const r = import.meta.glob('/src/routes/**/meta.ts', { eager: true });"
		]
	];

	const galleryRefusals: readonly Case[] = [
		...hostRefusals,
		[
			'a glob outside /src/routes/',
			"export const r = import.meta.glob('/src/lib/server/**');",
			SYNTAX
		],
		[
			'another glob in /src/routes/',
			"export const r = import.meta.glob('/src/routes/**/*.ts');",
			SYNTAX
		],
		[
			'the glob pattern as a template literal',
			'export const r = import.meta.glob(`/src/routes/**/meta.ts`);',
			SYNTAX
		],
		['the glob taken apart', 'const glob = import.meta.glob;\nexport const r = glob;', SYNTAX],
		["import.meta['glob']", "export const r: unknown = import.meta['glob'];", SYNTAX]
	];

	const routesRefusals: readonly Case[] = [
		...hostRefusals,
		[
			"import.meta.glob('/src/routes/**/meta.ts')",
			"export const r = import.meta.glob('/src/routes/**/meta.ts', { eager: true });",
			SYNTAX
		]
	];

	describe("the gallery's frame (src/gallery)", () => {
		it.each(galleryAllowed)('allows %s', async (_label, code) => {
			await expectAllowed(planted.gallery, code);
		});

		it.each(galleryRefusals)('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.gallery, code, ruleId);
		});
	});

	describe("the gallery's pages (src/routes)", () => {
		it.each(hostAllowed)('allows %s', async (_label, code) => {
			await expectAllowed(planted.routes, code);
		});

		it.each(routesRefusals)('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.routes, code, ruleId);
		});
	});

	describe('a relative import never reaches a looser file set', () => {
		const svelteReexport =
			'<script lang="ts" module>\n\texport { styleFindings } from \'../checks/scan/style\';\n</script>\n';
		const refused: readonly PathCase[] = [
			[
				"the public entry's export * of a check",
				real.publicEntry,
				"export * from '../checks/scan/style';"
			],
			[
				"the public entry's named re-export of a check",
				real.publicEntry,
				"export { styleFindings } from '../checks/scan/style';"
			],
			['a component importing a page', planted.component, "import '../routes/+layout';"],
			['a component importing the frame', planted.component, "import '../gallery/registry';"],
			['a component importing a spec', planted.component, "export { y } from './x.spec';"],
			[
				'a type-only import of a check',
				planted.component,
				"import type { T } from '../checks/x';\nexport type { T };"
			],
			['a dynamic import of a page', planted.component, "export const r = import('../routes/x');"],
			[
				'a type query of a check',
				planted.component,
				"export type T = typeof import('../checks/x');"
			],
			['a check by folder', planted.component, "import '../checks';"],
			[
				'a check with a query',
				planted.component,
				"import text from '../checks/x.ts?raw';\nexport { text };"
			],
			['a spec named with .js', planted.component, "import './x.spec.js';"],
			['a file outside src', planted.component, "import '../../vite.config';"],
			['the motion adapter importing a page', planted.motion, "import '../../routes/x';"],
			['a stray file importing a page', planted.stray, "import './routes/x';"],
			['the frame importing a check', planted.gallery, "import '../checks/x';"],
			['a page importing a spec', planted.routes, "import '../gallery/x.spec';"],
			['a page importing outside src', planted.routes, "import '../../package.json';"],
			["a component's script re-exporting a check", planted.componentSvelte, svelteReexport],
			['a re-export of an unread extension', planted.component, "export * from './evil.es';"],
			['an upper-case code extension', planted.component, "import './evil.JS';"],
			['an asset with no query', planted.component, "import './a.svg';"],
			['a dotted TypeScript name without its extension', planted.component, "import './a.b';"],
			['the frame importing an unread extension', planted.gallery, "import './x.es';"]
		];

		it.each(refused)('refuses %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, TIERS);
		});

		// Each allowed import names a file no case plants: two planted files
		// re-exporting each other hang the type-aware rules.
		const allowed: readonly PathCase[] = [
			[
				'a component importing the motion adapter',
				planted.component,
				"import { y } from './motion/y';\nexport { y };"
			],
			[
				'a component importing the lazy loader',
				planted.component,
				"import { y } from './lazy/y';\nexport { y };"
			],
			[
				'the motion adapter importing a component',
				planted.motion,
				"import { y } from '../y';\nexport { y };"
			],
			[
				'a page importing a component',
				planted.routes,
				"import { y } from '../lib/y';\nexport { y };"
			],
			[
				'a page importing the frame',
				planted.routes,
				"import { y } from '../gallery/y';\nexport { y };"
			],
			[
				'the frame importing a page',
				planted.gallery,
				"import { y } from '../routes/y';\nexport { y };"
			],
			[
				'a check importing a component',
				planted.check,
				"import { y } from '../lib/y';\nexport { y };"
			],
			[
				'a spec importing its sibling',
				planted.spec,
				"import { y } from './planted';\nexport { y };"
			],
			[
				'a component importing a raw text file',
				planted.component,
				"import t from './a.txt?raw';\nexport { t };"
			],
			[
				'a component importing an asset URL',
				planted.component,
				"import u from './a.svg?url';\nexport { u };"
			],
			['a component importing a stylesheet', planted.component, "import './styles/tokens.css';"],
			[
				'a component importing an extensionless file',
				planted.component,
				"import x from './y';\nexport { x };"
			]
		];

		it.each(allowed)('allows %s', async (_label, filePath, code) => {
			await expectAllowed(filePath, code);
		});
	});

	describe('no source file names fetch', () => {
		const loadWithFetch =
			"export const load = async ({ fetch }: { readonly fetch: typeof globalThis.fetch }) => {\n\tawait fetch('/x');\n};";
		const refused: readonly PathCase[] = [
			[
				"Mongoose's load({ fetch }) in the layout",
				real.layoutLoad,
				"import type { LayoutLoad } from './$types';\nexport const load: LayoutLoad = async ({ fetch }) => {\n\tawait fetch('https://x.invalid/');\n};"
			],
			['load({ fetch }) in a page', planted.routes, loadWithFetch],
			['load({ fetch }) in the frame', planted.gallery, loadWithFetch],
			['load({ fetch }) in a component', planted.component, loadWithFetch],
			[
				'a fetch prop',
				planted.componentSvelte,
				'<script lang="ts">\n\tinterface Props {\n\t\treadonly fetch: () => void;\n\t}\n\tlet { fetch }: Props = $props();\n</script>\n\n<button onclick={fetch}>x</button>\n'
			],
			[
				'a fetch member in an interface',
				planted.component,
				'export interface P {\n\treadonly fetch: () => void;\n}'
			],
			[
				"x['fetch']",
				planted.component,
				"export const r = (x: Readonly<Record<string, unknown>>) => x['fetch'];"
			],
			['{ fetch: f }', planted.component, 'export const r = (f: () => void) => ({ fetch: f });'],
			[
				"{ 'fetch': f }",
				planted.component,
				"export const r = (f: () => void) => ({ 'fetch': f });"
			],
			[
				'a fetch attribute',
				planted.componentSvelte,
				'<script lang="ts">\n\timport Y from \'./Y.svelte\';\n\tconst f = (): void => undefined;\n</script>\n\n<Y fetch={f} />\n'
			],
			[
				'a bind:fetch directive',
				planted.componentSvelte,
				'<script lang="ts">\n\timport Y from \'./Y.svelte\';\n\tlet f = $state(0);\n</script>\n\n<Y bind:fetch={f} />\n'
			]
		];

		it.each(refused)('refuses %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, SYNTAX);
		});

		it.each([
			['a check', planted.check],
			['a spec', planted.spec]
		])('allows load({ fetch }) in %s', async (_label, filePath) => {
			await expectAllowed(filePath, loadWithFetch);
		});
	});

	describe('no markup runs script or loads another document', () => {
		/** A component whose script declares `s`, then `markup`. */
		const component = (markup: string): string =>
			`<script lang="ts">\n\tconst s = 'x';\n</script>\n\n${markup}\n`;

		const refused: readonly Allowed[] = [
			['an iframe', component('<iframe title={s}></iframe>')],
			['a frame', component('<frame title={s} />')],
			['an object', component('<object title={s}></object>')],
			['an embed', component('<embed title={s} />')],
			['an iframe in mixed case', component('<iFrame title={s}></iFrame>')],
			['an object in mixed case', component('<oBJECT title={s}></oBJECT>')],
			['a script in a div', component('<div><script>console.log(s);</script></div>')],
			['a script in svelte:head', component('<svelte:head><script>1</script></svelte:head>')],
			['a script in an svg', component('<svg viewBox="0 0 1 1"><script>1</script></svg>')],
			['a script in mixed case', component('<div><sCRIPT>1</sCRIPT></div>')],
			[
				'an iframe in an svg foreignObject',
				component('<svg><foreignObject><iframe title={s}></iframe></foreignObject></svg>')
			],
			['srcdoc on an iframe', component('<iframe title="x" srcdoc={s}></iframe>')],
			['srcdoc on another element', component('<div srcdoc={s}></div>')],
			['srcDoc', component('<div srcDoc="x"></div>')],
			[
				'a shorthand srcdoc',
				'<script lang="ts">\n\tconst srcdoc = \'x\';\n</script>\n\n<div {srcdoc}></div>\n'
			],
			[
				'an iframe src from a ?url import',
				'<script lang="ts">\n\timport p from \'./evil.html?url\';\n</script>\n\n<iframe title="x" src={p}></iframe>\n'
			],
			[
				'svelte:element this="script"',
				component('<svelte:element this="script">{s}</svelte:element>')
			],
			[
				'svelte:element this="IFRAME"',
				component('<svelte:element this="IFRAME">{s}</svelte:element>')
			],
			[
				"svelte:element this={'iframe'}",
				component("<svelte:element this={'iframe'}>{s}</svelte:element>")
			],
			[
				'svelte:element this={`embed`}',
				component('<svelte:element this={`embed`}>{s}</svelte:element>')
			]
		];

		it.each(refused)('refuses %s in a component', async (_label, code) => {
			await expectScriptMarkupRefused(planted.componentSvelte, code);
		});

		const elsewhere: readonly PathCase[] = [
			planted.motionSvelte,
			planted.lazySvelte,
			planted.routesSvelte,
			planted.gallerySvelte
		].flatMap(
			(filePath) =>
				[
					[`an iframe in ${filePath}`, filePath, component('<iframe title={s}></iframe>')],
					[`a nested script in ${filePath}`, filePath, component('<div><script>1</script></div>')],
					[`srcdoc in ${filePath}`, filePath, component('<div srcdoc={s}></div>')],
					[
						`svelte:element this="script" in ${filePath}`,
						filePath,
						component('<svelte:element this="script">{s}</svelte:element>')
					]
				] as const
		);

		it.each(elsewhere)('refuses %s', async (_label, filePath, code) => {
			await expectScriptMarkupRefused(filePath, code);
		});

		const allowed: readonly Allowed[] = [
			[
				'the top-level script and module blocks',
				'<script lang="ts" module>\n\texport const m = \'x\';\n</script>\n\n<script lang="ts">\n\tconst s = \'x\';\n</script>\n\n<p>{s}</p>\n'
			],
			[
				'svelte:element with a dynamic tag',
				"<script lang=\"ts\">\n\tinterface Props {\n\t\treadonly tag: 'p' | 'span';\n\t}\n\tlet { tag }: Props = $props();\n</script>\n\n<svelte:element this={tag}>x</svelte:element>\n"
			],
			[
				'ordinary markup',
				component(
					'<div class="a"><img src="./a.png" alt={s} /><svg viewBox="0 0 1 1"><path d="M0 0" /></svg><p>script, iframe and srcdoc as words</p></div>'
				)
			],
			[
				'a component named like a banned element',
				'<script lang="ts">\n\timport Embed from \'./Embed.svelte\';\n</script>\n\n<Embed />\n'
			]
		];

		it.each(allowed)('allows %s', async (_label, code) => {
			await expectAllowed(planted.componentSvelte, code);
		});
	});

	describe('no markup carries an inline event handler', () => {
		/** A component whose script declares a string `s` and a function `handler`, then `markup`. */
		const component = (markup: string): string =>
			`<script lang="ts">\n\tconst s = 'x';\n\tconst handler = (): void => undefined;\n</script>\n\n${markup}\n`;

		/** A component whose script declares `name` as a string, then `markup`. */
		const declaring = (name: string, markup: string): string =>
			`<script lang="ts">\n\tconst ${name} = 'x';\n</script>\n\n${markup}\n`;

		const refused: readonly Allowed[] = [
			['ONERROR text on an img', component(`<img alt={s} src="./a.png" ONERROR="fetch('/z')" />`)],
			['ONCLICK with a string expression', component('<div ONCLICK={s}>x</div>')],
			['ONCLICK with a string literal', component(`<div ONCLICK={"fetch('/x')"}>x</div>`)],
			['a shorthand {ONERROR}', declaring('ONERROR', '<img alt="" src="./a.png" {ONERROR} />')],
			['a shorthand {OnError}', declaring('OnError', '<img alt="" src="./a.png" {OnError} />')],
			['OnError', component('<img alt={s} src="./a.png" OnError="x" />')],
			['oNcLiCk', component('<div oNcLiCk="x">{s}</div>')],
			['onClick with a function', component('<button onClick={handler}>{s}</button>')],
			['a lowercase onclick with text', component('<div onclick="x">{s}</div>')],
			[
				'a lowercase onclick with text around an expression',
				component('<div onclick="x{s}">x</div>')
			],
			['a lowercase onclick with no value', component('<div onclick>{s}</div>')],
			[
				'ONCLICK on svelte:element with a static tag',
				component(`<svelte:element this="div" ONCLICK="fetch('/x')">{s}</svelte:element>`)
			],
			[
				'OnClick on svelte:element with a dynamic tag',
				component('<svelte:element this={s} OnClick={s}>x</svelte:element>')
			],
			['ONCLICK on a custom element', component('<my-el ONCLICK="x">{s}</my-el>')],
			['ONLOAD in an svg', component('<svg viewBox="0 0 1 1" ONLOAD="x"><title>{s}</title></svg>')],
			[
				'ONLOAD in svelte:head',
				component('<svelte:head><meta name="x" content={s} ONLOAD="x" /></svelte:head>')
			]
		];

		it.each(refused)('refuses %s in a component', async (_label, code) => {
			await expectInlineHandlerRefused(planted.componentSvelte, code);
		});

		const elsewhere: readonly PathCase[] = [
			planted.motionSvelte,
			planted.lazySvelte,
			planted.routesSvelte,
			planted.gallerySvelte
		].flatMap(
			(filePath) =>
				[
					[
						`ONERROR text in ${filePath}`,
						filePath,
						component('<img alt={s} src="./a.png" ONERROR="x" />')
					],
					[`ONCLICK={s} in ${filePath}`, filePath, component('<div ONCLICK={s}>x</div>')],
					[
						`a shorthand {ONERROR} in ${filePath}`,
						filePath,
						declaring('ONERROR', '<img alt="" src="./a.png" {ONERROR} />')
					]
				] as const
		);

		it.each(elsewhere)('refuses %s', async (_label, filePath, code) => {
			await expectInlineHandlerRefused(filePath, code);
		});

		const allowed: readonly Allowed[] = [
			['onclick={handler}', component('<button onclick={handler}>{s}</button>')],
			['onclick={() => …}', component('<button onclick={() => handler()}>{s}</button>')],
			[
				'onclick="{handler}", one quoted expression',
				component('<button onclick="{handler}">{s}</button>')
			],
			[
				'a lowercase shorthand {onclick} holding a function',
				'<script lang="ts">\n\tconst onclick = (): void => undefined;\n</script>\n\n<button {onclick}>x</button>\n'
			],
			[
				"a component's capitalised on… props",
				'<script lang="ts">\n\timport Wizard from \'./Wizard.svelte\';\n\timport Button from \'./Button.svelte\';\n</script>\n\n<Wizard onboarding="x" />\n<Button ONCLICK="x" />\n'
			],
			[
				'svelte:window onkeydown={handler}',
				component('<svelte:window onkeydown={handler} />\n<p>{s}</p>')
			],
			[
				'attributes that merely hold "on"',
				component(
					'<div data-on="x" title="online" class="button">onclick and ONERROR as words {s}</div>'
				)
			]
		];

		it.each(allowed)('allows %s', async (_label, code) => {
			await expectAllowed(planted.componentSvelte, code);
		});
	});

	describe('no source file holds a remote URL in a string', () => {
		const refused: readonly PathCase[] = [
			[
				'a constant later used as an img src',
				planted.componentSvelte,
				'<script lang="ts">\n\tconst u = \'https://x/a.png\';\n</script>\n\n<img src={u} alt="" />\n'
			],
			[
				'a remote value passed to a component',
				planted.componentSvelte,
				'<script lang="ts">\n\timport Y from \'./Y.svelte\';\n</script>\n\n<Y url="https://x/a.png" />\n'
			],
			[
				'a template literal',
				planted.component,
				"const h = 'x';\nexport const r = `https://${h}/a`;"
			],
			['an import from a URL', planted.component, "import 'https://x/a.js';"],
			['a page', planted.routes, stringModule('https://x/a.png')],
			['the frame', planted.gallery, stringModule('//x/a.png')],
			['the motion adapter', planted.motion, stringModule('https://x/a.png')],
			['the lazy loader', planted.lazy, stringModule('https://x/a.png')]
		];

		it.each(refused)('refuses %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, REMOTE);
		});

		const hostPaths = [planted.component, planted.routes, planted.gallery];

		it.each(
			hostPaths.flatMap((filePath) => remoteSamples.map((text) => [filePath, text] as const))
		)('refuses in %s the string %j', async (filePath, text) => {
			await expectRefused(filePath, stringModule(text), REMOTE);
		});

		it.each(localSamples)('allows the local string %j', async (text) => {
			await expectAllowed(planted.component, stringModule(text));
		});

		it.each([
			['a check', planted.check],
			['a spec', planted.spec]
		])('allows a remote string in %s', async (_label, filePath) => {
			await expectAllowed(filePath, stringModule('https://x/a.png'));
		});

		it.each([...remoteSamples, ...localSamples])(
			'agrees with the network scan on %j',
			async (text) => {
				const messages = await lintPlanted(planted.component, stringModule(text));
				const refusedByLint = messages.some((message) => message.ruleId === REMOTE);
				expect(refusedByLint).toBe(isRemote(text));
			}
		);
	});

	describe('no source file holds a javascript: URL', () => {
		const scriptUrlSamples: readonly string[] = [
			'javascript:x',
			' JAVASCRIPT:x',
			'JaVaScRiPt:void(0)',
			'java\tscript:x',
			'java\nscript:x',
			'\u0001javascript:x',
			'\t\n javascript:x'
		];

		const refused: readonly PathCase[] = [
			[
				'an href in markup',
				planted.componentSvelte,
				'<script lang="ts">\n\tconst s = \'x\';\n</script>\n\n<a href="javascript:fetch(\'/y\')">{s}</a>\n'
			],
			[
				'an href written with a character reference',
				planted.componentSvelte,
				'<script lang="ts">\n\tconst s = \'x\';\n</script>\n\n<a href="&#106;avascript:x">{s}</a>\n'
			],
			[
				'an href expression',
				planted.componentSvelte,
				"<script lang=\"ts\">\n\tconst s = 'x';\n</script>\n\n<a href={'JavaScript:x'}>{s}</a>\n"
			],
			[
				'a template literal',
				planted.component,
				"const h = 'x';\nexport const r = `javascript:${h}`;"
			],
			['the motion adapter', planted.motion, stringModule(' JAVASCRIPT:fetch(1)')],
			['the lazy loader', planted.lazy, stringModule(' JAVASCRIPT:fetch(1)')],
			['a page', planted.routes, stringModule(' JAVASCRIPT:fetch(1)')],
			['the frame', planted.gallery, stringModule(' JAVASCRIPT:fetch(1)')]
		];

		it.each(refused)('refuses %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, SCRIPT_URL);
		});

		it.each(scriptUrlSamples)('refuses the string %j', async (text) => {
			await expectRefused(planted.component, stringModule(text), SCRIPT_URL);
		});

		it.each(['./a', 'javascript', 'javascript is a language', 'x javascript:y', 'javascripts:x'])(
			'allows the string %j',
			async (text) => {
				await expectAllowed(planted.component, stringModule(text));
			}
		);

		it('allows a plain href and the word in markup text', async () => {
			await expectAllowed(
				planted.componentSvelte,
				'<script lang="ts">\n\tconst s = \'x\';\n</script>\n\n<a href="./a">{s}</a>\n<p>javascript: the language</p>\n'
			);
		});

		it.each([
			['a check', planted.check],
			['a spec', planted.spec]
		])('allows a javascript: string in %s', async (_label, filePath) => {
			await expectAllowed(filePath, stringModule('javascript:x'));
		});
	});
});

type RuleEntry = Linter.RuleEntry | undefined;

interface BoundaryOptions {
	readonly globals: RuleEntry;
	readonly properties: RuleEntry;
	readonly imports: RuleEntry;
	readonly syntax: RuleEntry;
	readonly tiers: RuleEntry;
	readonly remote: RuleEntry;
	readonly scriptUrl: RuleEntry;
}

async function effectiveBoundary(filePath: string): Promise<BoundaryOptions> {
	const config = (await realLinter.calculateConfigForFile(filePath)) as Linter.Config;
	const rules = config.rules ?? {};
	return {
		globals: rules['no-restricted-globals'],
		properties: rules['no-restricted-properties'],
		imports: rules['no-restricted-imports'],
		syntax: rules['no-restricted-syntax'],
		tiers: rules[TIERS],
		remote: rules[REMOTE],
		scriptUrl: rules[SCRIPT_URL]
	};
}

type SetName = keyof typeof boundaryRules;

function expectedBoundary(name: SetName): BoundaryOptions {
	const rules = boundaryRules[name];
	return {
		globals: [2, { globals: rules.globals, checkGlobalObject: true }],
		properties: [2, ...rules.properties],
		imports: [2, { patterns: rules.imports }],
		syntax: [2, ...rules.syntax],
		tiers: [2, { tier: rules.tier }],
		remote: [2],
		scriptUrl: [2]
	};
}

const noBoundary: BoundaryOptions = {
	globals: undefined,
	properties: undefined,
	imports: undefined,
	syntax: undefined,
	tiers: undefined,
	remote: undefined,
	scriptUrl: undefined
};

/** What ESLint's own matching makes of one path: the set's rules, or none for a check. */
function boundaryOf(name: SetName | 'checks'): BoundaryOptions {
	return name === 'checks' ? noBoundary : expectedBoundary(name);
}

const codePattern = /\.(ts|mts|cts|js|mjs|cjs|svelte)$/;

function realCodeFiles(): readonly string[] {
	return readdirSync(`${uiRoot}src`, { recursive: true, encoding: 'utf8' })
		.filter((relative) => codePattern.test(relative))
		.map((relative) => `src/${relative.split(path.sep).join('/')}`);
}

describe('the file sets', { timeout: 120_000 }, () => {
	it.each([
		['src/foo.ts', 'components'],
		['src/lib/x.ts', 'components'],
		['src/lib/deep/X.svelte', 'components'],
		['src/lib/x.svelte.ts', 'components'],
		['src/app.d.ts', 'components'],
		['src/lib/motion/x.ts', 'motion'],
		['src/lib/motion/deep/X.svelte', 'motion'],
		['src/lib/lazy/x.ts', 'lazy'],
		['src/gallery/x.ts', 'gallery'],
		['src/gallery/X.svelte', 'gallery'],
		['src/routes/x.ts', 'routes'],
		['src/routes/components/button/+page.svelte', 'routes']
	] as const)('puts %s under the %s rules', async (filePath, name) => {
		expect(await effectiveBoundary(filePath)).toEqual(expectedBoundary(name));
	});

	it.each([
		['src/checks/x.spec.ts'],
		['src/checks/helper.ts'],
		['src/lib/x.spec.ts'],
		['src/lib/X.svelte.spec.ts'],
		['src/lib/motion/x.spec.ts'],
		['src/gallery/x.spec.ts'],
		['src/routes/x.svelte.spec.ts']
	])('leaves the check or spec %s free of the boundary rules', async (filePath) => {
		expect(await effectiveBoundary(filePath)).toEqual(noBoundary);
	});

	it('lists real code files to classify', () => {
		expect(realCodeFiles()).toContain('src/lib/index.ts');
	});

	it.each([
		...realCodeFiles(),
		...plantedPaths,
		'src/foo.ts',
		'src/lib/deep/X.svelte',
		'src/lib/x.svelte.ts',
		'src/lib/motion/deep/X.svelte',
		'src/lib/lazy/deep/x.js',
		'src/routes/components/button/+page.svelte',
		'src/checks/deep/x.svelte',
		'src/lib/X.svelte.spec.ts',
		'src/routes/x.svelte.spec.ts',
		'src/x.mjs',
		'src/gallery/x.cts'
	])("classifies %s as ESLint's own matching does", async (filePath) => {
		const name = fileSetOf(filePath);
		expect(name).toBeDefined();
		if (name === undefined) return;
		expect(await effectiveBoundary(filePath)).toEqual(boundaryOf(name));
	});

	it('ignores every inline config comment', async () => {
		const config = (await realLinter.calculateConfigForFile('src/lib/x.ts')) as Linter.Config;
		expect(config.linterOptions?.noInlineConfig).toBe(true);
	});

	it('lints every real file under src/ clean', async () => {
		const results = await realLinter.lintFiles(['src']);
		const problems = results.flatMap((result) =>
			result.messages.map(
				(message) =>
					`${result.filePath}:${String(message.line)} ${message.ruleId ?? ''} ${message.message}`
			)
		);
		expect(results.length).toBeGreaterThan(0);
		expect(problems).toEqual([]);
	});
});
