import { error, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { API_PREFIX } from '$lib/api/client';

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
	 * End the session via the backend (`POST /logout` flushes the store row and
	 * clears the cookie on ITS response) and mirror the cookie clears onto the
	 * browser's response — the SSR proxy rewrites the host, so SvelteKit will
	 * not pass the backend's `set-cookie` through on its own. Name-driven (from
	 * the backend's own headers) rather than hardcoding `zurfur.sid`.
	 */
	default: async ({ fetch, cookies }) => {
		const response = await fetch(`${API_PREFIX}/logout`, { method: 'POST', redirect: 'manual' });
		const endedWithRedirect = response.status >= 300 && response.status < 400;
		if (!endedWithRedirect) {
			error(502, 'Sign-out did not complete. Try again.');
		}
		for (const setCookie of response.headers.getSetCookie()) {
			const clearedName = setCookie.split('=')[0]?.trim();
			if (clearedName) cookies.delete(clearedName, { path: '/' });
		}
		redirect(303, '/');
	}
};
