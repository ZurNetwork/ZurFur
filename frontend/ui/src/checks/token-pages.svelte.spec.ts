import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet, tick, type Component } from 'svelte';
import colourPage from '../routes/components/colour/+page.svelte';
import motionPage from '../routes/components/motion-and-sound/+page.svelte';
import paperPage from '../routes/components/paper/+page.svelte';
import shadowsPage from '../routes/components/shadows/+page.svelte';
import spacePage from '../routes/components/space-and-shape/+page.svelte';
import typePage from '../routes/components/type/+page.svelte';
import TokenModelGate from '../gallery/TokenModelGate.svelte';
import UsageText from '../gallery/UsageText.svelte';
import { backgroundLabel, contrastReport } from '../gallery/contrast/contrast-pairs';
import { registry } from '../gallery/registry';
import { tokenOutcome } from '../gallery/token-source';
import { PAPER_CLASSES } from '../lib/styles/paper-classes';
import { PAPER_MOVES } from '../lib/motion/paper-moves';
import { type Skin } from '../lib/styles/environment';
import type { ParseOutcome, TokenModel } from '../lib/styles/token-model';
import '../lib/styles/index.ts';

function modelOf(): TokenModel {
	if (tokenOutcome.kind === 'refused') {
		throw new Error(tokenOutcome.refusals.map((refusal) => refusal.detail).join('; '));
	}
	return tokenOutcome.model;
}

const model = modelOf();
const skins: readonly Skin[] = ['dark', 'light'];

const tokenPages: readonly (readonly [string, Component])[] = [
	['colour', colourPage],
	['type', typePage],
	['space-and-shape', spacePage],
	['shadows', shadowsPage],
	['motion-and-sound', motionPage]
];

function columnsOf(skin: Skin): readonly HTMLElement[] {
	return [...document.querySelectorAll<HTMLElement>(`[data-gallery-skin=${skin}]`)];
}

/** Every element matching the selector inside any column of one skin. */
function inSkin(skin: Skin, selector: string): readonly HTMLElement[] {
	return columnsOf(skin).flatMap((column) => [...column.querySelectorAll<HTMLElement>(selector)]);
}

/** The first element matching the selector inside the columns of one skin. */
function firstIn(skin: Skin, selector: string): HTMLElement {
	const found = inSkin(skin, selector)[0];
	if (found === undefined) throw new Error(`nothing matches ${selector} in ${skin}`);
	return found;
}

/** The text of every column of one skin, joined. */
function textIn(skin: Skin): string {
	return columnsOf(skin)
		.map((column) => column.textContent)
		.join(' ');
}

/** The values of an attribute on every element that carries it, inside every column of one skin. */
function attributeValues(skin: Skin, attribute: string): readonly string[] {
	return inSkin(skin, `[${attribute}]`).flatMap((element) => {
		const value = element.getAttribute(attribute);
		return value === null ? [] : [value];
	});
}

describe('the token pages', () => {
	it('show every token of the model in both skins, between them', () => {
		const wanted = [...model.byName.keys()].toSorted();
		for (const skin of skins) {
			const shown: string[] = [];
			for (const [, page] of tokenPages) {
				const screen = render(page, {});
				shown.push(...attributeValues(skin, 'data-token'));
				screen.unmount();
			}
			expect(shown.toSorted(), `${skin} column`).toEqual(wanted);
		}
	});

	it.each(tokenPages)('the %s page prints each value in its own column', (_slug, page) => {
		render(page, {});
		for (const skin of skins) {
			const rows = inSkin(skin, '[data-token]');
			expect(rows.length, `${skin} rows`).toBeGreaterThan(0);
			for (const row of rows) {
				const name = row.getAttribute('data-token') ?? '';
				const value = model.byName.get(name)?.resolved[skin];
				expect(value, name).toBeDefined();
				expect(row.textContent, `${name} in ${skin}`).toContain(value);
			}
		}
	});

	it('type: shows every type style of the model in both skins', () => {
		render(typePage, {});
		const wanted = model.type.groups.flatMap((group) => group.styles.map((style) => style.name));
		expect(wanted).toHaveLength(9);
		for (const skin of skins) {
			expect(attributeValues(skin, 'data-type-style'), skin).toEqual(wanted);
		}
	});

	it('type: sets each style sample in its own numbers', () => {
		render(typePage, {});
		const style = getComputedStyle(firstIn('dark', '[data-type-style=title] .sample'));
		expect({
			size: style.fontSize,
			line: style.lineHeight,
			weight: style.fontWeight,
			spacing: style.letterSpacing
		}).toEqual({ size: '24px', line: '32px', weight: '700', spacing: '-0.24px' });
	});

	it('type: finds every font file loaded once the page is up', async () => {
		render(typePage, {});
		await expect
			.poll(() => inSkin('dark', '[data-font-state]').map((state) => state.textContent))
			.toEqual(model.type.fonts.map(() => 'loaded'));
	});

	it('motion: draws a curve for every easing', () => {
		render(motionPage, {});
		const easings = model.groups.find((group) => group.id === 'easing')?.tokens ?? [];
		expect(easings.length).toBeGreaterThan(0);
		for (const skin of skins) {
			const plotted = inSkin(skin, 'svg[role=img]').map((svg) => svg.getAttribute('aria-label'));
			expect(plotted).toEqual(easings.map((token) => token.name));
		}
	});
});

describe('the colour page', () => {
	it('shows every contrast row, for both skins', () => {
		render(colourPage, {});
		for (const skin of skins) {
			const report = contrastReport(model, skin);
			const wanted = report.rows.map(
				(row) => `${row.foreground} ${backgroundLabel(row.background)}`
			);
			expect(wanted.length).toBeGreaterThan(0);
			expect(attributeValues(skin, 'data-contrast-row'), skin).toEqual(wanted);
		}
	});

	it('lists the filled, decorative and excluded entries with their reasons', () => {
		render(colourPage, {});
		for (const skin of skins) {
			const report = contrastReport(model, skin);
			const text = textIn(skin);
			for (const entry of report.filled) expect(text).toContain(entry.part);
			for (const entry of report.decorative) expect(text).toContain(entry.why);
			for (const entry of report.excluded) expect(text).toContain(entry.reason);
		}
	});

	it('paints each swatch with its token', () => {
		render(colourPage, {});
		const swatch = firstIn('light', '[data-token=color-bg] .swatch');
		expect(getComputedStyle(swatch).backgroundColor).toBe('rgb(236, 229, 211)');
	});
});

describe('the paper page', () => {
	it('uses every paper class and every move', () => {
		render(paperPage, {});
		const missing = [...PAPER_CLASSES, ...PAPER_MOVES].filter(
			(name) => document.querySelector(`.${name}`) === null
		);
		expect(missing).toEqual([]);
	});

	it('puts the moves on their own sheets, in both skins', () => {
		render(paperPage, {});
		for (const skin of skins) {
			const moves = inSkin(skin, '[data-move]').map((element) => element.getAttribute('data-move'));
			expect(moves).toEqual([...PAPER_MOVES]);
		}
	});

	it('replaces a move element when Play again is pressed', async () => {
		render(paperPage, {});
		const sheet = firstIn('dark', '[data-move-sheet=paperin]');
		const before = sheet.querySelector('[data-move=paperin]');
		const button = [...sheet.querySelectorAll('button')].find(
			(candidate) => candidate.textContent === 'Play again'
		);
		if (button === undefined) throw new Error('no Play again button');
		button.click();
		await tick();
		const after = sheet.querySelector('[data-move=paperin]');
		expect(after).not.toBeNull();
		expect(after).not.toBe(before);
		expect(before?.isConnected).toBe(false);
	});

	it('lays the placed things on real buttons', () => {
		render(paperPage, {});
		const placed = [...document.querySelectorAll('.placed')];
		expect(placed.length).toBeGreaterThan(0);
		expect(placed.map((element) => element.tagName)).toEqual(placed.map(() => 'BUTTON'));
	});
});

describe('UsageText', () => {
	it('renders markup in a usage line as text', () => {
		const line = '<img src=x onerror=alert(1)>';
		const screen = render(UsageText, { text: line });
		expect(document.querySelectorAll('img')).toHaveLength(0);
		expect(screen.container.textContent).toBe(line);
	});

	it('renders markup inside a code span as text too', () => {
		const screen = render(UsageText, { text: 'See `<img src=x onerror=alert(1)>` here' });
		expect(document.querySelectorAll('img')).toHaveLength(0);
		expect(screen.container.querySelector('code')?.textContent).toBe(
			'<img src=x onerror=alert(1)>'
		);
		expect(screen.container.textContent).toBe('See <img src=x onerror=alert(1)> here');
	});

	it('renders code segments as code elements and text as text', () => {
		const screen = render(UsageText, { text: 'Use `space-2` for gaps.' });
		expect([...screen.container.querySelectorAll('code')].map((code) => code.textContent)).toEqual([
			'space-2'
		]);
		expect(screen.container.textContent).toBe('Use space-2 for gaps.');
	});
});

describe('a refused token parse', () => {
	it('is listed, not thrown', () => {
		const refused: ParseOutcome = {
			kind: 'refused',
			refusals: [
				{ reason: 'bad-name', where: 'tokens:color[3]', detail: '"Color Bad" is not a token name' }
			]
		};
		const content = createRawSnippet<[TokenModel]>(() => ({ render: () => '<p>content</p>' }));
		const screen = render(TokenModelGate, { outcome: refused, content });
		const items = [...screen.container.querySelectorAll('[data-token-refusals] li')];
		expect(items.map((item) => item.textContent)).toEqual([
			'tokens:color[3] bad-name "Color Bad" is not a token name'
		]);
	});
});

describe('the registry', () => {
	it('lists the six token pages with no refusals', () => {
		expect(registry.refusals).toEqual([]);
		const listed = registry.entries
			.filter((entry) => entry.group === 'Tokens and paper')
			.map((entry) => [entry.slug, entry.title]);
		expect(listed).toEqual([
			['colour', 'Colour'],
			['motion-and-sound', 'Motion and sound'],
			['paper', 'Paper'],
			['shadows', 'Shadows'],
			['space-and-shape', 'Space and shape'],
			['type', 'Type']
		]);
	});
});
