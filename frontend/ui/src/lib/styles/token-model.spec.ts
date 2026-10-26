import { describe, expect, it } from 'vitest';
import additions from './tokens.additions.json';
import tokens from './tokens.json';
import {
	findToken,
	isTokenName,
	parseTokenModel,
	resolveToken,
	type ModelRefusalReason,
	type ParseOutcome,
	type Refusal,
	type TokenModel
} from './token-model';

type Fields = Record<string, unknown>;

const token = (name: string, value: unknown): Fields => ({
	name,
	value,
	usage: 'Used for a test.'
});
const group = (...entries: Fields[]): Fields => ({ tokens: entries });

/** A small valid token file; the overrides replace whole sections. */
function fixture(over: Fields = {}): Fields {
	return {
		name: 'Test',
		version: 1,
		color: {
			themes: [
				{ id: 'dark', name: 'Dark' },
				{ id: 'light', name: 'Light' }
			],
			tokens: [token('color-bg', { dark: '#000000', light: '#ffffff' })]
		},
		type: { fonts: [], families: { ui: 'sans-serif', mono: 'monospace' }, groups: [] },
		sound: group(token('sound-open', 'triangle 660Hz')),
		...over
	};
}

function parsedOrFail(outcome: ParseOutcome): TokenModel {
	if (outcome.kind === 'refused') throw new Error(JSON.stringify(outcome.refusals));
	return outcome.model;
}

function reasons(outcome: ParseOutcome): readonly ModelRefusalReason[] {
	return outcome.kind === 'refused' ? outcome.refusals.map((refusal) => refusal.reason) : [];
}

function refusalsOf(outcome: ParseOutcome): readonly Refusal[] {
	return outcome.kind === 'refused' ? outcome.refusals : [];
}

/** A type section with one group of one style; the overrides replace the style's fields. */
function typeSection(family: string, style: Fields = {}): Fields {
	const heading = {
		name: 'Heading',
		fontSize: '24px',
		lineHeight: '32px',
		fontWeight: 600,
		letterSpacing: '-0.01em',
		sample: 'A heading',
		usage: 'Used for a test.',
		...style
	};
	const typeGroup = { name: 'UI', family, note: 'A note.', styles: [heading] };
	return {
		type: { fonts: [], families: { ui: 'sans-serif', mono: 'monospace' }, groups: [typeGroup] }
	};
}

const none = {};

describe('the real token files', () => {
	const model = parsedOrFail(parseTokenModel(tokens, additions));

	it('follows a reference in each skin', () => {
		expect(resolveToken(model, 'color-sheet', 'dark')).toBe(
			resolveToken(model, 'color-surface', 'dark')
		);
		expect(resolveToken(model, 'color-sheet', 'light')).toBe(
			resolveToken(model, 'color-surface-raised', 'light')
		);
		expect(resolveToken(model, 'color-sheet', 'dark')).not.toBe(
			resolveToken(model, 'color-sheet', 'light')
		);
	});

	it('follows a chain of references', () => {
		expect(resolveToken(model, 'color-visibility-private', 'dark')).toBe(
			resolveToken(model, 'color-secondary-rose', 'dark')
		);
		expect(resolveToken(model, 'color-visibility-private', 'light')).toBe('#904938');
	});

	it('gives a plain token the same text in both skins', () => {
		expect(resolveToken(model, 'space-4', 'dark')).toBe('16px');
		expect(resolveToken(model, 'space-4', 'light')).toBe('16px');
	});

	it('merges the additions into their group and marks where they came from', () => {
		const linear = findToken(model, 'easing-linear');
		expect([linear?.groupId, linear?.source, linear?.resolved.dark]).toEqual([
			'easing',
			'additions',
			'linear'
		]);
		expect(findToken(model, 'easing-spring')?.source).toBe('tokens');
		expect(model.groups.find((entry) => entry.id === 'easing')?.tokens.at(-1)?.name).toBe(
			'easing-linear'
		);
	});

	it('keeps sound apart from the groups, and still finds its tokens by name', () => {
		expect(model.groups.map((entry) => entry.id)).not.toContain('sound');
		expect(model.sound.tokens).toHaveLength(8);
		expect(findToken(model, 'sound-open')?.groupId).toBe('sound');
	});

	it('carries the themes and the type section', () => {
		expect(model.themes.map((theme) => theme.id)).toEqual(['dark', 'light']);
		expect(model.type.groups.flatMap((entry) => entry.styles)).toHaveLength(9);
		expect(model.type.families.ui).toContain('Atkinson Hyperlegible Next');
		expect(model.type.fonts).toHaveLength(3);
	});

	it('finds no token that is not there', () => {
		expect(findToken(model, 'nope')).toBeUndefined();
		expect(findToken(model, 'constructor')).toBeUndefined();
		expect(resolveToken(model, '__proto__', 'dark')).toBeUndefined();
	});
});

describe('references', () => {
	it('are followed per skin through a skinned target', () => {
		const model = parsedOrFail(
			parseTokenModel(
				fixture({
					radius: group(token('radius-a', '{color-bg}'), token('radius-b', '{radius-a}'))
				}),
				none
			)
		);
		expect(resolveToken(model, 'radius-b', 'dark')).toBe('#000000');
		expect(resolveToken(model, 'radius-b', 'light')).toBe('#ffffff');
	});

	it('refuse a cycle', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', '{radius-b}'), token('radius-b', '{radius-a}')) }),
			none
		);
		expect(reasons(outcome)).toContain('cyclic-reference');
	});

	it('refuse a token that refers to itself', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', '{radius-a}')) }),
			none
		);
		expect(reasons(outcome)).toContain('cyclic-reference');
	});

	it('refuse a cycle that only shows in one skin', () => {
		const outcome = parseTokenModel(
			fixture({
				color: {
					themes: [
						{ id: 'dark', name: 'Dark' },
						{ id: 'light', name: 'Light' }
					],
					tokens: [
						token('color-a', { dark: '#000000', light: '{color-b}' }),
						token('color-b', { dark: '#000000', light: '{color-a}' })
					]
				}
			}),
			none
		);
		expect(reasons(outcome)).toContain('cyclic-reference');
	});

	it('refuse a name nothing has', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', '{missing}')) }),
			none
		);
		expect(reasons(outcome)).toEqual(['dangling-reference', 'dangling-reference']);
	});

	it('refuse a reference inside a longer value', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', 'calc({color-bg} + 1px)')) }),
			none
		);
		expect(reasons(outcome)).toEqual(['embedded-reference']);
	});
});

describe('the token files', () => {
	it('refuse a bad name', () => {
		for (const name of ['x;y', 'X', '1x', '-x', 'a b', '']) {
			expect(
				reasons(parseTokenModel(fixture({ radius: group(token(name, '1px')) }), none))
			).toEqual(['bad-name']);
		}
	});

	it('read names with digits and hyphens', () => {
		expect(isTokenName('space-2xl')).toBe(true);
		expect(isTokenName('Space')).toBe(false);
		expect(isTokenName('x\n')).toBe(false);
	});

	it('refuse two tokens with one name', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', '1px'), token('radius-a', '2px')) }),
			none
		);
		expect(reasons(outcome)).toEqual(['duplicate-name']);
	});

	it('refuse a skinned value that lacks a skin', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', { dark: '1px' })) }),
			none
		);
		expect(reasons(outcome)).toEqual(['missing-skin']);
	});

	it('refuse a skinned value keyed by something that is not a theme', () => {
		const outcome = parseTokenModel(
			fixture({ radius: group(token('radius-a', { dark: '1px', light: '1px', sepia: '1px' })) }),
			none
		);
		expect(reasons(outcome)).toEqual(['unknown-theme']);
	});

	it('refuse a theme list that is not exactly dark and light', () => {
		const themes = (...ids: string[]): Fields => ({
			color: { themes: ids.map((id) => ({ id, name: id })), tokens: [] }
		});
		for (const ids of [['dark'], ['dark', 'dark', 'light'], ['dark', 'light', 'sepia'], []]) {
			expect(reasons(parseTokenModel(fixture(themes(...ids)), none))).toContain('bad-themes');
		}
	});

	it('refuse an addition whose name the token file has', () => {
		const outcome = parseTokenModel(fixture({ radius: group(token('radius-a', '1px')) }), {
			radius: group(token('radius-a', '2px'))
		});
		expect(reasons(outcome)).toEqual(['addition-exists']);
	});

	it('take an addition that names a group the token file lacks as a group of its own', () => {
		const model = parsedOrFail(
			parseTokenModel(fixture(), { shape: group(token('shape-a', 'circle')) })
		);
		expect(model.groups.at(-1)?.id).toBe('shape');
	});

	it.each([
		['nothing', undefined],
		['a number', 7],
		['a list', []],
		['text', 'tokens']
	])('refuse %s in place of the file, without throwing', (_label, input) => {
		expect(reasons(parseTokenModel(input, none))).toEqual(['malformed']);
		expect(reasons(parseTokenModel(fixture(), input))).toEqual(['malformed']);
	});

	it.each([
		['a group that is a list', { radius: [] }],
		['a group without tokens', { radius: {} }],
		['a token that is text', { radius: { tokens: ['x'] } }],
		['a token without a name', { radius: { tokens: [{ value: '1px', usage: 'u' }] } }],
		['a token without a usage line', { radius: { tokens: [{ name: 'radius-a', value: '1px' }] } }],
		['a value that is a number', { radius: group(token('radius-a', 4)) }],
		[
			'a skinned value holding a number',
			{ radius: group(token('radius-a', { dark: 1, light: 1 })) }
		],
		['a missing type section', { type: undefined }],
		['a type section without families', { type: { fonts: [], groups: [] } }],
		['a missing sound group', { sound: undefined }],
		['a name that is not text', { name: 7 }]
	])('refuse %s', (_label, over) => {
		expect(reasons(parseTokenModel(fixture(over), none))).toContain('malformed');
	});

	it.each([
		[
			'letterSpacing with a second declaration',
			'letterSpacing',
			'-0.01em; background-image: url(https://x.invalid/type.png)'
		],
		['fontSize with a second declaration', 'fontSize', '16px;color:red'],
		['lineHeight with !important', 'lineHeight', '24px !important'],
		['fontSize with a newline', 'fontSize', '16px\n']
	])('refuse a type style whose %s', (_label, field, value) => {
		const outcome = parseTokenModel(fixture(typeSection('ui', { [field]: value })), none);
		const expected = [
			{
				reason: 'value-off-allow-list',
				where: `type.groups[0].styles[0].${field}`,
				detail: `"${field}" is not on the allow-list`
			}
		];
		expect(refusalsOf(outcome)).toEqual(expected);
	});

	it.each(['ui);color:red;x:var(--a', 'Ui'])(
		'refuse a type group whose family %j is not a token name',
		(family) => {
			const outcome = parseTokenModel(fixture(typeSection(family)), none);
			const expected = [
				{
					reason: 'bad-name',
					where: 'type.groups[0].family',
					detail: '"family" must be a token name, such as ui'
				}
			];
			expect(refusalsOf(outcome)).toEqual(expected);
		}
	);

	it('read a type style with the design system values', () => {
		const model = parsedOrFail(parseTokenModel(fixture(typeSection('mono')), none));
		const style = model.type.groups[0]?.styles[0];
		expect([model.type.groups[0]?.family, style?.fontSize, style?.letterSpacing]).toEqual([
			'mono',
			'24px',
			'-0.01em'
		]);
	});

	it('read a name such as constructor as an ordinary group name', () => {
		const model = parsedOrFail(
			parseTokenModel(
				fixture(),
				JSON.parse('{"constructor": {"tokens": []}, "__proto__": {"tokens": []}}')
			)
		);
		expect(model.groups.map((entry) => entry.id)).toContain('constructor');
	});
});
