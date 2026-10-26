/*
 * Every spec that emulates media features lives here, in the client-media
 * project, which runs alone after every other project: the skins against the
 * OS colour scheme and the root attribute, reduced motion, a Still column
 * beside a Lively one, and the command's own refusals.
 */
import { commands } from 'vitest/browser';
import { afterEach, describe, expect, it } from 'vitest';
import tokens from '@zurfur/ui/tokens.json';
import additions from '../lib/styles/tokens.additions.json';
import { parseTokenModel, resolveToken, type TokenModel } from '../lib/styles/token-model.ts';
import { SKIN_ATTRIBUTE, type Skin } from '../lib/styles/environment.ts';
import type { MediaSettings } from './media-command.ts';
import '../lib/styles/index.ts';

const outcome = parseTokenModel(tokens, additions);
if (outcome.kind !== 'parsed') throw new Error('the token files do not parse');
const model: TokenModel = outcome.model;

/** Every colour and shadow token, by name. */
const skinnedNames: readonly string[] = model.groups
	.filter((group) => group.id === 'color' || group.id === 'shadow')
	.flatMap((group) => group.tokens.map((token) => token.name));

const root = document.documentElement;
const mounted: Element[] = [];

afterEach(async () => {
	for (const element of mounted.splice(0)) element.remove();
	root.removeAttribute(SKIN_ATTRIBUTE);
	const reset = await commands.emulateMedia({ colorScheme: null, reducedMotion: null });
	expect(reset).toEqual({ kind: 'applied' });
});

/** Emulates the settings and waits until the test frame sees them. */
async function emulate(settings: MediaSettings, query: string): Promise<void> {
	expect(await commands.emulateMedia(settings)).toEqual({ kind: 'applied' });
	await expect.poll(() => matchMedia(query).matches).toBe(true);
}

const emulateScheme = (scheme: Skin): Promise<void> =>
	emulate({ colorScheme: scheme }, `(prefers-color-scheme: ${scheme})`);

function mount(markup: string): HTMLElement {
	const container = document.createElement('div');
	container.innerHTML = markup;
	document.body.append(container);
	mounted.push(container);
	return container;
}

/**
 * A token value as Chromium serializes a custom property, normalized: runs of
 * whitespace to one space, no space inside parentheses or before a comma, one
 * after it, and lower case.
 */
function normalized(text: string): string {
	return text
		.trim()
		.replace(/\s+/g, ' ')
		.replace(/\(\s/g, '(')
		.replace(/\s\)/g, ')')
		.replace(/\s*,\s*/g, ', ')
		.toLowerCase();
}

/** Each token's computed value on the element, against its value in the skin; any mismatch is listed. */
function mismatches(element: Element, skin: Skin): readonly string[] {
	const style = getComputedStyle(element);
	return skinnedNames.flatMap((name) => {
		const expected = resolveToken(model, name, skin);
		const computed = style.getPropertyValue(`--${name}`);
		if (expected === undefined) return [`${name}: no value in the model`];
		if (computed === '') return [`${name}: not set on the element`];
		return normalized(computed) === normalized(expected)
			? []
			: [
					`${name}: computed ${JSON.stringify(computed)}, the ${skin} model says ${JSON.stringify(expected)}`
				];
	});
}

describe('the skins', () => {
	it('finds the colour and shadow tokens to compare', () => {
		expect(skinnedNames.length).toBeGreaterThan(50);
		expect(skinnedNames).toContain('color-bg');
		expect(skinnedNames).toContain('elevation-sheet');
	});

	it('is dark when the OS is dark and the root says nothing', async () => {
		await emulateScheme('dark');
		expect(mismatches(root, 'dark')).toEqual([]);
	});

	it('is light when the OS is light and the root says nothing', async () => {
		await emulateScheme('light');
		expect(mismatches(root, 'light')).toEqual([]);
	});

	it('is dark when the OS is light and the root says dark', async () => {
		await emulateScheme('light');
		root.setAttribute(SKIN_ATTRIBUTE, 'dark');
		expect(mismatches(root, 'dark')).toEqual([]);
	});

	it('is light when the OS is dark and the root says light', async () => {
		await emulateScheme('dark');
		root.setAttribute(SKIN_ATTRIBUTE, 'light');
		expect(mismatches(root, 'light')).toEqual([]);
	});

	describe.each(['dark', 'light'] as const)('with the OS %s', (scheme) => {
		it('wears dark nested in light', async () => {
			await emulateScheme(scheme);
			const container = mount(
				'<div data-skin="light"><div data-skin="dark" id="inner"></div></div>'
			);
			const inner = container.querySelector('#inner');
			expect(inner).not.toBeNull();
			if (inner !== null) expect(mismatches(inner, 'dark')).toEqual([]);
		});

		it('wears light nested in dark', async () => {
			await emulateScheme(scheme);
			const container = mount(
				'<div data-skin="dark"><div data-skin="light" id="inner"></div></div>'
			);
			const inner = container.querySelector('#inner');
			expect(inner).not.toBeNull();
			if (inner !== null) expect(mismatches(inner, 'light')).toEqual([]);
		});
	});
});

describe('reduced motion', () => {
	it('fades a page in for 0.16s instead', async () => {
		await emulate({ reducedMotion: 'reduce' }, '(prefers-reduced-motion: reduce)');
		const page = mount('<div class="pagein"></div>').firstElementChild;
		expect(page).not.toBeNull();
		if (page === null) return;
		const style = getComputedStyle(page);
		expect({ name: style.animationName, duration: style.animationDuration }).toEqual({
			name: 'pagefade',
			duration: '0.16s'
		});
	});

	it('stops a hop', async () => {
		await emulate({ reducedMotion: 'reduce' }, '(prefers-reduced-motion: reduce)');
		const hop = mount('<div class="hop"></div>').firstElementChild;
		expect(hop).not.toBeNull();
		if (hop !== null) expect(getComputedStyle(hop).animationName).toBe('none');
	});

	it('lets a page move in when motion is not reduced', async () => {
		await emulate({ reducedMotion: 'no-preference' }, '(prefers-reduced-motion: no-preference)');
		const page = mount('<div class="pagein"></div>').firstElementChild;
		expect(page).not.toBeNull();
		if (page !== null) expect(getComputedStyle(page).animationName).toBe('pagein');
	});
});

describe('a Still column beside a Lively one', () => {
	it('keeps a Lively ::before animation running', async () => {
		await emulate({ reducedMotion: 'no-preference' }, '(prefers-reduced-motion: no-preference)');
		const probeStyle = document.createElement('style');
		probeStyle.textContent =
			'@keyframes media-probe { to { opacity: 0.5; } } .media-probe::before { content: ""; animation: media-probe 1s infinite; }';
		document.head.append(probeStyle);
		mounted.push(probeStyle);
		const container = mount(
			'<div data-motion="still"><div class="media-probe" id="still"></div></div><div class="media-probe" id="lively"></div>'
		);
		const still = container.querySelector('#still');
		const lively = container.querySelector('#lively');
		expect(still).not.toBeNull();
		expect(lively).not.toBeNull();
		if (still === null || lively === null) return;
		expect(getComputedStyle(lively, '::before').animationName).toBe('media-probe');
		expect(getComputedStyle(still, '::before').animationName).toBe('none');
	});
});

describe('the emulateMedia command', () => {
	/** Sends a payload the command's type refuses, as a page could. */
	const send = (payload: unknown) => commands.emulateMedia(payload as MediaSettings);

	it.each([
		['a colour scheme of no-preference', { colorScheme: 'no-preference' }],
		['an unknown key', { colourScheme: 'dark' }],
		['forced colours', { forcedColors: 'active' }],
		['a reduced motion it does not know', { reducedMotion: 'yes' }],
		['a known key beside an unknown one', { colorScheme: 'dark', media: 'print' }],
		['a list', ['dark']],
		['text', 'dark'],
		['nothing', null]
	])('refuses %s and changes nothing', async (_label, payload) => {
		const before = {
			dark: matchMedia('(prefers-color-scheme: dark)').matches,
			forced: matchMedia('(forced-colors: active)').matches,
			print: matchMedia('print').matches
		};
		const result = await send(payload);
		expect(result.kind).toBe('refused');
		expect({
			dark: matchMedia('(prefers-color-scheme: dark)').matches,
			forced: matchMedia('(forced-colors: active)').matches,
			print: matchMedia('print').matches
		}).toEqual(before);
	});
});
