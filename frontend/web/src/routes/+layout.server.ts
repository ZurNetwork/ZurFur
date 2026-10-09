import type { LayoutServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { sessionOrAnonymous } from '$lib/server/session';

/**
 * One whoami per server render, shared with every page and the header via
 * layout data. A dead backend renders signed-out rather than a 500 — the
 * same graceful-degradation stance taken elsewhere in this seam — but only
 * unreachability degrades; a broken contract still surfaces (the program's
 * remaining error channel rejects into SvelteKit's 500). On a `(session)`
 * route the sign-in gate already asked, so its answer is reused.
 */
export const load: LayoutServerLoad = async ({ fetch, locals }) => {
	const session = locals.session ?? (await runApi(fetch, sessionOrAnonymous));
	return { session };
};
