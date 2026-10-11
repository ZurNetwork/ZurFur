import type { LayoutServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { sessionOrAnonymous } from '$lib/server/session';
import { denServed } from '$lib/server/den-served';

/**
 * One whoami per server render, shared with every page and the header via
 * layout data. A dead backend renders signed-out rather than a 500 — the
 * same graceful-degradation stance taken elsewhere in this seam — but only
 * unreachability degrades; a broken contract still surfaces (the program's
 * remaining error channel rejects into SvelteKit's 500). On a `(session)`
 * route the sign-in gate already asked, so its answer is reused. `denServed`
 * says whether this run can show a Den at all; on the everyday stack it is
 * false.
 */
export const load: LayoutServerLoad = async ({ fetch, locals }) => {
	const session = locals.session ?? (await runApi(fetch, sessionOrAnonymous));
	return { session, denServed: denServed() };
};
