/**
 * Every colour pair the design system claims at WCAG 2.2 AA, named by token,
 * and the report that measures them in a skin: each pair's ratio, minimum and
 * verdict, plus the filled parts, the decorative tokens and the excluded pair
 * with their reasons. Values come through the token model, so the same pairs
 * serve the spec, the Colour page and a browser harness.
 */
import { findToken, resolveToken, type TokenModel } from '../../lib/styles/token-model';
import type { Skin } from '../../lib/styles/environment';
import { composite, contrastRatio, parseColor, type Read, type Rgba } from './contrast';

/** A colour token's name as the token file writes it, with no leading dashes. */
export type ColorTokenName = `color-${string}`;

/** Which minimum applies: text, large text (24 px, or 18.66 px bold), or a non-text part. */
export type PairKind = 'text' | 'largeText' | 'nonText';

/** What a foreground sits on: an opaque token, or a translucent one laid over an opaque one. */
export type Background =
	ColorTokenName | { readonly layer: ColorTokenName; readonly over: ColorTokenName };

/** A foreground colour, the backgrounds it sits on, and where. */
export interface ContrastPair {
	readonly foreground: ColorTokenName;
	readonly backgrounds: readonly Background[];
	readonly kind: PairKind;
	readonly where: string;
}

/** A colour that fills a part; its text pairs are declared on the colour that sits on it. */
export interface FilledPart {
	readonly token: ColorTokenName;
	readonly part: string;
}

/** A token that no pair needs to cover, and why. */
export interface DecorativeToken {
	readonly token: ColorTokenName;
	readonly why: string;
}

/** A pair the design system documents as never used, and why it is not measured. */
export interface ExcludedPair {
	readonly foreground: ColorTokenName;
	readonly background: Background;
	readonly reason: string;
}

/** The lowest contrast ratio each kind may have (success criteria 1.4.3 and 1.4.11). */
export function minimumRatio(kind: PairKind): number {
	switch (kind) {
		case 'text':
			return 4.5;
		case 'largeText':
			return 3;
		case 'nonText':
			return 3;
	}
}

/** The surfaces: the two steps and the aliases that resolve to them. */
export const SURFACES: readonly ColorTokenName[] = [
	'color-surface',
	'color-surface-raised',
	'color-sheet',
	'color-note',
	'color-object'
];

/** Every soft fill a text colour may land on. */
export const SOFT_FILLS: readonly ColorTokenName[] = [
	'color-danger-soft',
	'color-secondary-iris-soft',
	'color-secondary-crown-soft',
	'color-secondary-rose-soft',
	'color-secondary-taupe-soft',
	'color-status-neutral-soft',
	'color-status-progress-soft',
	'color-status-waiting-soft',
	'color-status-done-soft',
	'color-visibility-private-soft',
	'color-visibility-listed-soft',
	'color-visibility-public-soft'
];

/** Every background that ink (`color-text`, its muted step, gold text) lands on. */
const INK_BACKGROUNDS: readonly Background[] = [
	'color-bg',
	...SURFACES,
	'color-pile-top',
	'color-hover',
	'color-selected',
	...SOFT_FILLS
];

/** Every background the focus ring lands on. */
const FOCUS_BACKGROUNDS: readonly Background[] = [
	'color-bg',
	...SURFACES,
	'color-hover',
	'color-selected',
	'color-pile-top'
];

/** A coloured text token on the surfaces and on its own soft tint. */
function tinted(foreground: ColorTokenName, soft: ColorTokenName, where: string): ContrastPair {
	return { foreground, backgrounds: [...SURFACES, soft], kind: 'text', where };
}

/** The declared pairs. */
export const CONTRAST_PAIRS: readonly ContrastPair[] = [
	{
		foreground: 'color-text',
		backgrounds: INK_BACKGROUNDS,
		kind: 'text',
		where: 'body and UI text on the desk, every surface, a pile top, hover, selection and soft fill'
	},
	{
		foreground: 'color-text-muted',
		backgrounds: INK_BACKGROUNDS,
		kind: 'text',
		where: 'timestamps, hints, handles, tree meta and the locked look, on the same backgrounds'
	},
	{
		foreground: 'color-accent-text',
		backgrounds: INK_BACKGROUNDS,
		kind: 'text',
		where: 'links, the selected option of the pill switch and the open tree item icon'
	},
	{
		foreground: 'color-on-accent',
		backgrounds: ['color-accent'],
		kind: 'text',
		where: 'the primary button label, a checked tick and the Toggle thumb on the gold fill'
	},
	{
		foreground: 'color-on-danger',
		backgrounds: ['color-danger'],
		kind: 'text',
		where: 'the label of the Delete button'
	},
	{
		foreground: 'color-danger-text',
		backgrounds: [...SURFACES, 'color-danger-soft'],
		kind: 'text',
		where: 'a failed upload or a rejected field, on the surfaces and in an error Toast'
	},
	tinted(
		'color-secondary-iris',
		'color-secondary-iris-soft',
		'tags, avatar initials, the plugin frame label'
	),
	tinted('color-secondary-crown', 'color-secondary-crown-soft', 'tags and avatars'),
	tinted('color-secondary-rose', 'color-secondary-rose-soft', 'tags and avatars'),
	tinted('color-secondary-taupe', 'color-secondary-taupe-soft', 'tags and avatars'),
	tinted('color-status-neutral', 'color-status-neutral-soft', 'neutral stages in a StatusPill'),
	tinted('color-status-progress', 'color-status-progress-soft', 'moving stages in a StatusPill'),
	tinted('color-status-waiting', 'color-status-waiting-soft', 'waiting stages in a StatusPill'),
	tinted('color-status-done', 'color-status-done-soft', 'finished stages and the success Toast'),
	tinted('color-visibility-private', 'color-visibility-private-soft', 'the Private marker chip'),
	tinted('color-visibility-listed', 'color-visibility-listed-soft', 'the Listed marker chip'),
	tinted('color-visibility-public', 'color-visibility-public-soft', 'the Public marker chip'),
	{
		foreground: 'color-on-pin',
		backgrounds: ['color-pin'],
		kind: 'text',
		where: 'the number on a Pin'
	},
	{
		foreground: 'color-border-strong',
		backgrounds: ['color-bg', ...SURFACES, 'color-hover'],
		kind: 'nonText',
		where: 'the edge of an input, select, checkbox, secondary button, off Toggle or query box'
	},
	{
		foreground: 'color-focus',
		backgrounds: FOCUS_BACKGROUNDS,
		kind: 'nonText',
		where: 'the keyboard focus ring, on whatever surrounds the control'
	},
	{
		foreground: 'color-accent-border',
		backgrounds: SURFACES,
		kind: 'nonText',
		where: 'the edge of the primary button'
	},
	{
		foreground: 'color-danger',
		backgrounds: SURFACES,
		kind: 'nonText',
		where: 'the Delete button against the surface it sits on'
	}
];

/** Colours that fill a part; the text on each is checked by the colour that sits on it. */
export const FILLED_PARTS: readonly FilledPart[] = [
	{ token: 'color-accent', part: 'the primary button, a checked Checkbox or Toggle, the Pin' },
	{ token: 'color-danger', part: 'the Delete button' }
];

/** Tokens drawn only where nothing needs to be told apart. */
export const DECORATIVE_TOKENS: readonly DecorativeToken[] = [
	{
		token: 'color-border',
		why: 'hairlines between panes, card outlines and dividers; never the only edge of a control'
	},
	{ token: 'color-peek-into', why: 'what a Pile peeks fade into; never text' },
	{
		token: 'color-backdrop',
		why: 'dims the page behind the phone drawer and a modal Dialog, which carries its own surface'
	},
	{
		token: 'color-veil',
		why: 'a neutral wash over blurred mature art, under the MatureCover panel that carries the words'
	},
	{ token: 'color-skeleton', why: 'a placeholder fill that holds no text' },
	{
		token: 'color-pin-ring',
		why: 'the inner ring of a Pin, the same in both skins so the pin holds on light art'
	},
	{
		token: 'color-pin-halo',
		why: 'the outer halo of a Pin, the same in both skins so the pin holds on dark art'
	}
];

/** Pairs measured nowhere, each with its reason; none is also declared in `CONTRAST_PAIRS`. */
export const EXCLUDED_PAIRS: readonly ExcludedPair[] = [
	{
		foreground: 'color-border-strong',
		background: 'color-pile-top',
		reason:
			'documented as never used: a strong edge does not hold 3:1 on a pile top in dark (about 2.8:1), so none sits on it'
	}
];

/** A background as a short label, `layer over under` for a translucent one. */
export function backgroundLabel(background: Background): string {
	return typeof background === 'string'
		? background
		: `${background.layer} over ${background.over}`;
}

/** What measuring one pair gave. */
export type RowResult =
	| { readonly kind: 'measured'; readonly ratio: number; readonly verdict: 'pass' | 'fail' }
	| { readonly kind: 'unreadable'; readonly reason: string };

/** One declared pair on one background, measured. */
export interface PairRow {
	readonly foreground: ColorTokenName;
	readonly background: Background;
	readonly kind: PairKind;
	readonly where: string;
	readonly minimum: number;
	readonly result: RowResult;
}

/** A filled part with its colour in the skin. */
export interface FilledRow extends FilledPart {
	readonly value: string | undefined;
}

/** A decorative token with its value in the skin. */
export interface DecorativeRow extends DecorativeToken {
	readonly value: string | undefined;
}

/** Every declared pair measured in one skin, and the lists that go with them. */
export interface ContrastReport {
	readonly skin: Skin;
	readonly rows: readonly PairRow[];
	readonly filled: readonly FilledRow[];
	readonly decorative: readonly DecorativeRow[];
	readonly excluded: readonly ExcludedPair[];
}

function unreadable<T>(reason: string): Read<T> {
	return { kind: 'unreadable', reason };
}

function colourOf(model: TokenModel, name: ColorTokenName, skin: Skin): Read<Rgba> {
	const text = resolveToken(model, name, skin);
	if (text === undefined) return unreadable(`no token named ${name}`);
	return parseColor(text);
}

function opaqueColourOf(model: TokenModel, name: ColorTokenName, skin: Skin): Read<Rgba> {
	const colour = colourOf(model, name, skin);
	if (colour.kind === 'unreadable') return colour;
	if (colour.value.alpha !== 1) {
		return unreadable(`${name} is translucent; declare what it lies over`);
	}
	return colour;
}

function backgroundColourOf(model: TokenModel, background: Background, skin: Skin): Read<Rgba> {
	if (typeof background === 'string') return opaqueColourOf(model, background, skin);
	const under = opaqueColourOf(model, background.over, skin);
	if (under.kind === 'unreadable') return under;
	const layer = colourOf(model, background.layer, skin);
	if (layer.kind === 'unreadable') return layer;
	return { kind: 'read', value: composite(layer.value, under.value) };
}

/** One foreground on one background in a skin, against the minimum its kind needs. */
export function measurePair(
	model: TokenModel,
	skin: Skin,
	foreground: ColorTokenName,
	background: Background,
	kind: PairKind
): RowResult {
	const minimum = minimumRatio(kind);
	const under = backgroundColourOf(model, background, skin);
	if (under.kind === 'unreadable') return under;
	const ink = colourOf(model, foreground, skin);
	if (ink.kind === 'unreadable') return ink;
	const ratio = contrastRatio(composite(ink.value, under.value), under.value);
	return { kind: 'measured', ratio, verdict: ratio >= minimum ? 'pass' : 'fail' };
}

/** Every declared pair measured in a skin, with the filled, decorative and excluded lists. */
export function contrastReport(model: TokenModel, skin: Skin): ContrastReport {
	const rows = CONTRAST_PAIRS.flatMap((pair) =>
		pair.backgrounds.map((background): PairRow => {
			const minimum = minimumRatio(pair.kind);
			return {
				foreground: pair.foreground,
				background,
				kind: pair.kind,
				where: pair.where,
				minimum,
				result: measurePair(model, skin, pair.foreground, background, pair.kind)
			};
		})
	);
	const filled = FILLED_PARTS.map((part) => ({
		...part,
		value: resolveToken(model, part.token, skin)
	}));
	const decorative = DECORATIVE_TOKENS.map((entry) => ({
		...entry,
		value: resolveToken(model, entry.token, skin)
	}));
	return { skin, rows, filled, decorative, excluded: EXCLUDED_PAIRS };
}

/** The colour tokens of a model, in file order. */
export function colourTokenNames(model: TokenModel): readonly string[] {
	const group = model.groups.find((entry) => entry.id === 'color');
	return group === undefined ? [] : group.tokens.map((token) => token.name);
}

/** True when the model has a token with this name. */
export function hasToken(model: TokenModel, name: string): boolean {
	return findToken(model, name) !== undefined;
}
