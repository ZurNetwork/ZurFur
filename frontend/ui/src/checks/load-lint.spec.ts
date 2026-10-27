/*
 * The load bans, over planted text: no timer in a component or the seam, no
 * load starter (Image, Audio, createElement, createElementNS) outside the
 * checks, the motion adapter building only spans, no inline style reached from
 * a component's or the seam's code, and no media markup, picture-loading
 * style or attribute, or prefixed <svelte:element> in any .svelte file under
 * src/. The HTML sinks these sets refuse belong to the boundary rules; their
 * cases here prove they hold.
 */
import { describe, expect, it } from 'vitest';
import { ESLint, type Linter } from 'eslint';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import {
	fileSetOf,
	inlineStyleMessage,
	loadRules,
	loadStarterMessage,
	mediaMessage,
	motionElementMessage,
	rawHtmlMessage,
	SVELTE_ELEMENT_FILES,
	timerMessage
} from '../../eslint.config.js';
import { DYNAMIC_ELEMENT } from './composition-table';

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
	storyHelperSvelte: 'src/stories/LoadX.svelte',
	check: 'src/checks/load-planted.ts',
	checkSvelte: 'src/checks/LoadX.svelte',
	spec: 'src/lib/load-planted.spec.ts'
} as const;

/** Real files a case lints planted text at; the file on disk is never written. */
const real = {
	skinStore: 'src/gallery/skin-store.ts',
	storybookPreview: '.storybook/preview.ts',
	card: 'src/lib/surfaces/Card.svelte'
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

		it.each(hostAndLazy)('refuses new Audio(value).play() in %s', async (_label, filePath) => {
			await expectRefused(
				filePath,
				'export const r = (value: string) => new Audio(value).play();',
				GLOBALS,
				loadStarterMessage
			);
		});

		it('refuses new Audio(value).play() in a story helper', async () => {
			await expectRefused(
				planted.storyHelperSvelte,
				'<script lang="ts">\n\tconst value = \'x\';\n\tvoid new Audio(value).play();\n</script>\n\n<p>{value}</p>\n',
				GLOBALS,
				loadStarterMessage
			);
		});

		it('gives every set but the checks Audio', () => {
			const withoutAudio = Object.entries(loadRules)
				.filter(([, rules]) => !rules.globals.some((entry) => entry.name === 'Audio'))
				.map(([name]) => name);
			expect(withoutAudio).toEqual([]);
		});

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

	describe('no SVG animation element and no feImage in any .svelte file', () => {
		/** A component whose script declares `s`, then `inner` inside an SVG rect. */
		const inRect = (inner: string): string =>
			component(`<svg viewBox="0 0 1 1"><rect width="1" height="1">${inner}</rect></svg>`);
		/** A component whose script declares `s`, then `inner` inside an SVG filter. */
		const inFilter = (inner: string): string =>
			component(`<svg viewBox="0 0 1 1"><filter id="f">${inner}</filter></svg>`);

		const elements: readonly (readonly [string, string])[] = [
			['<set>', inRect('<set attributeName="href" to={s} />')],
			['<animate>', inRect('<animate attributeName="fill" values={s} dur="1s" />')],
			['<animateMotion>', inRect('<animateMotion path="M0 0" dur="1s" />')],
			['<animateTransform>', inRect('<animateTransform attributeName="transform" dur="1s" />')],
			['<feImage> with no href', inFilter('<feImage />')],
			[
				'<feImage><set> in a filter',
				inFilter('<feImage><set attributeName="href" to={s} /></feImage>')
			],
			['<SET>', inRect('<SET attributeName="cursor" to={s} />')],
			['<animatemotion>', inRect('<animatemotion path="M0 0" dur="1s" />')],
			['<FeImAgE>', inFilter('<FeImAgE />')],
			[
				'svelte:element this="set"',
				inRect('<svelte:element this="set" attributeName="mask" to={s} />')
			],
			[
				"svelte:element this={'animateTransform'}",
				inRect('<svelte:element this={\'animateTransform\'} attributeName="transform" />')
			],
			['svelte:element this={`feImage`}', inFilter('<svelte:element this={`feImage`} />')],
			["svelte:element this={'svg:set'}", inRect("<svelte:element this={'svg:set'} to={s} />")],
			[
				'svelte:element this="svg:animate"',
				inRect('<svelte:element this="svg:animate" values={s} />')
			]
		];

		describe.each([
			['a component', planted.componentSvelte],
			['a story helper', planted.storyHelperSvelte]
		])('in %s', (_set, filePath) => {
			it.each(elements)('refuses %s', async (_label, code) => {
				await expectRefused(filePath, code, MARKUP, mediaMessage);
			});
		});

		it.each(['<set>', '<animate>', '<animateMotion>', '<animateTransform>', '<feImage>'])(
			'names %s in its message',
			(name) => {
				expect(mediaMessage).toContain(name);
			}
		);
	});

	describe('no picture reaches a style or a loading attribute from a value', () => {
		/** A component whose script declares `s`, a value under each attribute's name and a helper, then `markup`. */
		const valued = (markup: string): string =>
			`<script lang="ts">\n\tconst s = 'x';\n\tconst background = s;\n\tconst style = s;\n\tconst fill = s;\n\tconst src = s;\n\tfunction backgroundOf(address: string): string {\n\t\treturn 'url(' + address + ')';\n\t}\n</script>\n\n<p>{s}{background}{style}{fill}{src}{backgroundOf(s)}</p>\n${markup}\n`;

		const forms: readonly (readonly [string, string])[] = [
			['style="background-image: {s}"', valued('<span style="background-image: {s}"></span>')],
			['style={s}', valued('<span style={s}></span>')],
			['STYLE={s}', valued('<span STYLE={s}></span>')],
			['a shorthand {style}', valued('<span {style}></span>')],
			['style={s} on a component', valued('<Thing style={s} />')],
			['style={s} on <svelte:element>', valued('<svelte:element this="span" style={s} />')],
			['style:background-image={s}', valued('<span style:background-image={s}></span>')],
			[
				'style:background-image={backgroundOf(s)}',
				valued('<span style:background-image={backgroundOf(s)}></span>')
			],
			['style:background="var(--{s})"', valued('<span style:background="var(--{s})"></span>')],
			['a shorthand style:background', valued('<span style:background></span>')],
			['style:--art={s}', valued('<span style:--art={s}></span>')],
			['style:mask={s}', valued('<span style:mask={s}></span>')],
			['style:mask-image={s}', valued('<span style:mask-image={s}></span>')],
			['style:-webkit-mask-image={s}', valued('<span style:-webkit-mask-image={s}></span>')],
			['style:border-image={s}', valued('<span style:border-image={s}></span>')],
			['style:border-image-source={s}', valued('<span style:border-image-source={s}></span>')],
			['style:list-style={s}', valued('<span style:list-style={s}></span>')],
			['style:list-style-image={s}', valued('<span style:list-style-image={s}></span>')],
			['style:content={s}', valued('<span style:content={s}></span>')],
			['style:cursor={s}', valued('<span style:cursor={s}></span>')],
			['style:shape-outside={s}', valued('<span style:shape-outside={s}></span>')],
			['style:filter={s}', valued('<span style:filter={s}></span>')],
			['style:backdrop-filter={s}', valued('<span style:backdrop-filter={s}></span>')],
			['style:clip-path={s}', valued('<span style:clip-path={s}></span>')],
			['style:offset-path={s}', valued('<span style:offset-path={s}></span>')],
			['style:LEFT={s}', valued('<span style:LEFT={s}></span>')],
			[
				'style:background-image|important={s}',
				valued('<span style:background-image|important={s}></span>')
			],
			[
				'<table background={s}>',
				valued('<table background={s}><tbody><tr><td>x</td></tr></tbody></table>')
			],
			['background="./a.png"', valued('<div background="./a.png"></div>')],
			['BACKGROUND={s}', valued('<div BACKGROUND={s}></div>')],
			['a shorthand {background}', valued('<div {background}></div>')],
			...[
				'mask',
				'filter',
				'clip-path',
				'fill',
				'stroke',
				'marker',
				'marker-start',
				'marker-mid',
				'marker-end',
				'cursor'
			].map(
				(name) =>
					[
						`SVG ${name}={s}`,
						valued(`<svg viewBox="0 0 1 1"><rect width="1" height="1" ${name}={s} /></svg>`)
					] as const
			),
			['SVG MASK={s}', valued('<svg viewBox="0 0 1 1"><rect MASK={s} /></svg>')],
			['SVG a shorthand {fill}', valued('<svg viewBox="0 0 1 1"><rect {fill} /></svg>')],
			[
				'SVG fill="url(./a.svg#p)"',
				valued('<svg viewBox="0 0 1 1"><rect fill="url(./a.svg#p)" /></svg>')
			],
			[
				'SVG mask="url( \'x\')"',
				valued('<svg viewBox="0 0 1 1"><rect mask="url( \'x\')" /></svg>')
			],
			['SVG FILTER="URL(x)"', valued('<svg viewBox="0 0 1 1"><rect FILTER="URL(x)" /></svg>')],
			[
				'SVG cursor="url(x), auto"',
				valued('<svg viewBox="0 0 1 1"><rect cursor="url(x), auto" /></svg>')
			],
			[
				'SVG clip-path="url(#a) url(b)"',
				valued('<svg viewBox="0 0 1 1"><rect clip-path="url(#a) url(b)" /></svg>')
			],
			['<input type="image" src={s}>', valued('<input type="image" src={s} alt="x" />')],
			['<INPUT src="./a.png">', valued('<INPUT type="image" src="./a.png" alt="x" />')],
			['a shorthand {src} on an input', valued('<input type="image" {src} alt="x" />')]
		];

		describe.each([
			['a component', planted.componentSvelte],
			['a stories file', planted.storiesSvelte],
			['a story helper', planted.storyHelperSvelte],
			['a gallery file', planted.gallerySvelte]
		])('in %s', (_set, filePath) => {
			it.each(forms)('refuses %s', async (_label, code) => {
				await expectRefused(filePath, code, MARKUP, mediaMessage);
			});
		});

		it.each([
			["Pin's style:left={…}", component('<span style:left={s}></span>')],
			["Pin's style:top={…}", component('<span style:top={s}></span>')],
			['a type page font directive', component('<span style:font-size={s}></span>')],
			[
				'a font family from a token',
				component('<span style:font-family="var(--font-{s})"></span>')
			],
			['a colour swatch', component('<span style:background-color="var(--{s})"></span>')],
			['a static style: directive', component('<span style:background="var(--x)">{s}</span>')],
			['a static style attribute', component('<span style="color: red">{s}</span>')],
			['fill="none"', component('<svg viewBox="0 0 1 1"><rect fill="none" />{s}</svg>')],
			[
				'stroke="currentColor"',
				component('<svg viewBox="0 0 1 1"><rect stroke="currentColor" />{s}</svg>')
			],
			[
				'clip-path="url(#c)"',
				component('<svg viewBox="0 0 1 1"><rect clip-path="url(#c)" />{s}</svg>')
			],
			[
				'mask="url(\'#m\')"',
				component('<svg viewBox="0 0 1 1"><rect mask="url(\'#m\')" />{s}</svg>')
			],
			['an input with no src', component('<input type="text" value={s} />')]
		])('allows %s', async (_label, code) => {
			await expectAllowed(planted.componentSvelte, code);
		});
	});

	describe("no component reaches an element's inline style from code", () => {
		const writes: readonly (readonly [string, string])[] = [
			['style.backgroundImage =', 'node.style.backgroundImage = value;'],
			['style.setProperty', "node.style.setProperty('background-image', value);"],
			['style.cssText =', 'node.style.cssText = value;'],
			['Object.assign(node.style, …)', 'Object.assign(node.style, { backgroundImage: value });'],
			["node['style']", "node['style'].backgroundImage = value;"],
			['node[`style`]', 'node[`style`].backgroundImage = value;'],
			['a destructured style', 'const { style } = node;\n\tstyle.backgroundImage = value;'],
			['attributeStyleMap', "node.attributeStyleMap.set('background-image', value);"],
			["setAttribute('style', …)", "node.setAttribute('style', value);"],
			["setAttribute('STYLE', …)", "node.setAttribute('STYLE', value);"],
			['setAttribute(`style`, …)', 'node.setAttribute(`style`, value);'],
			["setAttribute('background', …)", "node.setAttribute('background', value);"],
			["setAttributeNS(null, 'style', …)", "node.setAttributeNS(null, 'style', value);"],
			["createAttribute('style')", "node.setAttributeNode(document.createAttribute('style'));"],
			[
				"createAttributeNS(null, 'background')",
				"node.setAttributeNode(document.createAttributeNS(null, 'background'));"
			]
		];

		const asModule = (body: string): string =>
			`export function paint(node: HTMLElement, value: string): void {\n\t${body}\n}\n`;

		describe.each([
			['a component', planted.component],
			['the seam', planted.environment]
		])('in %s', (_set, filePath) => {
			it.each(writes)('refuses %s', async (_label, body) => {
				await expectRefused(filePath, asModule(body), SYNTAX, inlineStyleMessage);
			});
		});

		it('refuses an {@attach} that writes node.style in a component', async () => {
			await expectRefused(
				planted.componentSvelte,
				component('<span {@attach (node) => { node.style.backgroundImage = s; }}></span>'),
				SYNTAX,
				inlineStyleMessage
			);
		});

		it.each([
			['a check', planted.check],
			['a spec', planted.spec]
		])('allows node.style in %s', async (_label, filePath) => {
			await expectAllowed(filePath, asModule('node.style.color = value;'));
		});

		it('allows setAttribute naming another attribute in a component', async () => {
			await expectAllowed(planted.component, asModule("node.setAttribute('data-x', value);"));
		});
	});

	describe('no <svelte:element> names a prefixed tag', () => {
		const card = (markup: string): string =>
			`<script lang="ts">\n\tconst src = './a.svg';\n</script>\n\n${markup}\n`;

		it.each([
			[
				"this={'svg:image'} with an href",
				card('<svg viewBox="0 0 1 1"><svelte:element this={\'svg:image\'} href={src} /></svg>')
			],
			[
				"this={'svg:script'}",
				card(
					'<svg viewBox="0 0 1 1"><svelte:element this={\'svg:script\'}>{src}</svelte:element></svg>'
				)
			],
			[
				'this="svg:image"',
				card('<svg viewBox="0 0 1 1"><svelte:element this="svg:image" href={src} /></svg>')
			],
			[
				'this={`svg:image`}',
				card('<svg viewBox="0 0 1 1"><svelte:element this={`svg:image`} href={src} /></svg>')
			],
			[
				"this={'svg:' + 'image'}",
				card("<svg viewBox=\"0 0 1 1\"><svelte:element this={'svg:' + 'image'} href={src} /></svg>")
			]
		])('refuses %s in Card, the one file allowed <svelte:element>', async (_label, code) => {
			await expectRefused(real.card, code, MARKUP, mediaMessage);
		});

		it('refuses this="svg:image" in a story helper', async () => {
			await expectRefused(
				planted.storyHelperSvelte,
				card('<svg viewBox="0 0 1 1"><svelte:element this="svg:image" href={src} /></svg>'),
				MARKUP,
				mediaMessage
			);
		});

		it('lists the same files as the lego table', () => {
			expect(SVELTE_ELEMENT_FILES).toEqual(DYNAMIC_ELEMENT.map((row) => row.file));
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
