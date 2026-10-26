/*
 * The design system's contrast claims, checked in both skins from the
 * committed tokens: 4.5:1 for text (1.4.3) and 3:1 for the parts of a control
 * or its state (1.4.11), compared unrounded. Formulas: relative luminance with
 * the 0.04045 sRGB threshold, and (L1 + 0.05) / (L2 + 0.05).
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
	backgroundLabel,
	colourTokenNames,
	CONTRAST_PAIRS,
	contrastReport,
	DECORATIVE_TOKENS,
	EXCLUDED_PAIRS,
	FILLED_PARTS,
	hasToken,
	measurePair,
	minimumRatio
} from '../gallery/contrast/contrast-pairs';
import {
	composite,
	contrastRatio,
	parseColor,
	relativeLuminance,
	type Read,
	type Rgba
} from '../gallery/contrast/contrast';
import type { Skin } from '../lib/styles/environment';
import { parseTokenModel, type TokenModel } from '../lib/styles/token-model';

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

	it('reads a 4-digit hex with its alpha', () => {
		expect(valueOf(parseColor('#abcd'))).toEqual({
			red: 170,
			green: 187,
			blue: 204,
			alpha: 221 / 255
		});
	});

	it.each(['hsl(0 0% 0%)', 'rgb(1 2)', 'rgb(300 0 0)', 'rgb(1 2 3 / 2)', '#ab', 'teal'])(
		'reports %s as unreadable',
		(text) => {
			expect(parseColor(text).kind).toBe('unreadable');
		}
	);
});

function modelOf(tokensText: string): TokenModel {
	const additions: unknown = JSON.parse(
		readFileSync(new URL('../lib/styles/tokens.additions.json', import.meta.url), 'utf8')
	);
	const outcome = parseTokenModel(JSON.parse(tokensText), additions);
	if (outcome.kind === 'refused') {
		throw new Error(outcome.refusals.map((refusal) => refusal.detail).join('; '));
	}
	return outcome.model;
}

const tokensText = readFileSync(new URL('../lib/styles/tokens.json', import.meta.url), 'utf8');
const model = modelOf(tokensText);
const SKINS: readonly Skin[] = ['dark', 'light'];

for (const skin of SKINS) {
	describe(`the ${skin} skin meets WCAG 2.2 AA`, () => {
		const report = contrastReport(model, skin);

		it('measures every declared pair', () => {
			const declared = CONTRAST_PAIRS.reduce((sum, pair) => sum + pair.backgrounds.length, 0);
			expect(report.rows).toHaveLength(declared);
			expect(declared).toBeGreaterThan(100);
		});

		for (const row of report.rows) {
			const name = `${row.foreground} on ${backgroundLabel(row.background)} (${row.kind})`;
			it(`${name}: ${row.where}`, () => {
				if (row.result.kind === 'unreadable') {
					throw new Error(`${name} could not be measured: ${row.result.reason}`);
				}
				const ratio = row.result.ratio;
				expect(
					ratio,
					`${name} is ${ratio.toFixed(3)}:1; it needs ${String(row.minimum)}:1`
				).toBeGreaterThanOrEqual(row.minimum);
				expect(row.result.verdict).toBe('pass');
			});
		}
	});
}

describe('the pairs are complete', () => {
	const declaredForegrounds = new Set<string>(CONTRAST_PAIRS.map((pair) => pair.foreground));
	const declaredBackgrounds = new Set<string>(
		CONTRAST_PAIRS.flatMap((pair) =>
			pair.backgrounds.flatMap((background) =>
				typeof background === 'string' ? [background] : [background.layer, background.over]
			)
		)
	);
	const filled = new Set<string>(FILLED_PARTS.map((part) => part.token));
	const decorative = new Set<string>(DECORATIVE_TOKENS.map((entry) => entry.token));

	it('finds the colour tokens', () => {
		expect(colourTokenNames(model).length).toBeGreaterThan(50);
	});

	it('uses every colour token as a foreground, a background, a filled part or a decorative one', () => {
		const unused = colourTokenNames(model).filter(
			(name) =>
				!declaredForegrounds.has(name) &&
				!declaredBackgrounds.has(name) &&
				!filled.has(name) &&
				!decorative.has(name)
		);
		expect(unused).toEqual([]);
	});

	it('gives every decorative token a reason', () => {
		const unexplained = DECORATIVE_TOKENS.filter((entry) => entry.why.trim() === '');
		expect(unexplained).toEqual([]);
	});

	it('keeps a decorative token out of every pair', () => {
		const covered = [...decorative].filter(
			(name) => declaredForegrounds.has(name) || declaredBackgrounds.has(name)
		);
		expect(covered).toEqual([]);
	});

	it('names only tokens the model holds', () => {
		const named: readonly string[] = [
			...CONTRAST_PAIRS.map((pair) => pair.foreground),
			...declaredBackgrounds,
			...FILLED_PARTS.map((part) => part.token),
			...DECORATIVE_TOKENS.map((entry) => entry.token),
			...EXCLUDED_PAIRS.flatMap((pair) => [
				pair.foreground,
				...(typeof pair.background === 'string'
					? [pair.background]
					: [pair.background.layer, pair.background.over])
			])
		];
		expect(named.filter((name) => !hasToken(model, name))).toEqual([]);
	});

	it('lists every excluded pair with a reason and declares none of them', () => {
		expect(EXCLUDED_PAIRS.length).toBeGreaterThan(0);
		for (const excluded of EXCLUDED_PAIRS) {
			expect(excluded.reason.trim()).not.toBe('');
			const declared = CONTRAST_PAIRS.some(
				(pair) =>
					pair.foreground === excluded.foreground &&
					pair.backgrounds.some(
						(background) => backgroundLabel(background) === backgroundLabel(excluded.background)
					)
			);
			expect(declared, `${excluded.foreground} is declared on an excluded background`).toBe(false);
		}
	});

	it('declares each foreground and background pair once', () => {
		const keys = CONTRAST_PAIRS.flatMap((pair) =>
			pair.backgrounds.map((background) => `${pair.foreground}|${backgroundLabel(background)}`)
		);
		const repeated = keys.filter((key, index) => keys.indexOf(key) !== index);
		expect(repeated).toEqual([]);
	});

	it('gives every filled part a text or non-text partner pair on its fill', () => {
		const partnerless = FILLED_PARTS.filter(
			(part) =>
				!CONTRAST_PAIRS.some((pair) =>
					pair.backgrounds.some((background) => background === part.token)
				)
		);
		expect(partnerless).toEqual([]);
	});

	it('reports the lists beside the rows', () => {
		const report = contrastReport(model, 'dark');
		expect(report.filled.map((part) => part.token)).toEqual(['color-accent', 'color-danger']);
		expect(report.decorative).toHaveLength(DECORATIVE_TOKENS.length);
		expect(report.decorative.every((entry) => entry.value !== undefined)).toBe(true);
		expect(report.excluded).toEqual(EXCLUDED_PAIRS);
	});
});

describe('the thresholds', () => {
	it.each([
		['text', 4.5],
		['largeText', 3],
		['nonText', 3]
	] as const)('puts %s at %s:1', (kind, minimum) => {
		expect(minimumRatio(kind)).toBe(minimum);
	});
});

describe('measuring one pair', () => {
	it('lays a translucent background over the opaque one beneath it', () => {
		const result = measurePair(
			model,
			'dark',
			'color-text',
			{ layer: 'color-veil', over: 'color-bg' },
			'text'
		);
		expect(result.kind).toBe('measured');
	});

	it('turns away a translucent background with nothing beneath it', () => {
		const result = measurePair(model, 'dark', 'color-text', 'color-backdrop', 'text');
		expect(result.kind).toBe('unreadable');
	});

	it('turns away a token the model does not hold', () => {
		const result = measurePair(model, 'dark', 'color-nowhere', 'color-bg', 'text');
		expect(result.kind).toBe('unreadable');
	});

	it('sets a verdict against the minimum of its kind', () => {
		const text = measurePair(model, 'dark', 'color-border-strong', 'color-pile-top', 'text');
		const edge = measurePair(model, 'dark', 'color-border-strong', 'color-pile-top', 'nonText');
		expect(text.kind === 'measured' && text.verdict).toBe('fail');
		expect(edge.kind === 'measured' && edge.verdict).toBe('fail');
		if (edge.kind === 'measured') expect(edge.ratio).toBeCloseTo(2.8, 1);
	});
});

describe('the spec bites', () => {
	it('fails a pair when a token value is made too faint', () => {
		const dimmed = tokensText.replace('"light": "#656368"', '"light": "#9a9a9a"');
		expect(dimmed).not.toBe(tokensText);
		const faint = contrastReport(modelOf(dimmed), 'light');
		const failing = faint.rows.filter(
			(row) => row.result.kind === 'measured' && row.result.verdict === 'fail'
		);
		expect(failing.length).toBeGreaterThan(0);
		expect(failing.every((row) => row.foreground === 'color-text-muted')).toBe(true);
	});
});
