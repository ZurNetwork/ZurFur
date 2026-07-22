/**
 * The `ZurfurApi` port (DD 39944194): every backend call the SvelteKit server
 * makes, as one service named by role with a prod Layer (real HTTP through the
 * `/api` split) and an in-memory Layer for tests — `adapter-mem` parity for
 * the frontend. Success payloads decode through Effect Schema at the boundary;
 * failures are the tagged union in {@link import('./errors')}.
 */

import { Context, Effect, Layer, Schema } from 'effect';
import { API_PREFIX, type FetchFunction } from '$lib/api/client';
import { isProblem, PROBLEM_CONTENT_TYPE, type Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import {
	ApiProblem,
	ContractViolation,
	NetworkFailure,
	NotAuthenticated,
	SignoutFailed
} from './errors';

/**
 * The JSON `/me` contract (ZMVP-151 slice 1), as the wire schema. Decodes into
 * the component-facing {@link Session} type — the `me` signature below is the
 * compile-time proof the two stay one shape.
 */
const SessionSchema = Schema.Struct({
	did: Schema.String,
	handle: Schema.NullOr(Schema.String),
	display_name: Schema.NullOr(Schema.String),
	avatar_url: Schema.NullOr(Schema.String)
});

/** What each `ZurfurApi` call does; failures per call are in the signature. */
export interface ZurfurApiShape {
	/** `GET /me` — who is signed in. Fails `NotAuthenticated` for anonymous/expired. */
	readonly me: Effect.Effect<
		Session,
		NotAuthenticated | ApiProblem | NetworkFailure | ContractViolation
	>;
	/**
	 * `POST /signin` — start the atproto OAuth flow; succeeds with the PDS
	 * authorize URL off the 303's `Location` (server-side `redirect: 'manual'`
	 * semantics — a browser fetch would return an opaque redirect).
	 */
	readonly startSignin: (
		handle: string
	) => Effect.Effect<string, ApiProblem | NetworkFailure | ContractViolation>;
	/**
	 * `POST /logout` — end the session backend-side; succeeds with the cookie
	 * names the backend cleared (for mirroring onto the browser's response —
	 * the SSR proxy rewrites the host, so SvelteKit won't pass `set-cookie`
	 * through on its own).
	 */
	readonly signout: Effect.Effect<ReadonlyArray<string>, SignoutFailed | NetworkFailure>;
}

/** The port tag — programs ask for `ZurfurApi`, the seam decides which Layer answers. */
export class ZurfurApi extends Context.Tag('web/ZurfurApi')<ZurfurApi, ZurfurApiShape>() {}

/**
 * The per-request `fetch` the live Layer speaks through — the SvelteKit event
 * `fetch` (SSR rewrite + cookie forwarding) or the browser's. Provided at the
 * seam per request; never baked into the runtime.
 */
export class RequestFetch extends Context.Tag('web/RequestFetch')<RequestFetch, FetchFunction>() {}

/** A fetch that reaches the backend or fails `NetworkFailure` — never throws through. */
function backendFetch(
	fetch: FetchFunction,
	path: string,
	init?: RequestInit
): Effect.Effect<Response, NetworkFailure> {
	return Effect.tryPromise({
		try: () => fetch(`${API_PREFIX}${path}`, init),
		catch: (cause) => new NetworkFailure({ cause })
	});
}

/** Parse the body as JSON or fail `ContractViolation` naming the endpoint and status. */
function parsedBody(response: Response, path: string): Effect.Effect<unknown, ContractViolation> {
	return Effect.tryPromise({
		try: () => response.json() as Promise<unknown>,
		catch: () => new ContractViolation({ path, status: response.status, detail: 'unparsable body' })
	});
}

/**
 * Classify a non-2xx response by the error contract: `application/problem+json`
 * with a well-formed problem becomes `NotAuthenticated` (the session branch) or
 * `ApiProblem`; anything else is a `ContractViolation`.
 */
function problemFailure(
	response: Response,
	path: string
): Effect.Effect<never, NotAuthenticated | ApiProblem | ContractViolation> {
	const violation = new ContractViolation({
		path,
		status: response.status,
		detail: 'no problem body'
	});
	const contentType = response.headers.get('content-type') ?? '';
	if (!contentType.startsWith(PROBLEM_CONTENT_TYPE)) return Effect.fail(violation);

	const classified = (
		body: unknown
	): Effect.Effect<never, NotAuthenticated | ApiProblem | ContractViolation> => {
		if (!isProblem(body)) return Effect.fail(violation);
		const problem: Problem = body;
		if (problem.code === 'not_authenticated') return Effect.fail(new NotAuthenticated({ problem }));
		return Effect.fail(new ApiProblem({ problem }));
	};
	return parsedBody(response, path).pipe(
		Effect.catchTag('ContractViolation', () => Effect.fail(violation)),
		Effect.flatMap(classified)
	);
}

/** The redirect-range check both signin and signout branch on. */
function isRedirectStatus(status: number): boolean {
	return status >= 300 && status < 400;
}

const liveMe = (fetch: FetchFunction) =>
	Effect.gen(function* () {
		const response = yield* backendFetch(fetch, '/me');
		if (!response.ok) return yield* problemFailure(response, '/me');
		const raw = yield* parsedBody(response, '/me');
		return yield* Schema.decodeUnknown(SessionSchema)(raw).pipe(
			Effect.mapError(
				() =>
					new ContractViolation({
						path: '/me',
						status: response.status,
						detail: 'malformed session payload'
					})
			)
		);
	});

const liveStartSignin = (fetch: FetchFunction, handle: string) =>
	Effect.gen(function* () {
		const form = new URLSearchParams({ handle });
		const init: RequestInit = { method: 'POST', body: form, redirect: 'manual' };
		const response = yield* backendFetch(fetch, '/signin', init);
		if (isRedirectStatus(response.status)) {
			const location = response.headers.get('location');
			if (location === null) {
				return yield* new ContractViolation({
					path: '/signin',
					status: response.status,
					detail: 'redirect carried no Location header'
				});
			}
			return location;
		}
		return yield* problemFailure(response, '/signin').pipe(
			Effect.catchTag('NotAuthenticated', ({ problem }) => new ApiProblem({ problem }))
		);
	});

const liveSignout = (fetch: FetchFunction) =>
	Effect.gen(function* () {
		const init: RequestInit = { method: 'POST', redirect: 'manual' };
		const response = yield* backendFetch(fetch, '/logout', init);
		if (!isRedirectStatus(response.status)) {
			return yield* new SignoutFailed({ status: response.status });
		}
		const clearedNames = response.headers
			.getSetCookie()
			.map((setCookie) => setCookie.split('=')[0]?.trim())
			.filter((name): name is string => name !== undefined && name !== '');
		return clearedNames;
	});

/** The prod Layer: real HTTP through the per-request {@link RequestFetch}. */
export const ZurfurApiLive: Layer.Layer<ZurfurApi, never, RequestFetch> = Layer.effect(
	ZurfurApi,
	Effect.gen(function* () {
		const fetch = yield* RequestFetch;
		return ZurfurApi.of({
			me: liveMe(fetch),
			startSignin: (handle) => liveStartSignin(fetch, handle),
			signout: liveSignout(fetch)
		});
	})
);

/** Anonymous-by-default stub behaviors for {@link zurfurApiTest}. */
const anonymousDefaults: ZurfurApiShape = {
	me: Effect.fail(
		new NotAuthenticated({
			problem: {
				type: 'urn:zurfur:error:not-authenticated',
				code: 'not_authenticated',
				title: 'not_authenticated',
				status: 401
			}
		})
	),
	startSignin: () => Effect.fail(new NetworkFailure({ cause: new TypeError('no signin stubbed') })),
	signout: Effect.fail(new SignoutFailed({ status: 500 }))
};

/**
 * The in-memory Layer (adapter-mem parity): tests hand in only the behaviors
 * they exercise; everything else answers like an anonymous, signin-less world.
 */
export function zurfurApiTest(overrides: Partial<ZurfurApiShape>): Layer.Layer<ZurfurApi> {
	const shape: ZurfurApiShape = { ...anonymousDefaults, ...overrides };
	return Layer.succeed(ZurfurApi, shape);
}
