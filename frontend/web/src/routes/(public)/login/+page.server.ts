import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { signinOutcome } from '$lib/server/session';
import { HttpStatus } from '$lib/api/http-status';
import { SIGNOUT_PARAM, SIGNOUT_UNCONFIRMED } from '$lib/api/signout';
import { SESSION_COOKIE_NAME } from '$lib/server/api-proxy';
import { callbackErrorMessage } from './callback-errors';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import { loginForm } from '$lib/server/forms/login';
import { problemMessage } from '$lib/server/forms/problem-message';

/**
 * A signed-in visitor has nothing to do here — bounce home (`/` is the
 * signed-in landing page; the session rides in from the root layout's one
 * whoami). The auth gate runs FIRST, fail-closed order (authorization
 * precedes validation — the `[id]` route's rule). Otherwise surface any
 * `?error=<code>` a failed `signin_callback` redirected back with, plus a
 * pristine {@link loginForm} superform for the sign-in form below it, and
 * whether the visitor just signed out without the backend confirming it (only
 * when the request carries no session cookie, so it can't be claimed falsely).
 */
export const load: PageServerLoad = async ({ parent, url, cookies }) => {
	const { session } = await parent();
	if (session !== undefined) redirect(HttpStatus.SeeOther, '/');

	const form = await superValidate(effect(loginForm));
	const errorCode = url.searchParams.get('error') ?? undefined;
	const callbackError = errorCode === undefined ? undefined : callbackErrorMessage(errorCode);
	// Trusted from the address only when no session cookie came with the request:
	// otherwise the page would claim a still-live cookie was cleared.
	const signoutUnconfirmed =
		url.searchParams.get(SIGNOUT_PARAM) === SIGNOUT_UNCONFIRMED &&
		cookies.get(SESSION_COOKIE_NAME) === undefined;
	return { callbackError, form, signoutUnconfirmed };
};

export const actions = {
	/**
	 * Validate locally against {@link loginForm} (shape + punycode rejection),
	 * then proxy the sign-in start through SSR (the browser cannot read the
	 * 303's Location cross-fetch): backend 303 → relay the PDS authorize URL
	 * as a real navigation; backend problem → rides back as the same form's
	 * `message` via {@link problemMessage}. One channel throughout — the
	 * typed handle, field errors, and the backend `Problem` all ride the
	 * superform.
	 */
	default: async ({ request, fetch }) => {
		const form = await superValidate(request, effect(loginForm));
		if (!form.valid) {
			return fail(HttpStatus.UnprocessableContent, { form });
		}
		const started = await runApi(fetch, signinOutcome(form.data.handle));
		if ('problem' in started) return problemMessage(form, started.problem);
		redirect(HttpStatus.SeeOther, started.location);
	}
} satisfies Actions;
