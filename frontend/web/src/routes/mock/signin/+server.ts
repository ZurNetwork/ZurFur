import { error, redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { HttpStatus } from '$lib/api/http-status';
import { mockModeEnabled, mockSignin } from '$lib/server/api/zurfur-api-mock';

/**
 * The mock sign-in callback (ZMVP-198): completes the login form's redirect
 * loop with no OAuth, no PDS, no backend. `/login`'s action
 * (`routes/login/+page.server.ts`) relays whatever URL `ZurfurApi.startSignin`
 * hands back as a REAL navigation, unconditionally — it knows nothing about
 * mock mode. When mock mode is on, that URL happens to be this route's own
 * `MOCK_SIGNIN_PATH` (minted by `mockStartSignin`); this GET mints the
 * session {@link mockSignin} stores and bounces home.
 *
 * Fails closed with a 404 unless {@link mockModeEnabled} says mock mode is
 * actually live — a stray hit against a real build, or a real dev run with
 * the flag unset, must look like the route doesn't exist, never like a
 * working shortcut into a session. Boot-time containment (the flag surviving
 * into a real, non-dev server) lives in `hooks.server.ts`, not here — this
 * route only ever answers per-request. A handle present but rejected by
 * {@link mockSignin}'s claim-tier check (⚠️ the mock does not accept
 * IDN/punycode handles the way `/login`'s own claim tier does — auth-time
 * punycode is real-backend-only) 400s naming the cause, rather than the
 * earlier revision's silent fallback to the fixture visitor.
 *
 * A `+server.ts` route does not match the Effect containment glob
 * (`src/**\/*.server.ts` needs a dot before "server"; `+server.ts` has none),
 * so this file calls only the plain helpers `zurfur-api-mock.ts` exports for
 * exactly that reason — no `effect` import here.
 */
export const GET: RequestHandler = ({ url }) => {
	if (!mockModeEnabled()) error(HttpStatus.NotFound);

	const rawHandle = url.searchParams.get('handle') ?? undefined;
	const session = mockSignin(rawHandle);
	if (session === undefined) {
		error(
			HttpStatus.BadRequest,
			'That handle failed claim-tier validation — the mock does not support IDN/punycode handles.'
		);
	}

	redirect(HttpStatus.SeeOther, '/');
};
