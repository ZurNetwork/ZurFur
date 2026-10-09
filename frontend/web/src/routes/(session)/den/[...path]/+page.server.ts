import { error, redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { HttpStatus } from '$lib/api/http-status';
import { denPage } from '$lib/server/den';
import { denServed } from '$lib/server/den-served';
import { runApi } from '$lib/server/runtime';

/**
 * The node at a Den address — the only code that reads the Den. One route
 * serves the root (`/den`) and everything below it. On a full page load it
 * also reads the folders above the node, so the first paint shows the tree
 * opened down to it; an in-app navigation reads only the node. A refused
 * address, and every miss, is the one not-found, answered 200 with the
 * not-found in the page. A stale page token reloads the folder from its
 * first page, once; an ended session goes to sign in. Where this run serves
 * no Den (the everyday stack), `/den` is the ordinary 404 with no Den call:
 * a signed-in visitor makes only the one `/me` call every signed-in page
 * makes, and an anonymous one is sent to sign in like on every such page.
 */
export const load: PageServerLoad = async ({ fetch, url, isDataRequest }) => {
	if (!denServed()) error(HttpStatus.NotFound);
	const result = await runApi(fetch, denPage(url.pathname, url.searchParams, !isDataRequest));
	if (result.result === 'reload') redirect(HttpStatus.SeeOther, result.location);
	if (result.result === 'signedOut') redirect(HttpStatus.SeeOther, '/login');
	return { den: result.data, trail: result.data.trail };
};
