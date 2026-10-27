/*
 * The load bans, over planted text: no timer in a component or the seam, no
 * load starter (Image, Audio, createElement, createElementNS) outside the
 * checks, the motion adapter building only spans, and no media markup or
 * picture-loading style in any .svelte file under src/. The HTML sinks these
 * sets refuse belong to the boundary rules; their cases here prove they hold.
 */
import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import {
	fileSetOf,
	loadRules,
	loadStarterMessage,
	mediaMessage,
	motionElementMessage,
	rawHtmlMessage,
	timerMessage
} from '../../eslint.config.js';

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

/** Planted files: virtual paths, one per file set, that never exist on disk. */
const planted = {
	component: 'src/lib/load-x.ts',
	componentSvelte: 'src/lib/LoadX.svelte',
	environment: 'src/lib/environment/load-x.ts',
	environmentSvelte: 'src/lib/environment/LoadX.svelte',
	motion: 'src/lib/motion/load-x.ts',
	motionSvelte: 'src/lib/motion/LoadX.svelte',
	lazy: 'src/lib/lazy/load-x.ts',
	gallery: 'src/gallery/load-x.ts',
	gallerySvelte: 'src/gallery/LoadX.svelte',
	routes: 'src/routes/load-x.ts',
	routesSvelte: 'src/routes/LoadX.svelte',
	stories: 'src/stories/load-x.ts',
	storiesSvelte: 'src/stories/LoadX.stories.svelte',
	check: 'src/checks/load-planted.ts',
	checkSvelte: 'src/checks/LoadX.svelte',
	spec: 'src/lib/load-planted.spec.ts'
} as const;

/** Real files a case lints planted text at; the file on disk is never written. */
const real = {
	skinStore: 'src/gallery/skin-store.ts',
	storybookPreview: '.storybook/preview.ts'
} as const;

const notOnDisk = (relative: string): boolean => !existsSync(`${uiRoot}${relative}`);

const plantedPaths = Object.values(planted);

/**
 * The real config, unchanged but for one parser option: the project service
 * refuses a file that isn't on disk, so the planted paths parse against the
 * package's tsconfig as a default project instead.
 */
const plantedOverride: Linter.Config = {
	languageOptions: {
		parserOptions: {
			projectService: {
				allowDefaultProject: plantedPaths,
				defaultProject: 'tsconfig.json',
				maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: plantedPaths.length
			}
		}
	}
};

const linter = new ESLint({ cwd: uiRoot, overrideConfig: plantedOverride });

const GLOBALS = 'ui-load/globals';
const PROPERTIES = 'ui-load/properties';
const SYNTAX = 'ui-load/syntax';
const MARKUP = 'ui-load/markup';

/** Every rule of the load bans; an allowed case fires none of them. */
const loadRuleIds: ReadonlySet<string> = new Set([GLOBALS, PROPERTIES, SYNTAX, MARKUP]);

async function lint(filePath: string, code: string): Promise<readonly Linter.LintMessage[]> {
	const [result] = await linter.lintText(code, { filePath });
	if (result === undefined) throw new Error(`no lint result for ${filePath}`);
	// The project service keeps the last text it was handed for a path.
	if (!notOnDisk(filePath)) {
		await linter.lintText(readFileSync(`${uiRoot}${filePath}`, 'utf8'), { filePath });
	}
	expect(result.messages.filter((message) => message.fatal === true)).toEqual([]);
	return result.messages;
}

/** Refused by `ruleId`, with a message that carries `why`. */
async function expectRefused(
	filePath: string,
	code: string,
	ruleId: string,
	why: string
): Promise<void> {
	const messages = await lint(filePath, code);
	const matching = messages.filter(
		(message) => message.ruleId === ruleId && message.message.includes(why)
	);
	expect(matching, JSON.stringify(messages.map((message) => message.ruleId))).not.toEqual([]);
}

async function expectAllowed(filePath: string, code: string): Promise<void> {
	const messages = await lint(filePath, code);
	const fired = messages.filter(
		(message) => message.ruleId !== null && loadRuleIds.has(message.ruleId)
	);
	expect(fired).toEqual([]);
}

/** A component whose script declares `s`, then `markup`. */
const component = (markup: string): string =>
	`<script lang="ts">\n\tconst s = 'x';\n</script>\n\n${markup}\n`;

type Case = readonly [label: string, code: string, ruleId: string, why: string];

describe('the load bans over planted text', { timeout: 120_000 }, () => {
	it('plants only at paths that are not on disk, and over real files only where they are', () => {
		expect(plantedPaths.filter((relative) => !notOnDisk(relative))).toEqual([]);
		expect(Object.values(real).filter(notOnDisk)).toEqual([]);
	});

	it.each([
		[planted.component, 'components'],
		[planted.environment, 'environment'],
		[planted.motion, 'motion'],
		[planted.lazy, 'lazy'],
		[planted.gallery, 'gallery'],
		[planted.routes, 'routes'],
		[planted.stories, 'stories'],
		[real.skinStore, 'storage'],
		[real.storybookPreview, 'storybookBrowser'],
		[planted.check, 'checks'],
		[planted.spec, 'checks']
	] as const)('plants %s in the %s set', (filePath, name) => {
		expect(fileSetOf(filePath)).toBe(name);
	});

	describe('no timer in a component or the seam', () => {
		const timers: readonly Case[] = [
			['setTimeout', 'export const r = setTimeout(() => undefined, 1);', GLOBALS, timerMessage],
			['setInterval', 'export const r = setInterval(() => undefined, 1);', GLOBALS, timerMessage],
			[
				'requestAnimationFrame',
				'export const r = requestAnimationFrame(() => undefined);',
				GLOBALS,
				timerMessage
			],
			[
				'requestIdleCallback',
				'export const r = requestIdleCallback(() => undefined);',
				GLOBALS,
				timerMessage
			],
			['scheduler', 'export const r = scheduler;', GLOBALS, timerMessage],
			[
				'window.setTimeout',
				'export const r = window.setTimeout(() => undefined, 1);',
				GLOBALS,
				timerMessage
			],
			[
				'globalThis.requestAnimationFrame',
				'export const r = globalThis.requestAnimationFrame(() => undefined);',
				GLOBALS,
				timerMessage
			],
			['AbortSignal.timeout', 'export const r = AbortSignal.timeout(1);', PROPERTIES, timerMessage]
		];

		it.each(timers)('refuses %s in a component', async (_label, code, ruleId, why) => {
			await expectRefused(planted.component, code, ruleId, why);
		});

		it.each(timers)('refuses %s in the seam', async (_label, code, ruleId, why) => {
			await expectRefused(planted.environment, code, ruleId, why);
		});

		it('refuses setTimeout in a component script', async () => {
			await expectRefused(
				planted.componentSvelte,
				'<script lang="ts">\n\tsetTimeout(() => undefined, 1);\n</script>\n\n<p>x</p>\n',
				GLOBALS,
				timerMessage
			);
		});

		it.each([
			['the host', planted.routes],
			['a check', planted.check]
		])('allows setTimeout in %s', async (_label, filePath) => {
			await expectAllowed(filePath, 'export const r = setTimeout(() => undefined, 1);');
		});
	});

	describe('no load starter outside the checks', () => {
		const starters: readonly Case[] = [
			['new Image()', 'export const r = new Image();', GLOBALS, loadStarterMessage],
			['Image() called bare', 'export const r = Image();', GLOBALS, loadStarterMessage],
			['new window.Image()', 'export const r = new window.Image();', GLOBALS, loadStarterMessage],
			[
				'document.createElement',
				"export const r = document.createElement('div');",
				PROPERTIES,
				loadStarterMessage
			],
			[
				'createElementNS',
				"export const r = document.createElementNS('http://www.w3.org/2000/svg', 'svg');",
				PROPERTIES,
				loadStarterMessage
			],
			[
				'a destructured createElement',
				'const { createElement } = document;\nexport const r = createElement;',
				PROPERTIES,
				loadStarterMessage
			]
		];

		it.each(starters)('refuses %s in a component', async (_label, code, ruleId, why) => {
			await expectRefused(planted.component, code, ruleId, why);
		});

		it.each(starters)('refuses %s in the seam', async (_label, code, ruleId, why) => {
			await expectRefused(planted.environment, code, ruleId, why);
		});

		it.each([
			['new Audio()', 'export const r = new Audio();'],
			['window.Audio', 'export const r = new window.Audio();']
		])('refuses %s in a component and the seam', async (_label, code) => {
			await expectRefused(planted.component, code, GLOBALS, loadStarterMessage);
			await expectRefused(planted.environment, code, GLOBALS, loadStarterMessage);
		});

		const hostAndLazy = [
			['a gallery file', planted.gallery],
			['a stories file', planted.stories],
			['a route', planted.routes],
			['the lazy loader', planted.lazy],
			["the gallery's store", real.skinStore],
			["Storybook's preview", real.storybookPreview]
		] as const;

		it.each(hostAndLazy)('refuses new Image() in %s', async (_label, filePath) => {
			await expectRefused(filePath, 'export const r = new Image();', GLOBALS, loadStarterMessage);
		});

		it.each(hostAndLazy)(
			'refuses createElement and createElementNS in %s',
			async (_l, filePath) => {
				await expectRefused(
					filePath,
					"export const r = document.createElement('img');",
					PROPERTIES,
					loadStarterMessage
				);
				await expectRefused(
					filePath,
					"export const r = document.createElementNS('http://www.w3.org/2000/svg', 'image');",
					PROPERTIES,
					loadStarterMessage
				);
			}
		);

		it.each([
			['a check', planted.check],
			['a spec', planted.spec]
		])('allows createElement, createElementNS, Image and Audio in %s', async (_label, filePath) => {
			await expectAllowed(
				filePath,
				"export const a = document.createElement('img');\nexport const b = document.createElementNS('http://www.w3.org/2000/svg', 'g');\nexport const c = new Image();\nexport const d = new Audio();"
			);
		});
	});

	describe('the motion adapter builds only spans', () => {
		it.each([
			["createElement('img')", "export const r = document.createElement('img');"],
			[
				'createElement(name)',
				"const name = 'span';\nexport const r = document.createElement(name);"
			],
			[
				"createElement('span', options)",
				"export const r = document.createElement('span', { is: 'x' });"
			],
			['createElement passed along', 'export const r = document.createElement;'],
			["document['createElement']('span')", "export const r = document['createElement']('span');"],
			[
				'createElementNS(…)',
				"export const r = document.createElementNS('http://www.w3.org/2000/svg', 'g');"
			],
			[
				'a destructured createElement',
				"const { createElement } = document;\nexport const r = createElement('span');"
			]
		])('refuses %s', async (_label, code) => {
			await expectRefused(planted.motion, code, SYNTAX, motionElementMessage);
		});

		it.each([
			['new Image()', 'export const r = new Image();'],
			['new Audio()', 'export const r = new Audio();']
		])('refuses %s', async (_label, code) => {
			await expectRefused(planted.motion, code, GLOBALS, loadStarterMessage);
		});

		it("allows createElement('span')", async () => {
			await expectAllowed(planted.motion, "export const r = document.createElement('span');");
		});

		it('allows a timer, which moves are made of', async () => {
			await expectAllowed(
				planted.motion,
				'export const r = requestAnimationFrame(() => undefined);'
			);
		});
	});

	describe('the HTML sinks stay refused (the boundary rules)', () => {
		const sinks: readonly (readonly [string, string, string])[] = [
			[
				'innerHTML',
				"export const r = (e: Element) => {\n\te.innerHTML = 'x';\n};",
				'no-restricted-properties'
			],
			[
				'outerHTML',
				"export const r = (e: Element) => {\n\te.outerHTML = 'x';\n};",
				'no-restricted-properties'
			],
			[
				'insertAdjacentHTML',
				"export const r = (e: Element) => {\n\te.insertAdjacentHTML('beforeend', 'x');\n};",
				'no-restricted-properties'
			],
			[
				'setHTMLUnsafe',
				"export const r = (e: Element) => {\n\te.setHTMLUnsafe('x');\n};",
				'no-restricted-properties'
			],
			[
				'createContextualFragment',
				"export const r = new Range().createContextualFragment('x');",
				'no-restricted-properties'
			],
			['document.write', "export const r = document.write('x');", 'no-restricted-properties'],
			['DOMParser', 'export const r = new DOMParser();', 'no-restricted-globals']
		];

		describe.each([
			['a component', planted.component],
			['the seam', planted.environment],
			['the motion adapter', planted.motion]
		])('in %s', (_set, filePath) => {
			it.each(sinks)('refuses %s', async (_label, code, ruleId) => {
				await expectRefused(filePath, code, ruleId, rawHtmlMessage);
			});
		});
	});

	describe('no media markup in any .svelte file', () => {
		const media: readonly (readonly [string, string])[] = [
			['srcset', component('<img src="./a.png" srcset="./a.png 1x" alt={s} />')],
			['srcSet', component('<img src="./a.png" srcSet={s} alt="" />')],
			[
				'imagesrcset',
				component('<svelte:head><link rel="preload" imagesrcset={s} /></svelte:head>')
			],
			['poster', component('<div poster={s}></div>')],
			[
				'a shorthand srcset',
				'<script lang="ts">\n\tconst srcset = \'x\';\n</script>\n\n<img src="./a.png" {srcset} alt="" />\n'
			],
			['<picture>', component('<picture><img src="./a.png" alt={s} /></picture>')],
			['<source>', component('<div><source src={s} /></div>')],
			['<video>', component('<video src={s}></video>')],
			['<audio>', component('<audio src={s}></audio>')],
			['<iframe>', component('<iframe title={s}></iframe>')],
			['<IFRAME>', component('<IFRAME title={s}></IFRAME>')],
			['<Video>', component('<Video src={s}></Video>')],
			['<object>', component('<object title={s}></object>')],
			['<embed>', component('<embed title={s} />')],
			['<track>', component('<div><track src={s} /></div>')],
			[
				'svelte:element this="video"',
				component('<svelte:element this="video">{s}</svelte:element>')
			],
			[
				"svelte:element this={'audio'}",
				component("<svelte:element this={'audio'}>{s}</svelte:element>")
			],
			[
				'<image href> inside an <svg>',
				component('<svg viewBox="0 0 1 1"><image href={s} /></svg>')
			],
			[
				'<image xlink:href>',
				component('<svg viewBox="0 0 1 1"><image xlink:href="./a.png" /></svg>')
			],
			['<use href>', component('<svg viewBox="0 0 1 1"><use href="#a" /></svg>')],
			[
				'<feImage href>',
				component('<svg viewBox="0 0 1 1"><filter id="f"><feImage href={s} /></filter></svg>')
			],
			['style="background: url(x)"', component('<div style="background: url(x)">{s}</div>')],
			['style:background="url(x)"', component('<div style:background="url(x)">{s}</div>')],
			['STYLE="background:URL(x)"', component('<div STYLE="background:URL(x)">{s}</div>')],
			['url( in a style expression', component('<div style={`background: url(${s})`}>{s}</div>')],
			['url( in a style string', component("<div style:background={'url(' + s + ')'}>{s}</div>")],
			[
				'image-set( in a style attribute',
				component('<div style="background: image-set(\'a.png\' 1x)">{s}</div>')
			]
		];

		it.each(media)('refuses %s in a component', async (_label, code) => {
			await expectRefused(planted.componentSvelte, code, MARKUP, mediaMessage);
		});

		it.each([
			['the seam', planted.environmentSvelte],
			['the motion adapter', planted.motionSvelte],
			['a gallery file', planted.gallerySvelte],
			['a route', planted.routesSvelte],
			['a stories file', planted.storiesSvelte],
			['a check harness', planted.checkSvelte]
		])('refuses <video>, srcset and a style url( in %s', async (_label, filePath) => {
			await expectRefused(filePath, component('<video src={s}></video>'), MARKUP, mediaMessage);
			await expectRefused(
				filePath,
				component('<img src="./a.png" srcset={s} alt="" />'),
				MARKUP,
				mediaMessage
			);
			await expectRefused(
				filePath,
				component('<div style="background: url(x)">{s}</div>'),
				MARKUP,
				mediaMessage
			);
		});

		it.each([
			['an img with a src', component('<img src="./a.png" alt={s} />')],
			['an svg drawing', component('<svg viewBox="0 0 1 1"><path d="M0 0" /><image /></svg>')],
			[
				'a style with no picture',
				component('<div style="color: red" style:padding="1px">{s}</div>')
			],
			['the words as text', component('<p>video, audio, url(x) and srcset as words {s}</p>')],
			['an <a href>', component('<a href="/x">{s}</a>')]
		])('allows %s', async (_label, code) => {
			await expectAllowed(planted.componentSvelte, code);
		});
	});

	it('gives every set but the checks its load rules', () => {
		expect(Object.keys(loadRules).sort()).toEqual(
			[
				'components',
				'environment',
				'gallery',
				'lazy',
				'motion',
				'routes',
				'stories',
				'storage',
				'storybookBrowser'
			].sort()
		);
	});
});
