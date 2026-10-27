/** Where the floating parts (the menu, the tooltip) are put, so they wear the skin of the copy they open from. */
import { SKIN_ATTRIBUTE } from '../styles/environment.ts';

/** Matches any element that sets a skin for the subtree below it. */
const SKIN_SCOPE = `[${SKIN_ATTRIBUTE}]`;

/**
 * The element a floating part opened from `anchor` portals into: the nearest
 * skin scope around it, or the body when that scope is the page itself or
 * there is none.
 */
export function portalTarget(anchor: Element): Element {
	const page = anchor.ownerDocument;
	const scope = anchor.closest(SKIN_SCOPE) ?? undefined;
	return scope === undefined || scope === page.documentElement ? page.body : scope;
}
