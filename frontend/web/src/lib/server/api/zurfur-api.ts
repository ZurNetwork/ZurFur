/**
 * The `ZurfurApi` port (DD 39944194): every backend call the SvelteKit server
 * makes, as one service named by role with a prod Layer (real HTTP through the
 * `/api` split) and an in-memory Layer for tests — `adapter-mem` parity for
 * the frontend. Success payloads decode through Effect Schema at the boundary;
 * failures are the tagged union in {@link import('./errors')}.
 */

import { type DescMessage, fromJson, type JsonValue, type MessageShape } from '@bufbuild/protobuf';
import { Context, Effect, Layer } from 'effect';
import { API_PREFIX, type FetchFunction } from '$lib/api/client';
import { PROBLEM_CONTENT_TYPE, type Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import { ProblemSchema } from './generated/zurfur/api/v1/problem_pb';
import { GetMeResponseSchema } from './generated/zurfur/api/v1/session_pb';
import {
	ApiProblem,
	ContractViolation,
	NetworkFailure,
	NotAuthenticated,
	SignoutFailed
} from './errors';

/**
 * The contract's boundary decoder (ZMVP-161; amended Decision 4 of
 * DD 39944194): `fromJson` against a GENERATED schema — produced from
 * `contract/zurfur/api/v1/*.proto`, so it structurally cannot drift from the
 * contract, which is a stronger property than the hand-written Effect Schema
 * it replaces offered. `ignoreUnknownFields: true` is MANDATORY here
 * (contract §6): protobuf-es rejects unknown fields by default, and
 * additive-only evolution is void without the tolerant reader. The
 * `contract_tolerant_reader` spec pins this exact function, so dropping the
 * option is a failing test, not a shipped incident.
 */
export function decodeContract<Desc extends DescMessage>(
	schema: Desc,
	value: unknown
): MessageShape<Desc> {
	return fromJson(schema, value as JsonValue, { ignoreUnknownFields: true });
}

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
		// Decode through the generated schema (ZMVP-162): the contract's one
		// Problem declaration, mapped to the plain component-facing interface.
		let problem: Problem;
		try {
			const message = decodeContract(ProblemSchema, body);
			problem = {
				type: message.type,
				code: message.code,
				title: message.title,
				detail: message.detail,
				status: message.status
			};
		} catch {
			// Each failure arm keeps its own detail — "no problem body" (wrong
			// content type), "unparsable body" (parsedBody's, propagated), and
			// this one — so a ContractViolation says which contract promise broke.
			return Effect.fail(
				new ContractViolation({ path, status: response.status, detail: 'malformed problem body' })
			);
		}
		if (problem.code === 'not_authenticated') return Effect.fail(new NotAuthenticated({ problem }));
		return Effect.fail(new ApiProblem({ problem }));
	};
	return parsedBody(response, path).pipe(Effect.flatMap(classified));
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
		// Decode through the generated schema, then map the message to the PLAIN
		// component-facing Session — generated types stay below the seam (the
		// containment guard applies to them unchanged), components get plain data.
		return yield* Effect.try({
			try: () => {
				const message = decodeContract(GetMeResponseSchema, raw);
				const session: Session = {
					did: message.did,
					handle: message.handle,
					displayName: message.displayName,
					avatarUrl: message.avatarUrl
				};
				return session;
			},
			catch: () =>
				new ContractViolation({
					path: '/me',
					status: response.status,
					detail: 'malformed session payload'
				})
		});
	});

const liveStartSignin = (fetch: FetchFunction, handle: string) =>
	Effect.gen(function* () {
		const form = new URLSearchParams({ handle });
		// redirect:'manual' exposing the real 303 + Location is server-only (undici)
		// behavior — a browser fetch would see an opaque status-0 redirect instead.
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
		// Server-only undici semantics again: the 303 + Set-Cookie stay readable.
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

/** The problem an unstubbed `me` fails with — the backend's anonymous 401 shape. */
const anonymousProblem: Problem = {
	type: 'urn:zurfur:error:not-authenticated',
	code: 'not_authenticated',
	title: 'not_authenticated',
	detail: 'No signed-in visitor (test default).',
	status: 401
};

/** Anonymous-by-default stub behaviors for {@link zurfurApiTest}. */
const anonymousDefaults: ZurfurApiShape = {
	me: Effect.fail(new NotAuthenticated({ problem: anonymousProblem })),
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
