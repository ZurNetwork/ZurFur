/*
 * The dark theme's colour pairs at WCAG 2.2 AA, checked on every gate.
 *
 * Thresholds: 4.5:1 for text and 3:1 for large text (1.4.3), 3:1 for the
 * parts of a control or its state against adjacent colours (1.4.11); ratios
 * are compared unrounded, so 4.499:1 fails 4.5:1. Formulas: relative
 * luminance with the 0.04045 sRGB threshold, and (L1 + 0.05) / (L2 + 0.05).
 *   https://www.w3.org/TR/WCAG22/#contrast-minimum
 *   https://www.w3.org/TR/WCAG22/#non-text-contrast
 *   https://www.w3.org/TR/WCAG22/#dfn-relative-luminance
 *   https://www.w3.org/TR/WCAG22/#dfn-contrast-ratio
 *   https://www.w3.org/TR/WCAG22/#dfn-large-scale
 *   https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
 *   https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
 */
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
	BACKGROUND_TOKENS,
	CONTRAST_PAIRS,
	DECORATIVE_TOKENS,
	FILLED_PARTS,
	minimumRatio,
	type Background,
	type ColorToken
} from './contrast-pairs';
import {
	composite,
	contrastRatio,
	darkRootProperties,
	parseColor,
	relativeLuminance,
	resolveProperty,
	type Read,
	type Rgba
} from './contrast';

function valueOf<T>(result: Read<T>): T {
	if (result.kind === 'unreadable') throw new Error(result.reason);
	return result.value;
}

const BLACK: Rgba = { red: 0, green: 0, blue: 0, alpha: 1 };
const WHITE: Rgba = { red: 255, green: 255, blue: 255, alpha: 1 };

describe('the contrast arithmetic', () => {
	it('spans 1:1 to 21:1', () => {
		expect(relativeLuminance(BLACK)).toBe(0);
		expect(relativeLuminance(WHITE)).toBe(1);
		expect(contrastRatio(BLACK, WHITE)).toBe(21);
		expect(contrastRatio(WHITE, BLACK)).toBe(21);
		expect(contrastRatio(WHITE, WHITE)).toBe(1);
	});

	it('puts #767676 just above 4.5:1 on white, and #777777 just below', () => {
		const passing = valueOf(parseColor('#767676'));
		const failing = valueOf(parseColor('#777777'));
		expect(contrastRatio(passing, WHITE)).toBeCloseTo(4.542, 3);
		expect(contrastRatio(failing, WHITE)).toBeCloseTo(4.478, 3);
	});

	it('blends a translucent colour over an opaque one, per channel', () => {
		const halfWhite = valueOf(parseColor('rgb(255 255 255 / 0.5)'));
		expect(composite(halfWhite, BLACK)).toEqual({
			red: 127.5,
			green: 127.5,
			blue: 127.5,
			alpha: 1
		});
	});

	it.each([
		['#abc', { red: 170, green: 187, blue: 204, alpha: 1 }],
		['#AABBCC', { red: 170, green: 187, blue: 204, alpha: 1 }],
		['#aabbcc33', { red: 170, green: 187, blue: 204, alpha: 0.2 }],
		['rgb(1 2 3)', { red: 1, green: 2, blue: 3, alpha: 1 }],
		['rgb(172 157 255 / 0.16)', { red: 172, green: 157, blue: 255, alpha: 0.16 }],
		['rgb(10 9 12 / 70%)', { red: 10, green: 9, blue: 12, alpha: 0.7 }],
		['rgba(1, 2, 3, 0.5)', { red: 1, green: 2, blue: 3, alpha: 0.5 }]
	])('reads %s', (text, color) => {
		expect(valueOf(parseColor(text))).toEqual(color);
	});

	it.each(['hsl(0 0% 0%)', 'rgb(1 2)', 'rgb(300 0 0)', 'rgb(1 2 3 / 2)', '#ab', 'teal'])(
		'reports %s as unreadable',
		(text) => {
			expect(parseColor(text).kind).toBe('unreadable');
		}
	);
});

describe('reading tokens.css', () => {
	const sample = `
		:root { --a: var(--b); --b: #fff; /* --c: #000; */ --loop: var(--loop); }
		:root[data-theme='light'] { --a: #000; }
		@media (prefers-contrast: more) {
			:root { --b: #111; }
		}
		.elsewhere { --d: #222; }
		:root { --b: #333; }
	`;
	const properties = valueOf(darkRootProperties(sample));

	it('reads every top-level :root block and nothing else, without comments', () => {
		expect([...properties.keys()]).toEqual(['--a', '--b', '--loop']);
	});

	it('lets a later top-level :root block win, as the cascade does', () => {
		expect(valueOf(resolveProperty(properties, '--b'))).toBe('#333');
	});

	it('follows var() to the value', () => {
		expect(valueOf(resolveProperty(properties, '--a'))).toBe('#333');
	});

	it('reports a missing or circular reference', () => {
		expect(resolveProperty(properties, '--missing').kind).toBe('unreadable');
		expect(resolveProperty(properties, '--loop').kind).toBe('unreadable');
	});
});

/** The app's source tree, read from disk: the node project loads stylesheets as empty modules. */
const SOURCE_ROOT = fileURLToPath(new URL('../../', import.meta.url));

function sourceText(file: string): string {
	return readFileSync(`${SOURCE_ROOT}${file}`, 'utf8');
}

const darkTokens = valueOf(darkRootProperties(sourceText('lib/styles/tokens.css')));

function tokenColor(token: ColorToken): Rgba {
	return valueOf(parseColor(valueOf(resolveProperty(darkTokens, token))));
}

function opaque(token: ColorToken): Rgba {
	const color = tokenColor(token);
	if (color.alpha !== 1) throw new Error(`${token} is translucent; declare what it lies over`);
	return color;
}

function backgroundColor(background: Background): Rgba {
	if (typeof background === 'string') return opaque(background);
	return composite(tokenColor(background.layer), opaque(background.over));
}

function backgroundLabel(background: Background): string {
	return typeof background === 'string'
		? background
		: `${background.layer} over ${background.over}`;
}

const pairCases = CONTRAST_PAIRS.flatMap((pair) =>
	pair.backgrounds.map((background) => ({ ...pair, background }))
);

describe('the dark theme meets WCAG 2.2 AA', () => {
	for (const { foreground, background, kind, where } of pairCases) {
		const name = `${foreground} on ${backgroundLabel(background)} (${kind})`;
		it(`${name}: ${where}`, () => {
			const under = backgroundColor(background);
			const ratio = contrastRatio(composite(tokenColor(foreground), under), under);
			const minimum = minimumRatio(kind);
			expect(
				ratio,
				`${name} is ${ratio.toFixed(3)}:1; it needs ${String(minimum)}:1`
			).toBeGreaterThanOrEqual(minimum);
		});
	}
});

/*
 * The completeness guard. It sees `--color-*` tokens named in a colour
 * declaration of a `.svelte` style block or a `.css` file, and token
 * definitions outside tokens.css. It does not see palette steps or literal
 * colours named outside tokens.css, paint properties outside its two lists
 * (such as `-webkit-text-fill-color` or `background-image`), or `opacity`
 * lowering a painted colour. None is used today.
 */

/** Every stylesheet that paints with the tokens, by path under `src/`; tokens.css only defines them. */
const SOURCES: ReadonlyMap<string, string> = new Map(
	readdirSync(SOURCE_ROOT, { recursive: true, encoding: 'utf8' })
		.filter((file) => /\.(svelte|css)$/.test(file) && file !== 'lib/styles/tokens.css')
		.map((file) => [file, sourceText(file)])
);

/** Properties that paint a foreground: text, an edge, an outline, an SVG paint, a ring. */
const FOREGROUND_PROPERTY =
	/^(color|caret-color|fill|stroke|text-decoration(-color)?|outline(-color)?|border(-(top|right|bottom|left|block|inline)(-(start|end))?)?(-color)?|box-shadow)$/;

const BACKGROUND_PROPERTY = /^background(-color)?$/;

const STYLE_BLOCK = /<style[^>]*>([\s\S]*?)<\/style>/g;

const DECLARATION = /([-\w]+)\s*:\s*([^;{}]+)(?=[;}])/g;

const TOKEN_DEFINITION = /^--(color|palette)-/;

const TOKEN_REFERENCE = /var\(\s*(--color-[\w-]+)/g;

interface TokenUse {
	readonly token: string;
	readonly property: string;
	readonly file: string;
}

function stylesheetText(file: string, source: string): string {
	const css = file.endsWith('.svelte')
		? [...source.matchAll(STYLE_BLOCK)].map((block) => block[1] ?? '').join('\n')
		: source;
	return css.replace(/\/\*[\s\S]*?\*\//g, '');
}

const tokenUses: readonly TokenUse[] = [...SOURCES].flatMap(([file, source]) =>
	[...stylesheetText(file, source).matchAll(DECLARATION)].flatMap(([, property = '', value = '']) =>
		[...value.matchAll(TOKEN_REFERENCE)].map(([, token = '']) => ({ token, property, file }))
	)
);

const declaredForegrounds = new Set<string>(CONTRAST_PAIRS.map((pair) => pair.foreground));

const backgroundTokens = new Set<string>(BACKGROUND_TOKENS);

const pairBackgroundTokens = CONTRAST_PAIRS.flatMap((pair) =>
	pair.backgrounds.flatMap((background) =>
		typeof background === 'string' ? [background] : [background.layer, background.over]
	)
);

function isFilledPart(use: TokenUse): boolean {
	return FILLED_PARTS.some((part) => part.token === use.token && part.file === use.file);
}

const decorative = new Set<string>(DECORATIVE_TOKENS.map((entry) => entry.token));

function describeUses(uses: readonly TokenUse[]): string {
	return uses.map((use) => `${use.token} as ${use.property} in ${use.file}`).join('\n');
}

describe('every colour token in use is declared', () => {
	it('finds the colour declarations in the sources', () => {
		expect([...SOURCES.keys()]).toContain('lib/styles/base.css');
		expect(tokenUses).toContainEqual({
			token: '--color-focus',
			property: 'outline',
			file: 'lib/styles/base.css'
		});
	});

	it('pairs every token painted as a foreground, unless it is decorative', () => {
		const undeclared = tokenUses.filter(
			(use) =>
				FOREGROUND_PROPERTY.test(use.property) &&
				!declaredForegrounds.has(use.token) &&
				!decorative.has(use.token)
		);
		expect(
			undeclared,
			`colour tokens painted with no declared pair:\n${describeUses(undeclared)}`
		).toEqual([]);
	});

	it('fills only with a declared background or a filled part, unless it is decorative', () => {
		const undeclared = tokenUses.filter(
			(use) =>
				BACKGROUND_PROPERTY.test(use.property) &&
				!backgroundTokens.has(use.token) &&
				!isFilledPart(use) &&
				!decorative.has(use.token)
		);
		expect(
			undeclared,
			`colour tokens filled that are not declared backgrounds:\n${describeUses(undeclared)}`
		).toEqual([]);
	});

	it('sets every pair on declared backgrounds only', () => {
		const strays = pairBackgroundTokens.filter((token) => !backgroundTokens.has(token));
		expect(strays).toEqual([]);
	});

	it('checks every filled part with a non-text pair of its own token', () => {
		const unchecked = FILLED_PARTS.filter(
			(part) =>
				!CONTRAST_PAIRS.some((pair) => pair.foreground === part.token && pair.kind === 'nonText')
		);
		expect(unchecked).toEqual([]);
	});

	it('defines colour and palette tokens nowhere but tokens.css', () => {
		const definitions = [...SOURCES].flatMap(([file, source]) =>
			[...stylesheetText(file, source).matchAll(DECLARATION)]
				.map(([, property = '']) => property)
				.filter((property) => TOKEN_DEFINITION.test(property))
				.map((property) => `${property} in ${file}`)
		);
		expect(definitions, `tokens defined outside tokens.css:\n${definitions.join('\n')}`).toEqual(
			[]
		);
	});

	it('lists as decorative only tokens still in use, and none that a pair already covers', () => {
		const used = new Set(tokenUses.map((use) => use.token));
		const stale = DECORATIVE_TOKENS.filter(
			(entry) => !used.has(entry.token) || declaredForegrounds.has(entry.token)
		);
		expect(stale).toEqual([]);
	});
});
