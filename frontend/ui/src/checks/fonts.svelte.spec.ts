import { describe, expect, it } from 'vitest';
import '../lib/styles/index.ts';
import tokens from '../lib/styles/tokens.json';
import additions from '../lib/styles/tokens.additions.json';
import { parseTokenModel, type FontFile, type TokenModel } from '../lib/styles/token-model';

function modelOf(): TokenModel {
	const outcome = parseTokenModel(tokens, additions);
	if (outcome.kind === 'refused') {
		throw new Error(outcome.refusals.map((refusal) => refusal.detail).join('; '));
	}
	return outcome.model;
}

/** The CSS `font` shorthand for one token font, at the lightest weight its range names. */
function shorthandOf(font: FontFile): string {
	const [lightest] = font.weight.split(' ');
	if (lightest === undefined) throw new Error(`${font.family}: no weight`);
	return `${font.style} ${lightest} 16px "${font.family} Variable"`;
}

const model = modelOf();

describe('the fonts in the browser', () => {
	it.each(model.type.fonts.map((font) => [`${font.family} ${font.style}`, font] as const))(
		'loads %s',
		async (_label, font) => {
			const shorthand = shorthandOf(font);
			const loaded = await document.fonts.load(shorthand);
			expect(loaded.length).toBeGreaterThan(0);
			expect(loaded.map((face) => [face.status, face.style])).toEqual(
				loaded.map(() => ['loaded', font.style])
			);
			expect(document.fonts.check(shorthand)).toBe(true);
		}
	);

	it("sets the body in Fontsource's family first", () => {
		const fontFamily = getComputedStyle(document.body).fontFamily;
		expect(fontFamily).toMatch(/^"Atkinson Hyperlegible Next Variable",/);
	});
});
