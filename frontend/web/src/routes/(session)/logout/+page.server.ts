import { error, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { signoutOutcome } from '$lib/server/session';
import { HttpStatus } from '$lib/api/http-status';

/**
 * `/logout` is an action, not a page — a signed-in visitor's stray GET just
 * goes home. (An anonymous one never reaches this load: the `(session)`
 * group guard bounces it to `/login` first.)
 */
export const load: PageServerLoad = () => {
	redirect(HttpStatus.SeeOther, '/');
};

export const actions = {
	/**
	 * End the session via the backend and mirror the cookie clears onto the
	 * browser's response — the SSR proxy rewrites the host, so SvelteKit will
	 * not pass the backend's `set-cookie` through on its own. Name-driven
	 * (from the backend's own headers) rather than hardcoding `zurfur.sid`.
	 * A failed sign-out is a bare 502: the error page picks its words from the
	 * route ("Sign-out did not complete"), so no message rides the page.
	 */
	default: async ({ fetch, cookies }) => {
		const outcome = await runApi(fetch, signoutOutcome);
		if ('failedStatus' in outcome) {
			error(HttpStatus.BadGateway);
		}
		for (const clearedName of outcome.clearedCookies) {
			cookies.delete(clearedName, { path: '/' });
		}
		redirect(HttpStatus.SeeOther, '/');
	}
} satisfies Actions;
