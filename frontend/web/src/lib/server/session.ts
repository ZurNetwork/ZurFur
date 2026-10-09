/**
 * Session programs over the {@link import('./api/zurfur-api').ZurfurApi} port —
 * the Effect successors of the pre-DD `getSession` helpers. Pages run these
 * through {@link import('./runtime').runApi}.
 */

import { Effect } from 'effect';
import type { Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import { ZurfurApi } from './api/zurfur-api';
import type { ApiProblem, ContractViolation, NetworkFailure } from './api/errors';

/**
 * Who is signed in, if anyone: `undefined` on the backend's 401
 * `not_authenticated` (anonymous or expired). Any other problem, a broken
 * contract, or an unreachable backend stays in the error channel.
 */
export const optionalSession: Effect.Effect<
	Session | undefined,
	ApiProblem | NetworkFailure | ContractViolation,
	ZurfurApi
> = Effect.gen(function* () {
	const api = yield* ZurfurApi;
	return yield* api.me;
}).pipe(Effect.catchTag('NotAuthenticated', () => Effect.succeed(undefined)));

/**
 * {@link optionalSession}, degrading an unreachable backend to anonymous too —
 * the root layout's stance: a dead backend renders signed-out rather than a
 * 500. A contract violation or unexpected problem still surfaces; a
 * regression must not masquerade as "signed out".
 */
export const sessionOrAnonymous: Effect.Effect<
	Session | undefined,
	ApiProblem | ContractViolation,
	ZurfurApi
> = optionalSession.pipe(Effect.catchTag('NetworkFailure', () => Effect.succeed(undefined)));

/** The two ways a sign-in start comes back: bounce to the PDS, or a problem to render. */
export type SigninOutcome = { location: string } | { problem: Problem };

/**
 * Start the atproto OAuth flow: the backend's 303 becomes `{location}` (the
 * PDS authorize URL to relay as a real navigation), a rejected handle becomes
 * `{problem}` for the page to render. Broken contract / dead backend stay in
 * the error channel — the action's 500s, as before Effect.
 */
export function signinOutcome(
	handle: string
): Effect.Effect<SigninOutcome, NetworkFailure | ContractViolation, ZurfurApi> {
	const started = Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const location = yield* api.startSignin(handle);
		return { location } satisfies SigninOutcome;
	});
	return started.pipe(
		Effect.catchTag('ApiProblem', ({ problem }) => Effect.succeed<SigninOutcome>({ problem }))
	);
}

/**
 * How a sign-out lands: the backend confirmed it (with the cookie names it
 * cleared), or it didn't — it answered with a failure, or couldn't be reached.
 */
export type SignoutOutcome =
	| { readonly clearedCookies: readonly string[] }
	| { readonly unconfirmed: 'refused' | 'unreachable' };

/**
 * End the session backend-side. Success carries the cookie names the backend
 * cleared (the action mirrors the clears onto the browser's response); a
 * failing answer or an unreachable backend is `{unconfirmed}`, never an
 * error, so the action can still clear the browser's session.
 */
export const signoutOutcome: Effect.Effect<SignoutOutcome, never, ZurfurApi> = Effect.gen(
	function* () {
		const api = yield* ZurfurApi;
		const clearedCookies = yield* api.signout;
		return { clearedCookies } satisfies SignoutOutcome;
	}
).pipe(
	Effect.catchTags({
		SignoutFailed: () => Effect.succeed<SignoutOutcome>({ unconfirmed: 'refused' }),
		NetworkFailure: () => Effect.succeed<SignoutOutcome>({ unconfirmed: 'unreachable' })
	})
);
