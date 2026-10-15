/**
 * Every colour pair the dark theme must hold at WCAG 2.2 AA: each text, icon,
 * control-edge and focus colour, on each background it sits on. Icons draw in
 * `currentColor`, so an icon's pair is its text's pair. The contrast spec
 * checks each pair, and fails when a token is used as a foreground without one.
 */

/** A semantic colour token from `tokens.css`. */
export type ColorToken = `--color-${string}`;

/** Which minimum applies: text, large text (24 px, or 18.66 px bold), or a non-text part. */
export type PairKind = 'text' | 'largeText' | 'nonText';

/** What a foreground sits on: an opaque token, or a translucent one laid over an opaque one. */
export type Background = ColorToken | { readonly layer: ColorToken; readonly over: ColorToken };

/** A foreground colour, the backgrounds it sits on, and where. */
export interface ContrastPair {
	readonly foreground: ColorToken;
	readonly backgrounds: readonly Background[];
	readonly kind: PairKind;
	readonly where: string;
}

/** A foreground token that paints a part with `background`, the one file that does it, and what the part is. */
export interface FilledPart {
	readonly token: ColorToken;
	readonly file: string;
	readonly part: string;
}

/** A token used in a colour declaration that no pair needs to cover, and why. */
export interface DecorativeToken {
	readonly token: ColorToken;
	readonly why: string;
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

/** The tree's current row, in the sidebar or the drawer. */
const SELECTED_ON_SURFACE: Background = { layer: '--color-selected', over: '--color-surface' };

/** Selected text in the pane. */
const SELECTED_ON_BG: Background = { layer: '--color-selected', over: '--color-bg' };

/** The declared pairs. */
export const CONTRAST_PAIRS: readonly ContrastPair[] = [
	{
		foreground: '--color-text',
		backgrounds: [
			'--color-bg',
			'--color-surface',
			'--color-surface-raised',
			'--color-hover',
			SELECTED_ON_SURFACE,
			SELECTED_ON_BG
		],
		kind: 'text',
		where:
			'body and pane text; the top bar, sidebar and drawer; tree and entry rows, hovered or current; secondary and form buttons; selected text'
	},
	{
		foreground: '--color-text-muted',
		backgrounds: [
			'--color-bg',
			'--color-surface',
			'--color-surface-raised',
			'--color-hover',
			SELECTED_ON_SURFACE,
			SELECTED_ON_BG
		],
		kind: 'text',
		where:
			"labels, the node header's facts, inert entries, empty states and badges in the pane; the path bar, section links, tree icons and hints, and View options in the bars; a text box's value; selected"
	},
	{
		foreground: '--color-accent',
		backgrounds: ['--color-bg', '--color-surface', SELECTED_ON_BG, SELECTED_ON_SURFACE],
		kind: 'text',
		where: 'links on the page, and the View options link in the sidebar and drawer; selected'
	},
	{
		foreground: '--color-on-accent',
		backgrounds: ['--color-accent'],
		kind: 'text',
		where: 'the primary button and the skip link'
	},
	{
		foreground: '--color-danger',
		backgrounds: ['--color-bg'],
		kind: 'text',
		where: "the destructive button's label"
	},
	{
		foreground: '--color-visibility-private',
		backgrounds: ['--color-bg', '--color-hover', SELECTED_ON_BG],
		kind: 'text',
		where: 'the visibility marker in the node header and on entry rows, hovered or selected'
	},
	{
		foreground: '--color-visibility-listed',
		backgrounds: ['--color-bg', '--color-hover', SELECTED_ON_BG],
		kind: 'text',
		where: 'the visibility marker in the node header and on entry rows, hovered or selected'
	},
	{
		foreground: '--color-visibility-public',
		backgrounds: ['--color-bg', '--color-hover', SELECTED_ON_BG],
		kind: 'text',
		where: 'the visibility marker in the node header and on entry rows, hovered or selected'
	},
	{
		foreground: '--color-control-edge',
		backgrounds: ['--color-bg', '--color-surface', '--color-surface-raised', '--color-hover'],
		kind: 'nonText',
		where:
			"the edge of a text box, a secondary or form button and a toggle's track: against the page or bar around it, and its own fill at rest and hovered"
	},
	{
		foreground: '--color-danger',
		backgrounds: ['--color-bg', '--color-surface-raised'],
		kind: 'nonText',
		where: "the destructive button's edge, and an invalid text box's edge outside and in"
	},
	{
		foreground: '--color-focus',
		backgrounds: ['--color-bg', '--color-surface', '--color-hover', SELECTED_ON_SURFACE],
		kind: 'nonText',
		where:
			'the focus ring, offset onto what surrounds the control: the page, the bars, and a neighbouring tree row hovered or current'
	},
	{
		foreground: '--color-accent',
		backgrounds: ['--color-surface', SELECTED_ON_SURFACE, '--color-hover'],
		kind: 'nonText',
		where:
			"the current tree row's bar, beside the sidebar and on its own row, hovered too; a checked toggle's track"
	},
	{
		foreground: '--color-on-accent',
		backgrounds: ['--color-accent'],
		kind: 'nonText',
		where: "a checked toggle's thumb on its track"
	},
	{
		foreground: '--color-text-muted',
		backgrounds: ['--color-surface-raised'],
		kind: 'nonText',
		where: "an unchecked toggle's thumb on its track"
	}
];

/** Tokens drawn only where nothing needs to be told apart. */
export const DECORATIVE_TOKENS: readonly DecorativeToken[] = [
	{
		token: '--color-border',
		why: 'dividers between bars and sections, the dialog and drawer outline, and the outline of a badge or a blank avatar: none marks a control or its state'
	},
	{
		token: '--color-backdrop',
		why: 'dims the page behind an open dialog; the dialog carries its own surface'
	}
];

/** Every token a pair may sit on, and the only tokens a `background` may name besides a filled part. */
export const BACKGROUND_TOKENS: readonly ColorToken[] = [
	'--color-bg',
	'--color-surface',
	'--color-surface-raised',
	'--color-hover',
	'--color-selected',
	'--color-accent'
];

/** Non-text parts painted with `background`, each checked by its foreground's non-text pair; paths are under `src/`. */
export const FILLED_PARTS: readonly FilledPart[] = [
	{
		token: '--color-text-muted',
		file: 'lib/components/ui/Toggle.svelte',
		part: "an unchecked toggle's thumb"
	},
	{
		token: '--color-on-accent',
		file: 'lib/components/ui/Toggle.svelte',
		part: "a checked toggle's thumb"
	}
];
