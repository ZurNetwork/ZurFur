import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parseCss } from 'svelte/compiler';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { parseTokenModel, type FontFile, type TokenModel } from '../lib/styles/token-model';

const resolver = createRequire(import.meta.url);
const stylesFolder = new URL('../lib/styles/', import.meta.url);

/** The Fontsource stylesheets the styles entry loads, in the order it loads them. */
const FONT_IMPORTS: readonly string[] = [
	'@fontsource-variable/atkinson-hyperlegible-next',
	'@fontsource-variable/atkinson-hyperlegible-next/wght-italic.css',
	'@fontsource-variable/atkinson-hyperlegible-mono'
];

/** The suffix Fontsource's variable packages add to a family's name. */
const VARIABLE_SUFFIX = ' Variable';

interface WeightRange {
	readonly low: number;
	readonly high: number;
}

/** One `@font-face` rule, read from the stylesheet `stylesheet` names. */
interface Face {
	readonly stylesheet: string;
	readonly family: string;
	readonly style: string;
	readonly weight: WeightRange;
	readonly src: string;
}

function readStyles(relative: string): string {
	return readFileSync(new URL(relative, stylesFolder), 'utf8');
}

function modelOf(): TokenModel {
	const tokens: unknown = JSON.parse(readStyles('tokens.json'));
	const additions: unknown = JSON.parse(readStyles('tokens.additions.json'));
	const outcome = parseTokenModel(tokens, additions);
	if (outcome.kind === 'refused') {
		throw new Error(outcome.refusals.map((refusal) => refusal.detail).join('; '));
	}
	return outcome.model;
}

/** A `font-weight` value, one weight or a `low high` range, as a range; undefined otherwise. */
function weightRangeOf(text: string): WeightRange | undefined {
	const parts = text.trim().split(/\s+/).map(Number);
	const [low, high = low] = parts;
	const isWeight = (value: number | undefined): value is number =>
		value !== undefined && Number.isInteger(value) && value >= 1 && value <= 1000;
	if (parts.length > 2 || !isWeight(low) || !isWeight(high) || low > high) return undefined;
	return { low, high };
}

function unquoted(text: string): string {
	return text.trim().replace(/^(['"])(.*)\1$/s, '$2');
}

/** Every `@font-face` rule at the top of `css`; a rule missing a field fails the spec. */
function facesIn(css: string, stylesheet: string): readonly Face[] {
	const sheet = parseCss(css);
	return sheet.children.flatMap((node) => {
		if (node.type !== 'Atrule' || node.name !== 'font-face' || node.block === null) return [];
		const fields = new Map(
			node.block.children.flatMap((child) =>
				child.type === 'Declaration' ? [[child.property, child.value] as const] : []
			)
		);
		const family = fields.get('font-family');
		const style = fields.get('font-style');
		const weightText = fields.get('font-weight');
		const src = fields.get('src');
		const weight = weightText === undefined ? undefined : weightRangeOf(weightText);
		if (family === undefined || style === undefined || weight === undefined || src === undefined) {
			throw new Error(`${stylesheet}: a @font-face rule lacks a family, style, weight or src`);
		}
		return [{ stylesheet, family: unquoted(family), style, weight, src }];
	});
}

/** True when `face` serves `font`: its variable family, the same style, a range covering the font's. */
function serves(face: Face, font: FontFile): boolean {
	const wanted = weightRangeOf(font.weight);
	if (wanted === undefined) return false;
	const sameFamily = face.family === `${font.family}${VARIABLE_SUFFIX}`;
	const covers = face.weight.low <= wanted.low && face.weight.high >= wanted.high;
	return sameFamily && face.style === font.style && covers;
}

/** The token fonts no face serves. */
function unservedFonts(fonts: readonly FontFile[], faces: readonly Face[]): readonly FontFile[] {
	return fonts.filter((font) => !faces.some((face) => serves(face, font)));
}

/** The package imports of a module's text: every specifier that is not a relative path. */
function packageImportsOf(text: string): readonly string[] {
	const imported = ts.preProcessFile(text, true, true).importedFiles;
	return imported.map((file) => file.fileName).filter((name) => !name.startsWith('.'));
}

function facesOf(specifier: string): readonly Face[] {
	const stylesheet = resolver.resolve(specifier);
	return facesIn(readFileSync(stylesheet, 'utf8'), specifier);
}

const model = modelOf();
const entryImports = packageImportsOf(readStyles('index.ts'));
const faces = entryImports.flatMap(facesOf);

describe('the fonts', () => {
	it('imports exactly the three Fontsource stylesheets into the styles entry', () => {
		expect(entryImports).toEqual(FONT_IMPORTS);
	});

	it('names three fonts in the tokens', () => {
		expect(model.type.fonts).toHaveLength(3);
	});

	it('serves every token font with a Fontsource face', () => {
		expect(faces.length).toBeGreaterThan(0);
		expect(unservedFonts(model.type.fonts, faces)).toEqual([]);
	});

	it("loads every face from the package's own files", () => {
		const remote = faces.filter((face) => !/^url\(\.\/files\/[a-z0-9-]+\.woff2\)/.test(face.src));
		expect(remote).toEqual([]);
	});

	describe('the face match', () => {
		const font: FontFile = {
			family: 'Atkinson Hyperlegible Next',
			file: 'fonts/x.woff2',
			weight: '200 800',
			style: 'italic'
		};
		const face: Face = {
			stylesheet: 'x.css',
			family: 'Atkinson Hyperlegible Next Variable',
			style: 'italic',
			weight: { low: 200, high: 800 },
			src: 'url(./files/x.woff2)'
		};

		it('serves a font from a matching face', () => {
			expect(unservedFonts([font], [face])).toEqual([]);
		});

		it.each([
			['another style', { ...face, style: 'normal' }],
			['the family without the variable suffix', { ...face, family: font.family }],
			['another family', { ...face, family: 'Atkinson Hyperlegible Mono Variable' }],
			['a narrower range at the bottom', { ...face, weight: { low: 300, high: 800 } }],
			['a narrower range at the top', { ...face, weight: { low: 200, high: 700 } }]
		])('refuses a face with %s', (_label, planted) => {
			expect(unservedFonts([font], [planted])).toEqual([font]);
		});

		it('refuses a font whose weight is not a range', () => {
			const unreadable: FontFile = { ...font, weight: 'bold' };
			expect(unservedFonts([unreadable], [face])).toEqual([unreadable]);
		});
	});

	describe('the face reader', () => {
		const rule = (body: string): string => `@font-face {\n${body}\n}\n`;
		const complete = [
			"font-family: 'A Variable';",
			'font-style: normal;',
			'font-weight: 200 800;',
			'src: url(./files/a.woff2);'
		];

		it('reads a rule', () => {
			const [face] = facesIn(rule(complete.join('\n')), 'a.css');
			expect(face).toEqual({
				stylesheet: 'a.css',
				family: 'A Variable',
				style: 'normal',
				weight: { low: 200, high: 800 },
				src: 'url(./files/a.woff2)'
			});
		});

		it.each(complete.map((line) => [line]))('fails a rule without %s', (line) => {
			const text = rule(complete.filter((kept) => kept !== line).join('\n'));
			expect(() => facesIn(text, 'a.css')).toThrow(/lacks/);
		});
	});

	it('finds a font import added beside the three', () => {
		const text = `${readStyles('index.ts')}import '@fontsource/inter';\n`;
		expect(packageImportsOf(text)).not.toEqual(FONT_IMPORTS);
	});
});
