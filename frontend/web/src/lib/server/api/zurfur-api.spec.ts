import { describe, expect, it } from 'vitest';
import { Effect } from 'effect';
import type { FetchFunction } from '$lib/api/client';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { RequestFetch, ZurfurApi, ZurfurApiLive } from './zurfur-api';
import type { ZurfurApiError } from './errors';

/** Run a program against the LIVE layer over a stubbed fetch. */
function runLive<A, E>(fetch: FetchFunction, program: Effect.Effect<A, E, ZurfurApi>): Promise<A> {
	const provided = program.pipe(
		Effect.provide(ZurfurApiLive),
		Effect.provideService(RequestFetch, fetch)
	);
	return Effect.runPromise(provided);
}

/** The program's failure, surfaced as the success value (no catch in specs). */
function failureOf<A>(
	program: Effect.Effect<A, ZurfurApiError, ZurfurApi>
): Effect.Effect<ZurfurApiError, A, ZurfurApi> {
	return Effect.flip(program);
}

const me = Effect.flatMap(ZurfurApi, (api) => api.me);
const startSignin = (handle: string) => Effect.flatMap(ZurfurApi, (api) => api.startSignin(handle));
const signout = Effect.flatMap(ZurfurApi, (api) => api.signout);

const aliceWire = {
	did: 'did:plc:alice',
	handle: 'alice.zurfur.app',
	display_name: 'Alice',
	avatar_url: null
};

describe('ZurfurApi.me (live)', () => {
	it('rides the /api prefix so both split halves route it', async () => {
		const { fetch, calls } = fetchStub(() => Response.json(aliceWire));
		await runLive(fetch, me);
		expect(calls).toEqual(['/api/me']);
	});

	it('decodes the session for a signed-in visitor', async () => {
		const { fetch } = fetchStub(() => Response.json(aliceWire));
		const session = await runLive(fetch, me);
		expect(session).toEqual(aliceWire);
	});

	it('fails NotAuthenticated on the 401 problem', async () => {
		const { fetch } = fetchStub(() => problemResponse(401, 'not_authenticated'));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure._tag).toBe('NotAuthenticated');
	});

	it('fails ApiProblem on any other problem, carrying it whole', async () => {
		const { fetch } = fetchStub(() => problemResponse(429, 'rate_limited'));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('rate_limited');
	});

	it('fails ContractViolation on a non-problem error response', async () => {
		const { fetch } = fetchStub(() => new Response('gateway timeout', { status: 504 }));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure._tag).toBe('ContractViolation');
		expect(failure.message).toMatch(/\/me responded 504/);
	});

	it('fails ContractViolation when a success body is not JSON', async () => {
		const { fetch } = fetchStub(() => new Response('not json', { status: 200 }));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure.message).toMatch(/\/me responded 200 — unparsable body/);
	});

	it('fails ContractViolation when the payload does not fit the session schema', async () => {
		const { fetch } = fetchStub(() => Response.json({ did: 42 }));
		const failure = await runLive(fetch, failureOf(me));
		expect(failure.message).toMatch(/malformed session payload/);
	});

	it('fails NetworkFailure when the backend is unreachable', async () => {
		const failure = await runLive(unreachableFetch(), failureOf(me));
		expect(failure._tag).toBe('NetworkFailure');
	});
});

describe('ZurfurApi.startSignin (live)', () => {
	it('returns the PDS authorize location from the 303', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const location = await runLive(fetch, startSignin('alice.zurfur.app'));
		expect(location).toBe(authorizeUrl);
	});

	it('fails ApiProblem when the backend rejects the handle', async () => {
		const { fetch } = fetchStub(() => problemResponse(422, 'invalid_request'));
		const failure = await runLive(fetch, failureOf(startSignin('not a handle')));
		expect(failure._tag).toBe('ApiProblem');
		if (failure._tag === 'ApiProblem') expect(failure.problem.code).toBe('invalid_request');
	});

	it('fails ContractViolation when a redirect arrives without a Location header', async () => {
		const { fetch } = fetchStub(() => new Response(null, { status: 303 }));
		const failure = await runLive(fetch, failureOf(startSignin('alice.zurfur.app')));
		expect(failure.message).toMatch(/no Location/);
	});

	it('fails ContractViolation on a problem-shaped body missing the problem content type', async () => {
		const mislabelled = () =>
			new Response(
				JSON.stringify({
					type: 'urn:zurfur:error:invalid-request',
					code: 'invalid_request',
					title: 'invalid_request',
					status: 422
				}),
				{ status: 422, headers: { 'content-type': 'application/json' } }
			);
		const { fetch } = fetchStub(mislabelled);
		const failure = await runLive(fetch, failureOf(startSignin('alice.zurfur.app')));
		expect(failure._tag).toBe('ContractViolation');
	});
});

describe('ZurfurApi.signout (live)', () => {
	it('returns the cookie names the backend cleared on the 303', async () => {
		const headers = new Headers({ location: '/' });
		headers.append('set-cookie', 'zurfur.sid=; Max-Age=0; Path=/');
		headers.append('set-cookie', 'zurfur.csrf=; Max-Age=0; Path=/');
		const { fetch } = fetchStub(() => new Response(null, { status: 303, headers }));
		const cleared = await runLive(fetch, signout);
		expect(cleared).toEqual(['zurfur.sid', 'zurfur.csrf']);
	});

	it('fails SignoutFailed when the backend does not answer with a redirect', async () => {
		const { fetch } = fetchStub(() => new Response(null, { status: 200 }));
		const failure = await runLive(fetch, failureOf(signout));
		expect(failure._tag).toBe('SignoutFailed');
		if (failure._tag === 'SignoutFailed') expect(failure.status).toBe(200);
	});
});
