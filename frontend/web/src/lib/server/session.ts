/**
 * Session programs over the {@link import('./api/zurfur-api').ZurfurApi} port —
 * the Effect successors of the pre-DD `getSession` helpers. Pages run these
 * through {@link import('./runtime').runApi}.
 */

import { Effect } from 'effect';
import { serviceUnavailableProblem, type Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import { type BrowserBindingCookie, ZurfurApi } from './api/zurfur-api';
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
 * What the sign-in gate learns from the backend: who is signed in, that
 * nobody is (a `401`), or that the backend didn't answer (unreachable or out
 * of time), which is never read as signed out.
 */
export type SessionCheck =
	| { readonly check: 'signedIn'; readonly session: Session }
	| { readonly check: 'signedOut' }
	| { readonly check: 'unanswered' };

/** {@link optionalSession} as the gate reads it; a broken contract or another problem stays in the error channel. */
export const sessionCheck: Effect.Effect<SessionCheck, ApiProblem | ContractViolation, ZurfurApi> =
	optionalSession.pipe(
		Effect.map((session): SessionCheck =>
			session === undefined ? { check: 'signedOut' } : { check: 'signedIn', session }
		),
		Effect.catchTag('NetworkFailure', () => Effect.succeed<SessionCheck>({ check: 'unanswered' }))
	);

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

/**
 * The two ways a sign-in start comes back: bounce to the PDS carrying the
 * browser-binding cookie, or a problem to render.
 */
export type SigninOutcome =
	| { readonly location: string; readonly browserBinding: BrowserBindingCookie }
	| { readonly problem: Problem };

/** What the login form says when a sign-in start can't reach Zurfur or runs out of time. */
export const SIGNIN_UNANSWERED_PROBLEM: Problem = serviceUnavailableProblem(
	"Zurfur isn't answering right now. Please try again."
);

/**
 * Start the atproto OAuth flow: the backend's 303 becomes `{location,
 * browserBinding}` (the PDS authorize URL to relay as a real navigation, and
 * the cookie to set on the browser's response), a rejected handle becomes
 * `{problem}` for the page to render, and so does a backend that doesn't
 * answer ({@link SIGNIN_UNANSWERED_PROBLEM}). A broken contract stays in the
 * error channel — the action's 500.
 */
export function signinOutcome(
	handle: string
): Effect.Effect<SigninOutcome, ContractViolation, ZurfurApi> {
	const started = Effect.gen(function* () {
		const api = yield* ZurfurApi;
		const started = yield* api.startSignin(handle);
		return {
			location: started.location,
			browserBinding: started.browserBinding
		} satisfies SigninOutcome;
	});
	return started.pipe(
		Effect.catchTags({
			ApiProblem: ({ problem }) => Effect.succeed<SigninOutcome>({ problem }),
			NetworkFailure: () => Effect.succeed<SigninOutcome>({ problem: SIGNIN_UNANSWERED_PROBLEM })
		})
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
