import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { runApi } from '$lib/server/runtime';
import { signinOutcome } from '$lib/server/session';
import { type Problem, ProblemKind, renderableStatus } from '$lib/api/problem';
import { HttpStatus } from '$lib/api/http-status';
import { callbackErrorMessage } from './callback-errors';

/**
 * Rendered through the same problem seam as backend problems, but minted
 * locally — an empty handle never needs a round-trip. Same shape, same
 * rendering path ({@link import('$lib/components/ProblemNote.svelte')}).
 */
const EMPTY_HANDLE_PROBLEM: Problem = {
	...ProblemKind.InvalidRequest,
	title: 'Enter a handle.',
	detail: 'A handle is required to start sign-in — e.g. you.bsky.social.',
	status: HttpStatus.UnprocessableContent
};

/**
 * A signed-in visitor has nothing to do here — bounce home (ruling 9b makes
 * `/` the signed-in landing; the session rides in from the root layout's one
 * whoami). Otherwise surface any `?error=<code>` a failed `signin_callback`
 * redirected back with.
 */
export const load: PageServerLoad = async ({ parent, url }) => {
	const { session } = await parent();
	if (session !== null) redirect(HttpStatus.SeeOther, '/');

	const errorCode = url.searchParams.get('error');
	const callbackError = errorCode === null ? null : callbackErrorMessage(errorCode);
	return { callbackError };
};

export const actions: Actions = {
	/**
	 * Proxy the sign-in start through SSR (the browser cannot read the 303's
	 * Location cross-fetch): backend 303 → relay the PDS authorize URL as a
	 * real navigation; backend problem → hand it to the page to render.
	 */
	default: async ({ request, fetch }) => {
		const form = await request.formData();
		const handleEntry = form.get('handle');
		const handle = typeof handleEntry === 'string' ? handleEntry.trim() : '';
		if (handle === '')
			return fail(HttpStatus.UnprocessableContent, { problem: EMPTY_HANDLE_PROBLEM });

		const started = await runApi(fetch, signinOutcome(handle));
		if ('problem' in started)
			return fail(renderableStatus(started.problem), { problem: started.problem });
		redirect(HttpStatus.SeeOther, started.location);
	}
};
