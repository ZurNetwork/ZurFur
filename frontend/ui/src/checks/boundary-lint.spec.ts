import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { boundaryRules } from '../../eslint.config.js';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** Planted files: virtual paths, one per file set, that never exist on disk. */
const planted = {
	component: 'src/lib/x.ts',
	componentSvelte: 'src/lib/X.svelte',
	stray: 'src/elsewhere.ts',
	motion: 'src/lib/motion/x.ts',
	lazy: 'src/lib/lazy/x.ts',
	gallery: 'src/gallery/x.ts',
	routes: 'src/routes/x.ts'
} as const;

const plantedPaths = Object.values(planted);

/**
 * The real config, unchanged but for one parser option: the project service
 * refuses a file that isn't on disk, so the planted paths parse against the
 * package's tsconfig as a default project instead.
 */
const plantedOverride: Linter.Config = {
	languageOptions: {
		parserOptions: {
			projectService: { allowDefaultProject: plantedPaths, defaultProject: 'tsconfig.json' }
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
	'svelte/no-at-html-tags'
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

	describe('the components (src/lib)', () => {
		it.each(componentRefusals)('refuses %s', async (_label, code, ruleId) => {
			await expectRefused(planted.component, code, ruleId);
		});

		it.each([
			['a peer dependency', "import { untrack } from 'svelte';\nexport { untrack };"],
			['a relative import', "import { y } from './y';\nexport { y };"],
			['as const', "export const r = ['a'] as const;"]
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
		['$gallery/…', "import { groupNames } from '$gallery/groups';\nexport { groupNames };"]
	];

	const hostRefusals: readonly Case[] = [
		['fetch', "export const r = fetch('/x');", GLOBALS],
		['new WebSocket', "export const r = new WebSocket('wss://x');", GLOBALS],
		['sessionStorage', 'export const r = sessionStorage;', GLOBALS],
		['document.cookie', 'export const r = document.cookie;', PROPERTIES],
		['process.env', 'export const r = process.env.X;', GLOBALS],
		['import.meta.env', 'export const r: unknown = import.meta.env;', SYNTAX],
		['$lib/y', "import { x } from '$lib/y';\nexport { x };", IMPORTS],
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
});

type RuleEntry = Linter.RuleEntry | undefined;

interface BoundaryOptions {
	readonly globals: RuleEntry;
	readonly properties: RuleEntry;
	readonly imports: RuleEntry;
	readonly syntax: RuleEntry;
}

async function effectiveBoundary(filePath: string): Promise<BoundaryOptions> {
	const config = (await realLinter.calculateConfigForFile(filePath)) as Linter.Config;
	const rules = config.rules ?? {};
	return {
		globals: rules['no-restricted-globals'],
		properties: rules['no-restricted-properties'],
		imports: rules['no-restricted-imports'],
		syntax: rules['no-restricted-syntax']
	};
}

type SetName = keyof typeof boundaryRules;

function expectedBoundary(name: SetName): BoundaryOptions {
	const rules = boundaryRules[name];
	return {
		globals: [2, { globals: rules.globals, checkGlobalObject: true }],
		properties: [2, ...rules.properties],
		imports: [2, { patterns: rules.imports }],
		syntax: [2, ...rules.syntax]
	};
}

const noBoundary: BoundaryOptions = {
	globals: undefined,
	properties: undefined,
	imports: undefined,
	syntax: undefined
};

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
