/**
 * A Den name with its number as plain text, for places that take only text:
 * the live region, `title=` tooltips and aria labels. The markup twin is
 * `ShownName.svelte`.
 */

import type { NameOrdinal } from '$lib/types/brand';

/** U+2068 FIRST STRONG ISOLATE: opens the name's own direction run. */
const FIRST_STRONG_ISOLATE = '\u2068';

/** U+2069 POP DIRECTIONAL ISOLATE: closes it. */
const POP_DIRECTIONAL_ISOLATE = '\u2069';

/**
 * The name isolated, then " (n)" outside the isolate, so a right-to-left
 * name can't swallow the number; with no number, the isolated name alone.
 * The number's placement relies on the backend's name rule, which refuses or
 * strips bidi controls in Den names.
 */
export function shownNameText(name: string, ordinal: NameOrdinal | undefined): string {
	const isolated = `${FIRST_STRONG_ISOLATE}${name}${POP_DIRECTIONAL_ISOLATE}`;
	return ordinal === undefined ? isolated : `${isolated} (${String(ordinal)})`;
}
