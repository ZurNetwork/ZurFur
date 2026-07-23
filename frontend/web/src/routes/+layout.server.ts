import type { LayoutServerLoad } from './$types';
import { anonymousWhenUnreachable, getSession } from '$lib/api/session';

/**
 * One whoami per server render, shared with every page and the header via
 * layout data. A dead backend renders signed-out rather than a 500 — the
 * same graceful-degradation stance the ZMVP-150 proof page took — but only
 * unreachability degrades; a broken contract still surfaces.
 */
export const load: LayoutServerLoad = async ({ fetch }) => {
	const session = await getSession(fetch).catch(anonymousWhenUnreachable);
	return { session };
};
