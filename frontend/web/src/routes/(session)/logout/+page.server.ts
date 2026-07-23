import { error, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { signoutOutcome } from '$lib/server/session';

/**
 * `/logout` is an action, not a page — a signed-in visitor's stray GET just
 * goes home. (An anonymous one never reaches this load: the `(session)`
 * group guard bounces it to `/login` first.)
 */
export const load: PageServerLoad = async () => {
	redirect(303, '/');
};

export const actions: Actions = {
	/**
	 * End the session via the backend and mirror the cookie clears onto the
	 * browser's response — the SSR proxy rewrites the host, so SvelteKit will
	 * not pass the backend's `set-cookie` through on its own. Name-driven
	 * (from the backend's own headers) rather than hardcoding `zurfur.sid`.
	 */
	default: async ({ fetch, cookies }) => {
		const outcome = await runApi(fetch, signoutOutcome);
		if ('failedStatus' in outcome) {
			error(502, 'Sign-out did not complete. Try again.');
		}
		for (const clearedName of outcome.clearedCookies) {
			cookies.delete(clearedName, { path: '/' });
		}
		redirect(303, '/');
	}
};
