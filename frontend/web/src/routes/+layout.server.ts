import type { LayoutServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { sessionOrAnonymous } from '$lib/server/session';

/**
 * One whoami per server render, shared with every page and the header via
 * layout data. A dead backend renders signed-out rather than a 500 — the
 * same graceful-degradation stance the ZMVP-150 proof page took — but only
 * unreachability degrades; a broken contract still surfaces (the program's
 * remaining error channel rejects into SvelteKit's 500).
 */
export const load: LayoutServerLoad = async ({ fetch }) => {
	const session = await runApi(fetch, sessionOrAnonymous);
	return { session };
};
