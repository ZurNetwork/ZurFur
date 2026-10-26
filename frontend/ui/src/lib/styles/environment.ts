/** The attribute that picks a subtree's skin, on `<html>` or any element below it. */
export const SKIN_ATTRIBUTE = 'data-skin';

/** The two skins a page can wear. */
export type Skin = 'dark' | 'light';

/**
 * Reads a skin from raw attribute text: exactly `dark` or `light`, nothing
 * else (no case folding, no trimming), and `undefined` for any other text.
 */
export function parseSkin(raw: string | undefined): Skin | undefined {
	if (raw === 'dark') return 'dark';
	if (raw === 'light') return 'light';
	return undefined;
}

/** The attribute that sets a subtree's motion mode. */
export const MOTION_ATTRIBUTE = 'data-motion';

/** The motion modes: `lively` is the default, `still` stops everything on its own. */
export type MotionMode = 'lively' | 'still';
