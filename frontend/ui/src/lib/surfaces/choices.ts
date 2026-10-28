/** The run-time choices the surface pieces make: each takes any value and falls back to the safe one. */
import { isIconName, type IconName } from '../icons/icons';
import type { AvatarSize, BadgeTone, CardElement, StatusTone, VisibilityLevel } from './types';

/** The most bars a Skeleton draws as lines. */
export const SKELETON_LINES_CAP = 12;

/** The element a Card becomes: one of the four it allows, otherwise a `div`. */
export function cardElementOf(value: unknown): CardElement {
	switch (value) {
		case 'section':
			return 'section';
		case 'article':
			return 'article';
		case 'li':
			return 'li';
		default:
			return 'div';
	}
}

/** The tone a Badge wears: one of the six, otherwise neutral. */
export function badgeToneOf(value: unknown): BadgeTone {
	switch (value) {
		case 'accent':
			return 'accent';
		case 'iris':
			return 'iris';
		case 'crown':
			return 'crown';
		case 'rose':
			return 'rose';
		case 'taupe':
			return 'taupe';
		default:
			return 'neutral';
	}
}

/** The tone a status part wears: one of the four, otherwise neutral. */
export function statusToneOf(value: unknown): StatusTone {
	switch (value) {
		case 'progress':
			return 'progress';
		case 'waiting':
			return 'waiting';
		case 'done':
			return 'done';
		default:
			return 'neutral';
	}
}

/** The visibility level named, or nothing when it is not one of the three. */
export function visibilityOf(value: unknown): VisibilityLevel | undefined {
	switch (value) {
		case 'private':
			return 'private';
		case 'listed':
			return 'listed';
		case 'public':
			return 'public';
		default:
			return undefined;
	}
}

/** The icon each visibility level wears. */
export function visibilityIconOf(level: VisibilityLevel): IconName {
	switch (level) {
		case 'private':
			return 'eye-off';
		case 'listed':
			return 'list';
		case 'public':
			return 'globe';
	}
}

/** The icon named, or nothing when the registry does not hold it. */
export function iconOf(value: unknown): IconName | undefined {
	return typeof value === 'string' && isIconName(value) ? value : undefined;
}

/** How many bars a Skeleton draws as lines: a whole number from 1 to the cap, otherwise one. */
export function skeletonLinesOf(value: unknown): number {
	const isCounted =
		typeof value === 'number' &&
		Number.isInteger(value) &&
		value >= 1 &&
		value <= SKELETON_LINES_CAP;
	return isCounted ? value : 1;
}

/** A grapheme that starts with a letter, a number or a pictograph. */
const SHOWN_START = /^[\p{L}\p{N}\p{Extended_Pictographic}]/u;

/** The size class an Avatar wears: one of the three, otherwise `md`. */
export function avatarSizeOf(value: unknown): AvatarSize {
	switch (value) {
		case 'sm':
			return 'sm';
		case 'lg':
			return 'lg';
		default:
			return 'md';
	}
}

/** The first whole grapheme that starts with a letter, a number or a pictograph, or nothing. */
export function initialOf(text: string): string | undefined {
	const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(text);
	for (const { segment } of graphemes) {
		if (SHOWN_START.test(segment)) return segment;
	}
	return undefined;
}
