import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	boundaryRules,
	fileSetOf,
	HOST_STORAGE_MESSAGE,
	httpEquivMessage,
	inlineHandlerMessage,
	rawHtmlMessage,
	scriptMarkupMessage,
	scriptModuleMessage,
	seamMotionMessage,
	STORAGE_MESSAGE
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
	stories: 'src/stories/x.ts',
	storiesSvelte: 'src/stories/X.stories.svelte',
	environment: 'src/lib/environment/x.ts',
	runesModule: 'src/lib/motion/x.svelte.ts',
	check: 'src/checks/planted.ts',
	checkSvelte: 'src/checks/X.svelte',
	spec: 'src/lib/planted.spec.ts'
} as const;

/**
 * Paths a later change may create: planted while they are not on disk, and
 * linted against the real project once they are, so the cases hold either way.
 */
const plannedOrReal = {
	skinStore: 'src/gallery/skin-store.ts',
	colourPage: 'src/routes/components/colour/+page.svelte',
	environment: 'src/lib/styles/environment.ts',
	button: 'src/lib/controls/Button.svelte',
	checkbox: 'src/lib/controls/Checkbox.svelte',
	toggle: 'src/lib/controls/Toggle.svelte',
	iconButton: 'src/lib/controls/IconButton.svelte',
	controlTypes: 'src/lib/controls/types.ts',
	context: 'src/lib/environment/context.ts',
	card: 'src/lib/surfaces/Card.svelte',
	togglePage: 'src/routes/components/toggle/+page.svelte',
	recording: 'src/checks/testing/RecordingEnvironment.svelte'
} as const;

const notOnDisk = (relative: string): boolean => !existsSync(`${uiRoot}${relative}`);

const plantedPaths = [...Object.values(planted), ...Object.values(plannedOrReal).filter(notOnDisk)];

/** Real files a case lints planted text at; the file on disk is never read or touched. */
const real = {
	publicEntry: 'src/lib/index.ts',
	layoutLoad: 'src/routes/+layout.ts',
	storybookPreview: '.storybook/preview.ts',
	storybookManager: '.storybook/manager.ts',
	stylesEntry: 'src/lib/styles/index.ts'
} as const;

/** Storybook's config files, linted as the CLI lints them. */
const storybookConfigFiles = [
	'.storybook/main.ts',
	'.storybook/preview.ts',
	'.storybook/manager.ts',
	'.storybook/vitest.setup.ts'
] as const;

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
	'ui/no-script-url',
	'ui/no-motion-import',
	'ui/no-provide-environment',
	'ui/spreads'
]);

async function lintPlanted(filePath: string, code: string): Promise<readonly Linter.LintMessage[]> {
	const [result] = await plantedLinter.lintText(code, { filePath });
	if (result === undefined) throw new Error(`no lint result for ${filePath}`);
	if (!notOnDisk(filePath)) await restoreFromDisk(filePath);
	const fatal = result.messages.filter((message) => message.fatal === true);
	expect(fatal).toEqual([]);
	return result.messages;
}

/**
 * Lints a real file's own text at its path again. The TypeScript project
 * service keeps the last text it was handed for a path, so after a plant over
 * a real file every file importing it would otherwise see the planted text.
 */
async function restoreFromDisk(filePath: string): Promise<void> {
	await plantedLinter.lintText(readFileSync(`${uiRoot}${filePath}`, 'utf8'), { filePath });
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

/** Refused by `ruleId`, with a message that carries `why`. */
async function expectRefusedBecause(
	filePath: string,
	code: string,
	ruleId: string,
	why: string
): Promise<void> {
	const messages = await lintPlanted(filePath, code);
	const matching = messages.filter(
		(message) => message.ruleId === ruleId && message.message.includes(why)
	);
	expect(matching).not.toEqual([]);
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
const MOTION_IMPORT = 'ui/no-motion-import';
const PROVIDE = 'ui/no-provide-environment';
const SPREADS = 'ui/spreads';

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

/** Storage reached as a member, past what the globals rule sees. */
const storageMemberRefusals: readonly Case[] = [
	['localStorage on any window', 'export const r = (w: Window) => w.localStorage;', PROPERTIES],
	[
		'a destructured localStorage',
		'export const r = (w: Window) => {\n\tconst { localStorage: s } = w;\n\treturn s;\n};',
		PROPERTIES
	],
	["w['sessionStorage']", "export const r = (w: Window) => w['sessionStorage'];", PROPERTIES],
	['window.document.cookie', 'export const r = window.document.cookie;', PROPERTIES],
	['cookie on any document', 'export const r = (d: Document) => d.cookie;', PROPERTIES]
];

/** The script-side twins of {@html}. */
const htmlSinkRefusals: readonly Case[] = [
	['innerHTML', "export const r = (e: Element) => {\n\te.innerHTML = 'x';\n};", PROPERTIES],
	['outerHTML', "export const r = (e: Element) => {\n\te.outerHTML = 'x';\n};", PROPERTIES],
	[
		'insertAdjacentHTML',
		"export const r = (e: Element) => {\n\te.insertAdjacentHTML('beforeend', 'x');\n};",
		PROPERTIES
	],
	[
		'createContextualFragment',
		"export const r = (range: Range) => range.createContextualFragment('x');",
		PROPERTIES
	],
	['document.write', "export const r = () => {\n\tdocument.write('x');\n};", PROPERTIES]
];

/** The host's own spellings of its three subpath imports, and the rest it refuses. */
const hostSubpathAllowed: readonly Allowed[] = [
	['@zurfur/ui/styles', "import '@zurfur/ui/styles';"],
	['@zurfur/ui/tokens.json', "import tokens from '@zurfur/ui/tokens.json';\nexport { tokens };"],
	[
		'@zurfur/ui/tokens.additions.json',
		"import additions from '@zurfur/ui/tokens.additions.json';\nexport { additions };"
	]
];

const hostDeepImportRefusals: readonly Case[] = [
	['a file inside the styles', "import '@zurfur/ui/src/lib/styles/tokens.css';", IMPORTS],
	['a path below styles', "import '@zurfur/ui/styles/x';", IMPORTS],
	['styles with a trailing slash', "import '@zurfur/ui/styles/';", IMPORTS],
	[
		'tokens.json?raw',
		"import tokens from '@zurfur/ui/tokens.json?raw';\nexport { tokens };",
		IMPORTS
	],
	['styles?inline', "import '@zurfur/ui/styles?inline';", IMPORTS],
	[
		'tokens.additions.json?raw',
		"import a from '@zurfur/ui/tokens.additions.json?raw';\nexport { a };",
		IMPORTS
	],
	['a made-up json beside the tokens', "import '@zurfur/ui/tokens.extra.json';", IMPORTS],
	['styles in another letter case', "import '@zurfur/ui/Styles';", IMPORTS],
	['the package in another letter case', "import '@Zurfur/UI/styles';", IMPORTS],
	['a re-export from inside the package', "export * from '@zurfur/ui/src/lib/x';", IMPORTS]
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
	...storageMemberRefusals,
	...htmlSinkRefusals,
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
	["the host's @zurfur/ui/styles", "import '@zurfur/ui/styles';", IMPORTS],
	[
		"the host's @zurfur/ui/tokens.json",
		"import tokens from '@zurfur/ui/tokens.json';\nexport { tokens };",
		IMPORTS
	],
	[
		"the host's @zurfur/ui/tokens.additions.json",
		"import a from '@zurfur/ui/tokens.additions.json';\nexport { a };",
		IMPORTS
	],
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
		const onDisk = plantedPaths.filter((relative) => !notOnDisk(relative));
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

	describe('no file holds raw HTML', () => {
		const atHtml = '<script lang="ts">\n\tconst x = \'<b>x</b>\';\n</script>\n\n{@html x}\n';
		const bound = (name: string): string =>
			`<script lang="ts">\n\tlet x = $state('');\n</script>\n\n<div contenteditable="true" bind:${name}={x}></div>\n`;
		const svelteSets = [
			['a component', planted.componentSvelte],
			['the motion adapter', planted.motionSvelte],
			['the lazy loader', planted.lazySvelte],
			['a route', planted.routesSvelte],
			['a gallery page', plannedOrReal.colourPage],
			['the gallery frame', planted.gallerySvelte],
			['a stories file', planted.storiesSvelte]
		] as const;

		it.each(svelteSets)('refuses {@html x} in %s', async (_label, filePath) => {
			await expectRefused(filePath, atHtml, 'svelte/no-at-html-tags');
		});

		it.each(
			svelteSets.flatMap(([label, filePath]) => [
				[`bind:innerHTML in ${label}`, filePath, bound('innerHTML')] as const,
				[`bind:outerHTML in ${label}`, filePath, bound('outerHTML')] as const
			])
		)('refuses %s', async (_label, filePath, code) => {
			await expectSyntaxMessage(filePath, code, rawHtmlMessage);
		});

		it('allows bind:textContent', async () => {
			await expectAllowed(planted.componentSvelte, bound('textContent'));
		});

		it.each([
			['a component', 'src/lib/X.svelte'],
			['the motion adapter', 'src/lib/motion/X.svelte'],
			['the lazy loader', 'src/lib/lazy/X.svelte'],
			['a route', 'src/routes/X.svelte'],
			['the gallery frame', 'src/gallery/X.svelte'],
			['a stories file', 'src/stories/X.stories.svelte'],
			['a check', 'src/checks/X.svelte']
		])('switches {@html} off nowhere: %s', async (_label, filePath) => {
			const config = (await realLinter.calculateConfigForFile(filePath)) as Linter.Config;
			expect(config.rules?.['svelte/no-at-html-tags']).toEqual([2]);
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
		['$app/paths', "import { resolve } from '$app/paths';\nexport { resolve };"],
		['@zurfur/ui', "import * as ui from '@zurfur/ui';\nexport { ui };"],
		['$gallery/…', "import { groupNames } from '$gallery/groups';\nexport { groupNames };"],
		['?raw', "import text from './a.txt?raw';\nexport { text };"],
		...hostSubpathAllowed
	];

	/** What every file of the host refuses, its stores included. */
	const everyHostRefusal: readonly Case[] = [
		['fetch', "export const r = fetch('/x');", GLOBALS],
		['new WebSocket', "export const r = new WebSocket('wss://x');", GLOBALS],
		['sessionStorage', 'export const r = sessionStorage;', GLOBALS],
		['globalThis.sessionStorage', 'export const r = globalThis.sessionStorage;', GLOBALS],
		['indexedDB', 'export const r = indexedDB;', GLOBALS],
		['caches', 'export const r = caches;', GLOBALS],
		['cookieStore', 'export const r: unknown = cookieStore;', GLOBALS],
		['document.cookie', 'export const r = document.cookie;', PROPERTIES],
		["document['cookie']", "export const r = document['cookie'];", PROPERTIES],
		["w['sessionStorage']", "export const r = (w: Window) => w['sessionStorage'];", PROPERTIES],
		['window.document.cookie', 'export const r = window.document.cookie;', PROPERTIES],
		...htmlSinkRefusals,
		...hostDeepImportRefusals,
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

	/** localStorage, which only the gallery's stores may touch. */
	const localStorageRefusals: readonly Case[] = [
		['localStorage', "export const r = localStorage.getItem('x');", GLOBALS],
		['window.localStorage', 'export const r = window.localStorage;', GLOBALS],
		['localStorage on any window', 'export const r = (w: Window) => w.localStorage;', PROPERTIES],
		[
			'a destructured localStorage',
			'export const r = (w: Window) => {\n\tconst { localStorage: s } = w;\n\treturn s;\n};',
			PROPERTIES
		]
	];

	const hostRefusals: readonly Case[] = [...everyHostRefusal, ...localStorageRefusals];

	describe("the gallery's stores (src/gallery/skin-store.ts)", () => {
		it.each([
			['localStorage', "export const r = localStorage.getItem('x');"],
			['window.localStorage', "export const r = window.localStorage.setItem('x', 'y');"],
			['localStorage on any window', 'export const r = (w: Window) => w.localStorage;'],
			...hostSubpathAllowed
		] as const)('allows %s', async (_label, code) => {
			await expectAllowed(plannedOrReal.skinStore, code);
		});

		it.each([
			...everyHostRefusal,
			['process.env', 'export const r = process.env.X;', GLOBALS],
			[
				"import.meta.glob('/src/routes/**/meta.ts')",
				"export const r = import.meta.glob('/src/routes/**/meta.ts', { eager: true });",
				SYNTAX
			]
		] as const)('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(plannedOrReal.skinStore, code, ruleId);
		});
	});

	describe('localStorage outside the stores', () => {
		it.each([
			['a gallery page', plannedOrReal.colourPage, 'svelte'],
			['the styles', plannedOrReal.environment, 'ts'],
			['another file in the frame', 'src/gallery/x.ts', 'ts'],
			['a route', planted.routes, 'ts']
		] as const)('refuses localStorage in %s', async (_label, filePath, kind) => {
			const code =
				kind === 'svelte'
					? '<script lang="ts">\n\tconst r = localStorage.getItem(\'x\');\n</script>\n\n{r}\n'
					: "export const r = localStorage.getItem('x');";
			await expectRefused(filePath, code, GLOBALS);
		});
	});

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

	describe('the stories (src/stories)', () => {
		/** A stories file in Svelte CSF, with `script` in its instance script and `markup` in its body. */
		const storiesFile = (script: string, markup: string): string =>
			`<script module lang="ts">\n\timport { defineMeta } from '@storybook/addon-svelte-csf';\n\n\tconst { Story } = defineMeta({ title: 'Planted/X' });\n</script>\n\n<script lang="ts">\n\t${script}\n</script>\n\n<Story name="Planted">${markup}</Story>\n`;

		it.each([
			...hostAllowed,
			['storybook/test', "import { fn } from 'storybook/test';\nexport const r = fn();"],
			[
				'Svelte CSF',
				"import { defineMeta } from '@storybook/addon-svelte-csf';\nexport { defineMeta };"
			]
		] as const)('allows %s in a stories module', async (_label, code) => {
			await expectAllowed(planted.stories, code);
		});

		it('allows a plain Svelte CSF stories file', async () => {
			await expectAllowed(
				planted.storiesSvelte,
				storiesFile('const words = "Send request";', '{words}')
			);
		});

		it.each([
			...routesRefusals,
			['localStorage', "export const r = localStorage.getItem('x');", GLOBALS],
			['a remote string', stringModule('https://x/a.png'), REMOTE],
			['a javascript: string', stringModule('javascript:alert(1)'), SCRIPT_URL],
			['a dynamic import()', "export const r = import('./y');", SYNTAX],
			[
				'an inline handler set as an attribute',
				"export const r = (e: Element) => {\n\te.setAttribute('onclick', 'x');\n};",
				SYNTAX
			]
		] as const)('refuses %s in a stories module', async (_label, code, ruleId) => {
			await expectRefused(planted.stories, code, ruleId);
		});

		it.each([
			['fetch', "const r = fetch('/x');", '{r}', GLOBALS],
			['localStorage', "const r = localStorage.getItem('x');", '{r}', GLOBALS],
			['sessionStorage', "const r = sessionStorage.getItem('x');", '{r}', GLOBALS],
			['a remote string', "const u = 'https://x/a.png';", '<img src={u} alt="" />', REMOTE],
			['a remote attribute', 'const r = 1;', '<img src="https://x/a.png" alt="{r}" />', REMOTE],
			['a javascript: string', "const u = 'javascript:alert(1)';", '<a href={u}>x</a>', SCRIPT_URL],
			['{@html}', "const x = '<b>x</b>';", '{@html x}', 'svelte/no-at-html-tags'],
			['a dynamic import()', "const r = import('./y');", '{r}', SYNTAX]
		] as const)('refuses %s in a stories file', async (_label, script, markup, ruleId) => {
			await expectRefused(planted.storiesSvelte, storiesFile(script, markup), ruleId);
		});

		it.each([
			['a text value', 'const r = 1;', '<button onclick="alert(1)">{r}</button>'],
			['another letter case', 'const f = (): void => undefined;', '<button onClick={f}>x</button>']
		] as const)(
			'refuses an inline handler with %s in a stories file',
			async (_label, script, markup) => {
				await expectInlineHandlerRefused(planted.storiesSvelte, storiesFile(script, markup));
			}
		);

		it.each([
			['a stories module', planted.stories, "import { x } from '../checks/x';\nexport { x };"],
			[
				'a stories file',
				planted.storiesSvelte,
				storiesFile("import { x } from '../checks/x';", '{x}')
			],
			['a stories module importing a spec', planted.stories, "import './frame/globals.spec';"]
		] as const)('refuses a check imported by %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, TIERS);
		});
	});

	describe("Storybook's browser-side config (.storybook/preview.ts, manager.ts)", () => {
		it.each([
			['the preview', real.storybookPreview],
			['the manager', real.storybookManager]
		] as const)('refuses fetch in %s', async (_label, filePath) => {
			await expectRefused(filePath, "export const r = fetch('/x');", GLOBALS);
		});

		it.each([
			['the preview', real.storybookPreview],
			['the manager', real.storybookManager]
		] as const)('refuses a remote string in %s', async (_label, filePath) => {
			await expectRefused(filePath, stringModule('https://x/a.js'), REMOTE);
		});

		it.each([
			['localStorage', 'export const r = localStorage;', GLOBALS],
			['sessionStorage', 'export const r = sessionStorage;', GLOBALS],
			['innerHTML', "export const r = (e: Element) => {\n\te.innerHTML = 'x';\n};", PROPERTIES],
			['a javascript: string', stringModule('javascript:alert(1)'), SCRIPT_URL],
			['a dynamic import()', "export const r = import('./x.ts');", SYNTAX],
			['import.meta.env', 'export const r: unknown = import.meta.env;', SYNTAX],
			['a check', "import '../src/checks/x';", TIERS],
			['a file outside src', "import './x.ts';", TIERS]
		] as const)('refuses %s in the preview', async (_label, code, ruleId) => {
			await expectRefused(real.storybookPreview, code, ruleId);
		});

		it('allows the preview its frame and the styles', async () => {
			await expectAllowed(
				real.storybookPreview,
				"import '@zurfur/ui/styles';\nimport StoryFrame from '../src/stories/frame/StoryFrame.svelte';\nexport { StoryFrame };"
			);
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
				// Only the public entry: no other component reaches the adapter (ui/no-motion-import).
				'the public entry importing the motion adapter',
				real.publicEntry,
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
			['an object in mixed case', component('<oBjEcT title={s}></oBjEcT>')],
			['a script in a div', component('<div><script>console.log(s);</script></div>')],
			['a script in svelte:head', component('<svelte:head><script>1</script></svelte:head>')],
			['a script in an svg', component('<svg viewBox="0 0 1 1"><script>1</script></svg>')],
			['a script in mixed case', component('<div><sCrIpT>1</sCrIpT></div>')],
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

		// A component may hold neither a module block nor <svelte:element>: the
		// host and the card show them here.
		it.each([
			[
				'the top-level script and module blocks in a page',
				planted.routesSvelte,
				'<script lang="ts" module>\n\texport const m = \'x\';\n</script>\n\n<script lang="ts">\n\tconst s = \'x\';\n</script>\n\n<p>{s}</p>\n'
			],
			[
				'svelte:element with a dynamic tag in the card',
				plannedOrReal.card,
				"<script lang=\"ts\">\n\tinterface Props {\n\t\treadonly tag: 'p' | 'span';\n\t}\n\tlet { tag }: Props = $props();\n</script>\n\n<svelte:element this={tag}>x</svelte:element>\n"
			]
		] as const)('allows %s', async (_label, filePath, code) => {
			await expectAllowed(filePath, code);
		});

		const allowed: readonly Allowed[] = [
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

	describe('every source set refuses raw HTML, a page refresh and the private file store', () => {
		type Because = readonly [label: string, code: string, ruleId: string, why: string];

		/** Svelte's raw-HTML helpers and the DOM calls that parse a string as HTML. */
		const rawHtmlRefusals: readonly Because[] = [
			[
				'createRawSnippet',
				"import { createRawSnippet } from 'svelte';\nexport { createRawSnippet };",
				IMPORTS,
				rawHtmlMessage
			],
			[
				'createRawSnippet under an alias',
				"import { createRawSnippet as raw } from 'svelte';\nexport { raw };",
				IMPORTS,
				rawHtmlMessage
			],
			[
				'a re-export of createRawSnippet',
				"export { createRawSnippet } from 'svelte';",
				IMPORTS,
				rawHtmlMessage
			],
			[
				'a re-export of createRawSnippet under an alias',
				"export { createRawSnippet as raw } from 'svelte';",
				IMPORTS,
				rawHtmlMessage
			],
			["export * from 'svelte'", "export * from 'svelte';", IMPORTS, rawHtmlMessage],
			[
				'createRawSnippet through a namespace import',
				"import * as s from 'svelte';\nexport const r = s.createRawSnippet;",
				PROPERTIES,
				rawHtmlMessage
			],
			[
				"s['createRawSnippet'] through a namespace import",
				"import * as s from 'svelte';\nexport const r = s['createRawSnippet'];",
				PROPERTIES,
				rawHtmlMessage
			],
			[
				'createRawSnippet destructured from a namespace import',
				"import * as s from 'svelte';\nconst { createRawSnippet: raw } = s;\nexport { raw };",
				PROPERTIES,
				rawHtmlMessage
			],
			['svelte/internal', "import 'svelte/internal';", IMPORTS, rawHtmlMessage],
			[
				'svelte/internal/client',
				"import * as $ from 'svelte/internal/client';\nexport { $ };",
				IMPORTS,
				rawHtmlMessage
			],
			[
				'a re-export of svelte/internal/server',
				"export * from 'svelte/internal/server';",
				IMPORTS,
				rawHtmlMessage
			],
			['new DOMParser()', 'export const r = new DOMParser();', GLOBALS, rawHtmlMessage],
			['window.DOMParser', 'export const r = new window.DOMParser();', GLOBALS, rawHtmlMessage],
			['self.DOMParser', 'export const r = new self.DOMParser();', GLOBALS, rawHtmlMessage],
			[
				'globalThis.DOMParser',
				'export const r = new globalThis.DOMParser();',
				GLOBALS,
				rawHtmlMessage
			],
			[
				'parseFromString on any object',
				"export const r = (p: { readonly parseFromString: (s: string, t: string) => unknown }) =>\n\tp.parseFromString('<b>x</b>', 'text/html');",
				PROPERTIES,
				rawHtmlMessage
			],
			[
				'parseHTMLUnsafe on any object',
				"export const r = (d: { readonly parseHTMLUnsafe: (s: string) => unknown }) =>\n\td.parseHTMLUnsafe('<b>x</b>');",
				PROPERTIES,
				rawHtmlMessage
			],
			[
				'setHTML on any object',
				"export const r = (e: { readonly setHTML: (s: string) => void }) => {\n\te.setHTML('<b>x</b>');\n};",
				PROPERTIES,
				rawHtmlMessage
			]
		];

		/** An element to set attributes on, `s` a string, and `ns` a namespace. */
		const onElement = (body: string): string =>
			`export const r = (e: Element, d: Document, s: string, ns: string) => {\n\t${body};\n};`;

		/** Attributes and elements, named by a literal, that run script or load a document. */
		const scriptAttributeRefusals: readonly Because[] = [
			[
				"setAttribute('onclick', s)",
				onElement("e.setAttribute('onclick', s)"),
				SYNTAX,
				inlineHandlerMessage
			],
			[
				"setAttribute('ONERROR', s)",
				onElement("e.setAttribute('ONERROR', s)"),
				SYNTAX,
				inlineHandlerMessage
			],
			[
				'setAttribute(`onload`, s)',
				onElement('e.setAttribute(`onload`, s)'),
				SYNTAX,
				inlineHandlerMessage
			],
			[
				"setAttribute('srcdoc', s)",
				onElement("e.setAttribute('srcdoc', s)"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"setAttribute('SrcDoc', s)",
				onElement("e.setAttribute('SrcDoc', s)"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"setAttributeNS(ns, 'onclick', s)",
				onElement("e.setAttributeNS(ns, 'onclick', s)"),
				SYNTAX,
				inlineHandlerMessage
			],
			[
				'setAttributeNS(ns, `srcdoc`, s)',
				onElement('e.setAttributeNS(ns, `srcdoc`, s)'),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createAttribute('onclick')",
				onElement("d.createAttribute('onclick')"),
				SYNTAX,
				inlineHandlerMessage
			],
			[
				"createAttribute('srcdoc')",
				onElement("d.createAttribute('srcdoc')"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createAttributeNS(ns, 'OnClick')",
				onElement("d.createAttributeNS(ns, 'OnClick')"),
				SYNTAX,
				inlineHandlerMessage
			],
			[
				'srcdoc on any object',
				'export const r = (f: HTMLIFrameElement, s: string) => {\n\tf.srcdoc = s;\n};',
				PROPERTIES,
				scriptMarkupMessage
			],
			[
				"createElement('script')",
				"export const r = document.createElement('script');",
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createElement('IFRAME')",
				onElement("d.createElement('IFRAME')"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				'createElement(`embed`)',
				onElement('d.createElement(`embed`)'),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createElement('Object')",
				onElement("d.createElement('Object')"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createElement('frame')",
				onElement("d.createElement('frame')"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createElementNS(ns, 'script')",
				onElement("d.createElementNS(ns, 'script')"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"createElementNS(ns, 'svg:script')",
				onElement("d.createElementNS(ns, 'svg:script')"),
				SYNTAX,
				scriptMarkupMessage
			],
			[
				"setAttribute('http-equiv', s)",
				onElement("e.setAttribute('HTTP-EQUIV', s)"),
				SYNTAX,
				httpEquivMessage
			],
			[
				"a quoted 'http-equiv' key, as a spread of attributes carries it",
				"export const r = { 'HTTP-EQUIV': 'refresh', content: '0' };",
				SYNTAX,
				httpEquivMessage
			],
			[
				'httpEquiv on any object',
				"export const r = (m: HTMLMetaElement) => {\n\tm.httpEquiv = 'refresh';\n};",
				PROPERTIES,
				httpEquivMessage
			]
		];

		/** The browser's private file store, refused with the set's own storage message. */
		const fileStoreRefusals = (why: string): readonly Because[] => [
			[
				'navigator.storage.getDirectory()',
				'export const r = navigator.storage.getDirectory();',
				PROPERTIES,
				why
			],
			['navigator.storage', 'export const r = navigator.storage;', PROPERTIES, why],
			["navigator['storage']", "export const r = navigator['storage'];", PROPERTIES, why],
			['window.navigator.storage', 'export const r = window.navigator.storage;', SYNTAX, why],
			['self.navigator.storage', 'export const r = self.navigator.storage;', SYNTAX, why],
			[
				'globalThis.navigator.storage',
				'export const r = globalThis.navigator.storage;',
				SYNTAX,
				why
			],
			[
				'a destructured storage',
				'const { storage } = navigator;\nexport const r = storage;',
				PROPERTIES,
				why
			],
			[
				'storage destructured from window.navigator',
				'const { storage } = window.navigator;\nexport const r = storage;',
				SYNTAX,
				why
			],
			[
				'getDirectory on any object',
				'export const r = (m: StorageManager) => m.getDirectory();',
				PROPERTIES,
				why
			],
			[
				'a file written to the private store',
				"export async function r(): Promise<void> {\n\tconst root = await window.navigator.storage.getDirectory();\n\tconst file = await root.getFileHandle('x', { create: true });\n\tconst writable = await file.createWritable();\n\tawait writable.close();\n}",
				PROPERTIES,
				why
			]
		];

		const allowed: readonly Allowed[] = [
			['untrack from svelte', "import { untrack } from 'svelte';\nexport { untrack };"],
			["setAttribute('title', s)", onElement("e.setAttribute('title', s)")],
			["setAttribute('data-skin', 'dark')", onElement("e.setAttribute('data-skin', 'dark')")],
			[
				'setAttribute with a name held in a constant',
				"const SKIN = 'data-skin';\nexport const r = (e: Element) => {\n\te.setAttribute(SKIN, 'dark');\n};"
			],
			["createElement('div')", "export const r = document.createElement('div');"],
			['navigator.language', 'export const r = navigator.language;']
		];

		const sourceSets = [
			['the components', planted.component, STORAGE_MESSAGE],
			['the motion adapter', planted.motion, STORAGE_MESSAGE],
			['the lazy loader', planted.lazy, STORAGE_MESSAGE],
			['the pages', planted.routes, HOST_STORAGE_MESSAGE],
			['the frame', planted.gallery, HOST_STORAGE_MESSAGE],
			["the gallery's stores", plannedOrReal.skinStore, HOST_STORAGE_MESSAGE]
		] as const;

		describe.each(sourceSets)('in %s (%s)', (_set, filePath, storageMessage) => {
			const refused = [
				...rawHtmlRefusals,
				...scriptAttributeRefusals,
				...fileStoreRefusals(storageMessage)
			];

			it.each(refused)('refuses %s', async (_label, code, ruleId, why) => {
				await expectRefusedBecause(filePath, code, ruleId, why);
			});

			it.each(allowed)('allows %s', async (_label, code) => {
				await expectAllowed(filePath, code);
			});
		});

		/** A component whose script declares `s`, then `markup`. */
		const component = (markup: string): string =>
			`<script lang="ts">\n\tconst s = 'x';\n</script>\n\n${markup}\n`;

		const httpEquivRefusals: readonly Allowed[] = [
			[
				'a meta refresh in the body',
				component(
					'<meta http-equiv="refresh" content="0;url=https://x.invalid/leak" />\n<p>{s}</p>'
				)
			],
			[
				'a meta refresh in svelte:head',
				component(
					'<svelte:head><meta http-equiv="refresh" content="0;url=https://x.invalid/leak" /></svelte:head>\n<p>{s}</p>'
				)
			],
			[
				'HTTP-EQUIV in svelte:head',
				component(
					'<svelte:head><meta HTTP-EQUIV="refresh" content="0" /></svelte:head>\n<p>{s}</p>'
				)
			],
			['http-equiv from an expression', component('<meta http-equiv={s} content="0" />')],
			[
				'http-equiv on svelte:element',
				component('<svelte:element this="meta" http-equiv="refresh" content={s} />')
			]
		];

		const svelteSets = [
			['the components', planted.componentSvelte],
			['the motion adapter', planted.motionSvelte],
			['the lazy loader', planted.lazySvelte],
			['the pages', planted.routesSvelte],
			['a gallery page', plannedOrReal.colourPage],
			['the frame', planted.gallerySvelte]
		] as const;

		describe.each(svelteSets)('in the markup of %s (%s)', (_set, filePath) => {
			it.each(httpEquivRefusals)('refuses %s', async (_label, code) => {
				await expectSyntaxMessage(filePath, code, httpEquivMessage);
			});
		});

		it.each([
			['the frame', planted.gallerySvelte],
			['a gallery page', plannedOrReal.colourPage]
		])('allows a meta description in the svelte:head of %s', async (_label, filePath) => {
			await expectAllowed(
				filePath,
				component('<svelte:head><meta name="description" content="x" /></svelte:head>\n<p>{s}</p>')
			);
		});

		it.each([
			['a check', planted.check],
			['a spec', planted.spec]
		])('allows createRawSnippet and DOMParser in %s', async (_label, filePath) => {
			await expectAllowed(
				filePath,
				"import { createRawSnippet } from 'svelte';\nexport const r = createRawSnippet(() => ({ render: () => '<b>x</b>' }));\nexport const d = new DOMParser().parseFromString('<b>x</b>', 'text/html');"
			);
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
	readonly motionImport: RuleEntry;
	readonly provide: RuleEntry;
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
		scriptUrl: rules[SCRIPT_URL],
		motionImport: rules[MOTION_IMPORT],
		provide: rules[PROVIDE]
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
		scriptUrl: [2],
		motionImport: rules.seam.motionImport ? [2] : undefined,
		provide: rules.seam.provideEnvironment ? [2] : undefined
	};
}

const noBoundary: BoundaryOptions = {
	globals: undefined,
	properties: undefined,
	imports: undefined,
	syntax: undefined,
	tiers: undefined,
	remote: undefined,
	scriptUrl: undefined,
	motionImport: undefined,
	provide: undefined
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
		['src/lib/environment/context.ts', 'environment'],
		['src/lib/environment/deep/X.svelte', 'environment'],
		['src/lib/environmental.ts', 'components'],
		['src/lib/controls/Button.svelte', 'components'],
		['src/gallery/x.ts', 'gallery'],
		['src/gallery/X.svelte', 'gallery'],
		['src/gallery/other.ts', 'gallery'],
		['src/gallery/skin-store.svelte.ts', 'gallery'],
		['src/gallery/deep/skin-store.ts', 'gallery'],
		['src/gallery/skin-store.ts', 'storage'],
		['src/routes/x.ts', 'routes'],
		['src/routes/components/button/+page.svelte', 'routes'],
		['src/routes/components/colour/+page.svelte', 'routes'],
		['src/lib/styles/environment.ts', 'components'],
		['src/stories/x.ts', 'stories'],
		['src/stories/frame/StoryFrame.svelte', 'stories'],
		['src/stories/controls/Button.stories.svelte', 'stories'],
		['src/stories/fixtures/people.ts', 'stories'],
		['.storybook/preview.ts', 'storybookBrowser'],
		['.storybook/manager.ts', 'storybookBrowser']
	] as const)('puts %s under the %s rules', async (filePath, name) => {
		expect(await effectiveBoundary(filePath)).toEqual(expectedBoundary(name));
	});

	it.each([
		['src/checks/x.spec.ts'],
		['src/checks/helper.ts'],
		['src/lib/x.spec.ts'],
		['src/lib/X.svelte.spec.ts'],
		['src/lib/motion/x.spec.ts'],
		['src/lib/environment/context.spec.ts'],
		['src/lib/environment/X.svelte.spec.ts'],
		['src/gallery/x.spec.ts'],
		['src/gallery/skin-store.spec.ts'],
		['src/routes/x.svelte.spec.ts'],
		['src/stories/frame/globals.spec.ts'],
		['src/stories/X.svelte.spec.ts']
	])('leaves the check or spec %s free of the boundary rules', async (filePath) => {
		expect(await effectiveBoundary(filePath)).toEqual(noBoundary);
	});

	it.each([['.storybook/main.ts'], ['.storybook/vitest.setup.ts']])(
		"leaves Storybook's Node-side %s free of the boundary rules",
		async (filePath) => {
			expect(fileSetOf(filePath)).toBeUndefined();
			expect(await effectiveBoundary(filePath)).toEqual(noBoundary);
		}
	);

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
		'src/lib/environment/deep/x.js',
		'src/lib/environment/context.spec.ts',
		'src/routes/components/button/+page.svelte',
		'src/checks/deep/x.svelte',
		'src/lib/X.svelte.spec.ts',
		'src/routes/x.svelte.spec.ts',
		'src/x.mjs',
		'src/gallery/x.cts',
		...Object.values(plannedOrReal),
		'src/gallery/other.ts',
		'src/gallery/skin-store.spec.ts',
		'src/gallery/skin-store.svelte.ts',
		'src/gallery/deep/skin-store.ts',
		'src/stories/deep/X.stories.svelte',
		'src/stories/skin-store.ts',
		'.storybook/preview.ts',
		'.storybook/manager.ts'
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

	it("lints Storybook's config files clean", async () => {
		const results = await realLinter.lintFiles([...storybookConfigFiles]);
		const problems = results.flatMap((result) =>
			result.messages.map(
				(message) =>
					`${result.filePath}:${String(message.line)} ${message.ruleId ?? ''} ${message.message}`
			)
		);
		expect(results.map((result) => path.relative(uiRoot, result.filePath)).sort()).toEqual(
			[...storybookConfigFiles].sort()
		);
		expect(problems).toEqual([]);
	});
});

/** A component: its script holds `body`, then `markup` follows. */
const component = (body: string, markup: string): string =>
	`<script lang="ts">\n${body}\n</script>\n\n${markup}\n`;

/** A component taking one `label` prop, destructured as `binding`. */
const withProps = (binding: string, markup: string): string =>
	component(
		`\tinterface Props {\n\t\treadonly label: string;\n\t\treadonly value?: string | undefined;\n\t}\n\tlet ${binding}: Props = $props();`,
		markup
	);

/** Every shipped set's plain module, and the component that stands for its .svelte files. */
const shippedModules = [
	['a control module', plannedOrReal.controlTypes],
	['the seam', plannedOrReal.context],
	['the motion adapter', planted.motion],
	['the lazy loader', planted.lazy]
] as const;

/** The host's files, each with the message its sets refuse with. */
const hostFiles = [
	['a gallery page', plannedOrReal.togglePage],
	['a page module', planted.routes],
	['the frame', planted.gallery],
	["the gallery's stores", plannedOrReal.skinStore],
	['a stories module', planted.stories],
	['a stories file', planted.storiesSvelte],
	["Storybook's preview", real.storybookPreview],
	["Storybook's manager", real.storybookManager]
] as const;

describe("the pattern's bans in what ships", { timeout: 120_000 }, () => {
	describe('props come in named and read-only (P5)', () => {
		it.each([
			[
				'$bindable()',
				withProps('{ label, value = $bindable() }', '<p>{label}{value}</p>'),
				[SYNTAX]
			],
			['a ...rest prop', withProps('{ label, ...rest }', '<p>{label}{rest.value}</p>'), [SYNTAX]],
			['a rest element alone', withProps('{ ...all }', '<p>{all.label}</p>'), [SYNTAX]],
			[
				'let props = $props() with {...props}',
				withProps('props', '<p {...props}>{props.label}</p>'),
				[SYNTAX, SPREADS]
			]
		] as const)('refuses %s in a control', async (_label, code, ruleIds) => {
			const messages = await lintPlanted(plannedOrReal.button, code);
			const fired = messages.map((message) => message.ruleId);
			for (const ruleId of ruleIds) expect(fired).toContain(ruleId);
		});

		it.each([
			['destructured props', withProps('{ label, value }', '<p>{label}{value}</p>')],
			[
				'$props.id() beside them',
				component(
					'\tinterface Props {\n\t\treadonly label: string;\n\t}\n\tlet { label }: Props = $props();\n\tconst id = $props.id();',
					'<p {id}>{label}</p>'
				)
			]
		])('allows %s', async (_label, code) => {
			await expectAllowed(plannedOrReal.button, code);
		});
	});

	describe("no stores and no context of a component's own (P6, P7)", () => {
		const contextImports = [
			'setContext',
			'getContext',
			'hasContext',
			'getAllContexts',
			'createContext'
		].map(
			(name) => [name, `import { ${name} } from 'svelte';\nexport const r = ${name};`] as const
		);

		it.each([
			['svelte/store', "import { writable } from 'svelte/store';\nexport const r = writable(0);"],
			[
				'svelte/store by type',
				"import type { Readable } from 'svelte/store';\nexport type R = Readable<number>;"
			],
			...contextImports,
			[
				'svelte as a namespace',
				"import * as svelte from 'svelte';\nexport const r = svelte.untrack;"
			]
		] as const)('refuses %s in a control module', async (_label, code) => {
			await expectRefused(plannedOrReal.controlTypes, code, IMPORTS);
		});

		it.each(shippedModules.filter(([, filePath]) => filePath !== plannedOrReal.context))(
			'refuses setContext in %s',
			async (_label, filePath) => {
				await expectRefused(
					filePath,
					"import { setContext } from 'svelte';\nexport const r = setContext;",
					IMPORTS
				);
			}
		);

		it.each(shippedModules)('refuses svelte/store in %s', async (_label, filePath) => {
			await expectRefused(
				filePath,
				"import { readable } from 'svelte/store';\nexport const r = readable(0);",
				IMPORTS
			);
		});

		it('refuses setContext in Button', async () => {
			await expectRefused(
				plannedOrReal.button,
				component("\timport { setContext } from 'svelte';\n\tsetContext('k', 1);", '<p>x</p>'),
				IMPORTS
			);
		});

		it.each(hostFiles)('refuses getContext in %s', async (_label, filePath) => {
			const code = filePath.endsWith('.svelte')
				? component(
						"\timport { getContext } from 'svelte';\n\tconst r = getContext('k');",
						'<p>{r}</p>'
					)
				: "import { getContext } from 'svelte';\nexport const r = getContext('k');";
			await expectRefused(filePath, code, IMPORTS);
		});

		it('allows the context functions in the seam', async () => {
			await expectAllowed(
				plannedOrReal.context,
				"import { getContext, hasContext, setContext } from 'svelte';\nexport const r = [getContext, hasContext, setContext];"
			);
		});

		it('allows setContext in a check', async () => {
			await expectAllowed(
				planted.check,
				"import { setContext } from 'svelte';\nexport const r = setContext;"
			);
		});
	});

	describe('only the host and the checks take provideEnvironment (P7)', () => {
		const refused: readonly PathCase[] = [
			[
				'Button, from the context module',
				plannedOrReal.button,
				component(
					"\timport { provideEnvironment, STILL_ENVIRONMENT } from '../environment/context';\n\tprovideEnvironment(STILL_ENVIRONMENT);",
					'<p>x</p>'
				)
			],
			[
				'Button, by its .ts name',
				plannedOrReal.button,
				component(
					"\timport { provideEnvironment } from '../environment/context.ts';\n\tconst r = provideEnvironment;",
					'<p>{r}</p>'
				)
			],
			[
				'Button, renamed, from the folder',
				plannedOrReal.button,
				component(
					"\timport { provideEnvironment as provide } from '../environment';\n\tconst r = provide;",
					'<p>{r}</p>'
				)
			],
			[
				'Button, through the public entry',
				plannedOrReal.button,
				component(
					"\timport { provideEnvironment } from '@zurfur/ui';\n\tconst r = provideEnvironment;",
					'<p>{r}</p>'
				)
			],
			[
				'Button, as a namespace',
				plannedOrReal.button,
				component(
					"\timport * as seam from '../environment/context';\n\tconst r = seam;",
					'<p>{r}</p>'
				)
			],
			[
				'a control module, re-exported',
				plannedOrReal.controlTypes,
				"export { provideEnvironment } from '../environment/context.ts';"
			],
			[
				'a control module, with export *',
				plannedOrReal.controlTypes,
				"export * from '../environment/context.ts';"
			],
			[
				'the motion adapter',
				planted.motion,
				"import { provideEnvironment } from '../environment/context.ts';\nexport const r = provideEnvironment;"
			],
			[
				'the lazy loader',
				planted.lazy,
				"import { provideEnvironment } from '../environment/context.ts';\nexport const r = provideEnvironment;"
			],
			[
				'the public entry, imported rather than re-exported',
				real.publicEntry,
				"import { provideEnvironment } from './environment/context.ts';\nexport const r = provideEnvironment;"
			]
		];

		it.each(refused)('refuses it in %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, PROVIDE);
		});

		const allowed: readonly PathCase[] = [
			[
				'useEnvironment in Button',
				plannedOrReal.button,
				component(
					"\timport { useEnvironment } from '../environment/context';\n\tconst { moves } = useEnvironment();",
					'<button {@attach moves.squish()}>x</button>'
				)
			],
			[
				"the public entry's re-export",
				real.publicEntry,
				"export { provideEnvironment, STILL_ENVIRONMENT } from './environment/context.ts';"
			],
			[
				'a gallery page, through the public entry',
				plannedOrReal.togglePage,
				component(
					"\timport { provideEnvironment } from '@zurfur/ui';\n\tconst r = provideEnvironment;",
					'<p>{r}</p>'
				)
			],
			[
				'a check',
				planted.check,
				"import { provideEnvironment } from '../lib/environment/context.ts';\nexport const r = provideEnvironment;"
			]
		];

		it.each(allowed)('allows %s', async (_label, filePath, code) => {
			await expectAllowed(filePath, code);
		});
	});

	describe('no module-level state in a shipped module (P6)', () => {
		const refused: readonly Allowed[] = [
			['a module-level let', 'let count = 0;\nexport const r = () => (count += 1);'],
			['an exported let', 'export let count = 0;'],
			['a module-level var', 'var count = 0;\nexport const r = () => count;'],
			['a module-level new Map()', 'export const cache = new Map<string, number>();'],
			['a module-level new Set()', 'const seen = new Set<string>();\nexport const r = seen;'],
			['a module-level new WeakMap()', 'export const r = new WeakMap<object, number>();'],
			['a module-level new WeakSet()', 'export const r = new WeakSet<object>();'],
			[
				'a new Map() in a class field',
				'export class Cache {\n\treadonly entries = new Map<string, number>();\n}'
			],
			['$state(', 'export const r = $state(0);'],
			['$derived(', 'export const r = $derived(1);'],
			['$state.raw(', 'export const r = $state.raw([]);'],
			['$derived.by(', 'export const r = $derived.by(() => 1);']
		];

		it.each(
			shippedModules.flatMap(([set, filePath]) =>
				refused.map(([label, code]) => [`${label} in ${set}`, filePath, code] as const)
			)
		)('refuses %s', async (_label, filePath, code) => {
			await expectRefused(filePath, code, SYNTAX);
		});

		it('refuses $state( in a runes module of the motion adapter', async () => {
			await expectRefused(planted.runesModule, 'export const r = $state(0);', SYNTAX);
		});

		it('refuses a rune in a class field of a runes module', async () => {
			await expectRefused(
				planted.runesModule,
				'export class Count {\n\tvalue = $state(0);\n}',
				SYNTAX
			);
		});

		it('allows a rune inside a function of a runes module, which is per call', async () => {
			await expectAllowed(
				planted.runesModule,
				'export function counter(): { readonly read: () => number } {\n\tconst count = $state(0);\n\tconst doubled = $derived(count * 2);\n\treturn { read: () => doubled };\n}'
			);
		});

		const allowed: readonly Allowed[] = [
			['a frozen constant', "export const NAMES: readonly string[] = Object.freeze(['a', 'b']);"],
			[
				'a let and a new Map() inside a function',
				'export function count(names: readonly string[]): number {\n\tconst seen = new Map<string, number>();\n\tlet total = 0;\n\tfor (const name of names) {\n\t\tseen.set(name, 1);\n\t\ttotal += 1;\n\t}\n\treturn total + seen.size;\n}'
			],
			[
				'a new Set() in an arrow function',
				'export const r = (xs: readonly string[]) => new Set(xs);'
			]
		];

		it.each(allowed)('allows %s in a control module', async (_label, code) => {
			await expectAllowed(plannedOrReal.controlTypes, code);
		});

		it("allows state and a Map in a component's own script", async () => {
			await expectAllowed(
				plannedOrReal.button,
				component(
					'\tlet count = $state(0);\n\tconst doubled = $derived(count * 2);\n\tconst cache = new Map<string, number>();\n\tconst press = (): void => {\n\t\tcount += 1;\n\t};',
					'<button onclick={press}>{doubled}{cache.size}</button>'
				)
			);
		});
	});

	describe('no <script module> in what ships (P6)', () => {
		const moduleBlock =
			'<script lang="ts" module>\n\texport const m = 1;\n</script>\n\n<p>{m}</p>\n';
		const contextModule =
			'<script lang="ts" context="module">\n\texport const m = 1;\n</script>\n\n<p>{m}</p>\n';

		it.each([
			['<script module> in Button', plannedOrReal.button, moduleBlock],
			['context="module" in Button', plannedOrReal.button, contextModule],
			['<script module> in the motion adapter', planted.motionSvelte, moduleBlock],
			['<script module> in the lazy loader', planted.lazySvelte, moduleBlock]
		])('refuses %s', async (_label, filePath, code) => {
			await expectSyntaxMessage(filePath, code, scriptModuleMessage);
		});
	});

	describe('no clock reads in what ships (P6)', () => {
		const refused: readonly Case[] = [
			['Date.now()', 'export const r = Date.now();', PROPERTIES],
			['performance.now()', 'export const r = performance.now();', PROPERTIES],
			['Temporal.Now', 'export const r: unknown = Temporal.Now;', PROPERTIES],
			['window.performance.now()', 'export const r = window.performance.now();', SYNTAX],
			['globalThis.Date.now()', 'export const r = globalThis.Date.now();', SYNTAX],
			['new Date()', 'export const r = new Date();', SYNTAX],
			['new Date', 'export const r = new Date;', SYNTAX],
			['new window.Date()', 'export const r = new window.Date();', SYNTAX],
			['Date() called bare', "export const r = Date('x');", SYNTAX],
			['self.Date()', 'export const r = self.Date();', SYNTAX],
			['a destructured Date.now', 'const { now } = Date;\nexport const r = now;', PROPERTIES]
		];

		it.each(
			shippedModules.flatMap(([set, filePath]) =>
				refused.map(
					([label, code, ruleId]) => [`${label} in ${set}`, filePath, code, ruleId] as const
				)
			)
		)('refuses %s', async (_label, filePath, code, ruleId) => {
			await expectRefused(filePath, code, ruleId);
		});

		it('refuses Date.now() in a control', async () => {
			await expectRefused(
				plannedOrReal.button,
				component('\tconst at = Date.now();', '<p>{at}</p>'),
				PROPERTIES
			);
		});

		it.each([
			['a given instant', 'export const r = new Date(0);'],
			['a parsed date', "export const r = Date.parse('2026-10-26');"],
			['a date from a prop', 'export const r = (iso: string) => new Date(iso);']
		])('allows %s', async (_label, code) => {
			await expectAllowed(plannedOrReal.controlTypes, code);
		});

		it('allows Date.now() in the host', async () => {
			await expectAllowed(planted.routes, 'export const r = Date.now();');
		});
	});

	describe('components move only through the seam', () => {
		const declaringMoves =
			'\tconst on = true;\n\tconst fade = (node: Element) => ({ duration: node.clientWidth });\n\tconst items = [1, 2];';

		const refused: readonly Allowed[] = [
			['transition:', component(declaringMoves, '<p transition:fade>{on}</p>')],
			['in:', component(declaringMoves, '<p in:fade>{on}</p>')],
			['out:', component(declaringMoves, '<p out:fade>{on}</p>')],
			[
				'animate:',
				component(
					declaringMoves,
					'<ul>{#each items as item (item)}<li animate:fade>{item}</li>{/each}</ul>'
				)
			],
			['a move class as text', component(declaringMoves, '<p class="zf-pill hop">{on}</p>')],
			['a move class as class:', component(declaringMoves, '<p class:paperin={on}>x</p>')],
			[
				'a move class in a list',
				component(declaringMoves, "<p class={['zf-pill', 'paperlift']}>{on}</p>")
			],
			['a move class as a key', component(declaringMoves, '<p class={{ deskdim: on }}>x</p>')]
		];

		it.each(refused)('refuses %s in Button', async (_label, code) => {
			await expectSyntaxMessage(plannedOrReal.button, code, seamMotionMessage);
		});

		it.each(refused)('refuses %s in the lazy loader', async (_label, code) => {
			await expectSyntaxMessage(planted.lazySvelte, code, seamMotionMessage);
		});

		it.each([
			['the seam', component(declaringMoves, '<p class="zf-tip">{on}{items.length}{fade}</p>')],
			[
				'names that merely hold a move',
				component(
					declaringMoves,
					'<p class="zf-hopper paperlife-shadowy">{on}{items.length}{fade}</p>'
				)
			]
		])('allows %s', async (_label, code) => {
			await expectAllowed(plannedOrReal.button, code);
		});

		it('allows a transition in the motion adapter', async () => {
			await expectAllowed(
				planted.motionSvelte,
				component("\timport { fade } from 'svelte/transition';", '<p transition:fade>x</p>')
			);
		});

		it('allows a move class on a gallery page', async () => {
			await expectAllowed(
				plannedOrReal.togglePage,
				component('\tconst s = 1;', '<p class="hop">{s}</p>')
			);
		});

		const motionImports: readonly PathCase[] = [
			[
				'Button, by path',
				plannedOrReal.button,
				component(
					"\timport { PAPER_MOVES } from '../motion/paper-moves.ts';",
					'<p>{PAPER_MOVES[0]}</p>'
				)
			],
			[
				'Button, a type',
				plannedOrReal.button,
				component(
					"\timport type { PaperMove } from '../motion/paper-moves';\n\tconst m: PaperMove = 'hop';",
					'<p>{m}</p>'
				)
			],
			[
				'Button, the stylesheet',
				plannedOrReal.button,
				component("\timport '../motion/paper-moves.css';", '<p>x</p>')
			],
			[
				'Button, through the public entry',
				plannedOrReal.button,
				component("\timport { PAPER_MOVES } from '@zurfur/ui';", '<p>{PAPER_MOVES[0]}</p>')
			],
			[
				'a control module re-exporting it',
				plannedOrReal.controlTypes,
				"export { PAPER_MOVES } from '../motion/paper-moves.ts';"
			],
			[
				'the seam',
				plannedOrReal.context,
				"import { PAPER_MOVES } from '../motion/paper-moves.ts';\nexport const r = PAPER_MOVES;"
			],
			['the lazy loader', planted.lazy, "import { y } from '../motion/y';\nexport { y };"]
		];

		it.each(motionImports)(
			'refuses an import of the motion adapter in %s',
			async (_label, filePath, code) => {
				await expectRefused(filePath, code, MOTION_IMPORT);
			}
		);

		it.each([
			[
				"the public entry's re-export",
				real.publicEntry,
				"export { PAPER_MOVES, type PaperMove } from './motion/paper-moves.ts';"
			],
			["the styles entry's stylesheet", real.stylesEntry, "import '../motion/paper-moves.css';"],
			['the adapter importing itself', planted.motion, "import { y } from './y';\nexport { y };"]
		] as const)('allows %s', async (_label, filePath, code) => {
			await expectAllowed(filePath, code);
		});
	});

	describe('the seam imports only svelte, svelte/attachments and its own files', () => {
		it.each([
			['gsap', "import { gsap } from 'gsap';\nexport const r = gsap;"],
			[
				'the motion adapter',
				"import { PAPER_MOVES } from '../motion/paper-moves.ts';\nexport const r = PAPER_MOVES;"
			],
			[
				'a control module',
				"import type { ButtonVariant } from '../controls/types';\nexport type R = ButtonVariant;"
			],
			['a path that climbs out', "import { y } from './../controls/y';\nexport const r = y;"],
			[
				'svelte in another letter case',
				"import { untrack } from 'Svelte';\nexport const r = untrack;"
			],
			['svelte/store', "import { writable } from 'svelte/store';\nexport const r = writable;"],
			['svelte/transition', "import { fade } from 'svelte/transition';\nexport const r = fade;"],
			['bits-ui', "import { Switch } from 'bits-ui';\nexport const r = Switch;"],
			['the public entry', "import { Icon } from '@zurfur/ui';\nexport const r = Icon;"]
		])('refuses %s', async (_label, code) => {
			await expectRefused(plannedOrReal.context, code, IMPORTS);
		});

		it.each([
			[
				'the context functions',
				"import { getContext, hasContext, setContext } from 'svelte';\nexport const r = [getContext, hasContext, setContext];"
			],
			[
				'the attachment type',
				"import type { Attachment } from 'svelte/attachments';\nexport type R = Attachment<HTMLElement>;"
			],
			[
				'its own files',
				"import { y } from './y';\nimport { z } from './z.ts';\nexport const r = [y, z];"
			]
		])('allows %s', async (_label, code) => {
			await expectAllowed(planted.environment, code);
		});
	});

	describe('the host shows controls only through the public entry', () => {
		const wrapped = [
			['bits-ui', "import { Switch } from 'bits-ui';\nexport const r = Switch;"],
			['a bits-ui subpath', "import 'bits-ui/x';"],
			[
				'@internationalized/date',
				"import { CalendarDate } from '@internationalized/date';\nexport const r = CalendarDate;"
			],
			['another @internationalized package', "import '@internationalized/number';"]
		] as const;

		it.each(
			hostFiles.flatMap(([set, filePath]) =>
				wrapped.map(([name, line]) => [`${name} in ${set}`, filePath, line] as const)
			)
		)('refuses %s', async (_label, filePath, line) => {
			const code = filePath.endsWith('.svelte')
				? component(`\t${line.replaceAll('\n', '\n\t')}`, '<p>x</p>')
				: line;
			await expectRefused(filePath, code, IMPORTS);
		});
	});

	describe('a spread carries only what bits-ui hands a {#snippet child(…)}', () => {
		const head =
			"\timport { Switch, Popover } from 'bits-ui';\n\tconst extra = { ONERROR: 'fetch(1)' };\n\tconst tag = 'p';";
		const inToggle = (markup: string): string => component(head, markup);
		const handOffSnippet =
			'{#snippet child({ props }: { readonly props: Record<string, unknown> })}<a {...props}>x</a>{/snippet}';

		const refused: readonly Allowed[] = [
			['<img {...extra}>', inToggle('<img alt="" src="./a.png" {...extra} /><p>{tag}</p>')],
			['<Switch.Root {...rest}>', inToggle('<Switch.Root {...extra}></Switch.Root><p>{tag}</p>')],
			[
				'a child prop spread inside a nested snippet',
				inToggle(
					'<Switch.Root>{#snippet child({ props })}{#snippet inner()}<button {...props}>x</button>{/snippet}{@render inner()}{/snippet}</Switch.Root><p>{tag}{extra.ONERROR}</p>'
				)
			],
			[
				'a spread of props.x',
				inToggle(
					'<Switch.Root>{#snippet child({ props })}<button {...props.x}>x</button>{/snippet}</Switch.Root><p>{tag}{extra.ONERROR}</p>'
				)
			],
			[
				'a spread inside {#snippet other({ props })}',
				inToggle(
					'<Switch.Root>{#snippet other({ props })}<button {...props}>x</button>{/snippet}</Switch.Root><p>{tag}{extra.ONERROR}</p>'
				)
			],
			[
				'the child props spread onto a component',
				inToggle(
					'<Switch.Root>{#snippet child({ props })}<Switch.Thumb {...props} />{/snippet}</Switch.Root><p>{tag}{extra.ONERROR}</p>'
				)
			],
			[
				'a name the each block shadows',
				inToggle(
					'<Switch.Root>{#snippet child({ props })}<span {...props}>x</span>{#each [extra] as props (props)}<button {...props}>x</button>{/each}{/snippet}</Switch.Root><p>{tag}</p>'
				)
			],
			[
				'a child parameter that is not destructured',
				inToggle(
					'<Switch.Root>{#snippet child(props)}<button {...props}>x</button>{/snippet}</Switch.Root><p>{tag}{extra.ONERROR}</p>'
				)
			],
			[
				'another name from the child snippet',
				inToggle(
					'<Switch.Root>{#snippet child({ checked })}<button {...checked}>x</button>{/snippet}</Switch.Root><p>{tag}{extra.ONERROR}</p>'
				)
			],
			[
				'<svelte:element> outside the card',
				inToggle('<svelte:element this={tag}>{extra.ONERROR}</svelte:element><Switch.Root />')
			],
			[
				'a top-level child snippet handed as trigger to our own component',
				component(
					"\timport Tooltip from './Tooltip.svelte';\n\tconst noop = (): void => {};",
					`${handOffSnippet}\n<Tooltip words="x" trigger={child} onPress={noop} />`
				)
			],
			[
				'a top-level child snippet rendered by hand',
				component(
					'\tinterface Props {\n\t\treadonly raw: string;\n\t\treadonly label: string;\n\t}\n\tlet { raw, label }: Props = $props();',
					`${handOffSnippet}\n{@render child({ props: { href: raw, ONCLICK: label, ping: raw } })}`
				)
			],
			[
				'a snippet passed as {child} to a bits-ui root and also rendered by hand',
				inToggle(
					'{#snippet child({ props })}<button {...props}>{tag}</button>{/snippet}<Switch.Root {child} />{@render child({ props: extra })}'
				)
			],
			[
				'a snippet passed as {child} to a bits-ui root and also named in the script',
				component(
					`${head}\n\tconst kept = child;`,
					'{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}{kept}</button>{/snippet}<Switch.Root {child} />'
				)
			],
			[
				'a snippet passed to a bits-ui root as another prop',
				inToggle(
					'{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}</button>{/snippet}<Switch.Root thumb={child} />'
				)
			],
			[
				'a child snippet under a bits-ui root that also renders it by hand',
				inToggle(
					'<Switch.Root>{#snippet child({ props })}<button {...props}>{tag}</button>{/snippet}{@render child({ props: extra })}</Switch.Root>'
				)
			],
			[
				'a child snippet under a component that is not from bits-ui',
				component(
					"\timport Tooltip from './Tooltip.svelte';\n\tconst noop = (): void => {};",
					'<Tooltip words="x" onPress={noop}>{#snippet child({ props })}<button {...props}>x</button>{/snippet}</Tooltip>'
				)
			],
			[
				'a child snippet under an alias of a bits-ui namespace',
				component(
					`${head}\n\tconst S = Switch;`,
					'<S.Root>{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}</button>{/snippet}</S.Root>'
				)
			],
			[
				'a snippet passed as {child} to an alias of a bits-ui namespace',
				component(
					`${head}\n\tconst S = Switch;`,
					'{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}</button>{/snippet}<S.Root {child} />'
				)
			],
			[
				'a child snippet under a same-named component that is not from bits-ui',
				component(
					"\timport Switch from './PillSwitch.svelte';",
					'<Switch.Root>{#snippet child({ props })}<button {...props}>x</button>{/snippet}</Switch.Root>'
				)
			]
		];

		it.each(refused)('refuses %s in a control', async (_label, code) => {
			await expectRefused(plannedOrReal.toggle, code, SPREADS);
		});

		it.each([
			['a page', planted.routesSvelte],
			['the frame', planted.gallerySvelte],
			['a stories file', planted.storiesSvelte],
			['a check', planted.checkSvelte],
			["a check's harness", plannedOrReal.recording]
		])('refuses a spread and <svelte:element> in %s', async (_label, filePath) => {
			const code = component(
				"\tconst extra = { title: 'x' };\n\tconst tag = 'p';",
				'<p {...extra}>x</p><svelte:element this={tag}>y</svelte:element>'
			);
			const messages = await lintPlanted(filePath, code);
			expect(messages.filter((message) => message.ruleId === SPREADS)).toHaveLength(2);
		});

		it.each([
			[
				'{#snippet child({ props })}<button {...props}>',
				inToggle(
					'<Switch.Root>{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}</button>{/snippet}</Switch.Root>'
				)
			],
			[
				'{#snippet child({ wrapperProps })}<div {...wrapperProps}>',
				inToggle(
					'<Popover.Root><Popover.Content>{#snippet child({ wrapperProps, props })}<div {...wrapperProps}><div {...props}>{tag}{extra.ONERROR}</div></div>{/snippet}</Popover.Content></Popover.Root>'
				)
			],
			[
				'the child props renamed',
				inToggle(
					'<Switch.Root>{#snippet child({ props: own })}<button {...own}>{tag}{extra.ONERROR}</button>{/snippet}</Switch.Root>'
				)
			],
			[
				'a top-level snippet passed as {child} to a bits-ui root',
				inToggle(
					'{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}</button>{/snippet}<Switch.Root {child} /><Switch.Root {child} />'
				)
			],
			[
				'a top-level snippet passed as child={child} to a bits-ui root',
				inToggle(
					'{#snippet child({ props })}<button {...props}>{tag}{extra.ONERROR}</button>{/snippet}<Switch.Root child={child} />'
				)
			]
		])('allows %s', async (_label, code) => {
			await expectAllowed(plannedOrReal.toggle, code);
		});

		it("allows IconButton's hand-off to Tooltip in IconButton alone", async () => {
			const iconButton = readFileSync(`${uiRoot}${plannedOrReal.iconButton}`, 'utf8');
			expect(iconButton).toContain('trigger={child}');
			await expectAllowed(plannedOrReal.iconButton, iconButton);
			await expectRefused(plannedOrReal.toggle, iconButton, SPREADS);
		});

		it.each([
			[
				'rendered by hand',
				'{#snippet child({ props })}<button {...props}>x</button>{/snippet}\n{@render child({ props: { title: label } })}'
			],
			[
				'handed to Tooltip as another prop',
				'{#snippet child({ props })}<button {...props}>x</button>{/snippet}\n<Tooltip words={label} trigger={child} label={child} onPress={noop} />'
			],
			[
				'handed as trigger to another component of ours',
				'{#snippet child({ props })}<button {...props}>x</button>{/snippet}\n<Toggle label={label} trigger={child} />'
			]
		])('refuses a child snippet %s in IconButton', async (_label, markup) => {
			const code = component(
				"\timport Tooltip from './Tooltip.svelte';\n\timport Toggle from './Toggle.svelte';\n\tconst label = 'x';\n\tconst noop = (): void => {};",
				markup
			);
			await expectRefused(plannedOrReal.iconButton, code, SPREADS);
		});
	});

	it("switches off Kit's resolve() rule for Button's link alone", async () => {
		const ruleOf = async (filePath: string): Promise<unknown> => {
			const config = (await realLinter.calculateConfigForFile(filePath)) as Linter.Config;
			const entry = config.rules?.['svelte/no-navigation-without-resolve'];
			return Array.isArray(entry) ? entry[0] : entry;
		};
		expect(await ruleOf(plannedOrReal.button)).toBe(0);
		expect(await ruleOf(plannedOrReal.checkbox)).toBe(2);
	});
});
