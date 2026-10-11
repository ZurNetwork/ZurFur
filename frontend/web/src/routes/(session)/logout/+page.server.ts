import { redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { signoutOutcome } from '$lib/server/session';
import { SESSION_COOKIE_NAME } from '$lib/server/api-proxy';
import { HttpStatus } from '$lib/api/http-status';
import { SIGNOUT_UNCONFIRMED_LOCATION } from '$lib/api/signout';

/**
 * `/logout` is an action, not a page — a signed-in visitor's stray GET just
 * goes home. (An anonymous one never reaches this load: the sign-in gate
 * bounces it to `/login` first.)
 */
export const load: PageServerLoad = () => {
	redirect(HttpStatus.SeeOther, '/');
};

export const actions = {
	/**
	 * End the session via the backend, and clear the browser's session cookie
	 * whatever the backend answers — so a shared computer never keeps a live
	 * cookie. On success, the backend's own cookie clears are mirrored too (the
	 * SSR proxy rewrites the host, so SvelteKit won't pass its `set-cookie`
	 * through) and the visitor lands home. When the backend refused or couldn't
	 * be reached, the sign-in page says the server's session may outlive this
	 * device's until it expires. The sign-in gate lets this action through,
	 * since it needs no session to run.
	 */
	default: async ({ fetch, cookies }) => {
		const outcome = await runApi(fetch, signoutOutcome);
		cookies.delete(SESSION_COOKIE_NAME, { path: '/' });
		if ('unconfirmed' in outcome) redirect(HttpStatus.SeeOther, SIGNOUT_UNCONFIRMED_LOCATION);
		for (const clearedName of outcome.clearedCookies) {
			if (clearedName !== SESSION_COOKIE_NAME) cookies.delete(clearedName, { path: '/' });
		}
		redirect(HttpStatus.SeeOther, '/');
	}
} satisfies Actions;
