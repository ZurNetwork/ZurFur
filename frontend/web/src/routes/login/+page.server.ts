import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { signinOutcome } from '$lib/server/session';
import { HttpStatus } from '$lib/api/http-status';
import { callbackErrorMessage } from './callback-errors';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import { loginForm } from '$lib/server/forms/login';
import { problemMessage } from '$lib/server/forms/problem-message';
import type { BrowserBindingCookie } from '$lib/server/api/zurfur-api';
import type { Cookies } from '@sveltejs/kit';

/**
 * A signed-in visitor has nothing to do here — bounce home (`/` is the
 * signed-in landing page; the session rides in from the root layout's one
 * whoami). The auth gate runs FIRST, fail-closed order (authorization
 * precedes validation — the `[id]` route's rule). Otherwise surface any
 * `?error=<code>` a failed `signin_callback` redirected back with, plus a
 * pristine {@link loginForm} superform for the sign-in form below it.
 */
export const load: PageServerLoad = async ({ parent, url }) => {
	const { session } = await parent();
	if (session !== undefined) redirect(HttpStatus.SeeOther, '/');

	const form = await superValidate(effect(loginForm));
	const errorCode = url.searchParams.get('error') ?? undefined;
	const callbackError = errorCode === undefined ? undefined : callbackErrorMessage(errorCode);
	return { callbackError, form };
};

export const actions = {
	/**
	 * Validate locally against {@link loginForm} (shape + punycode rejection),
	 * then proxy the sign-in start through SSR (the browser cannot read the
	 * 303's Location cross-fetch): backend 303 → set the browser-binding cookie
	 * on this response and relay the PDS authorize URL as a real navigation;
	 * backend problem → rides back as the same form's `message` via
	 * {@link problemMessage}. One channel throughout — the typed handle, field
	 * errors, and the backend `Problem` all ride the superform.
	 */
	default: async ({ request, fetch, cookies }) => {
		const form = await superValidate(request, effect(loginForm));
		if (!form.valid) {
			return fail(HttpStatus.UnprocessableContent, { form });
		}
		const started = await runApi(fetch, signinOutcome(form.data.handle));
		if ('problem' in started) return problemMessage(form, started.problem);
		relayBrowserBinding(cookies, started.browserBinding);
		redirect(HttpStatus.SeeOther, started.location);
	}
} satisfies Actions;

/**
 * Set the backend's browser-binding cookie on the browser's response: SSR
 * rewrites the host, so SvelteKit never passes the backend's `Set-Cookie`
 * through. The callback reaches the backend straight from the browser, so this
 * is the cookie's only hop through SvelteKit.
 */
function relayBrowserBinding(cookies: Cookies, binding: BrowserBindingCookie): void {
	cookies.set(binding.name, binding.value, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		secure: binding.secure,
		maxAge: binding.maxAge
	});
}
