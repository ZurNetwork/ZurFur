import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';
import { denPage } from '$lib/server/den';
import { runRead } from '$lib/server/unanswered';

/**
 * The node at a Den address — the only code that reads the Den. One route
 * serves the root (`/den`) and everything below it. On a full page load it
 * also reads the folders above the node, so the first paint shows the tree
 * opened down to it; an in-app navigation reads only the node. A refused
 * address, and every miss, is the one not-found, answered 200 with the
 * not-found in the page. A stale page token reloads the folder from its
 * first page, once; an ended session goes to sign in; a backend that
 * doesn't answer fails to the error view as "isn't answering".
 */
export const load: PageServerLoad = async ({ fetch, url, isDataRequest }) => {
	const result = await runRead(fetch, denPage(url.pathname, url.searchParams, !isDataRequest));
	if (result.result === 'reload') redirect(HttpStatus.SeeOther, result.location);
	if (result.result === 'signedOut') redirect(HttpStatus.SeeOther, '/login');
	return { den: result.data, trail: result.data.trail };
};
